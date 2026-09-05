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
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

use super::encoder_state::{build_encoder_state, should_parallel_decode_mjpeg, EncoderThreadState};

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
struct MjpegDecodeJob {
    data: Vec<u8>,
    sequence: u64,
}

fn mjpeg_decode_worker_count(available_parallelism: usize) -> usize {
    available_parallelism.max(1)
}

fn spawn_mjpeg_decode_workers(
    pipeline: &Arc<SharedVideoPipeline>,
    latest_frame: &Arc<ParkingRwLock<Option<Arc<VideoFrame>>>>,
    frame_seq_tx: &watch::Sender<u64>,
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
        let worker_latest_frame = latest_frame.clone();
        let worker_frame_seq_tx = frame_seq_tx.clone();
        let worker_buffer_pool = buffer_pool.clone();
        let thread_name = format!("mjpeg-decoder-{worker_id}");
        let spawn_result = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let mut decoder = MjpegToNv12Decoder::new(resolution);
                while let Ok(job) = rx.recv() {
                    let nv12_size = resolution.width as usize * resolution.height as usize * 3 / 2;
                    let mut nv12 = worker_buffer_pool.take(nv12_size);
                    let decode_result = decoder.decode_into(&job.data, &mut nv12);
                    worker_buffer_pool.put(job.data);

                    if let Err(error) = decode_result {
                        worker_buffer_pool.put(nv12);
                        warn!("Dropping undecodable MJPEG frame: {}", error);
                        continue;
                    }
                    if !worker_pipeline.running_flag.load(Ordering::Acquire) {
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
                    let published = {
                        let mut latest = worker_latest_frame.write();
                        if latest
                            .as_ref()
                            .is_some_and(|current| current.sequence >= job.sequence)
                        {
                            false
                        } else {
                            *latest = Some(frame);
                            true
                        }
                    };
                    if published {
                        let _ = worker_frame_seq_tx.send(job.sequence.wrapping_add(1));
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
    SetBitrate { bitrate_kbps: u32, gop: u32 },
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

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedVideoPipelineStats {
    pub current_fps: f32,
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
    subscribers: ParkingRwLock<Vec<mpsc::Sender<Arc<EncodedVideoFrame>>>>,
    stats: Mutex<SharedVideoPipelineStats>,
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
            stats: Mutex::new(SharedVideoPipelineStats::default()),
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
    pub fn subscribe(&self) -> mpsc::Receiver<Arc<EncodedVideoFrame>> {
        // A queued video frame is already stale when the next frame is ready.
        // Keep at most one pending frame so a slow WebRTC writer cannot make
        // the encoder wait or accumulate seconds of latency.
        let (tx, rx) = mpsc::channel(1);
        if let Some(frame) = self.bootstrap_frame.read().clone() {
            let _ = tx.try_send(frame);
        }
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

    fn update_h264_profile_level_id(&self, data: &[u8]) {
        let Some(profile_level_id) = h264_bitstream::extract_profile_level_id(data) else {
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
                let (sps, pps) = h264_bitstream::extract_sps_pps(data.as_ref());
                // Require metadata and payload to agree before advertising a
                // decoder bootstrap frame.
                let is_idr = ffmpeg_keyframe && h264_bitstream::is_keyframe(data.as_ref());
                let mut cache = self.parameter_sets.lock();
                if let Some(sps) = sps.as_ref() {
                    cache.h264_sps = Some(sps.clone());
                }
                if let Some(pps) = pps.as_ref() {
                    cache.h264_pps = Some(pps.clone());
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

    fn broadcast_encoded(&self, frame: Arc<EncodedVideoFrame>) {
        if frame.is_keyframe {
            *self.bootstrap_frame.write() = Some(frame.clone());
        }

        let subscribers = {
            let guard = self.subscribers.read();
            if guard.is_empty() {
                return;
            }
            guard.iter().cloned().collect::<Vec<_>>()
        };

        for tx in &subscribers {
            // Never await a consumer.  A full one-slot queue means the
            // consumer is behind; dropping this frame preserves bounded
            // latency and the receiver's sequence-gap logic requests a fresh
            // keyframe when necessary.
            let _ = tx.try_send(frame.clone());
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
        let preopened: Option<CaptureStream> = match open_capture_stream(
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
                if config.control_mode == VideoControlMode::SourceFollowing {
                    if let Some(source_fps) = s.source_fps() {
                        config.fps = source_fps.round().clamp(1.0, 120.0) as u32;
                    }
                }
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

        let mut encoder_config = config.clone();
        if parallel_mjpeg_decode {
            encoder_config.input_format = PixelFormat::Nv12;
            info!("Using parallel libyuv MJPEG decode with hardware encoding");
        }
        let mut encoder_state = build_encoder_state(&encoder_config)?;
        let _ = self.running.send(true);
        let _ = self.encoder_done.send(false);
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

        // Encoder loop uses a dedicated OS thread because FFmpeg work is synchronous.
        {
            let pipeline = pipeline.clone();
            let latest_frame = latest_frame.clone();
            let handle = tokio::runtime::Handle::current();
            std::thread::spawn(move || {
                let mut input_frame_count: u64 = 0;
                let mut encoded_frame_count: u64 = 0;
                let mut last_fps_time = Instant::now();
                let mut fps_frame_count: u64 = 0;
                let mut last_seq = *frame_seq_rx.borrow();
                let encode_error_throttler = LogThrottler::with_secs(ENCODE_ERROR_THROTTLE_SECS);
                let mut suppressed_encode_errors: HashMap<String, u64> = HashMap::new();

                while pipeline.running_flag.load(Ordering::Acquire) {
                    if handle.block_on(frame_seq_rx.changed()).is_err() {
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

                    input_frame_count = input_frame_count.wrapping_add(1);

                    match pipeline.encode_frame_sync(&mut encoder_state, &frame) {
                        Ok(encoded_frames) => {
                            for encoded_frame in encoded_frames {
                                let encoded_arc = Arc::new(encoded_frame);
                                pipeline.broadcast_encoded(encoded_arc);

                                encoded_frame_count = encoded_frame_count.wrapping_add(1);
                                fps_frame_count += 1;
                            }
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

                        handle.block_on(async {
                            let mut s = pipeline.stats.lock().await;
                            s.current_fps = current_fps;
                        });
                        trace!(
                            "Shared pipeline processed {} input frames, emitted {} encoded frames",
                            input_frame_count,
                            encoded_frame_count
                        );
                    }
                }

                pipeline.clear_cmd_tx();
                // Release encoder resources before allowing a replacement pipeline.
                drop(encoder_state);
                let _ = pipeline.encoder_done.send(true);
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
                            &latest_frame,
                            &frame_seq_tx,
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
                let grace_period = Duration::from_secs(AUTO_STOP_GRACE_PERIOD_SECS);
                let mut sequence: u64 = 0;
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

                    // Notify streaming only after the short-frame guard passes.
                    pipeline.notify_state(PipelineStateNotification::streaming(
                        resolution,
                        pixel_format,
                        active_fps,
                    ));

                    if let Some(senders) = mjpeg_decode_senders.as_mut() {
                        let mut pending = Some(MjpegDecodeJob {
                            data: owned,
                            sequence: meta.sequence,
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
                            buffer_pool.put(job.data);
                        }
                        continue;
                    }

                    let (frame_data, frame_format, frame_stride) =
                        if let Some(decoder) = mjpeg_decoder.as_mut() {
                            let nv12_size =
                                resolution.width as usize * resolution.height as usize * 3 / 2;
                            let mut nv12 = buffer_pool.take(nv12_size);
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
                            (nv12, PixelFormat::Nv12, resolution.width)
                        } else {
                            (owned, pixel_format, stride)
                        };
                    let frame = Arc::new(VideoFrame::from_pooled(
                        Arc::new(FrameBuffer::new(frame_data, Some(buffer_pool.clone()))),
                        resolution,
                        frame_format,
                        frame_stride,
                        meta.sequence,
                    ));
                    sequence = meta.sequence.wrapping_add(1);

                    {
                        let mut guard = latest_frame.write();
                        *guard = Some(frame);
                    }
                    let _ = frame_seq_tx.send(sequence);
                }

                // `running` represents completed lifecycle state, not a stop request.
                // Drop the V4L2 stream first so STREAMOFF, buffer teardown and FD close
                // have all completed before another consumer is told the device is free.
                drop(stream);
                pipeline.running_flag.store(false, Ordering::Release);
                let _ = frame_seq_tx.send(sequence.wrapping_add(1));
                let _ = pipeline.running.send(false);
                info!("Video pipeline stopped and capture device released");
            });
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

        let process_start = PROCESS_START.get_or_init(Instant::now);
        let current_ts_us = process_start.elapsed().as_micros() as i64;
        let start_ts_us = self.pipeline_start_time_us.load(Ordering::Acquire);
        let pts_ms = if start_ts_us == 0 {
            let start_ts_us = match self.pipeline_start_time_us.compare_exchange(
                0,
                current_ts_us,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => current_ts_us,
                Err(existing) => existing,
            };
            current_ts_us.saturating_sub(start_ts_us) / 1000
        } else {
            current_ts_us.saturating_sub(start_ts_us) / 1000
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
                    if codec == VideoEncoderType::H264 {
                        self.update_h264_profile_level_id(&data);
                    }

                    encoded_frames.push(EncodedVideoFrame {
                        data,
                        pts_ms,
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

    /// Stop the pipeline (non-blocking, does not wait for capture thread to exit)
    pub fn stop(&self) {
        if self.running_flag.swap(false, Ordering::AcqRel) {
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

        pipeline.broadcast_encoded(bootstrap.clone());
        let mut subscriber = pipeline.subscribe();
        let received = subscriber
            .try_recv()
            .expect("cached bootstrap frame should seed the subscriber queue");
        assert!(Arc::ptr_eq(&received, &bootstrap));
    }

    #[test]
    fn mjpeg_workers_match_available_cpu_count() {
        assert_eq!(mjpeg_decode_worker_count(1), 1);
        assert_eq!(mjpeg_decode_worker_count(2), 2);
        assert_eq!(mjpeg_decode_worker_count(4), 4);
        assert_eq!(mjpeg_decode_worker_count(64), 64);
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
