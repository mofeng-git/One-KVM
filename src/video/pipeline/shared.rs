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

use bytes::Bytes;
use parking_lot::Mutex as ParkingMutex;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{debug, error, info, trace, warn};

use super::delivery::{
    DeliveryStats, EncodedBatch, EncodedVideoFrameReceiver, FrameMailboxHandle,
    VideoFrameReservation, VideoSubscriber,
};
use super::encoder_state::{build_encoder_state, should_parallel_decode_mjpeg, EncoderThreadState};
use super::frame_mailbox::{FrameMailbox, PublishResult};

#[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
#[path = "dmabuf.rs"]
mod dmabuf;

#[cfg(any(
    test,
    all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm"))
))]
fn rkmpp_dma_eligible(
    backend: Option<EncoderBackend>,
    codec: VideoEncoderType,
    format: PixelFormat,
) -> bool {
    backend == Some(EncoderBackend::Rkmpp)
        && matches!(codec, VideoEncoderType::H264 | VideoEncoderType::H265)
        && matches!(
            format,
            PixelFormat::Bgr24
                | PixelFormat::Nv12
                | PixelFormat::Yuyv
                | PixelFormat::Rgb24
                | PixelFormat::Mjpeg
        )
}

#[cfg(test)]
mod dma_selection_tests {
    use super::*;
    #[test]
    fn only_selected_rkmpp_uses_dma() {
        for backend in [
            EncoderBackend::Software,
            EncoderBackend::Vaapi,
            EncoderBackend::Nvenc,
            EncoderBackend::Qsv,
            EncoderBackend::Amf,
            EncoderBackend::V4l2m2m,
        ] {
            for codec in [VideoEncoderType::H264, VideoEncoderType::H265] {
                for format in [
                    PixelFormat::Bgr24,
                    PixelFormat::Nv12,
                    PixelFormat::Yuyv,
                    PixelFormat::Rgb24,
                    PixelFormat::Mjpeg,
                ] {
                    assert!(!rkmpp_dma_eligible(Some(backend), codec, format));
                }
            }
        }
        assert!(!rkmpp_dma_eligible(
            None,
            VideoEncoderType::H264,
            PixelFormat::Nv12
        ));
        for codec in [VideoEncoderType::H264, VideoEncoderType::H265] {
            for format in [
                PixelFormat::Bgr24,
                PixelFormat::Nv12,
                PixelFormat::Yuyv,
                PixelFormat::Rgb24,
                PixelFormat::Mjpeg,
            ] {
                assert!(rkmpp_dma_eligible(
                    Some(EncoderBackend::Rkmpp),
                    codec,
                    format
                ));
            }
        }
        for format in [
            PixelFormat::Nv16,
            PixelFormat::Nv21,
            PixelFormat::Nv24,
            PixelFormat::Yuv420,
        ] {
            assert!(!rkmpp_dma_eligible(
                Some(EncoderBackend::Rkmpp),
                VideoEncoderType::H264,
                format
            ));
        }
        assert!(!rkmpp_dma_eligible(
            Some(EncoderBackend::Rkmpp),
            VideoEncoderType::VP9,
            PixelFormat::Nv12
        ));
    }
}

/// Grace period before auto-stopping pipeline when no subscribers (in seconds)
const AUTO_STOP_GRACE_PERIOD_SECS: u64 = 3;
/// After this many consecutive timeouts, log a prominent warning.
const CAPTURE_TIMEOUT_RESTART_THRESHOLD: u32 = 5;
const CAPTURE_TIMEOUT_SOFT_RESTART_THRESHOLD: u32 = 3;
/// Throttle repeated encoding errors to avoid log flooding
const ENCODE_ERROR_THROTTLE_SECS: u64 = 5;

static PROCESS_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

use crate::error::{AppError, Result};
use crate::utils::LogThrottler;
use crate::video::capture::runtime::{
    open_capture_stream, open_capture_stream_for_retry, CaptureOpenResult,
};
use crate::video::capture::status::{
    capture_error_log_key, classify_capture_io_error, is_device_lost_message,
    signal_status_from_capture_kind, CaptureIoErrorKind,
};
use crate::video::capture::{BridgeContext, CaptureReadError, CaptureStream};
use crate::video::codec::registry::{EncoderBackend, VideoEncoderType};
use crate::video::codec::MjpegToNv12Decoder;
use crate::video::codec::{h264_bitstream, h265_bitstream};
use crate::video::device::parse_bridge_kind;
use crate::video::device::VideoControlMode;
use crate::video::format::{PixelFormat, Resolution};

use crate::video::frame::{FrameBuffer, FrameBufferPool, VideoFrame};
use crate::video::recovery::{wait_for_source_change, CaptureRecoveryPolicy};
use crate::video::signal::SignalStatus;

const MIN_CAPTURE_FRAME_SIZE: usize = 128;
type VideoFrameMailbox = FrameMailbox<Arc<VideoFrame>>;

fn spawn_video_worker(
    name: &'static str,
    worker: impl FnOnce() + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(worker)
}

struct MjpegDecodeJob {
    data: Vec<u8>,
    sequence: u64,
}

fn mjpeg_decode_worker_count(available_parallelism: usize) -> usize {
    available_parallelism.max(1)
}

