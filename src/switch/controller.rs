//! KVM 切换器控制器
//!
//! 负责 BliSwitch / XH-HK4401 多路 KVM 切换器的串口控制。后台工作线程持有串口，
//! 解析切换器上报的当前通道心跳帧，并响应通道切换命令。

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::types::{SwitchState, SWITCH_DEFAULT_BAUD_RATE};
use crate::error::{AppError, Result};

/// 交换机控制器的运行时配置。
#[derive(Debug, Clone)]
pub struct SwitchControllerConfig {
    pub enabled: bool,
    pub device: String,
    pub baud_rate: u32,
    pub channel_count: u8,
}

impl Default for SwitchControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            baud_rate: SWITCH_DEFAULT_BAUD_RATE,
            channel_count: 8,
        }
    }
}

enum WorkerCommand {
    Switch(u8),
    Shutdown,
}

/// 控制器持有的单个工作线程句柄。
struct WorkerHandles {
    cmd_tx: Sender<WorkerCommand>,
    join: Option<thread::JoinHandle<()>>,
    current_channel: Arc<AtomicU8>,
    connected: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

struct SwitchInner {
    config: SwitchControllerConfig,
    worker: Option<WorkerHandles>,
}

/// 管理 KVM 切换器串口工作线程，支持配置热重载。
pub struct SwitchController {
    inner: RwLock<SwitchInner>,
}

impl SwitchController {
    pub fn new(config: SwitchControllerConfig) -> Self {
        Self {
            inner: RwLock::new(SwitchInner { config, worker: None }),
        }
    }

    fn spawn_worker(config: &SwitchControllerConfig) -> WorkerHandles {
        let (cmd_tx, cmd_rx) = channel::<WorkerCommand>();
        let current_channel = Arc::new(AtomicU8::new(0));
        let connected = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));

        let config = config.clone();
        let worker_current = current_channel.clone();
        let worker_connected = connected.clone();
        let worker_error = error.clone();
        let join = thread::spawn(move || {
            run_worker(config, cmd_rx, worker_current, worker_connected, worker_error);
        });

        WorkerHandles {
            cmd_tx,
            join: Some(join),
            current_channel,
            connected,
            error,
        }
    }

    async fn stop_worker(inner: &mut SwitchInner) {
        if let Some(worker) = inner.worker.take() {
            let _ = worker.cmd_tx.send(WorkerCommand::Shutdown);
            if let Some(join) = worker.join {
                let _ = join.join();
            }
        }
    }

    pub async fn init(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        if !inner.config.enabled {
            info!("KVM switch disabled in configuration");
            return Ok(());
        }
        if inner.config.device.trim().is_empty() {
            warn!("KVM switch enabled but no serial device configured");
            return Err(AppError::Config(
                "KVM switch device cannot be empty".to_string(),
            ));
        }

        info!(
            "Initializing KVM switch controller on {} @ {}",
            inner.config.device, inner.config.baud_rate
        );

        Self::stop_worker(&mut inner).await;
        inner.worker = Some(Self::spawn_worker(&inner.config));
        info!("KVM switch worker started");
        Ok(())
    }

    pub async fn reload(&self, config: SwitchControllerConfig) -> Result<()> {
        let mut inner = self.inner.write().await;

        info!("Reloading KVM switch controller configuration");
        Self::stop_worker(&mut inner).await;
        inner.config = config;

        if !inner.config.enabled {
            info!("KVM switch disabled after reload");
            return Ok(());
        }
        if inner.config.device.trim().is_empty() {
            return Err(AppError::Config(
                "KVM switch device cannot be empty".to_string(),
            ));
        }

        inner.worker = Some(Self::spawn_worker(&inner.config));
        info!("KVM switch worker restarted");
        Ok(())
    }

    /// 切换到给定的 1 基通道。
    pub async fn switch_to_channel(&self, channel: u8) -> Result<()> {
        let inner = self.inner.read().await;

        let Some(worker) = inner.worker.as_ref() else {
            return Err(AppError::Config(
                "KVM switch not initialized or disabled".to_string(),
            ));
        };

        if channel < 1 || channel > inner.config.channel_count {
            return Err(AppError::BadRequest(format!(
                "Invalid KVM switch channel: must be 1-{}",
                inner.config.channel_count
            )));
        }

        if !worker.connected.load(Ordering::Relaxed) {
            return Err(AppError::Internal(format!(
                "KVM switch serial device {} is not connected",
                worker
                    .error
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| inner.config.device.clone())
            )));
        }

        worker
            .cmd_tx
            .send(WorkerCommand::Switch(channel))
            .map_err(|_| AppError::Internal("KVM switch worker unavailable".to_string()))?;

        debug!("Requested KVM switch to channel {}", channel);
        Ok(())
    }

    /// 生成运行时状态快照。
    pub async fn state(&self) -> SwitchState {
        let inner = self.inner.read().await;
        let worker = inner.worker.as_ref();

        let (current_channel, connected, error) = match worker {
            Some(worker) => {
                let current = worker.current_channel.load(Ordering::Relaxed);
                (
                    if current == 0 { None } else { Some(current) },
                    worker.connected.load(Ordering::Relaxed),
                    worker.error.lock().unwrap().clone(),
                )
            }
            None => (None, false, None),
        };

        SwitchState {
            available: inner.config.enabled,
            connected,
            device: inner.config.device.clone(),
            baud_rate: inner.config.baud_rate,
            channel_count: inner.config.channel_count,
            current_channel,
            error,
        }
    }
}

