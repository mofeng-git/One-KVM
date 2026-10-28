//! Video streamer that integrates capture and streaming
//!
//! This module provides a high-level interface for video capture and streaming,
//! managing the lifecycle of the capture thread and MJPEG/WebRTC distribution.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

use super::device::{
    bridge as csi_bridge, enumerate_devices, find_best_device, is_csi_hdmi_bridge,
    parse_bridge_kind, resolve_video_input_config, select_recovery_device, VideoControlMode,
    VideoDevice, VideoDeviceInfo, VideoDeviceRecoveryHint,
};
use super::format::{PixelFormat, Resolution};
use super::frame::{FrameBuffer, FrameBufferPool, VideoFrame};
use crate::error::{AppError, Result};
use crate::events::{EventBus, StreamKind, SystemEvent};
use crate::stream::MjpegStreamHandler;
use crate::utils::LogThrottler;
use crate::video::capture::runtime::open_capture_stream;
use crate::video::capture::status::{
    capture_error_log_key, classify_capture_io_error, signal_status_from_capture_kind,
    CaptureIoErrorKind,
};
use crate::video::capture::{
    BridgeContext, CaptureReadError, CaptureStream, DEFAULT_CAPTURE_BUFFER_COUNT,
};
use crate::video::recovery::{wait_for_source_change, CaptureRecoveryPolicy};

const MIN_CAPTURE_FRAME_SIZE: usize = 128;

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

/// Fine-grained capture state; [`external_state`] maps to UI wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerState {
    /// Not initialized
    Uninitialized,
    /// Ready but not streaming
    Ready,
    /// Actively streaming
    Streaming,
    /// No video signal (generic / source not detected)
    NoSignal,
    /// HDMI cable not connected (DV_RX_POWER_PRESENT = false or ENOLINK)
    NoCable,
    /// TMDS signal present but timings not locked (ENOLCK)
    NoSync,
    /// Source timings are outside of what the capture hardware supports (ERANGE)
    OutOfRange,
    /// UVC/USB isochronous protocol error (kernel EPROTO/-71)
    UvcUsbError,
    /// UVC capture stalled (repeated DQBUF timeouts)
    UvcCaptureStall,
    /// Error occurred
    Error,
    /// Device was lost (unplugged)
    DeviceLost,
    /// Device is being recovered (reconnecting)
    Recovering,
    Busy,
}

impl StreamerState {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamerState::Uninitialized => "uninitialized",
            StreamerState::Ready => "ready",
            StreamerState::Streaming => "streaming",
            StreamerState::NoSignal => "no_signal",
            StreamerState::NoCable => "no_cable",
            StreamerState::NoSync => "no_sync",
            StreamerState::OutOfRange => "out_of_range",
            StreamerState::UvcUsbError => "uvc_usb_error",
            StreamerState::UvcCaptureStall => "uvc_capture_stall",
            StreamerState::Error => "error",
            StreamerState::DeviceLost => "device_lost",
            StreamerState::Recovering => "recovering",
            StreamerState::Busy => "device_busy",
        }
    }

    /// Parse a state string as produced by [`StreamerState::as_str`].
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "uninitialized" => StreamerState::Uninitialized,
            "ready" => StreamerState::Ready,
            "streaming" => StreamerState::Streaming,
            "no_signal" => StreamerState::NoSignal,
            "no_cable" => StreamerState::NoCable,
            "no_sync" => StreamerState::NoSync,
            "out_of_range" => StreamerState::OutOfRange,
            "uvc_usb_error" => StreamerState::UvcUsbError,
            "uvc_capture_stall" => StreamerState::UvcCaptureStall,
            "error" => StreamerState::Error,
            "device_lost" => StreamerState::DeviceLost,
            "recovering" => StreamerState::Recovering,
            "device_busy" | "busy" => StreamerState::Busy,
            _ => return None,
        })
    }

    pub fn is_no_signal_like(self) -> bool {
        matches!(
            self,
            StreamerState::NoSignal
                | StreamerState::NoCable
                | StreamerState::NoSync
                | StreamerState::OutOfRange
                | StreamerState::UvcUsbError
                | StreamerState::UvcCaptureStall
        )
    }

    pub fn external_state(self) -> (&'static str, Option<&'static str>) {
        match self {
            StreamerState::Streaming => ("streaming", None),
            StreamerState::Ready => ("ready", None),
            StreamerState::Uninitialized => ("uninitialized", None),
            StreamerState::Error => ("error", None),
            StreamerState::NoSignal => ("no_signal", Some("no_signal")),
            StreamerState::NoCable => ("no_signal", Some("no_cable")),
            StreamerState::NoSync => ("no_signal", Some("no_sync")),
            StreamerState::OutOfRange => ("no_signal", Some("out_of_range")),
            StreamerState::UvcUsbError => ("no_signal", Some("uvc_usb_error")),
            StreamerState::UvcCaptureStall => ("no_signal", Some("uvc_capture_stall")),
            StreamerState::DeviceLost => ("device_lost", Some("device_lost")),
            StreamerState::Recovering => ("device_lost", Some("recovering")),
            StreamerState::Busy => ("device_busy", None),
        }
    }
}

