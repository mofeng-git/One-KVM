//! Universal shared video encoding pipeline
//!
//! Supports multiple codecs: H264, H265, VP8, VP9
//! A single encoder broadcasts to multiple WebRTC sessions.
//!
//! Architecture:
//! ```text
//! V4L2 capture
//!        |
//!        v
//! SharedVideoPipeline (capture + encode + broadcast)
//!        |
//!        v
//!   ┌────┴────┬────────┬────────┐
//!   v         v        v        v
//! Session1  Session2  Session3  ...
//! ```

mod encoder_state;

use bytes::Bytes;
use parking_lot::Mutex as ParkingMutex;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

use self::encoder_state::{build_encoder_state, EncoderThreadState};

/// Grace period before auto-stopping pipeline when no subscribers (in seconds)
const AUTO_STOP_GRACE_PERIOD_SECS: u64 = 3;
/// After this many consecutive timeouts, log a prominent warning.
const CAPTURE_TIMEOUT_RESTART_THRESHOLD: u32 = 5;
const CAPTURE_TIMEOUT_STOP_THRESHOLD: u32 = 60;
const CAPTURE_TIMEOUT_SOFT_RESTART_THRESHOLD: u32 = 3;
const CSI_BRIDGE_NOSIGNAL_INTERVAL_MS: u64 = 500;
const NOSIGNAL_POLL_MAX: Duration = Duration::from_secs(20);
/// Throttle repeated encoding errors to avoid log flooding
const ENCODE_ERROR_THROTTLE_SECS: u64 = 5;

use crate::error::{AppError, Result};
use crate::utils::LogThrottler;
use crate::video::capture_limits::{should_validate_jpeg_frame, MIN_CAPTURE_FRAME_SIZE};
use crate::video::csi_bridge::{self, ProbeResult};
use crate::video::device::parse_bridge_kind;
use crate::video::encoder::registry::{EncoderBackend, VideoEncoderType};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::frame::{FrameBuffer, FrameBufferPool, VideoFrame};
use crate::video::v4l2r_capture::{is_source_changed_error, BridgeContext, V4l2rCaptureStream};
use crate::video::SignalStatus;
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use hwcodec::ffmpeg_hw::last_error_message as ffmpeg_hw_last_error;

/// Encoded video frame for distribution
#[derive(Debug, Clone)]
pub struct EncodedVideoFrame {
    /// Encoded data (Annex B for H264/H265, raw for VP8/VP9)
    pub data: Bytes,
    /// Presentation timestamp in milliseconds
    pub pts_ms: i64,
    /// Whether this is a keyframe
    pub is_keyframe: bool,
    /// Frame sequence number
    pub sequence: u64,
    /// Frame duration
    pub duration: Duration,
    /// Codec type
    pub codec: VideoEncoderType,
}

enum PipelineCmd {
    SetBitrate { bitrate_kbps: u32, gop: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineStateNotification {
    pub state: &'static str,
    pub reason: Option<&'static str>,
    pub next_retry_ms: Option<u64>,
}

impl PipelineStateNotification {
    fn streaming() -> Self {
        Self {
            state: "streaming",
            reason: None,
            next_retry_ms: None,
        }
    }

    fn no_signal(status: SignalStatus, next_retry_ms: Option<u64>) -> Self {
        Self {
            state: "no_signal",
            reason: Some(status.as_str()),
            next_retry_ms,
        }
    }

    fn device_busy(reason: &'static str) -> Self {
        Self {
            state: "device_busy",
            reason: Some(reason),
            next_retry_ms: None,
        }
    }
}

/// Shared video pipeline configuration
#[derive(Debug, Clone)]
pub struct SharedVideoPipelineConfig {
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Output codec type
    pub output_codec: VideoEncoderType,
    /// Bitrate preset (replaces raw bitrate_kbps)
    pub bitrate_preset: crate::video::encoder::BitratePreset,
    /// Target FPS
    pub fps: u32,
    /// Encoder backend (None = auto select best available)
    pub encoder_backend: Option<EncoderBackend>,
}

impl Default for SharedVideoPipelineConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution::HD720,
            input_format: PixelFormat::Yuyv,
            output_codec: VideoEncoderType::H264,
            bitrate_preset: crate::video::encoder::BitratePreset::Balanced,
            fps: 30,
            encoder_backend: None,
        }
    }
}

impl SharedVideoPipelineConfig {
    /// Get effective bitrate in kbps
    pub fn bitrate_kbps(&self) -> u32 {
        self.bitrate_preset.bitrate_kbps()
    }

    /// Get effective GOP size
    pub fn gop_size(&self) -> u32 {
        self.bitrate_preset.gop_size(self.fps)
    }

