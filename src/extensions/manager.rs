//! Extension process manager

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::types::*;

/// Maximum number of log lines to keep per extension
const LOG_BUFFER_SIZE: usize = 200;

/// Number of log lines to buffer before flushing to shared storage
const LOG_BATCH_SIZE: usize = 16;

/// Unix socket path for ttyd
pub const TTYD_SOCKET_PATH: &str = "/var/run/one-kvm/ttyd.sock";

/// Extension process with log buffer
struct ExtensionProcess {
    child: Child,
    logs: Arc<RwLock<VecDeque<String>>>,
}

/// Extension manager handles lifecycle of external processes
pub struct ExtensionManager {
    processes: RwLock<HashMap<ExtensionId, ExtensionProcess>>,
    /// Cached availability status (checked once at startup)
    availability: HashMap<ExtensionId, bool>,
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionManager {
    /// Create a new extension manager with cached availability
    pub fn new() -> Self {
        // Check availability once at startup
        let availability = ExtensionId::all()
            .iter()
            .map(|id| (*id, Path::new(id.binary_path()).exists()))
            .collect();

        Self {
            processes: RwLock::new(HashMap::new()),
            availability,
        }
    }

    /// Check if the binary for an extension is available (cached)
    pub fn check_available(&self, id: ExtensionId) -> bool {
        *self.availability.get(&id).unwrap_or(&false)
    }

    /// Get the current status of an extension
    pub async fn status(&self, id: ExtensionId) -> ExtensionStatus {
        if !self.check_available(id) {
            return ExtensionStatus::Unavailable;
        }

        let processes = self.processes.read().await;
        match processes.get(&id) {
            Some(proc) => {
                if let Some(pid) = proc.child.id() {
                    ExtensionStatus::Running { pid }
                } else {
                    ExtensionStatus::Stopped
                }
            }
            None => ExtensionStatus::Stopped,
        }
    }

    /// Start an extension with the given configuration
    pub async fn start(&self, id: ExtensionId, config: &ExtensionsConfig) -> Result<(), String> {
        if !self.check_available(id) {
            return Err(format!(
                "{} not found at {}",
                id.display_name(),
                id.binary_path()
            ));
        }

        // Stop existing process first
        self.stop(id).await.ok();

        // Build command arguments
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
            .map_err(|e| format!("Failed to start {}: {}", id.display_name(), e))?;

        let logs = Arc::new(RwLock::new(VecDeque::with_capacity(LOG_BUFFER_SIZE)));

        // Spawn log collector for stdout
        if let Some(stdout) = child.stdout.take() {
            let logs_clone = logs.clone();
            let id_clone = id;
            tokio::spawn(async move {
                Self::collect_logs(id_clone, stdout, logs_clone).await;
            });
        }

        // Spawn log collector for stderr
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

        Ok(())
    }

    /// Stop an extension
    pub async fn stop(&self, id: ExtensionId) -> Result<(), String> {
        let mut processes = self.processes.write().await;
        if let Some(mut proc) = processes.remove(&id) {
            tracing::info!("Stopping extension {}", id);
            if let Err(e) = proc.child.kill().await {
                tracing::warn!("Failed to kill {}: {}", id, e);
            }
        }
        Ok(())
    }

    /// Get recent logs for an extension
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

    /// Collect logs from a stream with batched writes to reduce lock contention
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

                    // Flush when batch is full
                    if local_buffer.len() >= LOG_BATCH_SIZE {
                        Self::flush_logs(&logs, &mut local_buffer).await;
                    }
                }
                Ok(None) => {
                    // Stream ended, flush remaining logs
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

    /// Flush buffered logs to shared storage
    async fn flush_logs(logs: &RwLock<VecDeque<String>>, buffer: &mut Vec<String>) {
        let mut logs = logs.write().await;
        for line in buffer.drain(..) {
            if logs.len() >= LOG_BUFFER_SIZE {
                logs.pop_front();
            }
            logs.push_back(line);
        }
    }

    /// Build command arguments for an extension
    async fn build_args(&self, id: ExtensionId, config: &ExtensionsConfig) -> Result<Vec<String>, String> {
        match id {
            ExtensionId::Ttyd => {
                let c = &config.ttyd;

                // Prepare socket directory and clean up old socket (async)
                Self::prepare_ttyd_socket().await?;

                let mut args = vec![
                    "-i".to_string(), TTYD_SOCKET_PATH.to_string(),  // Unix socket
                    "-b".to_string(), "/api/terminal".to_string(),   // Base path for reverse proxy
                    "-W".to_string(),                                 // Writable (allow input)
                ];

                // Add credential if set (still useful for additional security layer)
                if let Some(ref cred) = c.credential {
                    if !cred.is_empty() {
                        args.extend(["-c".to_string(), cred.clone()]);
                    }
                }

                // Add shell as last argument
                args.push(c.shell.clone());
                Ok(args)
            }

            ExtensionId::Gostc => {
                let c = &config.gostc;
                if c.key.is_empty() {
                    return Err("GOSTC client key is required".into());
                }

                let mut args = Vec::new();

                // Add TLS flag
                if c.tls {
                    args.push("--tls=true".to_string());
                }

                // Add server address
                if !c.addr.is_empty() {
                    args.extend(["-addr".to_string(), c.addr.clone()]);
                }

                // Add client key
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

                // Add peer URLs
                for peer in &c.peer_urls {
                    if !peer.is_empty() {
                        args.extend(["--peers".to_string(), peer.clone()]);
                    }
                }

                // Add virtual IP: use -d for DHCP if empty, or -i for specific IP
                if let Some(ref ip) = c.virtual_ip {
                    if !ip.is_empty() {
                        // Use specific IP with -i (must include CIDR, e.g., 10.0.0.1/24)
                        args.extend(["-i".to_string(), ip.clone()]);
                    } else {
                        // Empty string means use DHCP
                        args.push("-d".to_string());
                    }
                } else {
                    // None means use DHCP
                    args.push("-d".to_string());
                }

                Ok(args)
            }
        }
    }

    /// Prepare ttyd socket directory and clean up old socket file
    async fn prepare_ttyd_socket() -> Result<(), String> {
        let socket_path = Path::new(TTYD_SOCKET_PATH);

        // Ensure socket directory exists
        if let Some(socket_dir) = socket_path.parent() {
            if !socket_dir.exists() {
                tokio::fs::create_dir_all(socket_dir)
                    .await
                    .map_err(|e| format!("Failed to create socket directory: {}", e))?;
            }
        }

        // Remove old socket file if exists
        if tokio::fs::try_exists(TTYD_SOCKET_PATH).await.unwrap_or(false) {
            tokio::fs::remove_file(TTYD_SOCKET_PATH)
                .await
                .map_err(|e| format!("Failed to remove old socket: {}", e))?;
        }

        Ok(())
    }

    /// Health check - restart crashed processes that should be running
    pub async fn health_check(&self, config: &ExtensionsConfig) {
        // Collect extensions that need restart check
        let checks: Vec<_> = ExtensionId::all()
            .iter()
            .filter_map(|id| {
                let should_run = match id {
                    ExtensionId::Ttyd => config.ttyd.enabled,
                    ExtensionId::Gostc => config.gostc.enabled && !config.gostc.key.is_empty(),
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

        // Check which ones need restart (single read lock)
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

        // Restart all crashed extensions in parallel
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

    /// Start all enabled extensions in parallel
    pub async fn start_enabled(&self, config: &ExtensionsConfig) {
        use std::pin::Pin;
        use futures::Future;

        let mut start_futures: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>> = Vec::new();

        // Collect enabled extensions
        if config.ttyd.enabled && self.check_available(ExtensionId::Ttyd) {
            start_futures.push(Box::pin(async {
                if let Err(e) = self.start(ExtensionId::Ttyd, config).await {
                    tracing::error!("Failed to start ttyd: {}", e);
                }
            }));
        }

        if config.gostc.enabled
            && !config.gostc.key.is_empty()
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

        // Start all in parallel
        futures::future::join_all(start_futures).await;
    }

    /// Stop all running extensions in parallel
    pub async fn stop_all(&self) {
        let stop_futures: Vec<_> = ExtensionId::all()
            .iter()
            .map(|id| self.stop(*id))
            .collect();
        futures::future::join_all(stop_futures).await;
    }
}
