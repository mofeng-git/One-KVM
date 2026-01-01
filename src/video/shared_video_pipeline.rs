//! Universal shared video encoding pipeline
//!
//! Supports multiple codecs: H264, H265, VP8, VP9
//! A single encoder broadcasts to multiple WebRTC sessions.
//!
//! Architecture:
//! ```text
//! VideoCapturer (MJPEG/YUYV/NV12)
//!        |
//!        v (broadcast::Receiver<VideoFrame>)
//! SharedVideoPipeline (single encoder)
//!        |
//!        v (broadcast::Sender<EncodedVideoFrame>)
//!   ┌────┴────┬────────┬────────┐
//!   v         v        v        v
//! Session1  Session2  Session3  ...
//! ```

use bytes::Bytes;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

/// Grace period before auto-stopping pipeline when no subscribers (in seconds)
const AUTO_STOP_GRACE_PERIOD_SECS: u64 = 3;

use crate::error::{AppError, Result};
use crate::video::convert::{Nv12Converter, PixelConverter};
use crate::video::decoder::mjpeg::{MjpegTurboDecoder, MjpegVaapiDecoder, MjpegVaapiDecoderConfig};
use crate::video::encoder::h264::{H264Config, H264Encoder};
use crate::video::encoder::h265::{H265Config, H265Encoder};
use crate::video::encoder::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use crate::video::encoder::traits::EncoderConfig;
use crate::video::encoder::vp8::{VP8Config, VP8Encoder};
use crate::video::encoder::vp9::{VP9Config, VP9Encoder};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::frame::VideoFrame;

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

/// Shared video pipeline configuration
#[derive(Debug, Clone)]
pub struct SharedVideoPipelineConfig {
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Output codec type
    pub output_codec: VideoEncoderType,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Target FPS
    pub fps: u32,
    /// GOP size
    pub gop_size: u32,
    /// Encoder backend (None = auto select best available)
    pub encoder_backend: Option<EncoderBackend>,
}

impl Default for SharedVideoPipelineConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution::HD720,
            input_format: PixelFormat::Yuyv,
            output_codec: VideoEncoderType::H264,
            bitrate_kbps: 1000,
            fps: 30,
            gop_size: 30,
            encoder_backend: None,
        }
    }
}

impl SharedVideoPipelineConfig {
    /// Create H264 config
    pub fn h264(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H264,
            bitrate_kbps,
            ..Default::default()
        }
    }

    /// Create H265 config
    pub fn h265(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::H265,
            bitrate_kbps,
            ..Default::default()
        }
    }

    /// Create VP8 config
    pub fn vp8(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP8,
            bitrate_kbps,
            ..Default::default()
        }
    }

    /// Create VP9 config
    pub fn vp9(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            resolution,
            output_codec: VideoEncoderType::VP9,
            bitrate_kbps,
            ..Default::default()
        }
    }
}

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedVideoPipelineStats {
    pub frames_captured: u64,
    pub frames_encoded: u64,
    pub frames_dropped: u64,
    pub bytes_encoded: u64,
    pub keyframes_encoded: u64,
    pub avg_encode_time_ms: f32,
    pub current_fps: f32,
    pub errors: u64,
    pub subscribers: u64,
}


/// Universal video encoder trait object
#[allow(dead_code)]
trait VideoEncoderTrait: Send {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>>;
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;
    fn codec_name(&self) -> &str;
    fn request_keyframe(&mut self);
}

/// Encoded frame from encoder
#[allow(dead_code)]
struct EncodedFrame {
    data: Vec<u8>,
    pts: i64,
    key: i32,
}

/// H264 encoder wrapper
struct H264EncoderWrapper(H264Encoder);

impl VideoEncoderTrait for H264EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                pts: f.pts,
                key: f.key,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        self.0.request_keyframe()
    }
}

/// H265 encoder wrapper
struct H265EncoderWrapper(H265Encoder);