    /// Create H264 config with bitrate preset
    pub fn h264(resolution: Resolution, preset: crate::video::encoder::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H264,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create H265 config with bitrate preset
    pub fn h265(resolution: Resolution, preset: crate::video::encoder::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H265,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create VP8 config with bitrate preset
    pub fn vp8(resolution: Resolution, preset: crate::video::encoder::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP8,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create VP9 config with bitrate preset
    pub fn vp9(resolution: Resolution, preset: crate::video::encoder::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP9,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create config with legacy bitrate_kbps (for compatibility during migration)
    pub fn with_bitrate_kbps(mut self, bitrate_kbps: u32) -> Self {
        self.bitrate_preset = crate::video::encoder::BitratePreset::from_kbps(bitrate_kbps);
        self
    }
}

fn classify_encode_error(err: &AppError) -> String {
    let message = err.to_string();

    if message.contains("FFmpeg HW encode failed") {
        if message.contains("avcodec_send_packet failed") && message.contains("ret=-11") {
            "encode_ffmpeg_hw_send_packet_eagain".to_string()
        } else if message.contains("avcodec_send_frame failed") && message.contains("ret=-11") {
            "encode_ffmpeg_hw_send_frame_eagain".to_string()
        } else if message.contains("avcodec_receive_packet failed") && message.contains("ret=-11") {
            "encode_ffmpeg_hw_receive_packet_eagain".to_string()
        } else if message.contains("Resource temporarily unavailable") {
            "encode_ffmpeg_hw_eagain".to_string()
        } else if message.contains("avcodec_send_packet failed") {
            "encode_ffmpeg_hw_send_packet".to_string()
        } else if message.contains("avcodec_send_frame failed") {
            "encode_ffmpeg_hw_send_frame".to_string()
        } else if message.contains("avcodec_receive_packet failed") {
            "encode_ffmpeg_hw_receive_packet".to_string()
        } else {
            "encode_ffmpeg_hw".to_string()
        }
    } else {
        format!("encode_{}", message)
    }
}

fn log_encoding_error(
    throttler: &LogThrottler,
    suppressed_errors: &mut HashMap<String, u64>,
    err: &AppError,
) {
    let key = classify_encode_error(err);
    if throttler.should_log(&key) {
        let suppressed = suppressed_errors.remove(&key).unwrap_or(0);
        if suppressed > 0 {
            error!(
                "Encoding failed: {} (suppressed {} repeats)",
                err, suppressed
            );
        } else {
            error!("Encoding failed: {}", err);
        }
    } else {
        let counter = suppressed_errors.entry(key).or_insert(0);
        *counter = counter.saturating_add(1);
    }
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedVideoPipelineStats {
    pub current_fps: f32,
}

/// Universal shared video pipeline
pub struct SharedVideoPipeline {
    config: RwLock<SharedVideoPipelineConfig>,
    subscribers: ParkingRwLock<Vec<mpsc::Sender<Arc<EncodedVideoFrame>>>>,
    stats: Mutex<SharedVideoPipelineStats>,
    running: watch::Sender<bool>,
    running_rx: watch::Receiver<bool>,
    cmd_tx: ParkingRwLock<Option<tokio::sync::mpsc::UnboundedSender<PipelineCmd>>>,
    /// Fast running flag for blocking capture loop
    running_flag: AtomicBool,
    /// Frame sequence counter (atomic for lock-free access)
    sequence: AtomicU64,
    /// Atomic flag for keyframe request (avoids lock contention)
    keyframe_requested: AtomicBool,
    /// Pipeline start time for PTS calculation (epoch millis, 0 = not set)
    /// Uses AtomicI64 instead of Mutex for lock-free access
    pipeline_start_time_ms: AtomicI64,
    pending_sync_geometry: ParkingMutex<Option<(Resolution, PixelFormat)>>,
    state_notifier: ParkingRwLock<Option<Arc<dyn Fn(PipelineStateNotification) + Send + Sync>>>,
    last_state_notification: ParkingMutex<Option<PipelineStateNotification>>,
}

fn poll_bridge_subdev_after_no_signal(bridge_ctx: &BridgeContext, pipeline: &SharedVideoPipeline) {
    let Some(subdev_path) = bridge_ctx.subdev_path.as_ref() else {
        return;
    };
    let kind = bridge_ctx
        .kind
        .unwrap_or(csi_bridge::CsiBridgeKind::Unknown);
    let deadline = Instant::now() + NOSIGNAL_POLL_MAX;
    let mut poll_count: u32 = 0;
    info!(
        "No-signal poll: scanning subdev {:?} every {} ms (max {:?})",
        subdev_path, CSI_BRIDGE_NOSIGNAL_INTERVAL_MS, NOSIGNAL_POLL_MAX
    );
    loop {
        if !pipeline.running_flag.load(Ordering::Acquire) {
            return;
        }
        if Instant::now() >= deadline {
            info!(
                "No-signal poll: stopped after {:?} ({} attempts)",
                NOSIGNAL_POLL_MAX, poll_count
            );
            return;
        }
        let fd = match csi_bridge::open_subdev(subdev_path) {
            Ok(f) => f,
            Err(e) => {
                debug!(
                    "No-signal poll: open subdev {:?} failed: {}",
                    subdev_path, e
                );
                std::thread::sleep(Duration::from_millis(CSI_BRIDGE_NOSIGNAL_INTERVAL_MS));
                continue;
            }
        };
        match csi_bridge::probe_signal_thread_timeout(
            &fd,
            kind,
            csi_bridge::RK628_SUBDEV_PROBE_TIMEOUT,
        ) {
            Some(ProbeResult::Locked(mode)) => {
                info!(
                    "No-signal poll: locked {}x{} @ {} Hz — proceeding to capture re-open",
                    mode.width, mode.height, mode.pixelclock
                );
                return;
            }
            Some(other) => {
                poll_count = poll_count.saturating_add(1);
                if poll_count == 1 || poll_count.is_multiple_of(8) {
                    debug!(
                        "No-signal poll: attempt {} — still {:?}",
                        poll_count,
                        other.as_status()
                    );
                }
                if let Some(st) = other.as_status() {
                    pipeline.notify_state(PipelineStateNotification::no_signal(
                        st,
                        Some(CSI_BRIDGE_NOSIGNAL_INTERVAL_MS.saturating_add(50)),
                    ));
                }
            }
            None => {
                poll_count = poll_count.saturating_add(1);
                debug!(
                    "No-signal poll: attempt {} — probe ioctl timed out",
                    poll_count
                );
            }
        }
        std::thread::sleep(Duration::from_millis(CSI_BRIDGE_NOSIGNAL_INTERVAL_MS));
    }
}

impl SharedVideoPipeline {
    /// Create a new shared video pipeline
    pub fn new(config: SharedVideoPipelineConfig) -> Result<Arc<Self>> {
        info!(
            "Creating shared video pipeline: {} {}x{} @ {} (input: {})",
            config.output_codec,
            config.resolution.width,
            config.resolution.height,
            config.bitrate_preset,
            config.input_format
        );

        let (running_tx, running_rx) = watch::channel(false);

        let pipeline = Arc::new(Self {
            config: RwLock::new(config),
            subscribers: ParkingRwLock::new(Vec::new()),
            stats: Mutex::new(SharedVideoPipelineStats::default()),
            running: running_tx,
            running_rx,
            cmd_tx: ParkingRwLock::new(None),
            running_flag: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            keyframe_requested: AtomicBool::new(false),
            pipeline_start_time_ms: AtomicI64::new(0),
            pending_sync_geometry: ParkingMutex::new(None),
            state_notifier: ParkingRwLock::new(None),
            last_state_notification: ParkingMutex::new(None),
        });

        Ok(pipeline)
    }

    pub fn take_pending_sync_geometry(&self) -> Option<(Resolution, PixelFormat)> {
        self.pending_sync_geometry.lock().take()
    }

    pub fn set_state_notifier(
        &self,
        notifier: Option<Arc<dyn Fn(PipelineStateNotification) + Send + Sync>>,
    ) {
        *self.state_notifier.write() = notifier;
    }

    fn notify_state(&self, notification: PipelineStateNotification) {
        let should_emit = {
            let mut last = self.last_state_notification.lock();
            if last.as_ref() == Some(&notification) {
                false
            } else {
                *last = Some(notification);
                true
            }
        };
        if !should_emit {
            return;
        }
        tracing::debug!(
            "Pipeline state notification: state={}, reason={:?}",
            notification.state,
            notification.reason
        );
        if let Some(notifier) = self.state_notifier.read().clone() {
            notifier(notification);
        }
    }

    /// Subscribe to encoded frames
    pub fn subscribe(&self) -> mpsc::Receiver<Arc<EncodedVideoFrame>> {
        let (tx, rx) = mpsc::channel(4);
        self.subscribers.write().push(tx);
        rx
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.subscribers
            .read()
            .iter()
            .filter(|tx| !tx.is_closed())
            .count()
    }

    /// Request encoder to produce a keyframe on next encode
    ///
    /// This is useful when a new client connects and needs an immediate
    /// keyframe to start decoding the video stream.
    ///
    /// Uses an atomic flag to avoid lock contention with the encoding loop.
    pub async fn request_keyframe(&self) {
        self.keyframe_requested.store(true, Ordering::Release);
        info!("[Pipeline] Keyframe requested for new client");
    }

    fn send_cmd(&self, cmd: PipelineCmd) {
        let tx = self.cmd_tx.read().clone();
        if let Some(tx) = tx {
            let _ = tx.send(cmd);
        }
    }

    fn clear_cmd_tx(&self) {
        let mut guard = self.cmd_tx.write();
        *guard = None;
    }

    fn apply_cmd(&self, state: &mut EncoderThreadState, cmd: PipelineCmd) -> Result<()> {
        match cmd {
            PipelineCmd::SetBitrate { bitrate_kbps, gop } => {
                #[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
                let _ = gop;
                #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
                if state.ffmpeg_hw_enabled {
                    if let Some(ref mut pipeline) = state.ffmpeg_hw_pipeline {
                        pipeline
                            .reconfigure(bitrate_kbps as i32, gop as i32)
                            .map_err(|e| {
                                let detail = if e.is_empty() {
                                    ffmpeg_hw_last_error()
                                } else {
                                    e
                                };
                                AppError::VideoError(format!(
                                    "FFmpeg HW reconfigure failed: {}",
                                    detail
                                ))
                            })?;
                        return Ok(());
                    }
                }

                if let Some(ref mut encoder) = state.encoder {
                    encoder.set_bitrate(bitrate_kbps)?;
                }
            }
        }
        Ok(())
    }

    /// Get current stats
    pub async fn stats(&self) -> SharedVideoPipelineStats {
        self.stats.lock().await.clone()
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        *self.running_rx.borrow()
    }

    /// Subscribe to running state changes
    ///
    /// Returns a watch receiver that can be used to detect when the pipeline stops.
    /// This is useful for auto-cleanup when the pipeline auto-stops due to no subscribers.
    pub fn running_watch(&self) -> watch::Receiver<bool> {
        self.running_rx.clone()
    }

    async fn broadcast_encoded(&self, frame: Arc<EncodedVideoFrame>) {
        let subscribers = {
            let guard = self.subscribers.read();
            if guard.is_empty() {
                return;
            }
            guard.iter().cloned().collect::<Vec<_>>()
        };

        for tx in &subscribers {
            if tx.send(frame.clone()).await.is_err() {
                // Receiver dropped; cleanup happens below.
            }
        }

        if subscribers.iter().any(|tx| tx.is_closed()) {
            let mut guard = self.subscribers.write();
            guard.retain(|tx| !tx.is_closed());
        }
    }

    /// Start the pipeline by owning capture + encode in a single loop.
    ///
    /// Capture and encode stay tightly coupled to avoid maintaining separate
    /// raw-frame fan-out and direct-device execution paths.
    pub async fn start_with_device(
        self: &Arc<Self>,
        device_path: std::path::PathBuf,
        buffer_count: u32,
        _jpeg_quality: u8,
        subdev_path: Option<std::path::PathBuf>,
        bridge_kind: Option<String>,
        _v4l2_driver: Option<String>,
    ) -> Result<()> {
        if *self.running_rx.borrow() {
            warn!("Pipeline already running");
            return Ok(());
        }

        let mut config = self.config.read().await.clone();
        {
            let mut last = self.last_state_notification.lock();
            *last = None;
        }

        // Pre-open for DV negotiation; align encoder to probed size.
        let bridge_ctx_probe = BridgeContext::from_parts(
            subdev_path.clone(),
            parse_bridge_kind(bridge_kind.as_deref()),
        );
        let preopened: Option<V4l2rCaptureStream> = match V4l2rCaptureStream::open_with_bridge(
            &device_path,
            config.resolution,
            config.input_format,
            config.fps,
            buffer_count.max(1),
            Duration::from_secs(2),
            bridge_ctx_probe,
        ) {
            Ok(s) => {
                let negotiated_res = s.resolution();
                let negotiated_fmt = s.format();
                if negotiated_res != config.resolution || negotiated_fmt != config.input_format {
                    info!(
                            "Negotiated capture {}x{} {:?} (configured {}x{} {:?}) — aligning encoder to source",
                            negotiated_res.width,
                            negotiated_res.height,
                            negotiated_fmt,
                            config.resolution.width,
                            config.resolution.height,
                            config.input_format
                        );
                    config.resolution = negotiated_res;
                    config.input_format = negotiated_fmt;
                    *self.config.write().await = config.clone();
                }
                Some(s)
            }
            Err(AppError::CaptureNoSignal { kind }) => {
                debug!(
                    "Pre-probe: no signal — encoder uses configured geometry until capture opens"
                );
                let status = SignalStatus::from_str(&kind).unwrap_or(SignalStatus::NoSignal);
                self.notify_state(PipelineStateNotification::no_signal(
                    status,
                    Some(Duration::from_secs(2).as_millis() as u64),
                ));
                None
            }
            Err(e) => return Err(e),
        };

        let mut encoder_state = build_encoder_state(&config)?;
        let _ = self.running.send(true);
        self.running_flag.store(true, Ordering::Release);

        let pipeline = self.clone();
        let latest_frame: Arc<ParkingRwLock<Option<Arc<VideoFrame>>>> =
            Arc::new(ParkingRwLock::new(None));
        let (frame_seq_tx, mut frame_seq_rx) = watch::channel(0u64);
        let buffer_pool = Arc::new(FrameBufferPool::new(buffer_count.max(4) as usize));
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        {
            let mut guard = self.cmd_tx.write();
            *guard = Some(cmd_tx);
        }

        // Encoder loop (runs on tokio, consumes latest frame)
        {
            let pipeline = pipeline.clone();
            let latest_frame = latest_frame.clone();
            tokio::spawn(async move {
                let mut frame_count: u64 = 0;
                let mut last_fps_time = Instant::now();
                let mut fps_frame_count: u64 = 0;
                let mut last_seq = *frame_seq_rx.borrow();
                let encode_error_throttler = LogThrottler::with_secs(ENCODE_ERROR_THROTTLE_SECS);
                let mut suppressed_encode_errors: HashMap<String, u64> = HashMap::new();

                while pipeline.running_flag.load(Ordering::Acquire) {
                    if frame_seq_rx.changed().await.is_err() {
                        break;
                    }
                    if !pipeline.running_flag.load(Ordering::Acquire) {
                        break;
                    }

                    let seq = *frame_seq_rx.borrow();
                    if seq == last_seq {
                        continue;
                    }
                    last_seq = seq;

                    if pipeline.subscriber_count() == 0 {
                        continue;
                    }

                    while let Ok(cmd) = cmd_rx.try_recv() {
                        if let Err(e) = pipeline.apply_cmd(&mut encoder_state, cmd) {
                            error!("Failed to apply pipeline command: {}", e);
                        }
                    }

                    let frame = {
                        let guard = latest_frame.read();
                        guard.clone()
                    };
                    let frame = match frame {
                        Some(f) => f,
                        None => continue,
                    };

                    match pipeline.encode_frame_sync(&mut encoder_state, &frame, frame_count) {
                        Ok(Some(encoded_frame)) => {
                            let encoded_arc = Arc::new(encoded_frame);
                            pipeline.broadcast_encoded(encoded_arc).await;

                            frame_count += 1;
                            fps_frame_count += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log_encoding_error(
                                &encode_error_throttler,
                                &mut suppressed_encode_errors,
                                &e,
                            );
                        }
                    }

                    let fps_elapsed = last_fps_time.elapsed();
                    if fps_elapsed >= Duration::from_secs(1) {
                        let current_fps = fps_frame_count as f32 / fps_elapsed.as_secs_f32();
                        fps_frame_count = 0;
                        last_fps_time = Instant::now();

                        let mut s = pipeline.stats.lock().await;
                        s.current_fps = current_fps;
                    }
                }

                pipeline.clear_cmd_tx();
            });
        }

        // Capture loop (runs on thread, updates latest frame)
        {
            let pipeline = pipeline.clone();
            let latest_frame = latest_frame.clone();
            let frame_seq_tx = frame_seq_tx.clone();
            let buffer_pool = buffer_pool.clone();
            let bridge_ctx =
                BridgeContext::from_parts(subdev_path, parse_bridge_kind(bridge_kind.as_deref()));
            std::thread::spawn(move || {
                let mut stream: Option<V4l2rCaptureStream> = None;
                let mut initial_geometry: Option<(Resolution, PixelFormat)> = None;
                let mut resolution = config.resolution;
                let mut pixel_format = config.input_format;
                let mut stride: u32 = 0;

                match preopened {
                    Some(s) => {
                        resolution = s.resolution();
                        pixel_format = s.format();
                        stride = s.stride();
                        initial_geometry = Some((resolution, pixel_format));
                        stream = Some(s);
                    }
                    None => {
                        match V4l2rCaptureStream::open_with_bridge(
                            &device_path,
                            config.resolution,
                            config.input_format,
                            config.fps,
                            buffer_count.max(1),
                            Duration::from_secs(2),
                            bridge_ctx.clone(),
                        ) {
                            Ok(s) => {
                                resolution = s.resolution();
                                pixel_format = s.format();
                                stride = s.stride();
                                if resolution != config.resolution
                                    || pixel_format != config.input_format
                                {
                                    info!(
                                        "First capture open negotiated {}x{} {:?} but encoder expects {}x{} {:?} — stopping for dimension resync",
                                        resolution.width,
                                        resolution.height,
                                        pixel_format,
                                        config.resolution.width,
                                        config.resolution.height,
                                        config.input_format
                                    );
                                    pipeline.notify_state(PipelineStateNotification::device_busy(
                                        "config_changing",
                                    ));
                                    *pipeline.pending_sync_geometry.lock() =
                                        Some((resolution, pixel_format));
                                    let _ = pipeline.running.send(false);
                                    pipeline.running_flag.store(false, Ordering::Release);
                                    let _ = frame_seq_tx.send(1);
                                    return;
                                }
                                initial_geometry = Some((resolution, pixel_format));
                                stream = Some(s);
                            }
                            Err(AppError::CaptureNoSignal { kind }) => {
                                warn!(
                                    "Capture stream open reports no signal ({}) — pipeline will retry",
                                    kind
                                );
                                pipeline.notify_state(PipelineStateNotification::no_signal(
                                    SignalStatus::from_str(&kind).unwrap_or(SignalStatus::NoSignal),
                                    Some(CSI_BRIDGE_NOSIGNAL_INTERVAL_MS),
                                ));
                            }
                            Err(e) => {
                                error!("Failed to open capture stream: {}", e);
                                let _ = pipeline.running.send(false);
                                pipeline.running_flag.store(false, Ordering::Release);
                                let _ = frame_seq_tx.send(1);
                                return;
                            }
                        }
                    }
                }

                /// Helper: try to (re)open the capture stream. Returns:
                ///   * `Ok(Some(stream))` — opened successfully
                ///   * `Ok(None)` — CaptureNoSignal, keep retrying later
                ///   * `Err(())` — fatal (stop pipeline)
                enum OpenResult {
                    Opened(V4l2rCaptureStream),
                    NoSignal(SignalStatus),
                    Fatal,
                }

                fn open_or_retry(
                    device_path: &std::path::Path,
                    config: &SharedVideoPipelineConfig,
                    buffer_count: u32,
                    bridge_ctx: BridgeContext,
                ) -> OpenResult {
                    match V4l2rCaptureStream::open_with_bridge(
                        device_path,
                        config.resolution,
                        config.input_format,
                        config.fps,
                        buffer_count.max(1),
                        Duration::from_secs(2),
                        bridge_ctx,
                    ) {
                        Ok(s) => OpenResult::Opened(s),
                        Err(AppError::CaptureNoSignal { kind }) => {
                            debug!("Capture soft-restart: still no signal ({})", kind);
                            OpenResult::NoSignal(
                                SignalStatus::from_str(&kind).unwrap_or(SignalStatus::NoSignal),
                            )
                        }
                        Err(e) => {
                            error!("Capture soft-restart failed: {}", e);
                            OpenResult::Fatal
                        }
                    }
                }

                let mut no_subscribers_since: Option<Instant> = None;
                let grace_period = Duration::from_secs(AUTO_STOP_GRACE_PERIOD_SECS);
                let mut sequence: u64 = 0;
                let mut validate_counter: u64 = 0;
                let mut consecutive_timeouts: u32 = 0;
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

                while pipeline.running_flag.load(Ordering::Acquire) {
                    let subscriber_count = pipeline.subscriber_count();
                    if subscriber_count == 0 {
                        if no_subscribers_since.is_none() {
                            no_subscribers_since = Some(Instant::now());
                            trace!("No subscribers, starting grace period timer");
                        }

                        if let Some(since) = no_subscribers_since {
                            if since.elapsed() >= grace_period {
                                info!(
                                    "No subscribers for {}s, auto-stopping video pipeline",
                                    grace_period.as_secs()
                                );
                                let _ = pipeline.running.send(false);
                                pipeline.running_flag.store(false, Ordering::Release);
                                let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                break;
                            }
                        }

                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    } else if no_subscribers_since.is_some() {
                        trace!("Subscriber connected, resetting grace period timer");
                        no_subscribers_since = None;
                    }

                    // ── No usable stream?  Try to (re)open, back off on failure. ──
                    if stream.is_none() {
                        match open_or_retry(&device_path, &config, buffer_count, bridge_ctx.clone())
                        {
                            OpenResult::Opened(new_stream) => {
                                let new_res = new_stream.resolution();
                                let new_fmt = new_stream.format();
                                let new_stride = new_stream.stride();

                                // Pre-probe was skipped (no signal at pipeline start) but the
                                // encoder was sized to saved settings — if DV timings now
                                // disagree, we cannot encode until WebRTC resyncs dimensions.
                                if initial_geometry.is_none()
                                    && (new_res != config.resolution
                                        || new_fmt != config.input_format)
                                {
                                    info!(
                                        "Deferred capture open is {}x{} {:?} but encoder expects {}x{} {:?} — stopping for dimension resync",
                                        new_res.width,
                                        new_res.height,
                                        new_fmt,
                                        config.resolution.width,
                                        config.resolution.height,
                                        config.input_format
                                    );
                                    pipeline.notify_state(PipelineStateNotification::device_busy(
                                        "config_changing",
                                    ));
                                    *pipeline.pending_sync_geometry.lock() =
                                        Some((new_res, new_fmt));
                                    let _ = pipeline.running.send(false);
                                    pipeline.running_flag.store(false, Ordering::Release);
                                    let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                    break;
                                }

                                // If this is the very first successful open,
                                // record it and run normally.  Otherwise check
                                // for a geometry change — the encoder thread
                                // is pinned to the original geometry, so a
                                // change requires tearing the pipeline down
                                // and letting the upper layer rebuild.
                                match initial_geometry {
                                    Some((orig_res, orig_fmt))
                                        if orig_res != new_res || orig_fmt != new_fmt =>
                                    {
                                        info!(
                                            "Capture soft-restart detected geometry change \
                                             {:?}/{:?} -> {:?}/{:?}, stopping pipeline for \
                                             encoder rebuild",
                                            orig_res, orig_fmt, new_res, new_fmt
                                        );
                                        pipeline.notify_state(
                                            PipelineStateNotification::device_busy(
                                                "config_changing",
                                            ),
                                        );
                                        *pipeline.pending_sync_geometry.lock() =
                                            Some((new_res, new_fmt));
                                        let _ = pipeline.running.send(false);
                                        pipeline.running_flag.store(false, Ordering::Release);
                                        let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                        break;
                                    }
                                    _ => {}
                                }

                                if initial_geometry.is_none() {
                                    initial_geometry = Some((new_res, new_fmt));
                                }
                                resolution = new_res;
                                pixel_format = new_fmt;
                                stride = new_stride;
                                stream = Some(new_stream);
                                consecutive_timeouts = 0;
                                info!(
                                    "Capture stream (re)opened: {}x{} {:?} stride={}",
                                    resolution.width, resolution.height, pixel_format, stride
                                );
                            }
                            OpenResult::NoSignal(status) => {
                                consecutive_timeouts = consecutive_timeouts.saturating_add(1);
                                if consecutive_timeouts >= CAPTURE_TIMEOUT_STOP_THRESHOLD {
                                    warn!(
                                        "Capture soft-restart gave up after {} attempts, \
                                         stopping pipeline",
                                        consecutive_timeouts
                                    );
                                    let _ = pipeline.running.send(false);
                                    pipeline.running_flag.store(false, Ordering::Release);
                                    let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                    break;
                                }
                                let wait_ms = CSI_BRIDGE_NOSIGNAL_INTERVAL_MS;
                                pipeline.notify_state(PipelineStateNotification::no_signal(
                                    status,
                                    Some(wait_ms),
                                ));
                                std::thread::sleep(Duration::from_millis(wait_ms));
                                continue;
                            }
                            OpenResult::Fatal => {
                                let _ = pipeline.running.send(false);
                                pipeline.running_flag.store(false, Ordering::Release);
                                let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                break;
                            }
                        }
                    }

                    let mut owned = buffer_pool.take(MIN_CAPTURE_FRAME_SIZE);
                    let next_result = stream
                        .as_mut()
                        .expect("stream is Some above")
                        .next_into(&mut owned);
                    let meta = match next_result {
                        Ok(meta) => {
                            consecutive_timeouts = 0;
                            meta
                        }
                        Err(e) => {
                            // V4L2 driver reported V4L2_EVENT_SOURCE_CHANGE.
                            // The current capture is effectively invalidated:
                            // drop the stream so the next iteration re-opens
                            // via a fresh DV_TIMINGS probe.  This is the fast
                            // path for source-side resolution switches on
                            // RK628 / rkcif — sub-second recovery vs. the ~8 s
                            // timeout fallback.
                            if is_source_changed_error(&e) {
                                info!(
                                    "Capture reported SOURCE_CHANGE — \
                                     dropping stream for immediate re-open"
                                );
                                consecutive_timeouts = 0;
                                stream = None;
                                continue;
                            }
                            if e.kind() == std::io::ErrorKind::TimedOut {
                                consecutive_timeouts = consecutive_timeouts.saturating_add(1);
                                let probe_result = {
                                    let sr = stream.as_mut().expect("stream is Some above");
                                    sr.probe_bridge_signal_with_timeout(
                                        csi_bridge::RK628_SUBDEV_PROBE_TIMEOUT,
                                    )
                                };
                                match probe_result {
                                    Some(ProbeResult::Locked(mode)) => {
                                        let probed_resolution =
                                            Resolution::new(mode.width, mode.height);
                                        if probed_resolution == resolution {
                                            info!(
                                                "Capture timeout but bridge is locked at {}x{} — soft-restarting capture without encoder rebuild",
                                                probed_resolution.width,
                                                probed_resolution.height
                                            );
                                        } else {
                                            info!(
                                                "Capture timeout probe detected geometry change {}x{} -> {}x{} — soft-restarting capture for encoder rebuild",
                                                resolution.width,
                                                resolution.height,
                                                probed_resolution.width,
                                                probed_resolution.height
                                            );
                                            pipeline.notify_state(
                                                PipelineStateNotification::device_busy(
                                                    "config_changing",
                                                ),
                                            );
                                        }
                                        consecutive_timeouts = 0;
                                        stream = None;
                                        continue;
                                    }
                                    Some(other) => {
                                        let status =
                                            other.as_status().unwrap_or(SignalStatus::NoSignal);
                                        warn!(
                                            "Capture timeout probe reports no signal ({})",
                                            status.as_str()
                                        );
                                        pipeline.notify_state(
                                            PipelineStateNotification::no_signal(
                                                status,
                                                Some(Duration::from_secs(2).as_millis() as u64),
                                            ),
                                        );
                                        // Drop capture so RK628 / rkcif can release the queue,
                                        // then poll subdev on a fresh fd until timings lock (or
                                        // timeout).  Avoids sitting on DQBUF 2s × N with a dead
                                        // stream while `v4l2-ctl --query-dv-timings` already shows
                                        // a real mode.
                                        stream = None;
                                        consecutive_timeouts = 0;
                                        if bridge_ctx.has_subdev()
                                            && matches!(
                                                other,
                                                ProbeResult::NoSignal
                                                    | ProbeResult::NoSync
                                                    | ProbeResult::OutOfRange
                                            )
                                        {
                                            poll_bridge_subdev_after_no_signal(
                                                &bridge_ctx,
                                                &pipeline,
                                            );
                                        }
                                        continue;
                                    }
                                    None if bridge_ctx.has_subdev() => {
                                        warn!(
                                            "DV-timings probe timed out or failed — forcing stream re-open (RK628 / rkcif)"
                                        );
                                        consecutive_timeouts = 0;
                                        stream = None;
                                        poll_bridge_subdev_after_no_signal(&bridge_ctx, &pipeline);
                                        continue;
                                    }
                                    None => {
                                        warn!("Capture timeout - no signal?");
                                    }
                                }

                                if consecutive_timeouts >= CAPTURE_TIMEOUT_SOFT_RESTART_THRESHOLD {
                                    // Drop the stream so the next loop
                                    // iteration re-opens via the DV-timings
                                    // probe.  This catches source-side
                                    // resolution changes in ~6 s without
                                    // taking the encoder down.
                                    warn!(
                                        "Capture timed out {} consecutive times, \
                                         closing stream for soft-restart",
                                        consecutive_timeouts
                                    );
                                    pipeline.notify_state(PipelineStateNotification::no_signal(
                                        SignalStatus::UvcCaptureStall,
                                        Some(Duration::from_secs(2).as_millis() as u64),
                                    ));
                                    stream = None;
                                    continue;
                                }

                                if consecutive_timeouts == CAPTURE_TIMEOUT_RESTART_THRESHOLD {
                                    warn!(
                                        "Capture timed out {} consecutive times – no signal?",
                                        consecutive_timeouts
                                    );
                                }

                                if consecutive_timeouts >= CAPTURE_TIMEOUT_STOP_THRESHOLD {
                                    warn!(
                                        "Capture timed out {} consecutive times, stopping video pipeline",
                                        consecutive_timeouts
                                    );
                                    let _ = pipeline.running.send(false);
                                    pipeline.running_flag.store(false, Ordering::Release);
                                    let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                                    break;
                                }
                            } else {
                                consecutive_timeouts = 0;
                                // EIO (5) / EPIPE (32) / EPROTO (71) in next_into generally
                                // mean the source or UVC USB transport glitched mid-stream.
                                // Tear down the stream and let the open loop re-probe.
                                let os = e.raw_os_error();
                                if matches!(os, Some(5) | Some(32) | Some(71)) {
                                    if os == Some(71) {
                                        warn!(
                                            "Capture transient error (EPROTO/-71, often UVC USB): {} — soft-restart",
                                            e
                                        );
                                        pipeline.notify_state(
                                            PipelineStateNotification::no_signal(
                                                SignalStatus::UvcUsbError,
                                                Some(Duration::from_secs(2).as_millis() as u64),
                                            ),
                                        );
                                    } else {
                                        warn!(
                                            "Capture transient error ({}), closing stream for \
                                             soft-restart",
                                            e
                                        );
                                    }
                                    stream = None;
                                    continue;
                                }
                                let key = classify_capture_error(&e);
                                if capture_error_throttler.should_log(&key) {
                                    let suppressed =
                                        suppressed_capture_errors.remove(&key).unwrap_or(0);
                                    if suppressed > 0 {
                                        error!(
                                            "Capture error: {} (suppressed {} repeats)",
                                            e, suppressed
                                        );
                                    } else {
                                        error!("Capture error: {}", e);
                                    }
                                } else {
                                    let counter = suppressed_capture_errors.entry(key).or_insert(0);
                                    *counter = counter.saturating_add(1);
                                }
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
                        && should_validate_jpeg_frame(validate_counter)
                        && !VideoFrame::is_valid_jpeg_bytes(&owned[..frame_size])
                    {
                        continue;
                    }

                    owned.truncate(frame_size);
                    // Notify streaming only after frame validation passes —
                    // stale/warm-up frames from V4L2 kernel queues can cause
                    // DQBUF Ok with invalid data, which would prematurely
                    // clear the frontend error overlay.
                    pipeline.notify_state(PipelineStateNotification::streaming());
                    let frame = Arc::new(VideoFrame::from_pooled(
                        Arc::new(FrameBuffer::new(owned, Some(buffer_pool.clone()))),
                        resolution,
                        pixel_format,
                        stride,
                        meta.sequence,
                    ));
                    sequence = meta.sequence.wrapping_add(1);

                    {
                        let mut guard = latest_frame.write();
                        *guard = Some(frame);
                    }
                    let _ = frame_seq_tx.send(sequence);
                }

                pipeline.running_flag.store(false, Ordering::Release);
                let _ = pipeline.running.send(false);
                let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                info!("Video pipeline stopped");
            });
        }

        Ok(())
    }

    /// Encode a single frame (synchronous, no async locks)
    fn encode_frame_sync(
        &self,
        state: &mut EncoderThreadState,
        frame: &VideoFrame,
        frame_count: u64,
    ) -> Result<Option<EncodedVideoFrame>> {
        let fps = state.fps;
        let codec = state.codec;
        let input_format = state.input_format;
        let raw_frame = frame.data();

        // Calculate PTS from real capture timestamp (lock-free using AtomicI64)
        // This ensures smooth playback even when capture timing varies
        let frame_ts_ms = frame.capture_ts.elapsed().as_millis() as i64;
        // Convert Instant to a comparable value (negate elapsed to get "time since epoch")
        let current_ts_ms = -(frame_ts_ms);

        // Try to set start time if not yet set (first frame wins)
        let start_ts = self.pipeline_start_time_ms.load(Ordering::Acquire);
        let pts_ms = if start_ts == 0 {
            // First frame - try to set the start time
            // Use compare_exchange to ensure only one thread sets it
            let _ = self.pipeline_start_time_ms.compare_exchange(
                0,
                current_ts_ms,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            0 // First frame has PTS 0
        } else {
            // Subsequent frames: PTS = current - start
            current_ts_ms - start_ts
        };

        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        if state.ffmpeg_hw_enabled {
            if input_format != PixelFormat::Mjpeg {
                return Err(AppError::VideoError(
                    "FFmpeg HW pipeline requires MJPEG input".to_string(),
                ));
            }
            let pipeline = state.ffmpeg_hw_pipeline.as_mut().ok_or_else(|| {
                AppError::VideoError("FFmpeg HW pipeline not initialized".to_string())
            })?;

            if self.keyframe_requested.swap(false, Ordering::AcqRel) {
                pipeline.request_keyframe();
                debug!("[Pipeline] FFmpeg HW keyframe requested");
            }

            let packet = pipeline.encode(raw_frame, pts_ms).map_err(|e| {
                let detail = if e.is_empty() {
                    ffmpeg_hw_last_error()
                } else {
                    e
                };
                AppError::VideoError(format!("FFmpeg HW encode failed: {}", detail))
            })?;

            if let Some((data, is_keyframe)) = packet {
                let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
                return Ok(Some(EncodedVideoFrame {
                    data: Bytes::from(data),
                    pts_ms,
                    is_keyframe,
                    sequence,
                    duration: Duration::from_millis(1000 / fps as u64),
                    codec,
                }));
            }

            return Ok(None);
        }

        let decoded_buf = if input_format.is_compressed() {
            let decoder = state
                .mjpeg_decoder
                .as_mut()
                .ok_or_else(|| AppError::VideoError("MJPEG decoder not initialized".to_string()))?;
            let decoded = decoder.decode(raw_frame)?;
            Some(decoded)
        } else {
            None
        };
        let raw_frame = decoded_buf.as_deref().unwrap_or(raw_frame);

        // Debug log for H265
        if codec == VideoEncoderType::H265 && frame_count % 30 == 1 {
            debug!(
                "[Pipeline-H265] Processing frame #{}: input_size={}, pts_ms={}",
                frame_count,
                raw_frame.len(),
                pts_ms
            );
        }

        let needs_yuv420p = state.encoder_needs_yuv420p;
        let encoder = state
            .encoder
            .as_mut()
            .ok_or_else(|| AppError::VideoError("Encoder not initialized".to_string()))?;

        // Check and consume keyframe request (atomic, no lock contention)
        if self.keyframe_requested.swap(false, Ordering::AcqRel) {
            encoder.request_keyframe();
            debug!("[Pipeline] Keyframe will be generated for this frame");
        }

        let encode_result = if needs_yuv420p {
            // Software encoder with direct input conversion to YUV420P
            if let Some(conv) = state.yuv420p_converter.as_mut() {
                let yuv420p_data = conv.convert(raw_frame).map_err(|e| {
                    AppError::VideoError(format!("YUV420P conversion failed: {}", e))
                })?;
                encoder.encode_raw(yuv420p_data, pts_ms)
            } else {
                encoder.encode_raw(raw_frame, pts_ms)
            }
        } else if let Some(conv) = state.nv12_converter.as_mut() {
            // Hardware encoder with input conversion to NV12
            let nv12_data = conv
                .convert(raw_frame)
                .map_err(|e| AppError::VideoError(format!("NV12 conversion failed: {}", e)))?;
            encoder.encode_raw(nv12_data, pts_ms)
        } else {
            // Direct input (already in correct format)
            encoder.encode_raw(raw_frame, pts_ms)
        };

        match encode_result {
            Ok(frames) => {
                if !frames.is_empty() {
                    let encoded = frames.into_iter().next().unwrap();
                    let is_keyframe = encoded.key == 1;
                    let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;

                    // Debug log for H265 encoded frame
                    if codec == VideoEncoderType::H265 && (is_keyframe || frame_count % 30 == 1) {
                        debug!(
                            "[Pipeline-H265] Encoded frame #{}: output_size={}, keyframe={}, sequence={}",
                            frame_count,
                            encoded.data.len(),
                            is_keyframe,
                            sequence
                        );

                        // Log H265 NAL unit types in the encoded data
                        if is_keyframe {
                            let nal_types = parse_h265_nal_types(&encoded.data);
                            debug!("[Pipeline-H265] Keyframe NAL types: {:?}", nal_types);
                        }
                    }

                    Ok(Some(EncodedVideoFrame {
                        data: Bytes::from(encoded.data),
                        pts_ms,
                        is_keyframe,
                        sequence,
                        duration: Duration::from_millis(1000 / fps as u64),
                        codec,
                    }))
                } else {
                    if codec == VideoEncoderType::H265 {
                        warn!(
                            "[Pipeline-H265] Encoder returned no frames for frame #{}",
                            frame_count
                        );
                    }
                    Ok(None)
                }
            }
            Err(e) => {
                if codec == VideoEncoderType::H265 {
                    error!(
                        "[Pipeline-H265] Encode error at frame #{}: {}",
                        frame_count, e
                    );
                }
                Err(e)
            }
        }
    }

    /// Stop the pipeline (non-blocking, does not wait for capture thread to exit)
    pub fn stop(&self) {
        if *self.running_rx.borrow() {
            let _ = self.running.send(false);
            self.running_flag.store(false, Ordering::Release);
            self.clear_cmd_tx();
            info!("Stopping video pipeline");
        }
    }

    /// Stop the pipeline and wait for the capture thread to fully exit.
    ///
    /// This ensures the V4L2 device is released before returning, which is
    /// necessary when another consumer (e.g. MJPEG streamer) needs to open
    /// the same device immediately after.
    pub async fn stop_and_wait(&self, timeout: std::time::Duration) {
        self.stop();
        let mut rx = self.running_watch();
        if !*rx.borrow() {
            // Capture thread may still be running from a previous `stop()` call.
            // Wait for the "Video pipeline stopped" log (thread sets running=false
            // at exit), unless it already happened.
        }
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if !self.running_flag.load(Ordering::Acquire) {
                // Flag is cleared, but the capture thread may still be unwinding
                // (dropping the V4L2 stream). Give it a brief moment.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                break;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                warn!(
                    "Timed out waiting for video pipeline to stop after {:?}",
                    timeout
                );
                break;
            }
            let _ = tokio::time::timeout(remaining, rx.changed()).await;
        }
    }

    /// Set bitrate using preset
    pub async fn set_bitrate_preset(
        &self,
        preset: crate::video::encoder::BitratePreset,
    ) -> Result<()> {
        let bitrate_kbps = preset.bitrate_kbps();
        let gop = {
            let mut config = self.config.write().await;
            config.bitrate_preset = preset;
            config.gop_size()
        };
        self.send_cmd(PipelineCmd::SetBitrate { bitrate_kbps, gop });
        Ok(())
    }

    /// Set bitrate using raw kbps value (converts to appropriate preset)
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        let preset = crate::video::encoder::BitratePreset::from_kbps(bitrate_kbps);
        self.set_bitrate_preset(preset).await
    }

    /// Get current config
    pub async fn config(&self) -> SharedVideoPipelineConfig {
        self.config.read().await.clone()
    }
}

impl Drop for SharedVideoPipeline {
    fn drop(&mut self) {
        let _ = self.running.send(false);
    }
}

/// Parse H265 NAL unit types from Annex B data
fn parse_h265_nal_types(data: &[u8]) -> Vec<(u8, usize)> {
    let mut nal_types = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Find start code
        let nal_start = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            i + 4
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            i + 3
        } else {
            i += 1;
            continue;
        };

        if nal_start >= data.len() {
            break;
        }

        // Find next start code to get NAL size
        let mut nal_end = data.len();
        let mut j = nal_start + 1;
        while j + 3 <= data.len() {
            if (data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1)
                || (j + 4 <= data.len()
                    && data[j] == 0
                    && data[j + 1] == 0
                    && data[j + 2] == 0
                    && data[j + 3] == 1)
            {
                nal_end = j;
                break;
            }
            j += 1;
        }

        // H265 NAL type is in bits 1-6 of first byte
        let nal_type = (data[nal_start] >> 1) & 0x3F;
        let nal_size = nal_end - nal_start;
        nal_types.push((nal_type, nal_size));
        i = nal_end;
    }

    nal_types
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::encoder::BitratePreset;

    #[test]
    fn test_pipeline_config() {
        let h264 = SharedVideoPipelineConfig::h264(Resolution::HD1080, BitratePreset::Balanced);
        assert_eq!(h264.output_codec, VideoEncoderType::H264);

        let h265 = SharedVideoPipelineConfig::h265(Resolution::HD720, BitratePreset::Speed);
        assert_eq!(h265.output_codec, VideoEncoderType::H265);
    }
}