/// Video streamer service
pub struct Streamer {
    config: RwLock<StreamerConfig>,
    mjpeg_handler: Arc<MjpegStreamHandler>,
    pub(crate) snapshot_slot: super::snapshot::SnapshotSlot,
    current_device: RwLock<Option<VideoDeviceInfo>>,
    state: RwLock<StreamerState>,
    start_lock: tokio::sync::Mutex<()>,
    direct_stop: AtomicBool,
    direct_active: AtomicBool,
    direct_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    current_fps: AtomicU32,
    /// Event bus for broadcasting state changes (optional)
    events: RwLock<Option<Arc<EventBus>>>,
    last_published_state: RwLock<Option<(String, Option<String>, Option<u64>)>>,
    next_retry_ms: AtomicU64,
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
            snapshot_slot: Default::default(),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            next_retry_ms: AtomicU64::new(0),
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
            snapshot_slot: Default::default(),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            next_retry_ms: AtomicU64::new(0),
            config_changing: std::sync::atomic::AtomicBool::new(false),
            background_tasks_started: std::sync::atomic::AtomicBool::new(false),
            recovery_retry_count: std::sync::atomic::AtomicU32::new(0),
            recovery_in_progress: std::sync::atomic::AtomicBool::new(false),
            last_lost_device: RwLock::new(None),
            last_lost_reason: RwLock::new(None),
        })
    }

    pub async fn current_state_event(&self) -> SystemEvent {
        let state = *self.state.read().await;
        let device = self
            .current_device
            .read()
            .await
            .as_ref()
            .map(|d| d.path.display().to_string());
        let (external, reason) = state.external_state();
        let next = self.next_retry_ms.load(Ordering::Relaxed);

        SystemEvent::StreamStateChanged {
            kind: StreamKind::Video,
            state: external.to_string(),
            device,
            reason: reason.map(|s| s.to_string()),
            next_retry_ms: if next == 0 { None } else { Some(next) },
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

        // Surface a "device busy" state so the frontend can render a
        // "please wait" overlay for the (short) duration of the config
        // change.  The capture loop itself will flip to `Streaming` once
        // the first frame of the new geometry arrives.
        *self.state.write().await = StreamerState::Busy;
        self.publish_event(self.current_state_event().await).await;

        let devices = enumerate_devices()?;
        let device = if device_path.eq_ignore_ascii_case("auto") {
            devices
                .into_iter()
                .next()
                .ok_or_else(|| AppError::VideoError("No video devices found".to_string()))?
        } else {
            devices
                .into_iter()
                .find(|d| d.path.to_string_lossy() == device_path)
                .ok_or_else(|| AppError::VideoError("Video device not found".to_string()))?
        };

        let resolved = self.resolve_capture_config(&device, format, resolution, fps)?;
        let format = resolved.format;
        let resolution = resolved.resolution;
        let fps = resolved.fps;

        // IMPORTANT: Disconnect all MJPEG clients FIRST before stopping capture
        // This prevents race conditions where clients try to reconnect and reopen the device
        debug!("Disconnecting HTTP MJPEG clients before config change...");
        self.mjpeg_handler.disconnect_non_vnc_clients();

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
        debug!(
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
        debug!("Config applied, stream will start when client connects");

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

        let config = self.config.read().await;
        let resolved =
            self.resolve_capture_config(&device, config.format, config.resolution, config.fps)?;
        drop(config);

        // Update config with actual values
        {
            let mut config = self.config.write().await;
            config.device_path = Some(device.path.clone());
            config.format = resolved.format;
            config.resolution = resolved.resolution;
            config.fps = resolved.fps;
        }

        // Store device info
        *self.current_device.write().await = Some(device.clone());

        *self.state.write().await = StreamerState::Ready;

        info!(
            "Streamer initialized: {} @ {} {} fps",
            resolved.format, resolved.resolution, resolved.fps
        );
        Ok(())
    }

    /// Select best format for device
    fn select_format(
        &self,
        device: &VideoDeviceInfo,
        preferred: PixelFormat,
    ) -> Result<PixelFormat> {
        if is_csi_hdmi_bridge(device) {
            if !device.has_signal {
                info!(
                    "select_format: CSI bridge no signal, keeping preferred {:?}",
                    preferred
                );
                return Ok(preferred);
            }
            // Prefer the user-configured format if the device actually supports
            // it; otherwise fall back to the highest-priority format (formats
            // are pre-sorted by PixelFormat::priority(), e.g. NV12 > YUYV for rkcif/rk_hdmirx).
            if device.formats.iter().any(|f| f.format == preferred) {
                info!(
                    "select_format: CSI bridge with signal, using preferred {:?}",
                    preferred
                );
                return Ok(preferred);
            }
            let fmt =
                device.formats.first().map(|f| f.format).ok_or_else(|| {
                    AppError::VideoError("No supported formats found".to_string())
                })?;
            info!(
                "select_format: CSI bridge with signal, preferred {:?} unavailable, selected {:?} from {:?}",
                preferred,
                fmt,
                device.formats.iter().map(|f| f.format).collect::<Vec<_>>()
            );
            return Ok(fmt);
        }

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
        if is_csi_hdmi_bridge(device) && !device.has_signal {
            info!(
                "select_resolution: CSI bridge no signal, keeping preferred {}",
                preferred
            );
            return Ok(preferred);
        }

        let format_info = device
            .formats
            .iter()
            .find(|f| &f.format == format)
            .ok_or_else(|| AppError::VideoError("Format not found".to_string()))?;

        if is_csi_hdmi_bridge(device) {
            let res = format_info
                .resolutions
                .first()
                .map(|r| r.resolution())
                .unwrap_or(preferred);
            info!(
                "select_resolution: CSI bridge with signal, selected {} (preferred {}, available {:?})",
                res, preferred,
                format_info.resolutions.iter().map(|r| format!("{}x{}", r.width, r.height)).collect::<Vec<_>>()
            );
            return Ok(res);
        }

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

    fn resolve_capture_config(
        &self,
        device: &VideoDeviceInfo,
        requested_format: PixelFormat,
        requested_resolution: Resolution,
        requested_fps: u32,
    ) -> Result<super::device::ResolvedVideoInputConfig> {
        let mut resolved = resolve_video_input_config(
            device,
            requested_format,
            requested_resolution,
            requested_fps,
        );
        if device.control_mode == VideoControlMode::Configurable {
            resolved.format = self.select_format(device, resolved.format)?;
            resolved.resolution =
                self.select_resolution(device, &resolved.format, resolved.resolution)?;
        }
        Ok(resolved)
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
                StreamerState::Streaming => return Ok(()),
                s if s.is_no_signal_like() => return Ok(()),
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

        // A no-signal/source-change recovery keeps the existing capture thread
        // alive while it closes and re-opens the V4L2 stream.  HTTP clients may
        // reconnect while that thread is still probing.  Do not spawn a second
        // capture thread here: it would contend for the same video node and
        // overwrite `direct_handle`, making the original thread impossible to
        // join from `stop()`.
        if self.direct_active.load(Ordering::SeqCst) {
            debug!("Capture thread is already active; waiting for its recovery loop");
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
                    } else if zero_since.is_some() {
                        info!("Clients reconnected, canceling auto-pause");
                        zero_since = None;
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

    /// Direct capture loop for MJPEG mode.
    ///
    /// The outer `'session` loop allows "soft restarts": when no signal has been
    /// detected for `NOSIGNAL_SOFT_RESTART_SECS` the capture stream is closed and
    /// re-opened (re-probing format/resolution) without going through the full
    /// DeviceLost recovery path.  This handles the common CSI/HDMI-bridge case where
    /// the source switches resolution and the driver requires a new `s_fmt` call.
    fn run_direct_capture(self: Arc<Self>, device_path: PathBuf, _initial_config: StreamerConfig) {
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 200;
        const IDLE_STOP_DELAY_SECS: u64 = 5;
        const BUFFER_COUNT: u32 = DEFAULT_CAPTURE_BUFFER_COUNT;
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

        // Helper: drop the MJPEG online flag so any connected HTTP clients
        // exit their streaming tasks cleanly.  Replaces the old "push a
        // placeholder JPEG every second" scheme — the frontend now renders
        // its own overlay from `stream.state_changed` and doesn't need a
        // fake image to keep the connection alive.  Idempotent.
        let go_offline = || {
            self.mjpeg_handler.set_offline();
        };

        // Helper: record the back-off window on the streamer so it rides
        // along on the next `stream.state_changed` event; cleared when we
        // return to `Streaming`.
        let set_retry = |ms: u64| {
            self.next_retry_ms.store(ms, Ordering::Relaxed);
        };

        // Consecutive recovery attempts, shared with the common retry policy.
        let mut no_signal_restart_count: u32 = 0;
        let mut no_consumers_since: Option<std::time::Instant> = None;

        'session: loop {
            if self.direct_stop.load(Ordering::Relaxed) {
                break 'session;
            }
            if self.mjpeg_handler.client_count() == 0 {
                let since = no_consumers_since.get_or_insert_with(std::time::Instant::now);
                if since.elapsed() >= Duration::from_secs(IDLE_STOP_DELAY_SECS) {
                    info!("No MJPEG consumers during recovery; stopping capture");
                    break 'session;
                }
            } else {
                no_consumers_since = None;
            }

            // Re-read config at the start of each session so that a re_init_device()
            // call (from a previous soft-restart or recovery) is reflected here.
            let config = handle.block_on(async { self.config.read().await.clone() });

            // ── Resolve the CSI bridge subdev (if any) for this video ──────────
            //
            // The subdev is where QUERY_DV_TIMINGS and SOURCE_CHANGE events
            // actually live on RK628-on-rkcif.  It's stored in
            // `VideoDeviceInfo` during enumeration; we re-read it here
            // rather than caching on Streamer so a hot-plug recovery picks
            // up a possibly-different subdev path.
            let (bridge_ctx, control_mode) = handle.block_on(async {
                self.current_device
                    .read()
                    .await
                    .as_ref()
                    .map(|info| {
                        (
                            BridgeContext::from_parts(
                                info.subdev_path.clone(),
                                parse_bridge_kind(info.bridge_kind.as_deref()),
                            ),
                            info.control_mode,
                        )
                    })
                    .unwrap_or((BridgeContext::default(), VideoControlMode::Configurable))
            });
            let recovery_policy = CaptureRecoveryPolicy::new(control_mode);

            // ── STREAMON gate: for CSI bridges with a subdev, refuse to
            //    open the video node when the subdev reports no signal.
            //    On RK628 this prevents a kernel null-pointer deref.
            if let Some(subdev_path) = bridge_ctx.subdev_path.as_ref() {
                if let Some(status) = probe_subdev_signal(subdev_path, bridge_ctx.kind) {
                    let delay =
                        recovery_policy.retry_delay(no_signal_restart_count.saturating_add(1));
                    debug!(
                        "Pre-STREAMON gate: subdev {:?} reports {:?} — \
                             waiting for SOURCE_CHANGE (<= {:?}) before opening {:?}",
                        subdev_path, status, delay, device_path
                    );
                    set_retry(delay.as_millis() as u64);
                    go_offline();
                    set_state(status.into());
                    wait_for_source_change(&bridge_ctx, delay, || {
                        !self.direct_stop.load(Ordering::Relaxed)
                    });
                    no_signal_restart_count = no_signal_restart_count.saturating_add(1);
                    continue 'session;
                }
            }

            // ── Open the capture stream ─────────────────────────────────────────
            let mut stream_opt: Option<CaptureStream> = None;
            let mut last_error: Option<String> = None;

            for attempt in 0..MAX_RETRIES {
                if self.direct_stop.load(Ordering::Relaxed) {
                    self.direct_active.store(false, Ordering::SeqCst);
                    return;
                }

                match open_capture_stream(
                    &device_path,
                    config.resolution,
                    config.format,
                    config.fps,
                    BUFFER_COUNT,
                    Duration::from_secs(2),
                    bridge_ctx.clone(),
                    control_mode,
                ) {
                    Ok(stream) => {
                        stream_opt = Some(stream);
                        break;
                    }
                    Err(AppError::CaptureNoSignal { kind }) => {
                        // CSI bridge open-time DV-timings probe failed.
                        // Drop the HTTP stream so the frontend renders its
                        // "no signal" overlay, update the state with the
                        // fine-grained reason, and let the outer 'session
                        // loop back off before the next retry.
                        let status = signal_status_from_capture_kind(&kind);
                        debug!(
                            "CSI open probe reports no signal ({:?}), will soft-restart",
                            status
                        );
                        let delay =
                            recovery_policy.retry_delay(no_signal_restart_count.saturating_add(1));
                        set_retry(delay.as_millis() as u64);
                        go_offline();
                        set_state(status.into());
                        last_error = Some(format!("CaptureNoSignal({})", kind));
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
                    // If the open failed because of a no-signal condition, do
                    // *not* escalate to Error — instead keep the capture loop
                    // alive in NoSignal-like state and retry via the soft
                    // restart path.  This lets CSI bridges recover on their
                    // own when the source comes back (resolution change,
                    // host reboot, HDMI cable re-plug).
                    let was_no_signal = handle
                        .block_on(async { self.state().await })
                        .is_no_signal_like();
                    if !was_no_signal {
                        error!(
                            "Failed to open device {:?}: {}",
                            device_path,
                            last_error.unwrap_or_else(|| "unknown error".to_string())
                        );
                        self.mjpeg_handler.set_offline();
                        set_state(StreamerState::Error);
                        break 'session;
                    }

                    debug!("Open failed in NoSignal-like state, backing off before soft-restart");
                    if !recovery_policy.should_retry(no_signal_restart_count.saturating_add(1)) {
                        set_state(StreamerState::Error);
                        break 'session;
                    }
                    let delay =
                        recovery_policy.retry_delay(no_signal_restart_count.saturating_add(1));
                    set_retry(delay.as_millis() as u64);
                    wait_for_source_change(&bridge_ctx, delay, || {
                        !self.direct_stop.load(Ordering::Relaxed)
                    });
                    no_signal_restart_count = no_signal_restart_count.saturating_add(1);
                    continue 'session;
                }
            };

            let resolution = stream.resolution();
            let pixel_format = stream.format();
            let source_fps = stream
                .source_fps()
                .map(|fps| fps.round().clamp(1.0, 120.0) as u32)
                .unwrap_or(config.fps);
            let stride = stream.stride();

            if control_mode == VideoControlMode::SourceFollowing {
                handle.block_on(async {
                    let mut current = self.config.write().await;
                    current.resolution = resolution;
                    current.format = pixel_format;
                    current.fps = source_fps;
                });
            }

            info!(
                "Capture format: {}x{} {:?} stride={}",
                resolution.width, resolution.height, pixel_format, stride
            );

            let buffer_pool = Arc::new(FrameBufferPool::new(BUFFER_COUNT.max(4) as usize));
            // Preserve the no-signal state across an outer-loop re-open.  This
            // makes the first recovered frame transition the handler back
            // online and publish Streaming instead of silently inheriting the
            // previous offline state.
            let mut signal_present = !handle
                .block_on(async { self.state().await })
                .is_no_signal_like();
            let mut idle_since: Option<std::time::Instant> = None;

            let mut fps_frame_count: u64 = 0;
            let mut last_fps_time = std::time::Instant::now();
            let capture_error_throttler = LogThrottler::with_secs(5);
            let mut suppressed_capture_errors: HashMap<String, u64> = HashMap::new();

            // None = signal is present; Some(Instant) = when signal was first lost.
            let mut no_signal_since: Option<std::time::Instant> = None;
            // Whether the inner 'capture loop should trigger a soft restart.
            let mut need_soft_restart = false;

            // ── Inner capture loop ──────────────────────────────────────────────
            'capture: while !self.direct_stop.load(Ordering::Relaxed) {
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
                            break 'capture;
                        }
                    }
                } else if idle_since.is_some() {
                    trace!("Video consumers active, resetting idle timer");
                    idle_since = None;
                }

                let mut owned = buffer_pool.take(MIN_CAPTURE_FRAME_SIZE);
                let meta = match stream.next_into(&mut owned) {
                    Ok(meta) => meta,
                    Err(CaptureReadError::SourceChanged) => {
                        info!("Capture SOURCE_CHANGE — soft-restart for DV re-probe");
                        let delay =
                            recovery_policy.retry_delay(no_signal_restart_count.saturating_add(1));
                        set_retry(delay.as_millis() as u64);
                        go_offline();
                        set_state(StreamerState::NoSignal);
                        need_soft_restart = true;
                        break 'capture;
                    }
                    Err(CaptureReadError::Io(e)) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            continue 'capture;
                        }
                        if e.kind() == std::io::ErrorKind::TimedOut {
                            if signal_present {
                                signal_present = false;
                                let delay = recovery_policy
                                    .retry_delay(no_signal_restart_count.saturating_add(1));
                                set_retry(delay.as_millis() as u64);
                                go_offline();
                                set_state(StreamerState::NoSignal);
                                no_signal_since = Some(std::time::Instant::now());
                                self.current_fps.store(0, Ordering::Relaxed);
                                fps_frame_count = 0;
                                last_fps_time = std::time::Instant::now();
                            } else if let Some(since) = no_signal_since {
                                let delay = recovery_policy
                                    .retry_delay(no_signal_restart_count.saturating_add(1));
                                if since.elapsed() >= delay {
                                    info!(
                                        "NoSignal for {:?}, attempting soft restart (attempt {})",
                                        delay,
                                        no_signal_restart_count + 1
                                    );
                                    need_soft_restart = true;
                                    break 'capture;
                                }
                            }

                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue 'capture;
                        }

                        // Classify the capture error.
                        //
                        // Only errnos that mean "the device file is gone"
                        // (ENODEV, ENXIO, ESHUTDOWN) trigger the full
                        // DeviceLost → recovery path.
                        //
                        // EIO / EPIPE are common transient errors on rkcif
                        // when the source glitches or re-locks; those are
                        // treated as NoSignal + soft-restart so we recover
                        // in ~1 s instead of the 1 s recovery-poll loop.
                        match classify_capture_io_error(&e) {
                            CaptureIoErrorKind::DeviceLost => {
                                error!("Video device lost: {} - {}", device_path.display(), e);
                                go_offline();
                                set_retry(0);
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
                                break 'capture;
                            }
                            CaptureIoErrorKind::TransientSignal { status } => {
                                if status == Some(crate::video::signal::SignalStatus::UvcUsbError) {
                                    warn!(
                                        "Capture transient error (EPROTO/-71, often UVC USB): {}",
                                        e
                                    );
                                    if control_mode == VideoControlMode::Configurable {
                                        go_offline();
                                        set_state(StreamerState::UvcUsbError);
                                        need_soft_restart = true;
                                        break 'capture;
                                    }
                                } else {
                                    warn!(
                                    "Capture transient error ({}): treating as NoSignal + soft-restart",
                                    e
                                );
                                }
                                let delay = recovery_policy
                                    .retry_delay(no_signal_restart_count.saturating_add(1));
                                set_retry(delay.as_millis() as u64);
                                go_offline();
                                set_state(StreamerState::NoSignal);
                                need_soft_restart = true;
                                break 'capture;
                            }
                            CaptureIoErrorKind::Other => {}
                        }

                        let key = capture_error_log_key(&e);
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
                        continue 'capture;
                    }
                };

                let frame_size = meta.bytes_used;
                if frame_size < MIN_CAPTURE_FRAME_SIZE {
                    continue 'capture;
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
                    no_signal_since = None;
                    no_signal_restart_count = 0;
                    set_retry(0);
                    // Signal-loss handling marks the MJPEG handler offline so
                    // stale HTTP responses close cleanly.  Re-enable it on the
                    // first recovered frame so a reconnect can remain attached
                    // to this (still single) capture thread.
                    self.mjpeg_handler.set_online();
                    set_state(StreamerState::Streaming);

                    let fps_val = source_fps;
                    let recovered_device = device_path.display().to_string();
                    handle.block_on(async {
                        self.publish_event(SystemEvent::StreamRecovered {
                            device: recovered_device,
                        })
                        .await;
                    });
                    let dp = device_path.display().to_string();
                    let fmt = pixel_format.to_string();
                    let w = resolution.width;
                    let h = resolution.height;
                    handle.block_on(async {
                        self.publish_event(SystemEvent::StreamConfigApplied {
                            transition_id: None,
                            device: dp,
                            resolution: (w, h),
                            format: fmt,
                            fps: fps_val,
                        })
                        .await;
                    });
                }

                self.snapshot_slot.publish(&frame);
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
            } // 'capture

            // ── After inner loop ────────────────────────────────────────────────
            // The stream is dropped here, releasing the device FD.
            drop(stream);

            if self.direct_stop.load(Ordering::Relaxed) {
                break 'session;
            }

            if !need_soft_restart {
                // Normal exit (idle / device-lost / stop).
                break 'session;
            }

            no_signal_restart_count = no_signal_restart_count.saturating_add(1);
            // Continue 'session: the single open path performs QUERY_DV_TIMINGS,
            // applies the source mode, and owns the retry delay.
        } // 'session

        self.direct_active.store(false, Ordering::SeqCst);
        self.current_fps.store(0, Ordering::Relaxed);
    }

    /// `Streaming` or any no-signal-like state (capture thread still alive).
    pub async fn is_streaming(&self) -> bool {
        let s = self.state().await;
        s == StreamerState::Streaming || s.is_no_signal_like()
    }

    pub async fn re_init_device(self: &Arc<Self>, device_path: &str) -> Result<()> {
        let device = VideoDevice::open_readonly(device_path)
            .map_err(|e| AppError::VideoError(format!("Cannot open device for re-init: {}", e)))?;
        let device_info = device.info()?;

        let resolved = {
            let config = self.config.read().await;
            self.resolve_capture_config(&device_info, config.format, config.resolution, config.fps)
                .unwrap_or(super::device::ResolvedVideoInputConfig {
                    format: config.format,
                    resolution: config.resolution,
                    fps: config.fps,
                })
        };

        {
            let mut cfg = self.config.write().await;
            cfg.device_path = Some(device_info.path.clone());
            cfg.format = resolved.format;
            cfg.resolution = resolved.resolution;
            cfg.fps = resolved.fps;
        }
        *self.current_device.write().await = Some(device_info);

        info!(
            "Device re-initialized: {}x{} {:?}",
            resolved.resolution.width, resolved.resolution.height, resolved.format
        );
        Ok(())
    }

    /// Get stream statistics
    pub async fn stats(&self) -> StreamerStats {
        let config = self.config.read().await;
        let fps = self.current_fps.load(Ordering::Relaxed) as f32 / 100.0;
        let (state, reason) = self.state().await.external_state();

        StreamerStats {
            state: state.to_string(),
            reason: reason.map(str::to_string),
            device: self.current_device().await.map(|d| d.name),
            format: Some(config.format.to_string()),
            resolution: Some((config.resolution.width, config.resolution.height)),
            clients: self.mjpeg_handler.client_count(),
            target_fps: config.fps,
            fps,
        }
    }

    /// Dedupes `StreamStateChanged` on `(state, reason, next_retry_ms)`.
    async fn publish_event(&self, event: SystemEvent) {
        if let Some(events) = self.events.read().await.as_ref() {
            if let SystemEvent::StreamStateChanged {
                ref state,
                ref reason,
                next_retry_ms,
                ..
            } = event
            {
                let key = (state.clone(), reason.clone(), next_retry_ms);
                let mut last_state = self.last_published_state.write().await;
                if last_state.as_ref() == Some(&key) {
                    trace!(
                        "Skipping duplicate stream state event: {} (reason={:?})",
                        state,
                        reason
                    );
                    return;
                }
                *last_state = Some(key);
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
        let recovery_hint = self
            .current_device
            .read()
            .await
            .as_ref()
            .map(VideoDeviceRecoveryHint::from)
            .unwrap_or_else(|| VideoDeviceRecoveryHint {
                path: PathBuf::from(&device),
                name: String::new(),
                driver: String::new(),
                bus_info: String::new(),
                card: String::new(),
                is_capture_card: true,
            });

        // Store error info
        *self.last_lost_device.write().await = Some(device.clone());
        *self.last_lost_reason.write().await = Some(reason.clone());
        self.recovery_retry_count
            .store(0, std::sync::atomic::Ordering::Relaxed);

        // Publish device lost event
        self.publish_event(SystemEvent::StreamDeviceLost {
            kind: StreamKind::Video,
            device: device.clone(),
            reason: reason.clone(),
        })
        .await;

        // Start recovery task
        let streamer = Arc::clone(self);
        tokio::spawn(async move {
            let original_device_path = device.clone();

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
                if attempt == 1 || attempt.is_multiple_of(5) {
                    streamer
                        .publish_event(SystemEvent::StreamReconnecting {
                            device: original_device_path.clone(),
                            attempt,
                        })
                        .await;
                    info!(
                        "Attempting to recover video device {} (attempt {})",
                        original_device_path, attempt
                    );
                }

                let wait = if attempt == 1 {
                    std::time::Duration::from_millis(200)
                } else {
                    std::time::Duration::from_secs(1)
                };
                tokio::time::sleep(wait).await;

                let devices = match enumerate_devices() {
                    Ok(devices) => devices,
                    Err(e) => {
                        debug!("Failed to enumerate devices during recovery: {}", e);
                        continue;
                    }
                };

                let Some(device) = select_recovery_device(&devices, &recovery_hint) else {
                    debug!("No matching video device present yet for recovery");
                    continue;
                };

                let device_path = device.path.display().to_string();
                if device_path != original_device_path {
                    info!(
                        "Recovered video device path changed: {} -> {}",
                        original_device_path, device_path
                    );
                }

                if !std::path::Path::new(&device_path).exists() {
                    debug!("Device {} not present yet", device_path);
                    continue;
                }

                // Re-probe device to pick up resolution/format changes
                if let Err(e) = streamer.re_init_device(&device_path).await {
                    debug!(
                        "Failed to re-probe device format (attempt {}): {}",
                        attempt, e
                    );
                    // Don't skip – device exists, try restart anyway
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
            snapshot_slot: Default::default(),
            current_device: RwLock::new(None),
            state: RwLock::new(StreamerState::Uninitialized),
            start_lock: tokio::sync::Mutex::new(()),
            direct_stop: AtomicBool::new(false),
            direct_active: AtomicBool::new(false),
            direct_handle: tokio::sync::Mutex::new(None),
            current_fps: AtomicU32::new(0),
            events: RwLock::new(None),
            last_published_state: RwLock::new(None),
            next_retry_ms: AtomicU64::new(0),
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
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub device: Option<String>,
    pub format: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub clients: u64,
    /// Target FPS from configuration
    pub target_fps: u32,
    /// Current actual FPS
    pub fps: f32,
}

fn probe_subdev_signal(
    subdev_path: &std::path::Path,
    kind: Option<csi_bridge::CsiBridgeKind>,
) -> Option<crate::video::signal::SignalStatus> {
    let fd = match csi_bridge::open_subdev(subdev_path) {
        Ok(f) => f,
        Err(e) => {
            debug!(
                "probe_subdev_signal: failed to open {:?}: {}",
                subdev_path, e
            );
            return Some(crate::video::signal::SignalStatus::NoSignal);
        }
    };
    let kind = kind.unwrap_or(csi_bridge::CsiBridgeKind::Unknown);
    let probe = csi_bridge::probe_signal(&fd, kind);
    probe.as_status()
}

impl serde::Serialize for StreamerState {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
