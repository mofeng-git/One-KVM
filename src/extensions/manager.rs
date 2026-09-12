use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::protected_config::ProtectedConfigFile;
use super::types::*;
use super::validation::{validate_easytier_config, validate_frpc_config, validate_gostc_config};
use crate::events::EventBus;

const LOG_BUFFER_SIZE: usize = 200;

#[cfg(unix)]
pub const TTYD_SOCKET_PATH: &str = "/var/run/one-kvm/ttyd.sock";

#[cfg(windows)]
pub const TTYD_TCP_ADDR: &str = "127.0.0.1:7681";
#[cfg(windows)]
const TTYD_TCP_HOST: &str = "127.0.0.1";
#[cfg(windows)]
const TTYD_TCP_PORT: &str = "7681";

struct ExtensionProcess {
    child: Child,
    logs: Arc<RwLock<VecDeque<String>>>,
    _config_file: Option<ProtectedConfigFile>,
}

struct ExtensionLaunch {
    args: Vec<String>,
    config_file: Option<ProtectedConfigFile>,
}

pub struct ExtensionManager {
    processes: RwLock<HashMap<ExtensionId, ExtensionProcess>>,
    availability: HashMap<ExtensionId, bool>,
    event_bus: RwLock<Option<Arc<EventBus>>>,
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionManager {
    pub fn new() -> Self {
        let availability = ExtensionId::all()
            .iter()
            .map(|id| (*id, id.binary_path().exists()))
            .collect();

        Self {
            processes: RwLock::new(HashMap::new()),
            availability,
            event_bus: RwLock::new(None),
        }
    }

    pub async fn set_event_bus(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus);
    }

    async fn mark_ttyd_status_dirty(&self, id: ExtensionId) {
        if id != ExtensionId::Ttyd {
            return;
        }

        if let Some(ref event_bus) = *self.event_bus.read().await {
            event_bus.mark_device_info_dirty();
        }
    }

    pub fn check_available(&self, id: ExtensionId) -> bool {
        *self.availability.get(&id).unwrap_or(&false)
    }

    fn is_enabled_for_config(id: ExtensionId, config: &ExtensionsConfig) -> bool {
        match id {
            ExtensionId::Ttyd => config.ttyd.enabled,
            ExtensionId::Gostc => {
                config.gostc.enabled && validate_gostc_config(&config.gostc).is_ok()
            }
            ExtensionId::Easytier => {
                config.easytier.enabled && validate_easytier_config(&config.easytier).is_ok()
            }
            ExtensionId::Frpc => config.frpc.enabled && validate_frpc_config(&config.frpc).is_ok(),
        }
    }

