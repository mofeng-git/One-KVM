use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::types::*;
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
}

struct ExtensionLaunch {
    args: Vec<String>,
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
        if !id.compiled() {
            return false;
        }
        *self.availability.get(&id).unwrap_or(&false)
    }

    fn is_enabled_for_config(id: ExtensionId, config: &ExtensionsConfig) -> bool {
        match id {
            ExtensionId::Ttyd => config.ttyd.enabled,
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
        if !id.compiled() {
            return Err(format!("{id} is not compiled in"));
        }
        if !self.check_available(id) {
            return Err(format!(
                "{} not found at {}",
                id,
                id.binary_path().display()
            ));
        }

        self.stop(id).await.ok();

        let launch = self.build_launch(id, config).await?;

        tracing::info!("Starting extension {}: {}", id, id.binary_path().display());

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
        processes.insert(id, ExtensionProcess { child, logs });
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
        };
        Ok(ExtensionLaunch { args })
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
