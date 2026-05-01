use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::types::*;
use crate::events::EventBus;

const LOG_BUFFER_SIZE: usize = 200;
const LOG_BATCH_SIZE: usize = 16;

pub const TTYD_SOCKET_PATH: &str = "/var/run/one-kvm/ttyd.sock";

struct ExtensionProcess {
    child: Child,
    logs: Arc<RwLock<VecDeque<String>>>,
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
            .map(|id| (*id, Path::new(id.binary_path()).exists()))
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
            return Err(format!("{} not found at {}", id, id.binary_path()));
        }

        self.stop(id).await.ok();

        let args = self.build_args(id, config).await?;

        tracing::info!(
            "Starting extension {}: {} {}",
            id,
            id.binary_path(),
            args.join(" ")
        );

        let mut child = Command::new(id.binary_path())
            .args(&args)
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
        let mut local_buffer = Vec::with_capacity(LOG_BATCH_SIZE);

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    tracing::debug!("[{}] {}", id, line);
                    local_buffer.push(line);

                    if local_buffer.len() >= LOG_BATCH_SIZE {
                        Self::flush_logs(&logs, &mut local_buffer).await;
                    }
                }
                Ok(None) => {
                    if !local_buffer.is_empty() {
                        Self::flush_logs(&logs, &mut local_buffer).await;
                    }
                    break;
                }
                Err(e) => {
                    tracing::warn!("[{}] Error reading log: {}", id, e);
                    break;
                }
            }
        }
    }

    async fn flush_logs(logs: &RwLock<VecDeque<String>>, buffer: &mut Vec<String>) {
        let mut logs = logs.write().await;
        for line in buffer.drain(..) {
            if logs.len() >= LOG_BUFFER_SIZE {
                logs.pop_front();
            }
            logs.push_back(line);
        }
    }

    async fn build_args(
        &self,
        id: ExtensionId,
        config: &ExtensionsConfig,
    ) -> Result<Vec<String>, String> {
        match id {
            ExtensionId::Ttyd => {
                let c = &config.ttyd;

                Self::prepare_ttyd_socket().await?;

                let mut args = vec![
                    "-i".to_string(),
                    TTYD_SOCKET_PATH.to_string(),
                    "-b".to_string(),
                    "/api/terminal".to_string(),
                    "-W".to_string(),
                ];

                args.push(c.shell.clone());
                Ok(args)
            }

            ExtensionId::Gostc => {
                let c = &config.gostc;
                if c.addr.trim().is_empty() {
                    return Err("GOSTC server address is required".into());
                }
                if c.key.is_empty() {
                    return Err("GOSTC client key is required".into());
                }

                let mut args = Vec::new();

                if c.tls {
                    args.push("--tls=true".to_string());
                }

                args.extend(["-addr".to_string(), c.addr.trim().to_string()]);

                args.extend(["-key".to_string(), c.key.clone()]);

                Ok(args)
            }

            ExtensionId::Easytier => {
                let c = &config.easytier;
                if c.network_name.is_empty() {
                    return Err("EasyTier network name is required".into());
                }

                let mut args = vec![
                    "--network-name".to_string(),
                    c.network_name.clone(),
                    "--network-secret".to_string(),
                    c.network_secret.clone(),
                ];

                for peer in &c.peer_urls {
                    if !peer.is_empty() {
                        args.extend(["--peers".to_string(), peer.clone()]);
                    }
                }

                if let Some(ref ip) = c.virtual_ip {
                    if !ip.is_empty() {
                        args.extend(["-i".to_string(), ip.clone()]);
                    } else {
                        args.push("-d".to_string());
                    }
                } else {
                    args.push("-d".to_string());
                }

                Ok(args)
            }
        }
    }

    async fn prepare_ttyd_socket() -> Result<(), String> {
        let socket_path = Path::new(TTYD_SOCKET_PATH);

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
                let should_run = match id {
                    ExtensionId::Ttyd => config.ttyd.enabled,
                    ExtensionId::Gostc => {
                        config.gostc.enabled
                            && !config.gostc.key.is_empty()
                            && !config.gostc.addr.trim().is_empty()
                    }
                    ExtensionId::Easytier => {
                        config.easytier.enabled && !config.easytier.network_name.is_empty()
                    }
                };
                if should_run && self.check_available(*id) {
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
        use futures::Future;
        use std::pin::Pin;

        let mut start_futures: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>> = Vec::new();

        if config.ttyd.enabled && self.check_available(ExtensionId::Ttyd) {
            start_futures.push(Box::pin(async {
                if let Err(e) = self.start(ExtensionId::Ttyd, config).await {
                    tracing::error!("Failed to start ttyd: {}", e);
                }
            }));
        }

        if config.gostc.enabled
            && !config.gostc.key.is_empty()
            && !config.gostc.addr.trim().is_empty()
            && self.check_available(ExtensionId::Gostc)
        {
            start_futures.push(Box::pin(async {
                if let Err(e) = self.start(ExtensionId::Gostc, config).await {
                    tracing::error!("Failed to start gostc: {}", e);
                }
            }));
        }

        if config.easytier.enabled
            && !config.easytier.network_name.is_empty()
            && self.check_available(ExtensionId::Easytier)
        {
            start_futures.push(Box::pin(async {
                if let Err(e) = self.start(ExtensionId::Easytier, config).await {
                    tracing::error!("Failed to start easytier: {}", e);
                }
            }));
        }

        futures::future::join_all(start_futures).await;
    }

    pub async fn stop_all(&self) {
        let stop_futures: Vec<_> = ExtensionId::all().iter().map(|id| self.stop(*id)).collect();
        futures::future::join_all(stop_futures).await;
    }
}