fn spawn_mjpeg_decode_workers(
    pipeline: &Arc<SharedVideoPipeline>,
    frame_mailbox: &Arc<VideoFrameMailbox>,
    buffer_pool: &Arc<FrameBufferPool>,
    resolution: Resolution,
) -> Vec<SyncSender<MjpegDecodeJob>> {
    let available = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let worker_count = mjpeg_decode_worker_count(available);
    let mut senders = Vec::with_capacity(worker_count);

    for worker_id in 0..worker_count {
        // A rendezvous channel deliberately has no queue. If every decoder is
        // busy, capture drops the new compressed frame instead of building up
        // latency behind stale frames.
        let (tx, rx) = sync_channel::<MjpegDecodeJob>(0);
        let worker_pipeline = pipeline.clone();
        let worker_frame_mailbox = frame_mailbox.clone();
        let worker_buffer_pool = buffer_pool.clone();
        let thread_name = format!("mjpeg-decoder-{worker_id}");
        let spawn_result = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let mut decoder = MjpegToNv12Decoder::new(resolution);
                while let Ok(job) = rx.recv() {
                    if !worker_pipeline.running_flag.load(Ordering::Acquire) {
                        worker_buffer_pool.put(job.data);
                        break;
                    }
                    if worker_frame_mailbox.has_pending() {
                        worker_pipeline
                            .mjpeg_dropped_before_decode
                            .fetch_add(1, Ordering::Relaxed);
                        worker_buffer_pool.put(job.data);
                        continue;
                    }
                    let nv12_size = resolution.width as usize * resolution.height as usize * 3 / 2;
                    let mut nv12 = worker_buffer_pool.take(nv12_size);
                    let decode_started = Instant::now();
                    let decode_result = decoder.decode_into(&job.data, &mut nv12);
                    worker_buffer_pool.put(job.data);

                    if let Err(error) = decode_result {
                        worker_buffer_pool.put(nv12);
                        warn!("Dropping undecodable MJPEG frame: {}", error);
                        continue;
                    }
                    worker_pipeline.mjpeg_decode_time_ns.fetch_add(
                        decode_started.elapsed().as_nanos() as u64,
                        Ordering::Relaxed,
                    );
                    worker_pipeline
                        .mjpeg_decoded_frames
                        .fetch_add(1, Ordering::Relaxed);
                    if !worker_pipeline.running_flag.load(Ordering::Acquire) {
                        worker_pipeline
                            .decoded_frames_not_encoded
                            .fetch_add(1, Ordering::Relaxed);
                        worker_buffer_pool.put(nv12);
                        break;
                    }

                    let frame = Arc::new(VideoFrame::from_pooled(
                        Arc::new(FrameBuffer::new(nv12, Some(worker_buffer_pool.clone()))),
                        resolution,
                        PixelFormat::Nv12,
                        resolution.width,
                        job.sequence,
                    ));
                    if worker_frame_mailbox.publish(job.sequence, frame) != PublishResult::Published
                    {
                        worker_pipeline
                            .decoded_frames_not_encoded
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            });

        match spawn_result {
            Ok(_) => senders.push(tx),
            Err(error) => error!("Failed to start MJPEG decoder worker: {}", error),
        }
    }

    info!("Started {} parallel MJPEG decoder worker(s)", senders.len());
    senders
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use hwcodec::ffmpeg_hw::last_error_message as ffmpeg_hw_last_error;

/// Encoded video frame for distribution
#[derive(Debug, Clone)]
pub struct EncodedVideoFrame {
    /// Encoded data (Annex B for H264/H265, raw for VP8/VP9)
    pub data: Bytes,
    /// Presentation timestamp in milliseconds
    pub pts_ms: i64,
    /// Whether this frame can initialize a decoder without earlier frames.
    ///
    /// For H.264/H.265 this is stricter than the encoder packet flag: the
    /// payload must be IDR/IRAP and include all required parameter sets.
    pub is_keyframe: bool,
    /// Frame sequence number
    pub sequence: u64,
    /// Frame duration
    pub duration: Duration,
    /// Codec type
    pub codec: VideoEncoderType,
}

enum PipelineCmd {
    SetBitrate {
        preset: crate::video::codec::BitratePreset,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineStateNotification {
    pub state: &'static str,
    pub reason: Option<&'static str>,
    pub next_retry_ms: Option<u64>,
    pub applied_config: Option<PipelineAppliedConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineAppliedConfig {
    pub resolution: Resolution,
    pub format: PixelFormat,
    pub fps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineLifecycle {
    Running,
    Stopping,
    Stopped,
}

impl PipelineStateNotification {
    fn streaming(resolution: Resolution, format: PixelFormat, fps: u32) -> Self {
        Self {
            state: "streaming",
            reason: None,
            next_retry_ms: None,
            applied_config: Some(PipelineAppliedConfig {
                resolution,
                format,
                fps,
            }),
        }
    }

    fn no_signal(status: SignalStatus, next_retry_ms: Option<u64>) -> Self {
        Self {
            state: "no_signal",
            reason: Some(status.as_str()),
            next_retry_ms,
            applied_config: None,
        }
    }
}

/// Shared video pipeline configuration
#[derive(Debug, Clone)]
pub struct SharedVideoPipelineConfig {
    /// Whether the capture mode is configured by the client or follows HDMI.
    pub control_mode: VideoControlMode,
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Output codec type
    pub output_codec: VideoEncoderType,
    /// Bitrate preset (replaces raw bitrate_kbps)
    pub bitrate_preset: crate::video::codec::BitratePreset,
    /// Target FPS
    pub fps: u32,
    /// Encoder backend (None = auto select best available)
    pub encoder_backend: Option<EncoderBackend>,
}

impl Default for SharedVideoPipelineConfig {
    fn default() -> Self {
        Self {
            control_mode: VideoControlMode::Configurable,
            resolution: Resolution::HD720,
            input_format: PixelFormat::Yuyv,
            output_codec: VideoEncoderType::H264,
            bitrate_preset: crate::video::codec::BitratePreset::Balanced,
            fps: 30,
            encoder_backend: None,
        }
    }
}

impl SharedVideoPipelineConfig {
    /// Keep encoder timing aligned with the negotiated HDMI source on every open.
    fn align_source_fps(&mut self, source_fps: Option<f64>) {
        if self.control_mode == VideoControlMode::SourceFollowing {
            if let Some(fps) = source_fps {
                self.fps = fps.round().clamp(1.0, 120.0) as u32;
            }
        }
    }

    /// Get effective bitrate in kbps
    pub fn bitrate_kbps(&self) -> u32 {
        self.bitrate_preset.bitrate_kbps()
    }

    /// Get effective GOP size
    pub fn gop_size(&self) -> u32 {
        self.bitrate_preset.gop_size(self.fps)
    }

    /// Create H264 config with bitrate preset
    pub fn h264(resolution: Resolution, preset: crate::video::codec::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H264,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create H265 config with bitrate preset
    pub fn h265(resolution: Resolution, preset: crate::video::codec::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H265,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create VP8 config with bitrate preset
    pub fn vp8(resolution: Resolution, preset: crate::video::codec::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP8,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create VP9 config with bitrate preset
    pub fn vp9(resolution: Resolution, preset: crate::video::codec::BitratePreset) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP9,
            bitrate_preset: preset,
            ..Default::default()
        }
    }

    /// Create config with legacy bitrate_kbps (for compatibility during migration)
    pub fn with_bitrate_kbps(mut self, bitrate_kbps: u32) -> Self {
        self.bitrate_preset = crate::video::codec::BitratePreset::from_kbps(bitrate_kbps);
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

fn log_pipeline_performance(
    stats: &SharedVideoPipelineStats,
    previous: &SharedVideoPipelineStats,
    elapsed: f64,
) {
    let delta = |current: u64, previous: u64| current.saturating_sub(previous);
    let decoded = delta(stats.mjpeg_decoded_frames, previous.mjpeg_decoded_frames);
    let encoded_inputs = delta(stats.encoded_input_frames, previous.encoded_input_frames);
    let encoded = delta(stats.encoded_frames, previous.encoded_frames);
    let sent = delta(stats.sent_frames, previous.sent_frames);
    let send_errors = delta(stats.send_errors, previous.send_errors);
    let average_ms = |time_ns: u64, count: u64| {
        if count == 0 {
            0.0
        } else {
            time_ns as f64 / count as f64 / 1_000_000.0
        }
    };
    info!(
        "[VideoPerf] window_s={:.2} decoded_fps={:.2} encoded_fps={:.2} sent_fps={:.2} decode_ms={:.2} encode_ms={:.2} send_ms={:.2} encoded_mbps={:.2} dropped_before_decode={} decoded_unused={} backpressure_events={} slow_subscriber_skips={} send_errors={}",
        elapsed,
        decoded as f64 / elapsed,
        encoded as f64 / elapsed,
        sent as f64 / elapsed,
        average_ms(delta(stats.mjpeg_decode_time_ns, previous.mjpeg_decode_time_ns), decoded),
        average_ms(delta(stats.encode_time_ns, previous.encode_time_ns), encoded_inputs),
        average_ms(delta(stats.send_time_ns, previous.send_time_ns), sent + send_errors),
        delta(stats.encoded_bytes, previous.encoded_bytes) as f64 * 8.0 / elapsed / 1_000_000.0,
        delta(stats.mjpeg_dropped_before_decode, previous.mjpeg_dropped_before_decode),
        delta(stats.decoded_frames_not_encoded, previous.decoded_frames_not_encoded),
        delta(stats.output_backpressure_events, previous.output_backpressure_events),
        delta(stats.encoded_frames_skipped_for_slow_subscribers, previous.encoded_frames_skipped_for_slow_subscribers),
        send_errors,
    );
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedVideoPipelineStats {
    pub current_fps: f32,
    pub mjpeg_decoded_frames: u64,
    pub mjpeg_dropped_before_decode: u64,
    pub decoded_frames_not_encoded: u64,
    pub mjpeg_decode_time_ns: u64,
    pub encoded_input_frames: u64,
    pub encoded_frames: u64,
    pub encoded_bytes: u64,
    pub encode_time_ns: u64,
    pub output_backpressure_events: u64,
    pub encoded_frames_skipped_for_slow_subscribers: u64,
    pub sent_frames: u64,
    pub send_time_ns: u64,
    pub send_errors: u64,
}

#[derive(Default)]
struct CachedH26xParameterSets {
    h264_sps: Option<Vec<u8>>,
    h264_pps: Option<Vec<u8>>,
    h265_vps: Option<Vec<u8>>,
    h265_sps: Option<Vec<u8>>,
    h265_pps: Option<Vec<u8>>,
}

/// Universal shared video pipeline
pub struct SharedVideoPipeline {
    config: RwLock<SharedVideoPipelineConfig>,
    subscribers: ParkingRwLock<Vec<Arc<VideoSubscriber>>>,
    stats: ParkingMutex<SharedVideoPipelineStats>,
    frame_mailbox: FrameMailboxHandle,
    mjpeg_decoded_frames: AtomicU64,
    mjpeg_dropped_before_decode: AtomicU64,
    decoded_frames_not_encoded: AtomicU64,
    mjpeg_decode_time_ns: AtomicU64,
    encoded_input_frames: AtomicU64,
    encoded_frames: AtomicU64,
    encoded_bytes: AtomicU64,
    encode_time_ns: AtomicU64,
    output_backpressure_events: AtomicU64,
    output_backpressured: AtomicBool,
    encoded_frames_skipped_for_slow_subscribers: AtomicU64,
    delivery_stats: Arc<DeliveryStats>,
    running: watch::Sender<bool>,
    running_rx: watch::Receiver<bool>,
    /// Becomes true only after the synchronous encoder worker has exited and
    /// dropped its encoder handles.
    encoder_done: watch::Sender<bool>,
    encoder_done_rx: watch::Receiver<bool>,
    h264_profile_level_id: watch::Sender<Option<String>>,
    h264_profile_level_id_rx: watch::Receiver<Option<String>>,
    cmd_tx: ParkingRwLock<Option<tokio::sync::mpsc::UnboundedSender<PipelineCmd>>>,
    /// Fast running flag for blocking capture loop
    running_flag: AtomicBool,
    /// Frame sequence counter (atomic for lock-free access)
    sequence: AtomicU64,
    /// Atomic flag for keyframe request (avoids lock contention)
    keyframe_requested: AtomicBool,
    parameter_sets: ParkingMutex<CachedH26xParameterSets>,
    /// Most recent random-access frame with all decoder parameter sets.
    bootstrap_frame: ParkingRwLock<Option<Arc<EncodedVideoFrame>>>,
    /// Pipeline start time for monotonic PTS calculation (microseconds from process start).
    /// Uses AtomicI64 instead of Mutex for lock-free access.
    pipeline_start_time_us: AtomicI64,
    pending_sync_geometry: ParkingMutex<Option<(Resolution, PixelFormat)>>,
    device_lost_reason: ParkingMutex<Option<String>>,
    state_notifier: ParkingRwLock<Option<Arc<dyn Fn(PipelineStateNotification) + Send + Sync>>>,
    last_state_notification: ParkingMutex<Option<PipelineStateNotification>>,
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
        let (encoder_done_tx, encoder_done_rx) = watch::channel(true);
        let (h264_profile_tx, h264_profile_rx) = watch::channel(None);

        let pipeline = Arc::new(Self {
            config: RwLock::new(config),
            subscribers: ParkingRwLock::new(Vec::new()),
            stats: ParkingMutex::new(SharedVideoPipelineStats::default()),
            frame_mailbox: Arc::new(ParkingMutex::new(None)),
            mjpeg_decoded_frames: AtomicU64::new(0),
            mjpeg_dropped_before_decode: AtomicU64::new(0),
            decoded_frames_not_encoded: AtomicU64::new(0),
            mjpeg_decode_time_ns: AtomicU64::new(0),
            encoded_input_frames: AtomicU64::new(0),
            encoded_frames: AtomicU64::new(0),
            encoded_bytes: AtomicU64::new(0),
            encode_time_ns: AtomicU64::new(0),
            output_backpressure_events: AtomicU64::new(0),
            output_backpressured: AtomicBool::new(false),
            encoded_frames_skipped_for_slow_subscribers: AtomicU64::new(0),
            delivery_stats: Arc::new(DeliveryStats::default()),
            running: running_tx,
            running_rx,
            encoder_done: encoder_done_tx,
            encoder_done_rx,
            h264_profile_level_id: h264_profile_tx,
            h264_profile_level_id_rx: h264_profile_rx,
            cmd_tx: ParkingRwLock::new(None),
            running_flag: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            keyframe_requested: AtomicBool::new(false),
            parameter_sets: ParkingMutex::new(CachedH26xParameterSets::default()),
            bootstrap_frame: ParkingRwLock::new(None),
            pipeline_start_time_us: AtomicI64::new(0),
            pending_sync_geometry: ParkingMutex::new(None),
            device_lost_reason: ParkingMutex::new(None),
            state_notifier: ParkingRwLock::new(None),
            last_state_notification: ParkingMutex::new(None),
        });

        Ok(pipeline)
    }

    pub fn take_pending_sync_geometry(&self) -> Option<(Resolution, PixelFormat)> {
        self.pending_sync_geometry.lock().take()
    }

    pub fn take_device_lost_reason(&self) -> Option<String> {
        self.device_lost_reason.lock().take()
    }

    fn mark_device_lost(&self, reason: String) {
        *self.device_lost_reason.lock() = Some(reason);
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
    pub fn subscribe(&self) -> EncodedVideoFrameReceiver {
        let (tx, rx) = mpsc::channel(1);
        if let Some(frame) = self.bootstrap_frame.read().clone() {
            let batch: EncodedBatch = vec![frame].into();
            let _ = tx.try_send(batch);
        }
        self.subscribers.write().push(Arc::new(VideoSubscriber {
            sender: tx,
            needs_keyframe: AtomicBool::new(true),
            recovery_requested: AtomicBool::new(false),
        }));
        if let Some(mailbox) = self.frame_mailbox.lock().as_ref() {
            mailbox.notify();
        }
        EncodedVideoFrameReceiver::new(rx, self.frame_mailbox.clone(), self.delivery_stats.clone())
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.subscribers
            .read()
            .iter()
            .filter(|subscriber| !subscriber.sender.is_closed())
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
            PipelineCmd::SetBitrate { preset } => {
                let bitrate_kbps = preset.bitrate_kbps();
                #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
                if state.ffmpeg_hw_enabled {
                    if let Some(ref mut pipeline) = state.ffmpeg_hw_pipeline {
                        pipeline
                            .reconfigure(bitrate_kbps as i32, preset.gop_size(state.fps) as i32)
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
        self.stats_snapshot()
    }

    fn stats_snapshot(&self) -> SharedVideoPipelineStats {
        let mut stats = self.stats.lock().clone();
        stats.mjpeg_decoded_frames = self.mjpeg_decoded_frames.load(Ordering::Relaxed);
        stats.mjpeg_dropped_before_decode =
            self.mjpeg_dropped_before_decode.load(Ordering::Relaxed);
        stats.decoded_frames_not_encoded = self.decoded_frames_not_encoded.load(Ordering::Relaxed);
        stats.mjpeg_decode_time_ns = self.mjpeg_decode_time_ns.load(Ordering::Relaxed);
        stats.encoded_input_frames = self.encoded_input_frames.load(Ordering::Relaxed);
        stats.encoded_frames = self.encoded_frames.load(Ordering::Relaxed);
        stats.encoded_bytes = self.encoded_bytes.load(Ordering::Relaxed);
        stats.encode_time_ns = self.encode_time_ns.load(Ordering::Relaxed);
        stats.output_backpressure_events = self.output_backpressure_events.load(Ordering::Relaxed);
        stats.encoded_frames_skipped_for_slow_subscribers = self
            .encoded_frames_skipped_for_slow_subscribers
            .load(Ordering::Relaxed);
        stats.sent_frames = self.delivery_stats.sent_frames.load(Ordering::Relaxed);
        stats.send_time_ns = self.delivery_stats.send_time_ns.load(Ordering::Relaxed);
        stats.send_errors = self.delivery_stats.send_errors.load(Ordering::Relaxed);
        stats
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        *self.running_rx.borrow()
    }

    /// Lifecycle state derived from the stop-request flag and the capture
    /// thread's completion signal.  A stopping pipeline must never receive a
    /// new subscriber or be replaced before it releases the V4L2 device.
    pub fn lifecycle(&self) -> PipelineLifecycle {
        if self.running_flag.load(Ordering::Acquire) {
            PipelineLifecycle::Running
        } else if *self.running_rx.borrow() {
            PipelineLifecycle::Stopping
        } else {
            PipelineLifecycle::Stopped
        }
    }

    /// Subscribe to running state changes
    ///
    /// Returns a watch receiver that can be used to detect when the pipeline stops.
    /// This is useful for auto-cleanup when the pipeline auto-stops due to no subscribers.
    pub fn running_watch(&self) -> watch::Receiver<bool> {
        self.running_rx.clone()
    }

    pub fn h264_profile_level_id_watch(&self) -> watch::Receiver<Option<String>> {
        self.h264_profile_level_id_rx.clone()
    }

    fn update_h264_profile_level_id(&self, sps: &[u8]) {
        let Some(profile_level_id) = h264_bitstream::parse_profile_level_id_from_sps(sps) else {
            return;
        };
        if self.h264_profile_level_id.borrow().as_deref() == Some(profile_level_id.as_str()) {
            return;
        }
        let _ = self.h264_profile_level_id.send(Some(profile_level_id));
    }

    fn inspect_and_parameterize_packet(
        &self,
        codec: VideoEncoderType,
        data: Bytes,
        ffmpeg_keyframe: bool,
    ) -> (Bytes, bool) {
        match codec {
            VideoEncoderType::H264 => {
                let was_annex_b = h264_bitstream::is_annex_b(data.as_ref());
                let data = h264_bitstream::normalize_annex_b(data);
                if !was_annex_b && h264_bitstream::is_annex_b(data.as_ref()) {
                    debug!("[Pipeline] Converted length-prefixed H264 packet to Annex-B");
                }
                let inspection = h264_bitstream::inspect_annex_b(data.as_ref());
                let sps = inspection.sps;
                let pps = inspection.pps;
                // Require metadata and payload to agree before advertising a
                // decoder bootstrap frame.
                let is_idr = ffmpeg_keyframe && inspection.is_idr;
                let mut cache = self.parameter_sets.lock();
                if let Some(sps) = sps {
                    self.update_h264_profile_level_id(sps);
                    cache.h264_sps = Some(sps.to_vec());
                }
                if let Some(pps) = pps {
                    cache.h264_pps = Some(pps.to_vec());
                }

                if !is_idr {
                    return (data, false);
                }
                if sps.is_some() && pps.is_some() {
                    return (data, true);
                }

                match (&cache.h264_sps, &cache.h264_pps) {
                    (Some(cached_sps), Some(cached_pps)) => {
                        let mut output = Vec::with_capacity(
                            data.len() + cached_sps.len() + cached_pps.len() + 8,
                        );
                        output.extend_from_slice(&[0, 0, 0, 1]);
                        output.extend_from_slice(cached_sps);
                        output.extend_from_slice(&[0, 0, 0, 1]);
                        output.extend_from_slice(cached_pps);
                        output.extend_from_slice(data.as_ref());
                        debug!("[Pipeline] Prepended cached SPS/PPS to H264 IDR");
                        (Bytes::from(output), true)
                    }
                    // An IDR without SPS/PPS is not a decoder bootstrap frame.
                    _ => (data, false),
                }
            }
            VideoEncoderType::H265 => {
                let (vps, sps, pps) = h265_bitstream::extract_vps_sps_pps(data.as_ref());
                let is_irap = ffmpeg_keyframe && h265_bitstream::is_keyframe(data.as_ref());
                let mut cache = self.parameter_sets.lock();
                if let Some(vps) = vps.as_ref() {
                    cache.h265_vps = Some(vps.clone());
                }
                if let Some(sps) = sps.as_ref() {
                    cache.h265_sps = Some(sps.clone());
                }
                if let Some(pps) = pps.as_ref() {
                    cache.h265_pps = Some(pps.clone());
                }

                if !is_irap {
                    return (data, false);
                }
                if vps.is_some() && sps.is_some() && pps.is_some() {
                    return (data, true);
                }

                match (&cache.h265_vps, &cache.h265_sps, &cache.h265_pps) {
                    (Some(cached_vps), Some(cached_sps), Some(cached_pps)) => {
                        let mut output = Vec::with_capacity(
                            data.len()
                                + cached_vps.len()
                                + cached_sps.len()
                                + cached_pps.len()
                                + 12,
                        );
                        for parameter_set in [cached_vps, cached_sps, cached_pps] {
                            output.extend_from_slice(&[0, 0, 0, 1]);
                            output.extend_from_slice(parameter_set);
                        }
                        output.extend_from_slice(data.as_ref());
                        debug!("[Pipeline] Prepended cached VPS/SPS/PPS to H265 IRAP");
                        (Bytes::from(output), true)
                    }
                    _ => (data, false),
                }
            }
            _ => (data, ffmpeg_keyframe),
        }
    }

    fn reserve_outputs(&self) -> Option<Vec<VideoFrameReservation>> {
        let subscribers = self.subscribers.read();
        let mut active_subscribers = 0;
        let mut reservations = Vec::with_capacity(subscribers.len());
        for subscriber in subscribers.iter() {
            if subscriber.sender.is_closed() {
                continue;
            }
            active_subscribers += 1;
            if let Ok(permit) = subscriber.sender.clone().try_reserve_owned() {
                if subscriber.needs_keyframe.load(Ordering::Acquire)
                    && !subscriber.recovery_requested.swap(true, Ordering::AcqRel)
                {
                    self.keyframe_requested.store(true, Ordering::Release);
                }
                reservations.push(VideoFrameReservation {
                    subscriber: subscriber.clone(),
                    permit,
                });
            }
        }
        if active_subscribers > 0 && reservations.is_empty() {
            if !self.output_backpressured.swap(true, Ordering::AcqRel) {
                self.output_backpressure_events
                    .fetch_add(1, Ordering::Relaxed);
            }
            None
        } else {
            self.output_backpressured.store(false, Ordering::Release);
            Some(reservations)
        }
    }

    fn broadcast_encoded_batch(
        &self,
        frames: EncodedBatch,
        reservations: Vec<VideoFrameReservation>,
    ) {
        if frames.is_empty() {
            return;
        }
        self.encoded_frames
            .fetch_add(frames.len() as u64, Ordering::Relaxed);
        self.encoded_bytes.fetch_add(
            frames.iter().map(|frame| frame.data.len() as u64).sum(),
            Ordering::Relaxed,
        );
        if let Some(frame) = frames.iter().rev().find(|frame| frame.is_keyframe) {
            *self.bootstrap_frame.write() = Some(frame.clone());
        }
        {
            let mut subscribers = self.subscribers.write();
            subscribers.retain(|subscriber| !subscriber.sender.is_closed());
            for subscriber in subscribers.iter() {
                if !reservations
                    .iter()
                    .any(|reservation| Arc::ptr_eq(&reservation.subscriber, subscriber))
                {
                    subscriber.needs_keyframe.store(true, Ordering::Release);
                    subscriber
                        .recovery_requested
                        .store(false, Ordering::Release);
                    self.encoded_frames_skipped_for_slow_subscribers
                        .fetch_add(frames.len() as u64, Ordering::Relaxed);
                }
            }
        }
        for reservation in reservations {
            let batch = if reservation
                .subscriber
                .needs_keyframe
                .load(Ordering::Acquire)
            {
                let Some(first_keyframe) = frames.iter().position(|frame| frame.is_keyframe) else {
                    self.encoded_frames_skipped_for_slow_subscribers
                        .fetch_add(frames.len() as u64, Ordering::Relaxed);
                    continue;
                };
                reservation
                    .subscriber
                    .needs_keyframe
                    .store(false, Ordering::Release);
                reservation
                    .subscriber
                    .recovery_requested
                    .store(false, Ordering::Release);
                self.encoded_frames_skipped_for_slow_subscribers
                    .fetch_add(first_keyframe as u64, Ordering::Relaxed);
                if first_keyframe == 0 {
                    frames.clone()
                } else {
                    frames[first_keyframe..].to_vec().into()
                }
            } else {
                frames.clone()
            };
            reservation.permit.send(batch);
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
    ) -> Result<()> {
        if *self.running_rx.borrow() {
            warn!("Pipeline already running");
            return Ok(());
        }

        *self.parameter_sets.lock() = CachedH26xParameterSets::default();
        *self.bootstrap_frame.write() = None;

        let mut config = self.config.read().await.clone();
        let parallel_mjpeg_decode = should_parallel_decode_mjpeg(&config);
        {
            let mut last = self.last_state_notification.lock();
            *last = None;
        }

        // Pre-open for DV negotiation; align encoder to probed size.
        let bridge_ctx_probe = BridgeContext::from_parts(
            subdev_path.clone(),
            parse_bridge_kind(bridge_kind.as_deref()),
        );
        #[allow(unused_mut)]
        let mut preopened: Option<CaptureStream> = match open_capture_stream(
            &device_path,
            config.resolution,
            config.input_format,
            config.fps,
            buffer_count.max(1),
            Duration::from_secs(2),
            bridge_ctx_probe,
            config.control_mode,
        ) {
            Ok(s) => {
                let negotiated_res = s.resolution();
                let negotiated_fmt = s.format();
                let previous = (config.resolution, config.input_format, config.fps);
                config.align_source_fps(s.source_fps());
                config.resolution = negotiated_res;
                config.input_format = negotiated_fmt;
                if previous != (config.resolution, config.input_format, config.fps) {
                    info!(
                            "Negotiated capture {}x{} {:?} @ {} fps (configured {}x{} {:?} @ {} fps) — aligning encoder to source",
                            negotiated_res.width,
                            negotiated_res.height,
                            negotiated_fmt,
                            config.fps,
                            previous.0.width,
                            previous.0.height,
                            previous.1,
                            previous.2,
                        );
                }
                *self.config.write().await = config.clone();
                Some(s)
            }
            Err(AppError::CaptureNoSignal { kind }) => {
                debug!(
                    "Pre-probe: no signal — encoder uses configured geometry until capture opens"
                );
                let status = signal_status_from_capture_kind(&kind);
                self.notify_state(PipelineStateNotification::no_signal(
                    status,
                    Some(
                        CaptureRecoveryPolicy::new(config.control_mode)
                            .retry_delay(1)
                            .as_millis() as u64,
                    ),
                ));
                None
            }
            Err(e) => return Err(e),
        };

        #[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
        if dmabuf::eligible(&config) {
            if let Some(stream) = preopened.as_ref().filter(|s| s.supports_rkmpp_dmabuf()) {
                match dmabuf::prepare(stream, &config) {
                    Ok(encoder) => {
                        return dmabuf::start(
                            self.clone(),
                            preopened.take().expect("preopened DMA capture"),
                            encoder,
                            config,
                            device_path,
                            buffer_count,
                            BridgeContext::from_parts(
                                subdev_path,
                                parse_bridge_kind(bridge_kind.as_deref()),
                            ),
                        );
                    }
                    Err(error) => warn!(
                        "RKMPP DMA unavailable; using existing copy pipeline: {}",
                        error
                    ),
                }
            }
        }

        let mut encoder_config = config.clone();
        if parallel_mjpeg_decode {
            encoder_config.input_format = PixelFormat::Nv12;
            info!("Using parallel libyuv MJPEG decode with hardware encoding");
        }
        let mut encoder_state = build_encoder_state(&encoder_config)?;
        let frame_mailbox = Arc::new(VideoFrameMailbox::new());
        *self.frame_mailbox.lock() = Some(frame_mailbox.clone());
        self.mjpeg_decoded_frames.store(0, Ordering::Relaxed);
        self.mjpeg_dropped_before_decode.store(0, Ordering::Relaxed);
        self.decoded_frames_not_encoded.store(0, Ordering::Relaxed);
        self.mjpeg_decode_time_ns.store(0, Ordering::Relaxed);
        self.encoded_input_frames.store(0, Ordering::Relaxed);
        self.encoded_frames.store(0, Ordering::Relaxed);
        self.encoded_bytes.store(0, Ordering::Relaxed);
        self.encode_time_ns.store(0, Ordering::Relaxed);
        self.output_backpressure_events.store(0, Ordering::Relaxed);
        self.output_backpressured.store(false, Ordering::Release);
        self.encoded_frames_skipped_for_slow_subscribers
            .store(0, Ordering::Relaxed);
        self.delivery_stats.sent_frames.store(0, Ordering::Relaxed);
        self.delivery_stats.send_time_ns.store(0, Ordering::Relaxed);
        self.delivery_stats.send_errors.store(0, Ordering::Relaxed);
        for subscriber in self.subscribers.read().iter() {
            subscriber.needs_keyframe.store(true, Ordering::Release);
            subscriber
                .recovery_requested
                .store(false, Ordering::Release);
        }
        let _ = self.running.send(true);
        let _ = self.encoder_done.send(false);
        self.running_flag.store(true, Ordering::Release);

        let pipeline = self.clone();
        let buffer_pool = Arc::new(FrameBufferPool::new(buffer_count.max(4) as usize));
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        {
            let mut guard = self.cmd_tx.write();
            *guard = Some(cmd_tx);
        }

        // Encoder loop uses a dedicated OS thread because FFmpeg work is synchronous.
        {
            let pipeline = pipeline.clone();
            let frame_mailbox = frame_mailbox.clone();
            let encoder_worker = spawn_video_worker("video-encoder", move || {
                let mut input_frame_count: u64 = 0;
                let mut encoded_frame_count: u64 = 0;
                let mut last_fps_time = Instant::now();
                let mut fps_frame_count: u64 = 0;
                let perf_logging = std::env::var("ONE_KVM_VIDEO_STATS").as_deref() == Ok("1");
                let mut last_perf_time = Instant::now();
                let mut last_perf_stats = pipeline.stats_snapshot();
                let encode_error_throttler = LogThrottler::with_secs(ENCODE_ERROR_THROTTLE_SECS);
                let mut suppressed_encode_errors: HashMap<String, u64> = HashMap::new();

                while pipeline.running_flag.load(Ordering::Acquire) {
                    let Some((frame, reservations)) =
                        frame_mailbox.receive_when(|| pipeline.reserve_outputs())
                    else {
                        break;
                    };
                    if !pipeline.running_flag.load(Ordering::Acquire) {
                        break;
                    }

                    if reservations.is_empty() {
                        if parallel_mjpeg_decode {
                            pipeline
                                .decoded_frames_not_encoded
                                .fetch_add(1, Ordering::Relaxed);
                        }
                        continue;
                    }

                    while let Ok(cmd) = cmd_rx.try_recv() {
                        if let Err(e) = pipeline.apply_cmd(&mut encoder_state, cmd) {
                            error!("Failed to apply pipeline command: {}", e);
                        }
                    }

                    input_frame_count = input_frame_count.wrapping_add(1);
                    pipeline
                        .encoded_input_frames
                        .fetch_add(1, Ordering::Relaxed);
                    let encode_started = Instant::now();
                    let encode_result = pipeline.encode_frame_sync(&mut encoder_state, &frame);
                    pipeline.encode_time_ns.fetch_add(
                        encode_started.elapsed().as_nanos() as u64,
                        Ordering::Relaxed,
                    );

                    match encode_result {
                        Ok(encoded_frames) => {
                            encoded_frame_count =
                                encoded_frame_count.wrapping_add(encoded_frames.len() as u64);
                            fps_frame_count += encoded_frames.len() as u64;
                            let frames: EncodedBatch = encoded_frames
                                .into_iter()
                                .map(Arc::new)
                                .collect::<Vec<_>>()
                                .into();
                            pipeline.broadcast_encoded_batch(frames, reservations);
                        }
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

                        pipeline.stats.lock().current_fps = current_fps;
                        trace!(
                            "Shared pipeline processed {} input frames, emitted {} encoded frames; MJPEG decoded={}, dropped before decode={}, decoded but not encoded={}",
                            input_frame_count,
                            encoded_frame_count,
                            pipeline.mjpeg_decoded_frames.load(Ordering::Relaxed),
                            pipeline.mjpeg_dropped_before_decode.load(Ordering::Relaxed),
                            pipeline.decoded_frames_not_encoded.load(Ordering::Relaxed),
                        );
                    }

                    if perf_logging && last_perf_time.elapsed() >= Duration::from_secs(5) {
                        let elapsed = last_perf_time.elapsed().as_secs_f64();
                        let stats = pipeline.stats_snapshot();
                        log_pipeline_performance(&stats, &last_perf_stats, elapsed);
                        last_perf_stats = stats;
                        last_perf_time = Instant::now();
                    }
                }

                pipeline.clear_cmd_tx();
                // Release encoder resources before allowing a replacement pipeline.
                drop(encoder_state);
                let _ = pipeline.encoder_done.send(true);
            });
            if let Err(error) = encoder_worker {
                self.stop();
                let _ = self.running.send(false);
                let _ = self.encoder_done.send(true);
                return Err(AppError::VideoError(format!(
                    "Failed to start encoder worker: {error}"
                )));
            }
        }

        // Capture loop (runs on thread, updates latest frame)
        {
            let pipeline = pipeline.clone();
            let frame_mailbox = frame_mailbox.clone();
            let buffer_pool = buffer_pool.clone();
            let bridge_ctx =
                BridgeContext::from_parts(subdev_path, parse_bridge_kind(bridge_kind.as_deref()));
            let capture_worker = spawn_video_worker("video-capture", move || {
                let mut stream: Option<CaptureStream> = None;
                let mut initial_geometry: Option<(Resolution, PixelFormat)> = None;
                let mut resolution = config.resolution;
                let mut pixel_format = config.input_format;
                let mut active_fps = config.fps;
                let mut stride: u32 = 0;
                let mut mjpeg_decode_senders = parallel_mjpeg_decode
                    .then(|| {
                        spawn_mjpeg_decode_workers(
                            &pipeline,
                            &frame_mailbox,
                            &buffer_pool,
                            config.resolution,
                        )
                    })
                    .filter(|senders| !senders.is_empty());
                let mut next_mjpeg_decoder = 0usize;
                let mut mjpeg_decoder = parallel_mjpeg_decode
                    .then(|| MjpegToNv12Decoder::new(config.resolution))
                    .filter(|_| {
                        mjpeg_decode_senders
                            .as_ref()
                            .is_none_or(|senders| senders.is_empty())
                    });

                if let Some(s) = preopened {
                    resolution = s.resolution();
                    pixel_format = s.format();
                    active_fps = s
                        .source_fps()
                        .map(|fps| fps.round().clamp(1.0, 120.0) as u32)
                        .unwrap_or(config.fps);
                    stride = s.stride();
                    initial_geometry = Some((resolution, pixel_format));
                    stream = Some(s);
                }

                fn open_or_retry(
                    device_path: &std::path::Path,
                    config: &SharedVideoPipelineConfig,
                    buffer_count: u32,
                    bridge_ctx: BridgeContext,
                ) -> CaptureOpenResult {
                    match open_capture_stream_for_retry(
                        device_path,
                        config.resolution,
                        config.input_format,
                        config.fps,
                        buffer_count.max(1),
                        Duration::from_secs(2),
                        bridge_ctx,
                        config.control_mode,
                        is_device_lost_message,
                    ) {
                        CaptureOpenResult::NoSignal(status) => {
                            debug!("Capture soft-restart: still no signal ({:?})", status);
                            CaptureOpenResult::NoSignal(status)
                        }
                        CaptureOpenResult::DeviceLost(reason) => {
                            error!("Capture device lost during soft-restart: {}", reason);
                            CaptureOpenResult::DeviceLost(reason)
                        }
                        CaptureOpenResult::Fatal => {
                            error!("Capture soft-restart failed");
                            CaptureOpenResult::Fatal
                        }
                        opened => opened,
                    }
                }

                let mut no_subscribers_since: Option<Instant> = None;
                let mut capture_sequence = 0u64;
                let grace_period = Duration::from_secs(AUTO_STOP_GRACE_PERIOD_SECS);
                let mut consecutive_timeouts: u32 = 0;
                let recovery_policy = CaptureRecoveryPolicy::new(config.control_mode);
                let capture_error_throttler = LogThrottler::with_secs(5);
                let mut suppressed_capture_errors: HashMap<String, u64> = HashMap::new();

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
                                pipeline.running_flag.store(false, Ordering::Release);
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
                            CaptureOpenResult::Opened(new_stream) => {
                                let new_res = new_stream.resolution();
                                let new_fmt = new_stream.format();
                                let new_stride = new_stream.stride();
                                let new_fps = new_stream
                                    .source_fps()
                                    .map(|fps| fps.round().clamp(1.0, 120.0) as u32)
                                    .unwrap_or(config.fps);

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
                                    pipeline.notify_state(PipelineStateNotification::no_signal(
                                        SignalStatus::NoSignal,
                                        Some(recovery_policy.retry_delay(1).as_millis() as u64),
                                    ));
                                    *pipeline.pending_sync_geometry.lock() =
                                        Some((new_res, new_fmt));
                                    pipeline.running_flag.store(false, Ordering::Release);
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
                                            PipelineStateNotification::no_signal(
                                                SignalStatus::NoSignal,
                                                Some(recovery_policy.retry_delay(1).as_millis()
                                                    as u64),
                                            ),
                                        );
                                        *pipeline.pending_sync_geometry.lock() =
                                            Some((new_res, new_fmt));
                                        pipeline.running_flag.store(false, Ordering::Release);
                                        break;
                                    }
                                    _ => {}
                                }

                                if initial_geometry.is_none() {
                                    initial_geometry = Some((new_res, new_fmt));
                                }
                                resolution = new_res;
                                pixel_format = new_fmt;
                                active_fps = new_fps;
                                stride = new_stride;
                                stream = Some(new_stream);
                                consecutive_timeouts = 0;
                                info!(
                                    "Capture stream (re)opened: {}x{} {:?} stride={}",
                                    resolution.width, resolution.height, pixel_format, stride
                                );
                            }
                            CaptureOpenResult::NoSignal(status) => {
                                consecutive_timeouts = consecutive_timeouts.saturating_add(1);
                                if !recovery_policy.should_retry(consecutive_timeouts) {
                                    warn!(
                                        "Capture soft-restart gave up after {} attempts, \
                                         stopping pipeline",
                                        consecutive_timeouts
                                    );
                                    pipeline.running_flag.store(false, Ordering::Release);
                                    break;
                                }
                                let delay = recovery_policy.retry_delay(consecutive_timeouts);
                                pipeline.notify_state(PipelineStateNotification::no_signal(
                                    status,
                                    Some(delay.as_millis() as u64),
                                ));
                                if wait_for_source_change(&bridge_ctx, delay, || {
                                    pipeline.running_flag.load(Ordering::Acquire)
                                }) {
                                    info!("SOURCE_CHANGE woke capture retry");
                                }
                                continue;
                            }
                            CaptureOpenResult::DeviceLost(reason) => {
                                pipeline.mark_device_lost(reason);
                                pipeline.running_flag.store(false, Ordering::Release);
                                break;
                            }
                            CaptureOpenResult::Fatal => {
                                pipeline.running_flag.store(false, Ordering::Release);
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
                        Err(CaptureReadError::SourceChanged) => {
                            // V4L2 driver reported V4L2_EVENT_SOURCE_CHANGE.
                            // The current capture is effectively invalidated:
                            // drop the stream so the next iteration re-opens
                            // via a fresh DV_TIMINGS probe.  This is the fast
                            // path for source-side resolution switches on
                            // RK628 / rkcif; the retry policy is only a fallback
                            // when a driver does not provide usable events.
                            info!(
                                "Capture reported SOURCE_CHANGE — \
                                 dropping stream for immediate re-open"
                            );
                            if recovery_policy.control_mode() == VideoControlMode::SourceFollowing {
                                pipeline.notify_state(PipelineStateNotification::no_signal(
                                    SignalStatus::NoSignal,
                                    Some(recovery_policy.retry_delay(1).as_millis() as u64),
                                ));
                            }
                            consecutive_timeouts = 0;
                            stream = None;
                            continue;
                        }
                        Err(CaptureReadError::Io(e)) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                continue;
                            }
                            if e.kind() == std::io::ErrorKind::TimedOut {
                                consecutive_timeouts = consecutive_timeouts.saturating_add(1);
                                if recovery_policy.control_mode()
                                    == VideoControlMode::SourceFollowing
                                {
                                    let delay = recovery_policy.retry_delay(consecutive_timeouts);
                                    pipeline.notify_state(PipelineStateNotification::no_signal(
                                        SignalStatus::NoSignal,
                                        Some(delay.as_millis() as u64),
                                    ));
                                    stream = None;
                                    continue;
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
                            } else {
                                consecutive_timeouts = 0;
                                // EIO (5) / EPIPE (32) / EPROTO (71) in next_into generally
                                // mean the source or UVC USB transport glitched mid-stream.
                                // Tear down the stream and let the open loop re-probe.
                                match classify_capture_io_error(&e) {
                                    CaptureIoErrorKind::TransientSignal { status } => {
                                        if status == Some(SignalStatus::UvcUsbError) {
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
                                    CaptureIoErrorKind::DeviceLost => {
                                        error!("Capture device lost: {}", e);
                                        pipeline.mark_device_lost(e.to_string());
                                        pipeline.running_flag.store(false, Ordering::Release);
                                        break;
                                    }
                                    CaptureIoErrorKind::Other => {}
                                }
                                let key = capture_error_log_key(&e);
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

                    owned.truncate(frame_size);
                    capture_sequence = capture_sequence.wrapping_add(1);

                    if parallel_mjpeg_decode
                        && pixel_format == PixelFormat::Mjpeg
                        && frame_mailbox.has_pending()
                    {
                        pipeline
                            .mjpeg_dropped_before_decode
                            .fetch_add(1, Ordering::Relaxed);
                        buffer_pool.put(owned);
                        continue;
                    }

                    // Notify streaming only after the short-frame guard passes.
                    pipeline.notify_state(PipelineStateNotification::streaming(
                        resolution,
                        pixel_format,
                        active_fps,
                    ));

                    if let Some(senders) = mjpeg_decode_senders.as_mut() {
                        let mut pending = Some(MjpegDecodeJob {
                            data: owned,
                            sequence: capture_sequence,
                        });
                        for offset in 0..senders.len() {
                            let index = (next_mjpeg_decoder + offset) % senders.len();
                            let job = pending.take().expect("pending MJPEG decode job");
                            match senders[index].try_send(job) {
                                Ok(()) => {
                                    next_mjpeg_decoder = (index + 1) % senders.len();
                                    break;
                                }
                                Err(TrySendError::Full(job))
                                | Err(TrySendError::Disconnected(job)) => {
                                    pending = Some(job);
                                }
                            }
                        }
                        if let Some(job) = pending {
                            pipeline
                                .mjpeg_dropped_before_decode
                                .fetch_add(1, Ordering::Relaxed);
                            buffer_pool.put(job.data);
                        }
                        continue;
                    }

                    let (frame_data, frame_format, frame_stride) =
                        if let Some(decoder) = mjpeg_decoder.as_mut() {
                            let nv12_size =
                                resolution.width as usize * resolution.height as usize * 3 / 2;
                            let mut nv12 = buffer_pool.take(nv12_size);
                            let decode_started = Instant::now();
                            if let Err(error) = decoder.decode_into(&owned, &mut nv12) {
                                buffer_pool.put(owned);
                                buffer_pool.put(nv12);
                                let key = "capture_mjpeg_decode";
                                if capture_error_throttler.should_log(key) {
                                    error!("Dropping undecodable MJPEG frame: {}", error);
                                }
                                continue;
                            }
                            buffer_pool.put(owned);
                            pipeline.mjpeg_decode_time_ns.fetch_add(
                                decode_started.elapsed().as_nanos() as u64,
                                Ordering::Relaxed,
                            );
                            pipeline
                                .mjpeg_decoded_frames
                                .fetch_add(1, Ordering::Relaxed);
                            (nv12, PixelFormat::Nv12, resolution.width)
                        } else {
                            (owned, pixel_format, stride)
                        };
                    let frame = Arc::new(VideoFrame::from_pooled(
                        Arc::new(FrameBuffer::new(frame_data, Some(buffer_pool.clone()))),
                        resolution,
                        frame_format,
                        frame_stride,
                        capture_sequence,
                    ));
                    if frame_mailbox.publish(capture_sequence, frame) != PublishResult::Published
                        && mjpeg_decoder.is_some()
                    {
                        pipeline
                            .decoded_frames_not_encoded
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }

                // `running` represents completed lifecycle state, not a stop request.
                // Drop the V4L2 stream first so STREAMOFF, buffer teardown and FD close
                // have all completed before another consumer is told the device is free.
                drop(stream);
                pipeline.running_flag.store(false, Ordering::Release);
                frame_mailbox.close();
                let _ = pipeline.running.send(false);
                info!("Video pipeline stopped and capture device released");
            });
            if let Err(error) = capture_worker {
                self.stop();
                let _ = self.running.send(false);
                return Err(AppError::VideoError(format!(
                    "Failed to start capture worker: {error}"
                )));
            }
        }

        Ok(())
    }

    /// Encode a single frame (synchronous, no async locks)
    fn encode_frame_sync(
        &self,
        state: &mut EncoderThreadState,
        frame: &VideoFrame,
    ) -> Result<Vec<EncodedVideoFrame>> {
        let fps = state.fps;
        let codec = state.codec;
        let input_format = state.input_format;
        let raw_frame = frame.data();

        let pts_ms = self.pts_ms();

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
                let (data, is_keyframe) =
                    self.inspect_and_parameterize_packet(codec, Bytes::from(data), is_keyframe);
                let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
                return Ok(vec![EncodedVideoFrame {
                    data,
                    pts_ms,
                    is_keyframe,
                    sequence,
                    duration: Duration::from_millis(1000 / fps as u64),
                    codec,
                }]);
            }

            return Ok(Vec::new());
        }

        let decoded_buf = if input_format.is_compressed() {
            let decoder = state
                .mjpeg_decoder
                .as_mut()
                .ok_or_else(|| AppError::VideoError("MJPEG decoder not initialized".to_string()))?;
            let decoded = match decoder.decode(raw_frame) {
                Ok(decoded) => decoded,
                Err(err) => {
                    warn!("Dropping undecodable MJPEG frame before encode: {}", err);
                    return Ok(Vec::new());
                }
            };
            Some(decoded)
        } else {
            None
        };
        let compacted_buf = if decoded_buf.is_none() {
            compact_strided_frame_for_encoder(frame, raw_frame)?
        } else {
            None
        };
        let raw_frame = decoded_buf
            .as_deref()
            .or(compacted_buf.as_deref())
            .unwrap_or(raw_frame);

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
                if frames.is_empty() {
                    trace!("Encoder returned no frame ({})", codec);
                    return Ok(Vec::new());
                }

                let mut encoded_frames = Vec::with_capacity(frames.len());
                for encoded in frames {
                    let (data, is_keyframe) =
                        self.inspect_and_parameterize_packet(codec, encoded.data, encoded.key == 1);
                    let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    encoded_frames.push(EncodedVideoFrame {
                        data,
                        pts_ms: encoded.pts,
                        is_keyframe,
                        sequence,
                        duration: Duration::from_millis(1000 / fps as u64),
                        codec,
                    });
                }

                Ok(encoded_frames)
            }
            Err(e) => Err(e),
        }
    }

    fn pts_ms(&self) -> i64 {
        let current_ts_us = PROCESS_START
            .get_or_init(Instant::now)
            .elapsed()
            .as_micros() as i64;
        let start_ts_us = self.pipeline_start_time_us.load(Ordering::Acquire);
        let start_ts_us = if start_ts_us == 0 {
            match self.pipeline_start_time_us.compare_exchange(
                0,
                current_ts_us,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => current_ts_us,
                Err(existing) => existing,
            }
        } else {
            start_ts_us
        };
        current_ts_us.saturating_sub(start_ts_us) / 1000
    }

    /// Stop the pipeline (non-blocking, does not wait for capture thread to exit)
    pub fn stop(&self) {
        let was_running = self.running_flag.swap(false, Ordering::AcqRel);
        if let Some(frame_mailbox) = self.frame_mailbox.lock().as_ref() {
            frame_mailbox.close();
        }
        if was_running {
            self.clear_cmd_tx();
            info!("Stopping video pipeline");
        }
    }

    /// Stop the pipeline and wait for the capture thread to fully exit.
    ///
    /// This ensures the V4L2 device is released before returning, which is
    /// necessary when another consumer (e.g. MJPEG streamer) needs to open
    /// the same device immediately after.
    pub async fn stop_and_wait(&self, timeout: std::time::Duration) -> Result<()> {
        self.stop();
        let mut rx = self.running_watch();
        let mut encoder_rx = self.encoder_done_rx.clone();
        let deadline = tokio::time::Instant::now() + timeout;

        while *rx.borrow() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(AppError::VideoError(format!(
                    "Timed out waiting {:?} for video pipeline to release capture device",
                    timeout
                )));
            }
            match tokio::time::timeout(remaining, rx.changed()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) if !*rx.borrow() => break,
                Ok(Err(_)) => {
                    return Err(AppError::VideoError(
                        "Video pipeline lifecycle channel closed before capture device release"
                            .to_string(),
                    ));
                }
                Err(_) => {
                    return Err(AppError::VideoError(format!(
                        "Timed out waiting {:?} for video pipeline to release capture device",
                        timeout
                    )));
                }
            }
        }

        while !*encoder_rx.borrow() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(AppError::VideoError(format!(
                    "Timed out waiting {:?} for video encoder to release vendor session",
                    timeout
                )));
            }
            match tokio::time::timeout(remaining, encoder_rx.changed()).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) if *encoder_rx.borrow() => break,
                Ok(Err(_)) => {
                    return Err(AppError::VideoError(
                        "Video encoder lifecycle channel closed before vendor session release"
                            .to_string(),
                    ));
                }
                Err(_) => {
                    return Err(AppError::VideoError(format!(
                        "Timed out waiting {:?} for video encoder to release vendor session",
                        timeout
                    )));
                }
            }
        }

        Ok(())
    }

    /// Set bitrate using preset
    pub async fn set_bitrate_preset(
        &self,
        preset: crate::video::codec::BitratePreset,
    ) -> Result<()> {
        {
            let mut config = self.config.write().await;
            config.bitrate_preset = preset;
        }
        self.send_cmd(PipelineCmd::SetBitrate { preset });
        Ok(())
    }

    /// Set bitrate using raw kbps value (converts to appropriate preset)
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        let preset = crate::video::codec::BitratePreset::from_kbps(bitrate_kbps);
        self.set_bitrate_preset(preset).await
    }

    /// Get current config
    pub async fn config(&self) -> SharedVideoPipelineConfig {
        self.config.read().await.clone()
    }
}

fn compact_strided_frame_for_encoder(frame: &VideoFrame, data: &[u8]) -> Result<Option<Vec<u8>>> {
    let width = frame.resolution.width as usize;
    let height = frame.resolution.height as usize;
    let stride = frame.stride as usize;
    if width == 0 || height == 0 || stride == 0 || frame.format.is_compressed() {
        return Ok(None);
    }

    let compact_size = match frame.format {
        PixelFormat::Nv12 | PixelFormat::Nv21 | PixelFormat::Yuv420 | PixelFormat::Yvu420 => {
            width * height * 3 / 2
        }
        PixelFormat::Nv16 | PixelFormat::Yuyv | PixelFormat::Yvyu | PixelFormat::Uyvy => {
            width * height * 2
        }
        PixelFormat::Nv24 | PixelFormat::Rgb24 | PixelFormat::Bgr24 => width * height * 3,
        PixelFormat::Rgb565 => width * height * 2,
        PixelFormat::Grey => width * height,
        PixelFormat::Mjpeg | PixelFormat::Jpeg => return Ok(None),
    };

    if data.len() == compact_size {
        return Ok(None);
    }

    let mut out = vec![0u8; compact_size];
    match frame.format {
        PixelFormat::Nv12 | PixelFormat::Nv21 => {
            let src_y_size = stride * height;
            let src_uv_size = stride * height / 2;
            require_len(data, src_y_size + src_uv_size, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, width, width, height);
            copy_rows(
                data,
                src_y_size,
                stride,
                &mut out,
                width * height,
                width,
                width,
                height / 2,
            );
        }
        PixelFormat::Yuv420 | PixelFormat::Yvu420 => {
            let src_y_size = stride * height;
            let src_chroma_stride = stride / 2;
            let src_chroma_size = src_chroma_stride * height / 2;
            let dst_y_size = width * height;
            let dst_chroma_stride = width / 2;
            let dst_chroma_size = dst_chroma_stride * height / 2;
            require_len(data, src_y_size + src_chroma_size * 2, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, width, width, height);
            copy_rows(
                data,
                src_y_size,
                src_chroma_stride,
                &mut out,
                dst_y_size,
                dst_chroma_stride,
                dst_chroma_stride,
                height / 2,
            );
            copy_rows(
                data,
                src_y_size + src_chroma_size,
                src_chroma_stride,
                &mut out,
                dst_y_size + dst_chroma_size,
                dst_chroma_stride,
                dst_chroma_stride,
                height / 2,
            );
        }
        PixelFormat::Nv16 => {
            let src_y_size = stride * height;
            require_len(data, src_y_size + stride * height, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, width, width, height);
            copy_rows(
                data,
                src_y_size,
                stride,
                &mut out,
                width * height,
                width,
                width,
                height,
            );
        }
        PixelFormat::Nv24 => {
            let src_y_size = stride * height;
            let src_uv_stride = stride * 2;
            require_len(
                data,
                src_y_size + src_uv_stride * height,
                frame.format,
                stride,
            )?;
            copy_rows(data, 0, stride, &mut out, 0, width, width, height);
            copy_rows(
                data,
                src_y_size,
                src_uv_stride,
                &mut out,
                width * height,
                width * 2,
                width * 2,
                height,
            );
        }
        PixelFormat::Yuyv | PixelFormat::Yvyu | PixelFormat::Uyvy | PixelFormat::Rgb565 => {
            let row_bytes = width * 2;
            require_len(data, stride * height, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, row_bytes, row_bytes, height);
        }
        PixelFormat::Rgb24 | PixelFormat::Bgr24 => {
            let row_bytes = width * 3;
            require_len(data, stride * height, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, row_bytes, row_bytes, height);
        }
        PixelFormat::Grey => {
            require_len(data, stride * height, frame.format, stride)?;
            copy_rows(data, 0, stride, &mut out, 0, width, width, height);
        }
        PixelFormat::Mjpeg | PixelFormat::Jpeg => return Ok(None),
    }

    trace!(
        "Compacted strided {} frame for encoder: {} -> {} bytes (stride={}, width={})",
        frame.format,
        data.len(),
        out.len(),
        stride,
        width
    );
    Ok(Some(out))
}

fn require_len(data: &[u8], required: usize, format: PixelFormat, stride: usize) -> Result<()> {
    if data.len() < required {
        return Err(AppError::VideoError(format!(
            "{} frame too small for stride compaction: {} < {} (stride={})",
            format,
            data.len(),
            required,
            stride
        )));
    }
    Ok(())
}

fn copy_rows(
    src: &[u8],
    src_offset: usize,
    src_stride: usize,
    dst: &mut [u8],
    dst_offset: usize,
    dst_stride: usize,
    row_bytes: usize,
    rows: usize,
) {
    for row in 0..rows {
        let src_start = src_offset + row * src_stride;
        let dst_start = dst_offset + row * dst_stride;
        dst[dst_start..dst_start + row_bytes]
            .copy_from_slice(&src[src_start..src_start + row_bytes]);
    }
}

impl Drop for SharedVideoPipeline {
    fn drop(&mut self) {
        self.running_flag.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::codec::BitratePreset;

    #[tokio::test]
    async fn bitrate_commands_preserve_custom_values_and_gop_policy() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::default()).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        *pipeline.cmd_tx.write() = Some(tx);
        for preset in [
            BitratePreset::Custom(2500),
            BitratePreset::Custom(1000),
            BitratePreset::Speed,
            BitratePreset::Quality,
        ] {
            pipeline.set_bitrate_preset(preset).await.unwrap();
            let PipelineCmd::SetBitrate { preset: received } = rx.try_recv().unwrap();
            assert_eq!(received, preset);
            assert_eq!(pipeline.config().await.bitrate_preset, preset);
            // Rebuilt encoders must retain the preset's policy at the new FPS.
            let restored = SharedVideoPipelineConfig {
                bitrate_preset: received,
                fps: 60,
                ..Default::default()
            };
            assert_eq!(restored.bitrate_kbps(), preset.bitrate_kbps());
            assert_eq!(restored.gop_size(), preset.gop_size(60));
        }
    }

    #[test]
    fn source_reopen_updates_fps_and_gop_without_changing_geometry_or_bitrate() {
        let mut config = SharedVideoPipelineConfig {
            control_mode: VideoControlMode::SourceFollowing,
            resolution: Resolution::HD1080,
            fps: 60,
            bitrate_preset: BitratePreset::Quality,
            ..Default::default()
        };
        config.align_source_fps(Some(29.97));
        assert_eq!(config.fps, 30);
        assert_eq!(config.gop_size(), 60);
        assert_eq!(config.resolution, Resolution::HD1080);
        assert_eq!(config.bitrate_kbps(), 8000);
        config.align_source_fps(None);
        assert_eq!(config.fps, 30);
        config.align_source_fps(Some(59.94));
        assert_eq!(config.fps, 60);
        assert_eq!(config.gop_size(), 120);
    }

    #[test]
    fn configurable_capture_keeps_requested_fps() {
        let mut config = SharedVideoPipelineConfig::default();
        config.align_source_fps(Some(60.0));
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_pipeline_config() {
        let h264 = SharedVideoPipelineConfig::h264(Resolution::HD1080, BitratePreset::Balanced);
        assert_eq!(h264.output_codec, VideoEncoderType::H264);

        let h265 = SharedVideoPipelineConfig::h265(Resolution::HD720, BitratePreset::Speed);
        assert_eq!(h265.output_codec, VideoEncoderType::H265);
    }

    #[test]
    fn h264_keyframe_requires_idr_and_parameter_sets() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();

        let predicted = Bytes::from_static(&[0, 0, 0, 1, 0x41, 0xc0]);
        let (_, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H264, predicted, true);
        assert!(
            !key,
            "a driver flag must not turn a P-frame into a keyframe"
        );

        let parameter_sets =
            Bytes::from_static(&[0, 0, 0, 1, 0x67, 0x42, 0x40, 0x1f, 0, 0, 0, 1, 0x68, 0xce]);
        let (_, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H264, parameter_sets, false);
        assert!(!key, "parameter sets alone are not a keyframe");

        let idr = Bytes::from_static(&[0, 0, 0, 1, 0x65, 0x88]);
        let (_, key) = pipeline.inspect_and_parameterize_packet(VideoEncoderType::H264, idr, false);
        assert!(!key, "an IDR without a driver key flag is not trusted");

        let idr = Bytes::from_static(&[0, 0, 0, 1, 0x65, 0x88]);
        let (bootstrap, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H264, idr, true);
        assert!(
            key,
            "matching driver metadata and IDR payload should bootstrap"
        );
        assert!(h264_bitstream::has_sps_pps(bootstrap.as_ref()));
        assert!(h264_bitstream::is_keyframe(bootstrap.as_ref()));
    }

    #[test]
    fn h264_profile_is_updated_from_parameter_only_packet() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let packet =
            Bytes::from_static(&[0, 0, 0, 1, 0x67, 0x42, 0xe0, 0x1f, 0, 0, 0, 1, 0x68, 0xce]);
        let (_, keyframe) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H264, packet, false);
        assert!(!keyframe);
        assert_eq!(
            pipeline.h264_profile_level_id_watch().borrow().as_deref(),
            Some("42e01f")
        );
    }

    #[tokio::test]
    async fn decoder_statistics_report_work_and_drops() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        pipeline.mjpeg_decoded_frames.store(30, Ordering::Relaxed);
        pipeline
            .mjpeg_dropped_before_decode
            .store(10, Ordering::Relaxed);
        pipeline
            .decoded_frames_not_encoded
            .store(5, Ordering::Relaxed);
        let stats = pipeline.stats().await;
        assert_eq!(stats.mjpeg_decoded_frames, 30);
        assert_eq!(stats.mjpeg_dropped_before_decode, 10);
        assert_eq!(stats.decoded_frames_not_encoded, 5);
    }

    #[test]
    fn encoder_packet_pts_is_preserved() {
        use super::super::encoder_state::{EncodedFrame, VideoEncoderTrait};

        struct TestEncoder;

        impl VideoEncoderTrait for TestEncoder {
            fn encode_raw(&mut self, _data: &[u8], _pts_ms: i64) -> Result<Vec<EncodedFrame>> {
                Ok(vec![EncodedFrame {
                    data: Bytes::from_static(&[0, 0, 0, 1, 0x41, 0xc0]),
                    key: 0,
                    pts: 321,
                }])
            }

            fn set_bitrate(&mut self, _bitrate_kbps: u32) -> Result<()> {
                Ok(())
            }

            fn codec_name(&self) -> &str {
                "test-h264"
            }

            fn request_keyframe(&mut self) {}
        }

        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let mut state = EncoderThreadState {
            encoder: Some(Box::new(TestEncoder)),
            mjpeg_decoder: None,
            nv12_converter: None,
            yuv420p_converter: None,
            encoder_needs_yuv420p: false,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_pipeline: None,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_enabled: false,
            fps: 30,
            codec: VideoEncoderType::H264,
            input_format: PixelFormat::Nv12,
        };
        let frame = VideoFrame::from_pooled(
            Arc::new(FrameBuffer::new(vec![0; 1280 * 720 * 3 / 2], None)),
            Resolution::HD720,
            PixelFormat::Nv12,
            1280,
            1,
        );
        let encoded = pipeline.encode_frame_sync(&mut state, &frame).unwrap();
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].pts_ms, 321);
    }

    #[test]
    fn h265_keyframe_requires_irap_and_parameter_sets() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h265(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();

        let trail = Bytes::from_static(&[0, 0, 0, 1, 1 << 1, 1, 0xaa]);
        let (_, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H265, trail, true);
        assert!(
            !key,
            "a driver flag must not turn a trailing frame into a keyframe"
        );

        let parameter_sets = Bytes::from_static(&[
            0,
            0,
            0,
            1,
            32 << 1,
            1,
            0xaa,
            0,
            0,
            0,
            1,
            33 << 1,
            1,
            0xbb,
            0,
            0,
            0,
            1,
            34 << 1,
            1,
            0xcc,
        ]);
        let (_, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H265, parameter_sets, false);
        assert!(!key, "parameter sets alone are not a keyframe");

        let irap = Bytes::from_static(&[0, 0, 0, 1, 19 << 1, 1, 0xdd]);
        let (_, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H265, irap, false);
        assert!(!key, "an IRAP without a driver key flag is not trusted");

        let irap = Bytes::from_static(&[0, 0, 0, 1, 19 << 1, 1, 0xdd]);
        let (bootstrap, key) =
            pipeline.inspect_and_parameterize_packet(VideoEncoderType::H265, irap, true);
        assert!(
            key,
            "matching driver metadata and IRAP payload should bootstrap"
        );
        assert!(h265_bitstream::has_vps_sps_pps(bootstrap.as_ref()));
        assert!(h265_bitstream::is_keyframe(bootstrap.as_ref()));
    }

    #[test]
    fn new_subscriber_receives_cached_bootstrap_frame() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let bootstrap = Arc::new(EncodedVideoFrame {
            data: Bytes::from_static(&[
                0, 0, 0, 1, 0x67, 0x42, 0x40, 0x1f, 0, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65, 0x88,
            ]),
            pts_ms: 0,
            is_keyframe: true,
            sequence: 1,
            duration: Duration::from_millis(33),
            codec: VideoEncoderType::H264,
        });

        pipeline.broadcast_encoded_batch(vec![bootstrap.clone()].into(), Vec::new());
        let mut subscriber = pipeline.subscribe();
        let received = subscriber
            .try_recv()
            .expect("cached bootstrap frame should seed the subscriber queue");
        assert!(Arc::ptr_eq(&received, &bootstrap));
    }

    fn delivery_test_pipeline() -> Arc<SharedVideoPipeline> {
        SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap()
    }

    fn delivery_test_frame(sequence: u64, is_keyframe: bool) -> Arc<EncodedVideoFrame> {
        Arc::new(EncodedVideoFrame {
            data: Bytes::from_static(&[0, 0, 0, 1, 0x41, 0xc0]),
            pts_ms: sequence as i64 * 33,
            is_keyframe,
            sequence,
            duration: Duration::from_millis(33),
            codec: VideoEncoderType::H264,
        })
    }

    fn delivery_test_raw_frame(sequence: u64) -> Arc<VideoFrame> {
        Arc::new(VideoFrame::from_pooled(
            Arc::new(FrameBuffer::new(vec![0; 6], None)),
            Resolution {
                width: 2,
                height: 2,
            },
            PixelFormat::Nv12,
            2,
            sequence,
        ))
    }

    #[test]
    fn encoded_batch_delivers_all_frames_in_order() {
        let pipeline = delivery_test_pipeline();
        let mut receiver = pipeline.subscribe();
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(
            vec![
                delivery_test_frame(1, true),
                delivery_test_frame(2, false),
                delivery_test_frame(3, false),
            ]
            .into(),
            reservations,
        );
        assert_eq!(receiver.try_recv().unwrap().sequence, 1);
        assert_eq!(receiver.try_recv().unwrap().sequence, 2);
        assert_eq!(receiver.try_recv().unwrap().sequence, 3);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn single_subscriber_backpressure_preserves_reference_chain() {
        let pipeline = delivery_test_pipeline();
        let mut receiver = pipeline.subscribe();
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(1, true)].into(), reservations);
        assert!(pipeline.reserve_outputs().is_none());
        assert!(pipeline.reserve_outputs().is_none());
        assert_eq!(pipeline.stats_snapshot().output_backpressure_events, 1);
        assert_eq!(receiver.try_recv().unwrap().sequence, 1);
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(2, false)].into(), reservations);
        assert_eq!(receiver.try_recv().unwrap().sequence, 2);
        let stats = pipeline.stats_snapshot();
        assert_eq!(stats.encoded_frames, 2);
        assert_eq!(stats.encoded_frames_skipped_for_slow_subscribers, 0);
    }

    #[test]
    fn output_capacity_wakes_encoder_with_latest_raw_frame() {
        use std::sync::mpsc as sync_mpsc;

        let pipeline = delivery_test_pipeline();
        let mailbox = Arc::new(VideoFrameMailbox::new());
        *pipeline.frame_mailbox.lock() = Some(mailbox.clone());
        let mut receiver = pipeline.subscribe();
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(1, true)].into(), reservations);
        mailbox.publish(1, delivery_test_raw_frame(1));
        mailbox.publish(2, delivery_test_raw_frame(2));
        let (waiting_sender, waiting_receiver) = sync_mpsc::sync_channel(1);
        let (result_sender, result_receiver) = sync_mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = mailbox.receive_when(|| {
                let reservations = pipeline.reserve_outputs();
                if reservations.is_none() {
                    let _ = waiting_sender.try_send(());
                }
                reservations
            });
            let (frame, reservations) = result.unwrap();
            result_sender
                .send((frame.sequence, reservations.len()))
                .unwrap();
        });
        waiting_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(receiver.try_recv().unwrap().sequence, 1);
        assert_eq!(
            result_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            (2, 1)
        );
        worker.join().unwrap();
    }

    #[test]
    fn receiver_drop_wakes_encoder_waiting_for_capacity() {
        use std::sync::mpsc as sync_mpsc;

        let pipeline = delivery_test_pipeline();
        let mailbox = Arc::new(VideoFrameMailbox::new());
        *pipeline.frame_mailbox.lock() = Some(mailbox.clone());
        let receiver = pipeline.subscribe();
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(1, true)].into(), reservations);
        mailbox.publish(1, delivery_test_raw_frame(1));
        let (waiting_sender, waiting_receiver) = sync_mpsc::sync_channel(1);
        let (result_sender, result_receiver) = sync_mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = mailbox.receive_when(|| {
                let reservations = pipeline.reserve_outputs();
                if reservations.is_none() {
                    let _ = waiting_sender.try_send(());
                }
                reservations
            });
            result_sender.send(result.unwrap().1.is_empty()).unwrap();
        });
        waiting_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        drop(receiver);
        assert!(result_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn slow_subscriber_is_isolated_and_resumes_at_keyframe() {
        let pipeline = delivery_test_pipeline();
        let mut fast_receiver = pipeline.subscribe();
        let mut slow_receiver = pipeline.subscribe();
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(1, true)].into(), reservations);
        pipeline.keyframe_requested.store(false, Ordering::Release);
        assert_eq!(fast_receiver.try_recv().unwrap().sequence, 1);
        let reservations = pipeline.reserve_outputs().unwrap();
        assert_eq!(reservations.len(), 1);
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(2, false)].into(), reservations);
        assert_eq!(fast_receiver.try_recv().unwrap().sequence, 2);
        assert!(!pipeline.keyframe_requested.load(Ordering::Acquire));
        assert_eq!(slow_receiver.try_recv().unwrap().sequence, 1);
        let reservations = pipeline.reserve_outputs().unwrap();
        assert_eq!(reservations.len(), 2);
        assert!(pipeline.keyframe_requested.swap(false, Ordering::AcqRel));
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(3, false)].into(), reservations);
        assert_eq!(fast_receiver.try_recv().unwrap().sequence, 3);
        assert!(matches!(
            slow_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        let reservations = pipeline.reserve_outputs().unwrap();
        assert!(!pipeline.keyframe_requested.load(Ordering::Acquire));
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(4, true)].into(), reservations);
        assert_eq!(fast_receiver.try_recv().unwrap().sequence, 4);
        assert_eq!(slow_receiver.try_recv().unwrap().sequence, 4);
        let reservations = pipeline.reserve_outputs().unwrap();
        pipeline.broadcast_encoded_batch(vec![delivery_test_frame(5, false)].into(), reservations);
        assert_eq!(fast_receiver.try_recv().unwrap().sequence, 5);
        assert_eq!(slow_receiver.try_recv().unwrap().sequence, 5);
        assert_eq!(
            pipeline
                .stats_snapshot()
                .encoded_frames_skipped_for_slow_subscribers,
            2
        );
    }

    #[test]
    fn send_statistics_include_successes_errors_and_duration() {
        let pipeline = delivery_test_pipeline();
        let receiver = pipeline.subscribe();
        receiver.record_send(Duration::from_micros(100), true);
        receiver.record_send(Duration::from_micros(200), false);
        let stats = pipeline.stats_snapshot();
        assert_eq!(stats.sent_frames, 1);
        assert_eq!(stats.send_errors, 1);
        assert_eq!(stats.send_time_ns, 300_000);
    }

    #[test]
    fn mjpeg_workers_match_available_cpu_count() {
        assert_eq!(mjpeg_decode_worker_count(1), 1);
        assert_eq!(mjpeg_decode_worker_count(2), 2);
        assert_eq!(mjpeg_decode_worker_count(4), 4);
        assert_eq!(mjpeg_decode_worker_count(64), 64);
    }

    #[test]
    fn stop_wakes_idle_encoder_even_after_capture_requested_stop() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let mailbox = Arc::new(VideoFrameMailbox::new());
        *pipeline.frame_mailbox.lock() = Some(mailbox.clone());
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender.send(mailbox.receive().is_none()).unwrap();
        });
        pipeline.stop();
        assert!(receiver.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn stop_request_does_not_publish_worker_exit() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let _ = pipeline.running.send(true);
        pipeline.running_flag.store(true, Ordering::Release);

        pipeline.stop();

        assert!(!pipeline.running_flag.load(Ordering::Acquire));
        assert!(pipeline.is_running());
        assert_eq!(pipeline.lifecycle(), PipelineLifecycle::Stopping);

        // Simulate the capture thread's common cleanup tail.
        let _ = pipeline.running.send(false);
        assert!(!pipeline.is_running());
        assert_eq!(pipeline.lifecycle(), PipelineLifecycle::Stopped);
    }

    #[tokio::test]
    async fn stop_and_wait_observes_completed_worker_cleanup() {
        let pipeline = SharedVideoPipeline::new(SharedVideoPipelineConfig::h264(
            Resolution::HD720,
            BitratePreset::Balanced,
        ))
        .unwrap();
        let _ = pipeline.running.send(true);
        let _ = pipeline.encoder_done.send(false);
        pipeline.running_flag.store(true, Ordering::Release);

        let worker = pipeline.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            let _ = worker.running.send(false);
            tokio::time::sleep(Duration::from_millis(30)).await;
            let _ = worker.encoder_done.send(true);
        });

        let started = Instant::now();
        pipeline
            .stop_and_wait(Duration::from_secs(1))
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(!pipeline.is_running());
    }
}
