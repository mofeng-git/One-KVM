//! Video streamer that integrates capture and streaming
//!
//! This module provides a high-level interface for video capture and streaming,
//! managing the lifecycle of the capture thread and MJPEG/WebRTC distribution.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

use super::device::{enumerate_devices, find_best_device, VideoDeviceInfo};
use super::format::{PixelFormat, Resolution};
use super::frame::{FrameBuffer, FrameBufferPool, VideoFrame};
use crate::error::{AppError, Result};
use crate::events::{EventBus, SystemEvent};
use crate::stream::MjpegStreamHandler;
use crate::utils::LogThrottler;
use crate::video::v4l2r_capture::V4l2rCaptureStream;

/// Minimum valid frame size for capture
const MIN_CAPTURE_FRAME_SIZE: usize = 128;
/// Validate JPEG header every N frames to reduce overhead
const JPEG_VALIDATE_INTERVAL: u64 = 30;

/// Streamer configuration
#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// Device path (None = auto-detect)
    pub device_path: Option<PathBuf>,
    /// Desired resolution
    pub resolution: Resolution,
    /// Desired format
    pub format: PixelFormat,
    /// Desired FPS
    pub fps: u32,
    /// JPEG quality (1-100)
    pub jpeg_quality: u8,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            device_path: None,
            resolution: Resolution::HD1080,
            format: PixelFormat::Mjpeg,
            fps: 30,
            jpeg_quality: 80,
        }
    }
}

/// Streamer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerState {
    /// Not initialized
    Uninitialized,
    /// Ready but not streaming
    Ready,
    /// Actively streaming
    Streaming,
    /// No video signal
    NoSignal,
    /// Error occurred
    Error,
    /// Device was lost (unplugged)
    DeviceLost,
    /// Device is being recovered (reconnecting)
    Recovering,
}

/// Video streamer service
pub struct Streamer {
    config: RwLock<StreamerConfig>,
    mjpeg_handler: Arc<MjpegStreamHandler>,
    current_device: RwLock<Option<VideoDeviceInfo>>,
    state: RwLock<StreamerState>,
    start_lock: tokio::sync::Mutex<()>,
    direct_stop: AtomicBool,
    direct_active: AtomicBool,
    direct_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    current_fps: AtomicU32,
    /// Event bus for broadcasting state changes (optional)
    events: RwLock<Option<Arc<EventBus>>>,
    /// Last published state (for change detection)
    last_published_state: RwLock<Option<StreamerState>>,
    /// Flag to indicate config is being changed (prevents auto-start during config change)
    config_changing: std::sync::atomic::AtomicBool,
    /// Flag to indicate background tasks (stats, cleanup, monitor) have been started
    /// These tasks should only be started once per Streamer instance
    background_tasks_started: std::sync::atomic::AtomicBool,
    /// Device recovery retry count
    recovery_retry_count: std::sync::atomic::AtomicU32,
    /// Device recovery in progress flag
    recovery_in_progress: std::sync::atomic::AtomicBool,
    /// Last lost device path (for recovery)
    last_lost_device: RwLock<Option<String>>,
    /// Last lost device reason (for logging)
    last_lost_reason: RwLock<Option<String>>,
}