impl VideoEncoderTrait for H265EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                pts: f.pts,
                key: f.key,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        self.0.request_keyframe()
    }
}

/// VP8 encoder wrapper
struct VP8EncoderWrapper(VP8Encoder);

impl VideoEncoderTrait for VP8EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                pts: f.pts,
                key: f.key,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        // VP8 encoder doesn't support request_keyframe yet
    }
}

/// VP9 encoder wrapper
struct VP9EncoderWrapper(VP9Encoder);

impl VideoEncoderTrait for VP9EncoderWrapper {
    fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<EncodedFrame>> {
        let frames = self.0.encode_raw(data, pts_ms)?;
        Ok(frames
            .into_iter()
            .map(|f| EncodedFrame {
                data: f.data,
                pts: f.pts,
                key: f.key,
            })
            .collect())
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.0.set_bitrate(bitrate_kbps)
    }

    fn codec_name(&self) -> &str {
        self.0.codec_name()
    }

    fn request_keyframe(&mut self) {
        // VP9 encoder doesn't support request_keyframe yet
    }
}

/// Universal shared video pipeline
pub struct SharedVideoPipeline {
    config: RwLock<SharedVideoPipelineConfig>,
    encoder: Mutex<Option<Box<dyn VideoEncoderTrait + Send>>>,
    nv12_converter: Mutex<Option<Nv12Converter>>,
    yuv420p_converter: Mutex<Option<PixelConverter>>,
    mjpeg_decoder: Mutex<Option<MjpegVaapiDecoder>>,
    /// Turbojpeg decoder for direct MJPEG->YUV420P (optimized for software encoders)
    mjpeg_turbo_decoder: Mutex<Option<MjpegTurboDecoder>>,
    nv12_buffer: Mutex<Vec<u8>>,
    /// YUV420P buffer for turbojpeg decoder output
    yuv420p_buffer: Mutex<Vec<u8>>,
    /// Whether the encoder needs YUV420P (true) or NV12 (false)
    encoder_needs_yuv420p: AtomicBool,
    frame_tx: broadcast::Sender<EncodedVideoFrame>,
    stats: Mutex<SharedVideoPipelineStats>,
    running: watch::Sender<bool>,
    running_rx: watch::Receiver<bool>,
    /// Frame sequence counter (atomic for lock-free access)
    sequence: AtomicU64,
    /// Atomic flag for keyframe request (avoids lock contention)
    keyframe_requested: AtomicBool,
}

impl SharedVideoPipeline {
    /// Create a new shared video pipeline
    pub fn new(config: SharedVideoPipelineConfig) -> Result<Arc<Self>> {
        info!(
            "Creating shared video pipeline: {} {}x{} @ {} kbps (input: {})",
            config.output_codec,
            config.resolution.width,
            config.resolution.height,
            config.bitrate_kbps,
            config.input_format
        );

        let (frame_tx, _) = broadcast::channel(8);  // Reduced from 64 for lower latency
        let (running_tx, running_rx) = watch::channel(false);
        let nv12_size = (config.resolution.width * config.resolution.height * 3 / 2) as usize;
        let yuv420p_size = nv12_size; // Same size as NV12

        let pipeline = Arc::new(Self {
            config: RwLock::new(config),
            encoder: Mutex::new(None),
            nv12_converter: Mutex::new(None),
            yuv420p_converter: Mutex::new(None),
            mjpeg_decoder: Mutex::new(None),
            mjpeg_turbo_decoder: Mutex::new(None),
            nv12_buffer: Mutex::new(vec![0u8; nv12_size]),
            yuv420p_buffer: Mutex::new(vec![0u8; yuv420p_size]),
            encoder_needs_yuv420p: AtomicBool::new(false),
            frame_tx,
            stats: Mutex::new(SharedVideoPipelineStats::default()),
            running: running_tx,
            running_rx,
            sequence: AtomicU64::new(0),
            keyframe_requested: AtomicBool::new(false),
        });

        Ok(pipeline)
    }