// ---------------------------------------------------------------------------
// 串口工作线程
// ---------------------------------------------------------------------------

fn run_worker(
    config: SwitchControllerConfig,
    cmd_rx: Receiver<WorkerCommand>,
    current_channel: Arc<AtomicU8>,
    connected: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    // 外层循环：打开串口并驱动操作循环；断开后自动重连。
    loop {
        let mut port = match serialport::new(&config.device, config.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(port) => port,
            Err(e) => {
                connected.store(false, Ordering::Relaxed);
                *error.lock().unwrap() = Some(format!("Failed to open {}: {}", config.device, e));
                warn!("KVM switch serial open failed: {}", e);
                if drain_shutdown(&cmd_rx) {
                    return;
                }
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        connected.store(true, Ordering::Relaxed);
        current_channel.store(0, Ordering::Relaxed);
        *error.lock().unwrap() = None;
        info!("KVM switch serial {} connected", config.device);

        let mut data: Vec<u8> = Vec::new();

        // 操作循环：读取心跳帧解析当前通道，并处理切换/关闭命令。
        'operation: loop {
            loop {
                match cmd_rx.try_recv() {
                    Ok(WorkerCommand::Shutdown) => {
                        connected.store(false, Ordering::Relaxed);
                        info!("KVM switch worker shutting down");
                        return;
                    }
                    Ok(WorkerCommand::Switch(channel)) => {
                        send_channel(&mut port, &config, channel, &current_channel);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        connected.store(false, Ordering::Relaxed);
                        return;
                    }
                }
            }

            let available = match port.bytes_to_read() {
                Ok(n) => n,
                Err(e) => {
                    warn!("KVM switch serial poll error: {}", e);
                    connected.store(false, Ordering::Relaxed);
                    *error.lock().unwrap() = Some(format!("Serial poll failed: {}", e));
                    break 'operation;
                }
            };
            if available > 0 {
                let mut buf = vec![0u8; available as usize];
                match port.read(&mut buf) {
                    Ok(read) => {
                        data.extend_from_slice(&buf[..read]);
                        if let Some(channel) = parse_current_channel(&data) {
                            current_channel.store(channel, Ordering::Relaxed);
                        }
                        // 仅保留可能跨帧匹配的尾部字节。
                        if data.len() > 32 {
                            let keep = data.len().saturating_sub(16);
                            data.drain(..keep);
                        }
                    }
                    Err(e) => {
                        warn!("KVM switch serial read error: {}", e);
                        connected.store(false, Ordering::Relaxed);
                        *error.lock().unwrap() = Some(format!("Serial read failed: {}", e));
                        break 'operation;
                    }
                }
            }

            thread::sleep(Duration::from_millis(20));
        }

        if drain_shutdown(&cmd_rx) {
            return;
        }
        connected.store(false, Ordering::Relaxed);
        thread::sleep(Duration::from_secs(1));
    }
}

/// 发送 1 基通道切换命令，并在协议 2 下乐观更新当前通道。
fn send_channel(
    port: &mut Box<dyn serialport::SerialPort>,
    config: &SwitchControllerConfig,
    channel: u8,
    current_channel: &Arc<AtomicU8>,
) {
    let cmd = build_switch_cmd(channel);
    if let Err(e) = port.write_all(&cmd).and_then(|_| port.flush()) {
        warn!("KVM switch write failed: {}", e);
        return;
    }
    debug!("KVM switch command sent for channel {}: {:?}", channel, cmd);
}

/// 生成切换到 1 基通道的原始字节。
///
/// 协议 1：`SW{port}\r\nAG{port:02d}gA`；协议 2：`G{port:02d}gA\x00`。
pub fn build_switch_cmd(channel: u8) -> Vec<u8> {
    // BliSwitch / XH-HK4401 协议（V1）：切换到第 N 路发送 `SW{N}\r\nAG{NN}gA`。
    format!("SW{}\r\nAG{:02}gA", channel, channel).into_bytes()
}

/// 从心跳字节流中解析最近一次上报的当前通道。
///
/// 切换器以 5 字节帧 `G0{1..8}gA` 上报当前通道（协议 1），协议 2 末尾追加一个 `\x00`；
/// 部分固件还会回显一个前导 `A`（`AG0{1..8}gA`）。这里统一匹配 5 字节的 `G0{1..8}gA`
/// 子帧（与 BliSwitch 官方文档一致，且能匹配实际的 `G03gA` 心跳流）。
///
/// 返回当前通道（1 基）。
fn parse_current_channel(data: &[u8]) -> Option<u8> {
    let mut channel = None;
    let mut i = 0usize;
    while i + 5 <= data.len() {
        if data[i] == b'G'
            && data[i + 1] == b'0'
            && (b'1'..=b'8').contains(&data[i + 2])
            && data[i + 3] == b'g'
            && data[i + 4] == b'A'
        {
            channel = Some(data[i + 2] - b'0');
            i += 5;
            continue;
        }
        i += 1;
    }
    channel
}

/// 排空待处理命令；收到 Shutdown 时返回 `true`。
fn drain_shutdown(cmd_rx: &Receiver<WorkerCommand>) -> bool {
    loop {
        match cmd_rx.try_recv() {
            Ok(WorkerCommand::Shutdown) => return true,
            Ok(WorkerCommand::Switch(_)) => {}
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_switch_cmd_v1() {
        assert_eq!(
            build_switch_cmd(1),
            b"SW1\r\nAG01gA".to_vec()
        );
        assert_eq!(
            build_switch_cmd(8),
            b"SW8\r\nAG08gA".to_vec()
        );
    }

    #[test]
    fn test_parse_v1_heartbeat() {
        assert_eq!(parse_current_channel(b"AG01gA"), Some(1));
        assert_eq!(parse_current_channel(b"G01gA"), Some(1));
    }

    #[test]
    fn test_parse_raw_observed_heartbeat() {
        // 实际 BliSwitch 线路上观测到的心跳流。
        let data = b"G03gAG03gAG03gAG03gA";
        assert_eq!(parse_current_channel(data), Some(3));
    }

    #[test]
    fn test_parse_last_frame_wins() {
        let data = b"garbage AG02gA more AG07gA";
        assert_eq!(parse_current_channel(data), Some(7));
    }

    #[test]
    fn test_parse_no_match() {
        assert_eq!(parse_current_channel(b"nothing"), None);
    }
}