impl Streamer {
    /// Create a new streamer
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(StreamerConfig::default()),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            config_changing: std::sync::atomic::AtomicBool::new(false),
            background_tasks_started: std::sync::atomic::AtomicBool::new(false),
            recovery_retry_count: std::sync::atomic::AtomicU32::new(0),
            recovery_in_progress: std::sync::atomic::AtomicBool::new(false),
            last_lost_device: RwLock::new(None),
            last_lost_reason: RwLock::new(None),
        })
    }

    /// Create with specific config
    pub fn with_config(config: StreamerConfig) -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(config),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            config_changing: std::sync::atomic::AtomicBool::new(false),
            background_tasks_started: std::sync::atomic::AtomicBool::new(false),
            recovery_retry_count: std::sync::atomic::AtomicU32::new(0),
            recovery_in_progress: std::sync::atomic::AtomicBool::new(false),
            last_lost_device: RwLock::new(None),
            last_lost_reason: RwLock::new(None),
        })
    }

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> SystemEvent {
        let state = *self.state.read().await;
        let device = self
            .current_device
            .read()
            .await
            .as_ref()
            .map(|d| d.path.display().to_string());

        SystemEvent::StreamStateChanged {
            state: match state {
                StreamerState::Uninitialized => "uninitialized".to_string(),
                StreamerState::Ready => "ready".to_string(),
                StreamerState::Streaming => "streaming".to_string(),
                StreamerState::NoSignal => "no_signal".to_string(),
                StreamerState::Error => "error".to_string(),
                StreamerState::DeviceLost => "device_lost".to_string(),
                StreamerState::Recovering => "recovering".to_string(),
            },
            device,
        }
    }

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    /// Get current state
    pub async fn state(&self) -> StreamerState {
        *self.state.read().await
    }

    /// Check if config is currently being changed
    /// When true, auto-start should be blocked to prevent device busy errors
    pub fn is_config_changing(&self) -> bool {
        self.config_changing
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get MJPEG handler for stream endpoints
    pub fn mjpeg_handler(&self) -> Arc<MjpegStreamHandler> {
        self.mjpeg_handler.clone()
    }

    /// Get current device info
    pub async fn current_device(&self) -> Option<VideoDeviceInfo> {
        self.current_device.read().await.clone()
    }

    /// Get current video configuration (format, resolution, fps)
    pub async fn current_video_config(&self) -> (PixelFormat, Resolution, u32) {
        let config = self.config.read().await;
        (config.format, config.resolution, config.fps)
    }

    /// Get current capture configuration for direct pipelines
    pub async fn current_capture_config(
        &self,
    ) -> (Option<PathBuf>, Resolution, PixelFormat, u32, u8) {
        let config = self.config.read().await;
        (
            config.device_path.clone(),
            config.resolution,
            config.format,
            config.fps,
            config.jpeg_quality,
        )
    }

    /// List available video devices
    pub async fn list_devices(&self) -> Result<Vec<VideoDeviceInfo>> {
        enumerate_devices()
    }

    /// Validate and apply requested video parameters without auto-selection
    pub async fn apply_video_config(
        self: &Arc<Self>,
        device_path: &str,
        format: PixelFormat,
        resolution: Resolution,
        fps: u32,
    ) -> Result<()> {
        // Set config_changing flag to prevent frontend mode sync during config change
        self.config_changing
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let result = self
            .apply_video_config_inner(device_path, format, resolution, fps)
            .await;

        // Clear the flag after config change is complete
        // The stream will be started by MJPEG client connection, not here
        self.config_changing
            .store(false, std::sync::atomic::Ordering::SeqCst);

        result
    }

    /// Internal implementation of apply_video_config
    async fn apply_video_config_inner(
        self: &Arc<Self>,
        device_path: &str,
        format: PixelFormat,
        resolution: Resolution,
        fps: u32,
    ) -> Result<()> {
        // Publish "config changing" event
        self.publish_event(SystemEvent::StreamConfigChanging {
            transition_id: None,
            reason: "device_switch".to_string(),
        })
        .await;

        let devices = enumerate_devices()?;
        let device = devices
            .into_iter()
            .find(|d| d.path.to_string_lossy() == device_path)
            .ok_or_else(|| AppError::VideoError("Video device not found".to_string()))?;

        // Validate format
        let fmt_info = device
            .formats
            .iter()
            .find(|f| f.format == format)
            .ok_or_else(|| AppError::VideoError("Requested format not supported".to_string()))?;

        // Validate resolution
        if !fmt_info.resolutions.is_empty()
            && !fmt_info
                .resolutions
                .iter()
                .any(|r| r.width == resolution.width && r.height == resolution.height)
        {
            return Err(AppError::VideoError(
                "Requested resolution not supported".to_string(),
            ));
        }

        // IMPORTANT: Disconnect all MJPEG clients FIRST before stopping capture
        // This prevents race conditions where clients try to reconnect and reopen the device
        info!("Disconnecting all MJPEG clients before config change...");
        self.mjpeg_handler.disconnect_all_clients();

        // Give clients time to receive the disconnect signal and close their connections
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Stop active capture and wait for device release
        if self.direct_active.load(Ordering::SeqCst) {
            info!("Stopping existing capture before applying new config...");
            self.stop().await?;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // Update config
        {
            let mut cfg = self.config.write().await;
            cfg.device_path = Some(device.path.clone());
            cfg.format = format;
            cfg.resolution = resolution;
            cfg.fps = fps;
        }

        *self.current_device.write().await = Some(device.clone());
        *self.state.write().await = StreamerState::Ready;

        // Publish "config applied" event
        info!(
            "Publishing StreamConfigApplied event: {}x{} {:?} @ {}fps",
            resolution.width, resolution.height, format, fps
        );
        self.publish_event(SystemEvent::StreamConfigApplied {
            transition_id: None,
            device: device_path.to_string(),
            resolution: (resolution.width, resolution.height),
            format: format!("{:?}", format),
            fps,
        })
        .await;

        // Note: We don't auto-start here anymore.
        // The stream will be started when MJPEG client connects (handlers.rs:790)
        // This avoids race conditions between config change and client reconnection.
        info!("Config applied, stream will start when client connects");

        Ok(())
    }

    /// Initialize with auto-detected device
    pub async fn init_auto(self: &Arc<Self>) -> Result<()> {
        info!("Auto-detecting video device...");

        let device = find_best_device()?;
        info!("Found device: {} ({})", device.name, device.path.display());

        self.init_with_device(device).await
    }

    /// Initialize with specific device
    pub async fn init_with_device(self: &Arc<Self>, device: VideoDeviceInfo) -> Result<()> {
        info!(
            "Initializing streamer with device: {} ({})",
            device.name,
            device.path.display()
        );

        // Determine best format for this device
        let config = self.config.read().await;
        let format = self.select_format(&device, config.format)?;
        let resolution = self.select_resolution(&device, &format, config.resolution)?;

        drop(config);

        // Update config with actual values
        {
            let mut config = self.config.write().await;
            config.device_path = Some(device.path.clone());
            config.format = format;
            config.resolution = resolution;
        }

        // Store device info
        *self.current_device.write().await = Some(device.clone());

        *self.state.write().await = StreamerState::Ready;

        info!("Streamer initialized: {} @ {}", format, resolution);
        Ok(())
    }

    /// Select best format for device
    fn select_format(
        &self,
        device: &VideoDeviceInfo,
        preferred: PixelFormat,
    ) -> Result<PixelFormat> {
        // Check if preferred format is available
        if device.formats.iter().any(|f| f.format == preferred) {
            return Ok(preferred);
        }

        // Select best available format
        device
            .formats
            .first()
            .map(|f| f.format)
            .ok_or_else(|| AppError::VideoError("No supported formats found".to_string()))
    }

    /// Select best resolution for format
    fn select_resolution(
        &self,
        device: &VideoDeviceInfo,
        format: &PixelFormat,
        preferred: Resolution,
    ) -> Result<Resolution> {
        let format_info = device
            .formats
            .iter()
            .find(|f| &f.format == format)
            .ok_or_else(|| AppError::VideoError("Format not found".to_string()))?;

        // Check if preferred resolution is available
        if format_info.resolutions.is_empty()
            || format_info
                .resolutions
                .iter()
                .any(|r| r.width == preferred.width && r.height == preferred.height)
        {
            return Ok(preferred);
        }

        // Select largest available resolution
        format_info
            .resolutions
            .first()
            .map(|r| r.resolution())
            .ok_or_else(|| AppError::VideoError("No resolutions available".to_string()))
    }

    /// Restart capture for recovery (direct capture path)
    async fn restart_capture(self: &Arc<Self>) -> Result<()> {
        self.direct_stop.store(false, Ordering::SeqCst);
        self.start().await?;

        // Wait briefly for the capture thread to initialize the device.
        // If it fails immediately, the state will flip to Error/DeviceLost.
        for _ in 0..5 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let state = *self.state.read().await;
            match state {
                StreamerState::Streaming | StreamerState::NoSignal => return Ok(()),
                StreamerState::Error | StreamerState::DeviceLost => {
                    return Err(AppError::VideoError(
                        "Failed to restart capture".to_string(),
                    ))
                }
                _ => {}
            }
        }

        Err(AppError::VideoError(
            "Capture restart timed out".to_string(),
        ))
    }

    /// Start streaming
    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let _lock = self.start_lock.lock().await;

        let state = self.state().await;
        if state == StreamerState::Streaming {
            return Ok(());
        }

        if state == StreamerState::Uninitialized {
            // Auto-initialize if not done
            self.init_auto().await?;
        }

        let device = self
            .current_device
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError::VideoError("No video device configured".to_string()))?;

        let config = self.config.read().await.clone();
        self.direct_stop.store(false, Ordering::SeqCst);
        self.direct_active.store(true, Ordering::SeqCst);

        let streamer = self.clone();
        let handle = tokio::task::spawn_blocking(move || {
            streamer.run_direct_capture(device.path, config);
        });
        *self.direct_handle.lock().await = Some(handle);

        // Set MJPEG handler online before starting capture
        self.mjpeg_handler.set_online();

        // Start background tasks only once per Streamer instance
        // Use compare_exchange to atomically check and set the flag
        if self
            .background_tasks_started
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
        {
            info!("Starting background tasks (stats, cleanup, monitor)");

            // Start stats broadcast task (sends stats updates every 1 second)
            let stats_ref = Arc::downgrade(self);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;

                    if let Some(streamer) = stats_ref.upgrade() {
                        let clients_stat = streamer.mjpeg_handler().get_clients_stat();
                        let clients = clients_stat.len() as u64;

                        streamer
                            .publish_event(SystemEvent::StreamStatsUpdate {
                                clients,
                                clients_stat,
                            })
                            .await;
                    } else {
                        break;
                    }
                }
            });

            // Start client cleanup task (removes stale clients every 5s)
            self.mjpeg_handler.clone().start_cleanup_task();

            // Start auto-pause monitor task (stops stream if no clients)
            let monitor_ref = Arc::downgrade(self);
            let monitor_handler = self.mjpeg_handler.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
                let mut zero_since: Option<std::time::Instant> = None;

                loop {
                    interval.tick().await;

                    let Some(streamer) = monitor_ref.upgrade() else {
                        break;
                    };

                    // Check auto-pause configuration
                    let config = monitor_handler.auto_pause_config();
                    if !config.enabled {
                        zero_since = None;
                        continue;
                    }

                    let count = monitor_handler.client_count();

                    if count == 0 {
                        if zero_since.is_none() {
                            zero_since = Some(std::time::Instant::now());
                            info!(
                                "No clients connected, starting shutdown timer ({}s)",
                                config.shutdown_delay_secs
                            );
                        } else if let Some(since) = zero_since {
                            if since.elapsed().as_secs() >= config.shutdown_delay_secs {
                                info!(
                                    "Auto-pausing stream (no clients for {}s)",
                                    config.shutdown_delay_secs
                                );
                                if let Err(e) = streamer.stop().await {
                                    error!("Auto-pause failed: {}", e);
                                }
                                break;
                            }
                        }
                    } else {
                        if zero_since.is_some() {
                            info!("Clients reconnected, canceling auto-pause");
                            zero_since = None;
                        }
                    }
                }
            });
        } else {
            debug!("Background tasks already started, skipping");
        }

        *self.state.write().await = StreamerState::Streaming;

        // Publish state change event so DeviceInfo broadcaster can update frontend
        self.publish_event(self.current_state_event().await).await;

        info!("Streaming started");
        Ok(())
    }

    /// Stop streaming
    pub async fn stop(&self) -> Result<()> {
        self.direct_stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.direct_handle.lock().await.take() {
            let _ = handle.await;
        }
        self.direct_active.store(false, Ordering::SeqCst);

        self.mjpeg_handler.set_offline();
        *self.state.write().await = StreamerState::Ready;

        // Publish state change event so DeviceInfo broadcaster can update frontend
        self.publish_event(self.current_state_event().await).await;

        info!("Streaming stopped");
        Ok(())
    }

    /// Direct capture loop for MJPEG mode (single loop, no broadcast)
    fn run_direct_capture(self: Arc<Self>, device_path: PathBuf, config: StreamerConfig) {
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 200;
        const IDLE_STOP_DELAY_SECS: u64 = 5;
        const BUFFER_COUNT: u32 = 2;

        let handle = tokio::runtime::Handle::current();
        let mut last_state = StreamerState::Streaming;

        let mut set_state = |new_state: StreamerState| {
            if new_state != last_state {
                handle.block_on(async {
                    *self.state.write().await = new_state;
                    self.publish_event(self.current_state_event().await).await;
                });
                last_state = new_state;
            }
        };

        let mut stream_opt: Option<V4l2rCaptureStream> = None;
        let mut last_error: Option<String> = None;

        for attempt in 0..MAX_RETRIES {
            if self.direct_stop.load(Ordering::Relaxed) {
                self.direct_active.store(false, Ordering::SeqCst);
                return;
            }

            match V4l2rCaptureStream::open(
                &device_path,
                config.resolution,
                config.format,
                config.fps,
                BUFFER_COUNT,
                Duration::from_secs(2),
            ) {
                Ok(stream) => {
                    stream_opt = Some(stream);
                    break;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("busy") || err_str.contains("resource") {
                        warn!(
                            "Device busy on attempt {}/{}, retrying in {}ms...",
                            attempt + 1,
                            MAX_RETRIES,
                            RETRY_DELAY_MS
                        );
                        std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
                        last_error = Some(err_str);
                        continue;
                    }
                    last_error = Some(err_str);
                    break;
                }
            }
        }

        let mut stream = match stream_opt {
            Some(stream) => stream,
            None => {
                error!(
                    "Failed to open device {:?}: {}",
                    device_path,
                    last_error.unwrap_or_else(|| "unknown error".to_string())
                );
                self.mjpeg_handler.set_offline();
                set_state(StreamerState::Error);
                self.direct_active.store(false, Ordering::SeqCst);
                self.current_fps.store(0, Ordering::Relaxed);
                return;
            }
        };

        let resolution = stream.resolution();
        let pixel_format = stream.format();
        let stride = stream.stride();

        info!(
            "Capture format: {}x{} {:?} stride={}",
            resolution.width, resolution.height, pixel_format, stride
        );

        let buffer_pool = Arc::new(FrameBufferPool::new(BUFFER_COUNT.max(4) as usize));
        let mut signal_present = true;
        let mut validate_counter: u64 = 0;
        let mut idle_since: Option<std::time::Instant> = None;

        let mut fps_frame_count: u64 = 0;
        let mut last_fps_time = std::time::Instant::now();
        let capture_error_throttler = LogThrottler::with_secs(5);
        let mut suppressed_capture_errors: HashMap<String, u64> = HashMap::new();

        let classify_capture_error = |err: &std::io::Error| -> String {
            let message = err.to_string();
            if message.contains("dqbuf failed") && message.contains("EINVAL") {
                "capture_dqbuf_einval".to_string()
            } else if message.contains("dqbuf failed") {
                "capture_dqbuf".to_string()
            } else {
                format!("capture_{:?}", err.kind())
            }
        };

        while !self.direct_stop.load(Ordering::Relaxed) {
            let mjpeg_clients = self.mjpeg_handler.client_count();
            if mjpeg_clients == 0 {
                if idle_since.is_none() {
                    idle_since = Some(std::time::Instant::now());
                    trace!("No active video consumers, starting idle timer");
                } else if let Some(since) = idle_since {
                    if since.elapsed().as_secs() >= IDLE_STOP_DELAY_SECS {
                        info!(
                            "No active video consumers for {}s, stopping capture",
                            IDLE_STOP_DELAY_SECS
                        );
                        self.mjpeg_handler.set_offline();
                        set_state(StreamerState::Ready);
                        break;
                    }
                }
            } else if idle_since.is_some() {
                trace!("Video consumers active, resetting idle timer");
                idle_since = None;
            }

            let mut owned = buffer_pool.take(MIN_CAPTURE_FRAME_SIZE);
            let meta = match stream.next_into(&mut owned) {
                Ok(meta) => meta,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::TimedOut {
                        if signal_present {
                            signal_present = false;
                            self.mjpeg_handler.set_offline();
                            set_state(StreamerState::NoSignal);
                            self.current_fps.store(0, Ordering::Relaxed);
                            fps_frame_count = 0;
                            last_fps_time = std::time::Instant::now();
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    let is_device_lost = match e.raw_os_error() {
                        Some(6) => true,   // ENXIO
                        Some(19) => true,  // ENODEV
                        Some(5) => true,   // EIO
                        Some(32) => true,  // EPIPE
                        Some(108) => true, // ESHUTDOWN
                        _ => false,
                    };

                    if is_device_lost {
                        error!("Video device lost: {} - {}", device_path.display(), e);
                        self.mjpeg_handler.set_offline();
                        handle.block_on(async {
                            *self.last_lost_device.write().await =
                                Some(device_path.display().to_string());
                            *self.last_lost_reason.write().await = Some(e.to_string());
                        });
                        set_state(StreamerState::DeviceLost);
                        handle.block_on(async {
                            let streamer = Arc::clone(&self);
                            tokio::spawn(async move {
                                streamer.start_device_recovery_internal().await;
                            });
                        });
                        break;
                    }

                    let key = classify_capture_error(&e);
                    if capture_error_throttler.should_log(&key) {
                        let suppressed = suppressed_capture_errors.remove(&key).unwrap_or(0);
                        if suppressed > 0 {
                            error!("Capture error: {} (suppressed {} repeats)", e, suppressed);
                        } else {
                            error!("Capture error: {}", e);
                        }
                    } else {
                        let counter = suppressed_capture_errors.entry(key).or_insert(0);
                        *counter = counter.saturating_add(1);
                    }
                    continue;
                }
            };

            let frame_size = meta.bytes_used;
            if frame_size < MIN_CAPTURE_FRAME_SIZE {
                continue;
            }

            validate_counter = validate_counter.wrapping_add(1);
            if pixel_format.is_compressed()
                && validate_counter % JPEG_VALIDATE_INTERVAL == 0
                && !VideoFrame::is_valid_jpeg_bytes(&owned[..frame_size])
            {
                continue;
            }

            owned.truncate(frame_size);
            let frame = VideoFrame::from_pooled(
                Arc::new(FrameBuffer::new(owned, Some(buffer_pool.clone()))),
                resolution,
                pixel_format,
                stride,
                meta.sequence,
            );

            if !signal_present {
                signal_present = true;
                self.mjpeg_handler.set_online();
                set_state(StreamerState::Streaming);
            }

            self.mjpeg_handler.update_frame(frame);

            fps_frame_count += 1;
            let fps_elapsed = last_fps_time.elapsed();
            if fps_elapsed >= std::time::Duration::from_secs(1) {
                let current_fps = fps_frame_count as f32 / fps_elapsed.as_secs_f32();
                fps_frame_count = 0;
                last_fps_time = std::time::Instant::now();
                self.current_fps
                    .store((current_fps * 100.0) as u32, Ordering::Relaxed);
            }
        }

        self.direct_active.store(false, Ordering::SeqCst);
        self.current_fps.store(0, Ordering::Relaxed);
    }

    /// Check if streaming
    pub async fn is_streaming(&self) -> bool {
        self.state().await == StreamerState::Streaming
    }

    /// Get stream statistics
    pub async fn stats(&self) -> StreamerStats {
        let config = self.config.read().await;
        let fps = self.current_fps.load(Ordering::Relaxed) as f32 / 100.0;

        StreamerStats {
            state: self.state().await,
            device: self.current_device().await.map(|d| d.name),
            format: Some(config.format.to_string()),
            resolution: Some((config.resolution.width, config.resolution.height)),
            clients: self.mjpeg_handler.client_count(),
            target_fps: config.fps,
            fps,
        }
    }

    /// Publish event to event bus (if configured)
    /// For StreamStateChanged events, only publishes if state actually changed (de-duplication)
    async fn publish_event(&self, event: SystemEvent) {
        if let Some(events) = self.events.read().await.as_ref() {
            // For state change events, check if state actually changed
            if let SystemEvent::StreamStateChanged { ref state, .. } = event {
                let current_state = match state.as_str() {
                    "uninitialized" => StreamerState::Uninitialized,
                    "ready" => StreamerState::Ready,
                    "streaming" => StreamerState::Streaming,
                    "no_signal" => StreamerState::NoSignal,
                    "error" => StreamerState::Error,
                    "device_lost" => StreamerState::DeviceLost,
                    "recovering" => StreamerState::Recovering,
                    _ => StreamerState::Error,
                };

                let mut last_state = self.last_published_state.write().await;
                if *last_state == Some(current_state) {
                    // State hasn't changed, skip publishing
                    trace!("Skipping duplicate stream state event: {}", state);
                    return;
                }
                *last_state = Some(current_state);
            }

            events.publish(event);
        }
    }

    /// Start device recovery task (internal implementation)
    ///
    /// This method starts a background task that attempts to reconnect
    /// to the video device after it was lost. It retries every 1 second
    /// until the device is recovered.
    async fn start_device_recovery_internal(self: &Arc<Self>) {
        // Check if recovery is already in progress
        if self
            .recovery_in_progress
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            debug!("Device recovery already in progress, skipping");
            return;
        }

        // Get last lost device info (from direct capture)
        let device = if let Some(device) = self.last_lost_device.read().await.clone() {
            device
        } else {
            self.current_device
                .read()
                .await
                .as_ref()
                .map(|d| d.path.display().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        let reason = self
            .last_lost_reason
            .read()
            .await
            .clone()
            .unwrap_or_else(|| "Device lost".to_string());

        // Store error info
        *self.last_lost_device.write().await = Some(device.clone());
        *self.last_lost_reason.write().await = Some(reason.clone());
        self.recovery_retry_count
            .store(0, std::sync::atomic::Ordering::Relaxed);

        // Publish device lost event
        self.publish_event(SystemEvent::StreamDeviceLost {
            device: device.clone(),
            reason: reason.clone(),
        })
        .await;

        // Start recovery task
        let streamer = Arc::clone(self);
        tokio::spawn(async move {
            let device_path = device.clone();

            loop {
                let attempt = streamer
                    .recovery_retry_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1;

                // Check if still in device lost state
                let current_state = *streamer.state.read().await;
                if current_state != StreamerState::DeviceLost
                    && current_state != StreamerState::Recovering
                {
                    info!("Stream state changed during recovery, stopping recovery task");
                    break;
                }

                // Update state to Recovering
                *streamer.state.write().await = StreamerState::Recovering;

                // Publish reconnecting event (every 5 attempts to avoid spam)
                if attempt == 1 || attempt % 5 == 0 {
                    streamer
                        .publish_event(SystemEvent::StreamReconnecting {
                            device: device_path.clone(),
                            attempt,
                        })
                        .await;
                    info!(
                        "Attempting to recover video device {} (attempt {})",
                        device_path, attempt
                    );
                }

                // Wait before retry (1 second)
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                // Check if device file exists
                let device_exists = std::path::Path::new(&device_path).exists();
                if !device_exists {
                    debug!("Device {} not present yet", device_path);
                    continue;
                }

                // Try to restart capture
                match streamer.restart_capture().await {
                    Ok(_) => {
                        info!(
                            "Video device {} recovered after {} attempts",
                            device_path, attempt
                        );
                        streamer
                            .recovery_in_progress
                            .store(false, std::sync::atomic::Ordering::SeqCst);

                        // Publish recovered event
                        streamer
                            .publish_event(SystemEvent::StreamRecovered {
                                device: device_path.clone(),
                            })
                            .await;

                        // Clear error info
                        *streamer.last_lost_device.write().await = None;
                        *streamer.last_lost_reason.write().await = None;
                        return;
                    }
                    Err(e) => {
                        debug!("Failed to restart capture (attempt {}): {}", attempt, e);
                    }
                }
            }

            streamer
                .recovery_in_progress
                .store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
}

impl Default for Streamer {
    fn default() -> Self {
        Self {
            config: RwLock::new(StreamerConfig::default()),
            mjpeg_handler: Arc::new(MjpegStreamHandler::new()),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            config_changing: std::sync::atomic::AtomicBool::new(false),
            background_tasks_started: std::sync::atomic::AtomicBool::new(false),
            recovery_retry_count: std::sync::atomic::AtomicU32::new(0),
            recovery_in_progress: std::sync::atomic::AtomicBool::new(false),
            last_lost_device: RwLock::new(None),
            last_lost_reason: RwLock::new(None),
        }
    }
}

/// Streamer statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamerStats {
    pub state: StreamerState,
    pub device: Option<String>,
    pub format: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub clients: u64,
    /// Target FPS from configuration
    pub target_fps: u32,
    /// Current actual FPS
    pub fps: f32,
}

impl serde::Serialize for StreamerState {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            StreamerState::Uninitialized => "uninitialized",
            StreamerState::Ready => "ready",
            StreamerState::Streaming => "streaming",
            StreamerState::NoSignal => "no_signal",
            StreamerState::Error => "error",
            StreamerState::DeviceLost => "device_lost",
            StreamerState::Recovering => "recovering",
        };
        serializer.serialize_str(s)
    }
}