    /// Initialize encoder based on config
    async fn init_encoder(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        let registry = EncoderRegistry::global();

        // Helper to get codec name for specific backend
        let get_codec_name = |format: VideoEncoderType, backend: Option<EncoderBackend>| -> Option<String> {
            match backend {
                Some(b) => registry.encoder_with_backend(format, b).map(|e| e.codec_name.clone()),
                None => registry.best_encoder(format, false).map(|e| e.codec_name.clone()),
            }
        };

        // Create encoder based on codec type
        let encoder: Box<dyn VideoEncoderTrait + Send> = match config.output_codec {
            VideoEncoderType::H264 => {
                let encoder_config = H264Config {
                    base: EncoderConfig::h264(config.resolution, config.bitrate_kbps),
                    bitrate_kbps: config.bitrate_kbps,
                    gop_size: config.gop_size,
                    fps: config.fps,
                    input_format: crate::video::encoder::h264::H264InputFormat::Nv12,
                };

                let encoder = if let Some(ref backend) = config.encoder_backend {
                    // Specific backend requested
                    let codec_name = get_codec_name(VideoEncoderType::H264, Some(*backend))
                        .ok_or_else(|| AppError::VideoError(format!(
                            "Backend {:?} does not support H.264", backend
                        )))?;
                    info!("Creating H264 encoder with backend {:?} (codec: {})", backend, codec_name);
                    H264Encoder::with_codec(encoder_config, &codec_name)?
                } else {
                    // Auto select
                    H264Encoder::new(encoder_config)?
                };

                info!("Created H264 encoder: {}", encoder.codec_name());
                Box::new(H264EncoderWrapper(encoder))
            }
            VideoEncoderType::H265 => {
                let encoder_config = H265Config::low_latency(config.resolution, config.bitrate_kbps);

                let encoder = if let Some(ref backend) = config.encoder_backend {
                    let codec_name = get_codec_name(VideoEncoderType::H265, Some(*backend))
                        .ok_or_else(|| AppError::VideoError(format!(
                            "Backend {:?} does not support H.265", backend
                        )))?;
                    info!("Creating H265 encoder with backend {:?} (codec: {})", backend, codec_name);
                    H265Encoder::with_codec(encoder_config, &codec_name)?
                } else {
                    H265Encoder::new(encoder_config)?
                };

                info!("Created H265 encoder: {}", encoder.codec_name());
                Box::new(H265EncoderWrapper(encoder))
            }
            VideoEncoderType::VP8 => {
                let encoder_config = VP8Config::low_latency(config.resolution, config.bitrate_kbps);

                let encoder = if let Some(ref backend) = config.encoder_backend {
                    let codec_name = get_codec_name(VideoEncoderType::VP8, Some(*backend))
                        .ok_or_else(|| AppError::VideoError(format!(
                            "Backend {:?} does not support VP8", backend
                        )))?;
                    info!("Creating VP8 encoder with backend {:?} (codec: {})", backend, codec_name);
                    VP8Encoder::with_codec(encoder_config, &codec_name)?
                } else {
                    VP8Encoder::new(encoder_config)?
                };

                info!("Created VP8 encoder: {}", encoder.codec_name());
                Box::new(VP8EncoderWrapper(encoder))
            }
            VideoEncoderType::VP9 => {
                let encoder_config = VP9Config::low_latency(config.resolution, config.bitrate_kbps);

                let encoder = if let Some(ref backend) = config.encoder_backend {
                    let codec_name = get_codec_name(VideoEncoderType::VP9, Some(*backend))
                        .ok_or_else(|| AppError::VideoError(format!(
                            "Backend {:?} does not support VP9", backend
                        )))?;
                    info!("Creating VP9 encoder with backend {:?} (codec: {})", backend, codec_name);
                    VP9Encoder::with_codec(encoder_config, &codec_name)?
                } else {
                    VP9Encoder::new(encoder_config)?
                };

                info!("Created VP9 encoder: {}", encoder.codec_name());
                Box::new(VP9EncoderWrapper(encoder))
            }
        };

        // Determine if encoder needs YUV420P (software encoders) or NV12 (hardware encoders)
        let codec_name = encoder.codec_name();
        let needs_yuv420p = codec_name.contains("libvpx") || codec_name.contains("libx265");

        info!(
            "Encoder {} needs {} format",
            codec_name,
            if needs_yuv420p { "YUV420P" } else { "NV12" }
        );

        // Create converter or decoder based on input format and encoder needs
        info!("Initializing input format handler for: {} -> {}",
              config.input_format,
              if needs_yuv420p { "YUV420P" } else { "NV12" });

        let (nv12_converter, yuv420p_converter, mjpeg_decoder, mjpeg_turbo_decoder) = if needs_yuv420p {
            // Software encoder needs YUV420P
            match config.input_format {
                PixelFormat::Yuv420 => {
                    info!("Using direct YUV420P input (no conversion)");
                    (None, None, None, None)
                }
                PixelFormat::Yuyv => {
                    info!("Using YUYV->YUV420P converter");
                    (None, Some(PixelConverter::yuyv_to_yuv420p(config.resolution)), None, None)
                }
                PixelFormat::Nv12 => {
                    info!("Using NV12->YUV420P converter");
                    (None, Some(PixelConverter::nv12_to_yuv420p(config.resolution)), None, None)
                }
                PixelFormat::Rgb24 => {
                    info!("Using RGB24->YUV420P converter");
                    (None, Some(PixelConverter::rgb24_to_yuv420p(config.resolution)), None, None)
                }
                PixelFormat::Bgr24 => {
                    info!("Using BGR24->YUV420P converter");
                    (None, Some(PixelConverter::bgr24_to_yuv420p(config.resolution)), None, None)
                }
                PixelFormat::Mjpeg | PixelFormat::Jpeg => {
                    // Use turbojpeg for direct MJPEG->YUV420P (no intermediate NV12)
                    info!("Using turbojpeg MJPEG decoder (direct YUV420P output)");
                    let turbo_decoder = MjpegTurboDecoder::new(config.resolution)?;
                    (None, None, None, Some(turbo_decoder))
                }
                _ => {
                    return Err(AppError::VideoError(format!(
                        "Unsupported input format: {}",
                        config.input_format
                    )));
                }
            }
        } else {
            // Hardware encoder needs NV12
            match config.input_format {
                PixelFormat::Nv12 => {
                    info!("Using direct NV12 input (no conversion)");
                    (None, None, None, None)
                }
                PixelFormat::Yuyv => {
                    info!("Using YUYV->NV12 converter");
                    (Some(Nv12Converter::yuyv_to_nv12(config.resolution)), None, None, None)
                }
                PixelFormat::Rgb24 => {
                    info!("Using RGB24->NV12 converter");
                    (Some(Nv12Converter::rgb24_to_nv12(config.resolution)), None, None, None)
                }
                PixelFormat::Bgr24 => {
                    info!("Using BGR24->NV12 converter");
                    (Some(Nv12Converter::bgr24_to_nv12(config.resolution)), None, None, None)
                }
                PixelFormat::Mjpeg | PixelFormat::Jpeg => {
                    info!("Using MJPEG decoder (NV12 output)");
                    let decoder_config = MjpegVaapiDecoderConfig {
                        resolution: config.resolution,
                        use_hwaccel: true,
                    };
                    let decoder = MjpegVaapiDecoder::new(decoder_config)?;
                    (None, None, Some(decoder), None)
                }
                _ => {
                    return Err(AppError::VideoError(format!(
                        "Unsupported input format: {}",
                        config.input_format
                    )));
                }
            }
        };

        *self.encoder.lock().await = Some(encoder);
        *self.nv12_converter.lock().await = nv12_converter;
        *self.yuv420p_converter.lock().await = yuv420p_converter;
        *self.mjpeg_decoder.lock().await = mjpeg_decoder;
        *self.mjpeg_turbo_decoder.lock().await = mjpeg_turbo_decoder;
        self.encoder_needs_yuv420p.store(needs_yuv420p, Ordering::Release);

        Ok(())
    }