    pub async fn status(&self, id: ExtensionId) -> ExtensionStatus {
        if !self.check_available(id) {
            return ExtensionStatus::Unavailable;
        }

        let mut processes = self.processes.write().await;
        let exited = {
            let Some(proc) = processes.get_mut(&id) else {
                return ExtensionStatus::Stopped;
            };

            match proc.child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!("Extension {} exited with status {}", id, status);
                    true
                }
                Ok(None) => {
                    return match proc.child.id() {
                        Some(pid) => ExtensionStatus::Running { pid },
                        None => ExtensionStatus::Stopped,
                    };
                }
                Err(e) => {
                    tracing::warn!("Failed to query status for {}: {}", id, e);
                    return match proc.child.id() {
                        Some(pid) => ExtensionStatus::Running { pid },
                        None => ExtensionStatus::Stopped,
                    };
                }
            }
        };

        if exited {
            processes.remove(&id);
        }

        ExtensionStatus::Stopped
    }

    pub async fn start(&self, id: ExtensionId, config: &ExtensionsConfig) -> Result<(), String> {
        if !self.check_available(id) {
            return Err(format!(
                "{} not found at {}",
                id,
                id.binary_path().display()
            ));
        }

        self.stop(id).await.ok();

        let launch = self.build_launch(id, config).await?;

        tracing::info!(
            "Starting extension {}: {} {}",
            id,
            id.binary_path().display(),
            launch.args.join(" ")
        );

        let mut child = Command::new(id.binary_path())
            .args(&launch.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to start {}: {}", id, e))?;

        let logs = Arc::new(RwLock::new(VecDeque::with_capacity(LOG_BUFFER_SIZE)));

        if let Some(stdout) = child.stdout.take() {
            let logs_clone = logs.clone();
            let id_clone = id;
            tokio::spawn(async move {
                Self::collect_logs(id_clone, stdout, logs_clone).await;
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let logs_clone = logs.clone();
            let id_clone = id;
            tokio::spawn(async move {
                Self::collect_logs(id_clone, stderr, logs_clone).await;
            });
        }

        let pid = child.id();
        tracing::info!("Extension {} started with PID {:?}", id, pid);
        Self::push_log(
            &logs,
            format!("Extension {} started with PID {:?}", id, pid),
        )
        .await;

        let mut processes = self.processes.write().await;
        processes.insert(
            id,
            ExtensionProcess {
                child,
                logs,
                _config_file: launch.config_file,
            },
        );
        drop(processes);
        self.mark_ttyd_status_dirty(id).await;

        Ok(())
    }

    pub async fn stop(&self, id: ExtensionId) -> Result<(), String> {
        let mut processes = self.processes.write().await;
        if let Some(mut proc) = processes.remove(&id) {
            tracing::info!("Stopping extension {}", id);
            if let Err(e) = proc.child.kill().await {
                tracing::warn!("Failed to kill {}: {}", id, e);
            }
            drop(processes);
            self.mark_ttyd_status_dirty(id).await;
        }
        Ok(())
    }

    pub async fn logs(&self, id: ExtensionId, lines: usize) -> Vec<String> {
        let processes = self.processes.read().await;
        if let Some(proc) = processes.get(&id) {
            let logs = proc.logs.read().await;
            let start = logs.len().saturating_sub(lines);
            logs.range(start..).cloned().collect()
        } else {
            Vec::new()
        }
    }

    async fn collect_logs<R: tokio::io::AsyncRead + Unpin>(
        id: ExtensionId,
        reader: R,
        logs: Arc<RwLock<VecDeque<String>>>,
    ) {
        let reader = BufReader::new(reader);
        let mut lines = reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    tracing::info!("[{}] {}", id, line);
                    Self::push_log(&logs, line).await;
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    tracing::warn!("[{}] Error reading log: {}", id, e);
                    break;
                }
            }
        }
    }

    async fn push_log(logs: &RwLock<VecDeque<String>>, line: String) {
        let mut logs = logs.write().await;
        if logs.len() >= LOG_BUFFER_SIZE {
            logs.pop_front();
        }
        logs.push_back(line);
    }

    async fn build_launch(
        &self,
        id: ExtensionId,
        config: &ExtensionsConfig,
    ) -> Result<ExtensionLaunch, String> {
        let args = match id {
            ExtensionId::Ttyd => {
                let c = &config.ttyd;

                let mut args = Self::build_ttyd_listen_args().await?;

                args.push(c.shell.clone());
                args
            }

            ExtensionId::Gostc => {
                let c = &config.gostc;
                validate_gostc_config(c)?;

                // gostc defaults to TLS, so disabling it must be explicit.
                let mut args = vec![format!("--tls={}", c.tls)];

                args.extend(["-addr".to_string(), c.addr.trim().to_string()]);

                args.extend(["-key".to_string(), c.key.clone()]);

                args
            }

            ExtensionId::Easytier => {
                return Self::build_easytier_launch(&config.easytier).await;
            }

            ExtensionId::Frpc => {
                return Self::build_frpc_launch(&config.frpc).await;
            }
        };

        Ok(ExtensionLaunch {
            args,
            config_file: None,
        })
    }

    async fn build_easytier_launch(config: &EasytierConfig) -> Result<ExtensionLaunch, String> {
        validate_easytier_config(config)?;

        match config.config_mode {
            EasytierConfigMode::Quick => Ok(ExtensionLaunch {
                args: Self::build_easytier_quick_args(config),
                config_file: None,
            }),
            EasytierConfigMode::Full => {
                let config_file = ProtectedConfigFile::create(
                    "EasyTier",
                    "easytier.toml",
                    config.custom_toml.as_str(),
                )
                .await?;

                Ok(ExtensionLaunch {
                    args: vec!["-c".to_string(), Self::path_to_arg(config_file.path())],
                    config_file: Some(config_file),
                })
            }
        }
    }

    fn build_easytier_quick_args(config: &EasytierConfig) -> Vec<String> {
        let mut args = vec![
            "--network-name".to_string(),
            config.network_name.clone(),
            "--network-secret".to_string(),
            config.network_secret.clone(),
        ];

        for peer in &config.peer_urls {
            if !peer.is_empty() {
                args.extend(["--peers".to_string(), peer.clone()]);
            }
        }

        if let Some(ref ip) = config.virtual_ip {
            if !ip.is_empty() {
                args.extend(["-i".to_string(), ip.clone()]);
            } else {
                args.push("-d".to_string());
            }
        } else {
            args.push("-d".to_string());
        }

        args
    }

    async fn build_frpc_launch(config: &FrpcConfig) -> Result<ExtensionLaunch, String> {
        validate_frpc_config(config)?;

        let config_text = match config.config_mode {
            FrpcConfigMode::Quick => Self::build_frpc_quick_toml(config)?,
            FrpcConfigMode::Full => config.custom_toml.clone(),
        };

        let config_file =
            ProtectedConfigFile::create("FRPC", "frpc.toml", config_text.as_str()).await?;

        Ok(ExtensionLaunch {
            args: vec!["-c".to_string(), Self::path_to_arg(config_file.path())],
            config_file: Some(config_file),
        })
    }

    fn build_frpc_quick_toml(config: &FrpcConfig) -> Result<String, String> {
        if config.proxy_name.trim().is_empty() {
            return Err("FRPC proxy name is required".into());
        }
        if config.server_addr.trim().is_empty() {
            return Err("FRPC server address is required".into());
        }
        if config.token.is_empty() {
            return Err("FRPC token is required".into());
        }
        if config.local_ip.trim().is_empty() {
            return Err("FRPC local IP is required".into());
        }

        let proxy_type = match config.proxy_type {
            FrpProxyType::Tcp => "tcp",
            FrpProxyType::Udp => "udp",
            FrpProxyType::Http => "http",
            FrpProxyType::Https => "https",
            FrpProxyType::Stcp => "stcp",
            FrpProxyType::Sudp => "sudp",
            FrpProxyType::Xtcp => "xtcp",
        };

        let mut toml = String::new();
        toml.push_str(&format!(
            "serverAddr = {}\nserverPort = {}\n\n",
            Self::toml_string(config.server_addr.trim()),
            config.server_port
        ));
        toml.push_str("[auth]\n");
        toml.push_str("method = \"token\"\n");
        toml.push_str(&format!("token = {}\n\n", Self::toml_string(&config.token)));
        toml.push_str("[transport]\n");
        toml.push_str("protocol = \"tcp\"\n\n");
        toml.push_str("[transport.tls]\n");
        toml.push_str(&format!("enable = {}\n\n", config.tls));
        toml.push_str("[[proxies]]\n");
        toml.push_str(&format!(
            "name = {}\ntype = {}\nlocalIP = {}\nlocalPort = {}\n",
            Self::toml_string(config.proxy_name.trim()),
            Self::toml_string(proxy_type),
            Self::toml_string(config.local_ip.trim()),
            config.local_port
        ));

        match config.proxy_type {
            FrpProxyType::Tcp | FrpProxyType::Udp => {
                let remote_port = config.remote_port.ok_or_else(|| {
                    "FRPC remote port is required for TCP/UDP proxies".to_string()
                })?;
                toml.push_str(&format!("remotePort = {}\n", remote_port));
            }
            FrpProxyType::Http | FrpProxyType::Https => {
                if let Some(domain) = config
                    .custom_domain
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    toml.push_str(&format!(
                        "customDomains = [{}]\n",
                        Self::toml_string(domain)
                    ));
                }
            }
            FrpProxyType::Stcp | FrpProxyType::Sudp | FrpProxyType::Xtcp => {
                if !config.secret_key.is_empty() {
                    toml.push_str(&format!(
                        "secretKey = {}\n",
                        Self::toml_string(&config.secret_key)
                    ));
                }
            }
        }

        Ok(toml)
    }

    fn toml_string(value: &str) -> String {
        serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
    }

    fn path_to_arg(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    #[cfg(unix)]
    async fn build_ttyd_listen_args() -> Result<Vec<String>, String> {
        Self::prepare_ttyd_socket().await?;

        Ok(vec![
            "-i".to_string(),
            TTYD_SOCKET_PATH.to_string(),
            "-b".to_string(),
            "/api/terminal".to_string(),
            "-W".to_string(),
        ])
    }

    #[cfg(windows)]
    async fn build_ttyd_listen_args() -> Result<Vec<String>, String> {
        let cwd = std::env::var("USERPROFILE")
            .ok()
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string())
            });

        Ok(vec![
            "-i".to_string(),
            TTYD_TCP_HOST.to_string(),
            "-p".to_string(),
            TTYD_TCP_PORT.to_string(),
            "-b".to_string(),
            "/api/terminal".to_string(),
            "-w".to_string(),
            cwd,
            "-W".to_string(),
        ])
    }

    #[cfg(unix)]
    async fn prepare_ttyd_socket() -> Result<(), String> {
        let socket_path = std::path::Path::new(TTYD_SOCKET_PATH);

        if let Some(socket_dir) = socket_path.parent() {
            if !socket_dir.exists() {
                tokio::fs::create_dir_all(socket_dir)
                    .await
                    .map_err(|e| format!("Failed to create socket directory: {}", e))?;
            }
        }

        if tokio::fs::try_exists(TTYD_SOCKET_PATH)
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_file(TTYD_SOCKET_PATH)
                .await
                .map_err(|e| format!("Failed to remove old socket: {}", e))?;
        }

        Ok(())
    }

    pub async fn health_check(&self, config: &ExtensionsConfig) {
        let checks: Vec<_> = ExtensionId::all()
            .iter()
            .filter_map(|id| {
                if Self::is_enabled_for_config(*id, config) && self.check_available(*id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        let needs_restart: Vec<_> = {
            let processes = self.processes.read().await;
            checks
                .into_iter()
                .filter(|id| {
                    if let Some(proc) = processes.get(id) {
                        proc.child.id().is_none()
                    } else {
                        true
                    }
                })
                .collect()
        };

        let restart_futures: Vec<_> = needs_restart
            .into_iter()
            .map(|id| async move {
                tracing::info!("Health check: restarting {}", id);
                if let Err(e) = self.start(id, config).await {
                    tracing::error!("Failed to restart {}: {}", id, e);
                }
            })
            .collect();

        futures::future::join_all(restart_futures).await;
    }

    pub async fn start_enabled(&self, config: &ExtensionsConfig) {
        let start_futures: Vec<_> = ExtensionId::all()
            .iter()
            .filter(|id| Self::is_enabled_for_config(**id, config) && self.check_available(**id))
            .map(|id| async move {
                if let Err(e) = self.start(*id, config).await {
                    tracing::error!("Failed to start {}: {}", id, e);
                }
            })
            .collect();

        futures::future::join_all(start_futures).await;
    }

    pub async fn stop_all(&self) {
        let stop_futures: Vec<_> = ExtensionId::all().iter().map(|id| self.stop(*id)).collect();
        futures::future::join_all(stop_futures).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn easytier_launch_revalidates_full_configuration() {
        let config = EasytierConfig {
            config_mode: EasytierConfigMode::Full,
            custom_toml: "instance_name = [".to_string(),
            ..Default::default()
        };

        let error = ExtensionManager::build_easytier_launch(&config)
            .await
            .err()
            .expect("invalid full configuration should fail launch validation");
        assert!(error.starts_with("EasyTier full configuration is not valid TOML:"));
    }

    #[test]
    fn easytier_quick_mode_keeps_command_line_arguments() {
        let config = EasytierConfig {
            network_name: "one-kvm".to_string(),
            network_secret: "secret".to_string(),
            peer_urls: vec![
                "tcp://peer-one:11010".to_string(),
                String::new(),
                "udp://peer-two:11010".to_string(),
            ],
            virtual_ip: Some("10.20.30.40/24".to_string()),
            ..Default::default()
        };

        assert_eq!(
            ExtensionManager::build_easytier_quick_args(&config),
            vec![
                "--network-name",
                "one-kvm",
                "--network-secret",
                "secret",
                "--peers",
                "tcp://peer-one:11010",
                "--peers",
                "udp://peer-two:11010",
                "-i",
                "10.20.30.40/24",
            ]
        );
    }

    #[tokio::test]
    async fn easytier_full_mode_uses_protected_temporary_config() {
        let config_text = "instance_name = \"one-kvm\"\n";
        let config = EasytierConfig {
            config_mode: EasytierConfigMode::Full,
            network_name: "ignored-quick-network".to_string(),
            custom_toml: config_text.to_string(),
            ..Default::default()
        };

        let launch = ExtensionManager::build_easytier_launch(&config)
            .await
            .expect("full EasyTier launch should build");
        assert_eq!(launch.args[0], "-c");

        let config_path = std::path::PathBuf::from(&launch.args[1]);
        assert_eq!(
            config_path.file_name().and_then(|name| name.to_str()),
            Some("easytier.toml")
        );
        assert_eq!(
            tokio::fs::read_to_string(&config_path).await.unwrap(),
            config_text
        );

        drop(launch);
        assert!(!config_path.exists());
    }

    #[test]
    fn easytier_auto_start_uses_fields_for_selected_mode() {
        let mut config = ExtensionsConfig::default();
        config.easytier.enabled = true;
        config.easytier.network_name = "quick-network".to_string();
        assert!(ExtensionManager::is_enabled_for_config(
            ExtensionId::Easytier,
            &config
        ));

        config.easytier.config_mode = EasytierConfigMode::Full;
        assert!(!ExtensionManager::is_enabled_for_config(
            ExtensionId::Easytier,
            &config
        ));

        config.easytier.network_name.clear();
        config.easytier.custom_toml = "instance_name = \"one-kvm\"".to_string();
        assert!(ExtensionManager::is_enabled_for_config(
            ExtensionId::Easytier,
            &config
        ));
    }
}