    /// Subscribe to encoded frames
    pub fn subscribe(&self) -> broadcast::Receiver<EncodedVideoFrame> {
        self.frame_tx.subscribe()
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.frame_tx.receiver_count()
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

    /// Get current stats
    pub async fn stats(&self) -> SharedVideoPipelineStats {
        let mut stats = self.stats.lock().await.clone();
        stats.subscribers = self.frame_tx.receiver_count() as u64;
        stats
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

    /// Get current codec
    pub async fn current_codec(&self) -> VideoEncoderType {
        self.config.read().await.output_codec
    }

    /// Switch codec (requires restart)
    pub async fn switch_codec(&self, codec: VideoEncoderType) -> Result<()> {
        let was_running = self.is_running();

        if was_running {
            self.stop();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        {
            let mut config = self.config.write().await;
            config.output_codec = codec;
        }

        // Clear encoder state
        *self.encoder.lock().await = None;
        *self.nv12_converter.lock().await = None;
        *self.yuv420p_converter.lock().await = None;
        *self.mjpeg_decoder.lock().await = None;
        *self.mjpeg_turbo_decoder.lock().await = None;
        self.encoder_needs_yuv420p.store(false, Ordering::Release);

        info!("Switched to {} codec", codec);
        Ok(())
    }

    /// Start the pipeline
    pub async fn start(self: &Arc<Self>, mut frame_rx: broadcast::Receiver<VideoFrame>) -> Result<()> {
        if *self.running_rx.borrow() {
            warn!("Pipeline already running");
            return Ok(());
        }

        self.init_encoder().await?;
        let _ = self.running.send(true);

        let config = self.config.read().await.clone();
        info!("Starting {} pipeline", config.output_codec);

        let pipeline = self.clone();

        tokio::spawn(async move {
            let mut frame_count: u64 = 0;
            let mut last_fps_time = Instant::now();
            let mut fps_frame_count: u64 = 0;
            let mut running_rx = pipeline.running_rx.clone();

            // Local counters for batch stats update (reduce lock contention)
            let mut local_frames_encoded: u64 = 0;
            let mut local_bytes_encoded: u64 = 0;
            let mut local_keyframes: u64 = 0;
            let mut local_errors: u64 = 0;
            let mut local_dropped: u64 = 0;

            // Track when we last had subscribers for auto-stop feature
            let mut no_subscribers_since: Option<Instant> = None;
            let grace_period = Duration::from_secs(AUTO_STOP_GRACE_PERIOD_SECS);

            loop {
                tokio::select! {
                    biased;

                    _ = running_rx.changed() => {
                        if !*running_rx.borrow() {
                            break;
                        }
                    }

                    result = frame_rx.recv() => {
                        match result {
                            Ok(video_frame) => {
                                let subscriber_count = pipeline.frame_tx.receiver_count();

                                if subscriber_count == 0 {
                                    // Track when we started having no subscribers
                                    if no_subscribers_since.is_none() {
                                        no_subscribers_since = Some(Instant::now());
                                        trace!("No subscribers, starting grace period timer");
                                    }

                                    // Check if grace period has elapsed
                                    if let Some(since) = no_subscribers_since {
                                        if since.elapsed() >= grace_period {
                                            info!(
                                                "No subscribers for {}s, auto-stopping video pipeline",
                                                grace_period.as_secs()
                                            );
                                            // Signal stop and break out of loop
                                            let _ = pipeline.running.send(false);
                                            break;
                                        }
                                    }

                                    // Skip encoding but continue loop (within grace period)
                                    continue;
                                } else {
                                    // Reset the no-subscriber timer when we have subscribers again
                                    if no_subscribers_since.is_some() {
                                        trace!("Subscriber connected, resetting grace period timer");
                                        no_subscribers_since = None;
                                    }
                                }

                                match pipeline.encode_frame(&video_frame, frame_count).await {
                                    Ok(Some(encoded_frame)) => {
                                        let _ = pipeline.frame_tx.send(encoded_frame.clone());

                                        // Update local counters (no lock)
                                        local_frames_encoded += 1;
                                        local_bytes_encoded += encoded_frame.data.len() as u64;
                                        if encoded_frame.is_keyframe {
                                            local_keyframes += 1;
                                        }

                                        frame_count += 1;
                                        fps_frame_count += 1;
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        error!("Encoding failed: {}", e);
                                        local_errors += 1;
                                    }
                                }

                                // Batch update stats every second (reduces lock contention)
                                if last_fps_time.elapsed() >= Duration::from_secs(1) {
                                    let current_fps = fps_frame_count as f32 / last_fps_time.elapsed().as_secs_f32();
                                    fps_frame_count = 0;
                                    last_fps_time = Instant::now();

                                    // Single lock acquisition for all stats
                                    let mut s = pipeline.stats.lock().await;
                                    s.frames_encoded += local_frames_encoded;
                                    s.bytes_encoded += local_bytes_encoded;
                                    s.keyframes_encoded += local_keyframes;
                                    s.errors += local_errors;
                                    s.frames_dropped += local_dropped;
                                    s.current_fps = current_fps;

                                    // Reset local counters
                                    local_frames_encoded = 0;
                                    local_bytes_encoded = 0;
                                    local_keyframes = 0;
                                    local_errors = 0;
                                    local_dropped = 0;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                local_dropped += n;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                }
            }

            info!("Video pipeline stopped");
        });

        Ok(())
    }

    /// Encode a single frame
    async fn encode_frame(&self, frame: &VideoFrame, frame_count: u64) -> Result<Option<EncodedVideoFrame>> {
        let config = self.config.read().await;
        let raw_frame = frame.data();
        let fps = config.fps;
        let codec = config.output_codec;
        drop(config);

        let pts_ms = (frame_count * 1000 / fps as u64) as i64;

        // Debug log for H265
        if codec == VideoEncoderType::H265 && frame_count % 30 == 1 {
            debug!(
                "[Pipeline-H265] Processing frame #{}: input_size={}, pts_ms={}",
                frame_count,
                raw_frame.len(),
                pts_ms
            );
        }

        let mut mjpeg_decoder = self.mjpeg_decoder.lock().await;
        let mut mjpeg_turbo_decoder = self.mjpeg_turbo_decoder.lock().await;
        let mut nv12_converter = self.nv12_converter.lock().await;
        let mut yuv420p_converter = self.yuv420p_converter.lock().await;
        let needs_yuv420p = self.encoder_needs_yuv420p.load(Ordering::Acquire);
        let mut encoder_guard = self.encoder.lock().await;

        let encoder = encoder_guard.as_mut().ok_or_else(|| {
            AppError::VideoError("Encoder not initialized".to_string())
        })?;

        // Check and consume keyframe request (atomic, no lock contention)
        if self.keyframe_requested.swap(false, Ordering::AcqRel) {
            encoder.request_keyframe();
            debug!("[Pipeline] Keyframe will be generated for this frame");
        }

        let encode_result = if mjpeg_turbo_decoder.is_some() {
            // Optimized path: MJPEG -> YUV420P directly via turbojpeg (for software encoders)
            let turbo = mjpeg_turbo_decoder.as_mut().unwrap();
            let mut yuv420p_buffer = self.yuv420p_buffer.lock().await;
            let written = turbo.decode_to_yuv420p_buffer(raw_frame, &mut yuv420p_buffer)
                .map_err(|e| AppError::VideoError(format!("turbojpeg decode failed: {}", e)))?;
            encoder.encode_raw(&yuv420p_buffer[..written], pts_ms)
        } else if mjpeg_decoder.is_some() {
            // MJPEG input: decode to NV12 (for hardware encoders)
            let decoder = mjpeg_decoder.as_mut().unwrap();
            let nv12_frame = decoder.decode(raw_frame)
                .map_err(|e| AppError::VideoError(format!("MJPEG decode failed: {}", e)))?;

            let required_size = (nv12_frame.width * nv12_frame.height * 3 / 2) as usize;
            let mut nv12_buffer = self.nv12_buffer.lock().await;
            if nv12_buffer.len() < required_size {
                nv12_buffer.resize(required_size, 0);
            }

            let written = nv12_frame.copy_to_packed_nv12(&mut nv12_buffer)
                .expect("Buffer too small");

            // Debug log for H265 after MJPEG decode
            if codec == VideoEncoderType::H265 && frame_count % 30 == 1 {
                debug!(
                    "[Pipeline-H265] MJPEG decoded: nv12_size={}, frame_width={}, frame_height={}",
                    written, nv12_frame.width, nv12_frame.height
                );
            }

            encoder.encode_raw(&nv12_buffer[..written], pts_ms)
        } else if needs_yuv420p && yuv420p_converter.is_some() {
            // Software encoder with direct input conversion to YUV420P
            let conv = yuv420p_converter.as_mut().unwrap();
            let yuv420p_data = conv.convert(raw_frame)
                .map_err(|e| AppError::VideoError(format!("YUV420P conversion failed: {}", e)))?;
            encoder.encode_raw(yuv420p_data, pts_ms)
        } else if nv12_converter.is_some() {
            // Hardware encoder with input conversion to NV12
            let conv = nv12_converter.as_mut().unwrap();
            let nv12_data = conv.convert(raw_frame)
                .map_err(|e| AppError::VideoError(format!("NV12 conversion failed: {}", e)))?;
            encoder.encode_raw(nv12_data, pts_ms)
        } else {
            // Direct input (already in correct format)
            encoder.encode_raw(raw_frame, pts_ms)
        };

        drop(encoder_guard);
        drop(nv12_converter);
        drop(yuv420p_converter);
        drop(mjpeg_decoder);
        drop(mjpeg_turbo_decoder);

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

                    let config = self.config.read().await;
                    Ok(Some(EncodedVideoFrame {
                        data: Bytes::from(encoded.data),
                        pts_ms,
                        is_keyframe,
                        sequence,
                        duration: Duration::from_millis(1000 / config.fps as u64),
                        codec,
                    }))
                } else {
                    if codec == VideoEncoderType::H265 {
                        warn!("[Pipeline-H265] Encoder returned no frames for frame #{}", frame_count);
                    }
                    Ok(None)
                }
            }
            Err(e) => {
                if codec == VideoEncoderType::H265 {
                    error!("[Pipeline-H265] Encode error at frame #{}: {}", frame_count, e);
                }
                Err(e)
            },
        }
    }

    /// Stop the pipeline
    pub fn stop(&self) {
        if *self.running_rx.borrow() {
            let _ = self.running.send(false);
            info!("Stopping video pipeline");
        }
    }

    /// Set bitrate
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<()> {
        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate_kbps)?;
            self.config.write().await.bitrate_kbps = bitrate_kbps;
        }
        Ok(())
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
        } else if i + 3 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 1
        {
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

    #[test]
    fn test_pipeline_config() {
        let h264 = SharedVideoPipelineConfig::h264(Resolution::HD1080, 4000);
        assert_eq!(h264.output_codec, VideoEncoderType::H264);

        let h265 = SharedVideoPipelineConfig::h265(Resolution::HD720, 2000);
        assert_eq!(h265.output_codec, VideoEncoderType::H265);
    }
}
