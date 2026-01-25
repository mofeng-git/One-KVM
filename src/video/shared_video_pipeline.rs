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
use parking_lot::RwLock as ParkingRwLock;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

/// Grace period before auto-stopping pipeline when no subscribers (in seconds)
const AUTO_STOP_GRACE_PERIOD_SECS: u64 = 3;
/// Minimum valid frame size for capture
const MIN_CAPTURE_FRAME_SIZE: usize = 128;
/// Validate JPEG header every N frames to reduce overhead
const JPEG_VALIDATE_INTERVAL: u64 = 30;

use crate::error::{AppError, Result};
use crate::video::convert::{Nv12Converter, PixelConverter};
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use crate::video::decoder::MjpegRkmppDecoder;
use crate::video::decoder::MjpegTurboDecoder;
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
use hwcodec::ffmpeg_hw::{last_error_message as ffmpeg_hw_last_error, HwMjpegH264Config, HwMjpegH264Pipeline};
use v4l::buffer::Type as BufferType;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;
use v4l::video::capture::Parameters;
use v4l::Format;
use crate::video::encoder::h264::{detect_best_encoder, H264Config, H264Encoder, H264InputFormat};
use crate::video::encoder::h265::{
    detect_best_h265_encoder, H265Config, H265Encoder, H265InputFormat,
};
use crate::video::encoder::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use crate::video::encoder::traits::EncoderConfig;
use crate::video::encoder::vp8::{detect_best_vp8_encoder, VP8Config, VP8Encoder};
use crate::video::encoder::vp9::{detect_best_vp9_encoder, VP9Config, VP9Encoder};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::frame::{FrameBuffer, FrameBufferPool, VideoFrame};

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

/// Pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedVideoPipelineStats {
    pub current_fps: f32,
}

struct EncoderThreadState {
    encoder: Option<Box<dyn VideoEncoderTrait + Send>>,
    mjpeg_decoder: Option<MjpegDecoderKind>,
    nv12_converter: Option<Nv12Converter>,
    yuv420p_converter: Option<PixelConverter>,
    encoder_needs_yuv420p: bool,
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    ffmpeg_hw_pipeline: Option<HwMjpegH264Pipeline>,
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    ffmpeg_hw_enabled: bool,
    fps: u32,
    codec: VideoEncoderType,
    input_format: PixelFormat,
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

enum MjpegDecoderKind {
    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    Rkmpp(MjpegRkmppDecoder),
    Turbo(MjpegTurboDecoder),
}

impl MjpegDecoderKind {
    fn decode(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            MjpegDecoderKind::Rkmpp(decoder) => decoder.decode_to_nv12(data),
            MjpegDecoderKind::Turbo(decoder) => decoder.decode_to_rgb(data),
        }
    }
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
        });

        Ok(pipeline)
    }

    fn build_encoder_state(config: &SharedVideoPipelineConfig) -> Result<EncoderThreadState> {
        let registry = EncoderRegistry::global();

        // Helper to get codec name for specific backend
        let get_codec_name =
            |format: VideoEncoderType, backend: Option<EncoderBackend>| -> Option<String> {
                match backend {
                    Some(b) => registry
                        .encoder_with_backend(format, b)
                        .map(|e| e.codec_name.clone()),
                    None => registry
                        .best_encoder(format, false)
                        .map(|e| e.codec_name.clone()),
                }
            };

        let needs_mjpeg_decode = config.input_format.is_compressed();

        // Check if RKMPP backend is available for direct input optimization
        let is_rkmpp_available = registry
            .encoder_with_backend(VideoEncoderType::H264, EncoderBackend::Rkmpp)
            .is_some();
        let use_yuyv_direct =
            is_rkmpp_available && !needs_mjpeg_decode && config.input_format == PixelFormat::Yuyv;
        let use_rkmpp_direct = is_rkmpp_available
            && !needs_mjpeg_decode
            && matches!(
                config.input_format,
                PixelFormat::Yuyv
                    | PixelFormat::Yuv420
                    | PixelFormat::Rgb24
                    | PixelFormat::Bgr24
                    | PixelFormat::Nv12
                    | PixelFormat::Nv16
                    | PixelFormat::Nv21
                    | PixelFormat::Nv24
            );

        if use_yuyv_direct {
            info!(
                "RKMPP backend detected with YUYV input, enabling YUYV direct input optimization"
            );
        } else if use_rkmpp_direct {
            info!(
                "RKMPP backend detected with {} input, enabling direct input optimization",
                config.input_format
            );
        }

        let selected_codec_name = match config.output_codec {
            VideoEncoderType::H264 => {
                if use_rkmpp_direct {
                    // Force RKMPP backend for direct input
                    get_codec_name(VideoEncoderType::H264, Some(EncoderBackend::Rkmpp)).ok_or_else(
                        || {
                            AppError::VideoError(
                                "RKMPP backend not available for H.264".to_string(),
                            )
                        },
                    )?
                } else if let Some(ref backend) = config.encoder_backend {
                    // Specific backend requested
                    get_codec_name(VideoEncoderType::H264, Some(*backend)).ok_or_else(|| {
                        AppError::VideoError(format!(
                            "Backend {:?} does not support H.264",
                            backend
                        ))
                    })?
                } else {
                    // Auto select best available encoder
                    let (_encoder_type, detected) =
                        detect_best_encoder(config.resolution.width, config.resolution.height);
                    detected.ok_or_else(|| {
                        AppError::VideoError("No H.264 encoder available".to_string())
                    })?
                }
            }
            VideoEncoderType::H265 => {
                if use_rkmpp_direct {
                    get_codec_name(VideoEncoderType::H265, Some(EncoderBackend::Rkmpp)).ok_or_else(
                        || {
                            AppError::VideoError(
                                "RKMPP backend not available for H.265".to_string(),
                            )
                        },
                    )?
                } else if let Some(ref backend) = config.encoder_backend {
                    get_codec_name(VideoEncoderType::H265, Some(*backend)).ok_or_else(|| {
                        AppError::VideoError(format!(
                            "Backend {:?} does not support H.265",
                            backend
                        ))
                    })?
                } else {
                    let (_encoder_type, detected) =
                        detect_best_h265_encoder(config.resolution.width, config.resolution.height);
                    detected.ok_or_else(|| {
                        AppError::VideoError("No H.265 encoder available".to_string())
                    })?
                }
            }
            VideoEncoderType::VP8 => {
                if let Some(ref backend) = config.encoder_backend {
                    get_codec_name(VideoEncoderType::VP8, Some(*backend)).ok_or_else(|| {
                        AppError::VideoError(format!("Backend {:?} does not support VP8", backend))
                    })?
                } else {
                    let (_encoder_type, detected) =
                        detect_best_vp8_encoder(config.resolution.width, config.resolution.height);
                    detected.ok_or_else(|| {
                        AppError::VideoError("No VP8 encoder available".to_string())
                    })?
                }
            }
            VideoEncoderType::VP9 => {
                if let Some(ref backend) = config.encoder_backend {
                    get_codec_name(VideoEncoderType::VP9, Some(*backend)).ok_or_else(|| {
                        AppError::VideoError(format!("Backend {:?} does not support VP9", backend))
                    })?
                } else {
                    let (_encoder_type, detected) =
                        detect_best_vp9_encoder(config.resolution.width, config.resolution.height);
                    detected.ok_or_else(|| {
                        AppError::VideoError("No VP9 encoder available".to_string())
                    })?
                }
            }
        };

        let is_rkmpp_encoder = selected_codec_name.contains("rkmpp");
        let is_software_encoder = selected_codec_name.contains("libx264")
            || selected_codec_name.contains("libx265")
            || selected_codec_name.contains("libvpx");

        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        if needs_mjpeg_decode && is_rkmpp_encoder && config.output_codec == VideoEncoderType::H264 {
            info!("Initializing FFmpeg HW MJPEG->H264 pipeline (no fallback)");
            let hw_config = HwMjpegH264Config {
                decoder: "mjpeg_rkmpp".to_string(),
                encoder: selected_codec_name.clone(),
                width: config.resolution.width as i32,
                height: config.resolution.height as i32,
                fps: config.fps as i32,
                bitrate_kbps: config.bitrate_kbps() as i32,
                gop: config.gop_size() as i32,
                thread_count: 1,
            };
            let pipeline = HwMjpegH264Pipeline::new(hw_config).map_err(|e| {
                let detail = if e.is_empty() { ffmpeg_hw_last_error() } else { e };
                AppError::VideoError(format!(
                    "FFmpeg HW MJPEG->H264 init failed: {}",
                    detail
                ))
            })?;
            info!("Using FFmpeg HW MJPEG->H264 pipeline");
            return Ok(EncoderThreadState {
                encoder: None,
                mjpeg_decoder: None,
                nv12_converter: None,
                yuv420p_converter: None,
                encoder_needs_yuv420p: false,
                #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
                ffmpeg_hw_pipeline: Some(pipeline),
                #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
                ffmpeg_hw_enabled: true,
                fps: config.fps,
                codec: config.output_codec,
                input_format: config.input_format,
            });
        }

        let pipeline_input_format = if needs_mjpeg_decode {
            if is_rkmpp_encoder {
                info!(
                    "MJPEG input detected, using RKMPP decoder ({} -> NV12 with NV16 fallback)",
                    config.input_format
                );
                #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
                {
                    let decoder = MjpegRkmppDecoder::new(config.resolution)?;
                    let pipeline_format = PixelFormat::Nv12;
                    (Some(MjpegDecoderKind::Rkmpp(decoder)), pipeline_format)
                }
                #[cfg(not(any(target_arch = "aarch64", target_arch = "arm")))]
                {
                    return Err(AppError::VideoError(
                        "RKMPP MJPEG decode is only supported on ARM builds".to_string(),
                    ));
                }
            } else if is_software_encoder {
                info!(
                    "MJPEG input detected, using TurboJPEG decoder ({} -> RGB24)",
                    config.input_format
                );
                let decoder = MjpegTurboDecoder::new(config.resolution)?;
                (Some(MjpegDecoderKind::Turbo(decoder)), PixelFormat::Rgb24)
            } else {
                return Err(AppError::VideoError(
                    "MJPEG input requires RKMPP or software encoder".to_string(),
                ));
            }
        } else {
            (None, config.input_format)
        };
        let (mjpeg_decoder, pipeline_input_format) = pipeline_input_format;

        // Create encoder based on codec type
        let encoder: Box<dyn VideoEncoderTrait + Send> = match config.output_codec {
            VideoEncoderType::H264 => {
                let codec_name = selected_codec_name.clone();

                let is_rkmpp = codec_name.contains("rkmpp");
                let direct_input_format = if is_rkmpp {
                    match pipeline_input_format {
                        PixelFormat::Yuyv => Some(H264InputFormat::Yuyv422),
                        PixelFormat::Yuv420 => Some(H264InputFormat::Yuv420p),
                        PixelFormat::Rgb24 => Some(H264InputFormat::Rgb24),
                        PixelFormat::Bgr24 => Some(H264InputFormat::Bgr24),
                        PixelFormat::Nv12 => Some(H264InputFormat::Nv12),
                        PixelFormat::Nv16 => Some(H264InputFormat::Nv16),
                        PixelFormat::Nv21 => Some(H264InputFormat::Nv21),
                        PixelFormat::Nv24 => Some(H264InputFormat::Nv24),
                        _ => None,
                    }
                } else if codec_name.contains("libx264") {
                    match pipeline_input_format {
                        PixelFormat::Nv12 => Some(H264InputFormat::Nv12),
                        PixelFormat::Nv16 => Some(H264InputFormat::Nv16),
                        PixelFormat::Nv21 => Some(H264InputFormat::Nv21),
                        PixelFormat::Yuv420 => Some(H264InputFormat::Yuv420p),
                        _ => None,
                    }
                } else {
                    None
                };

                // Choose input format: prefer direct input when supported
                let h264_input_format = if let Some(fmt) = direct_input_format {
                    fmt
                } else if codec_name.contains("libx264") {
                    H264InputFormat::Yuv420p
                } else {
                    H264InputFormat::Nv12
                };

                let encoder_config = H264Config {
                    base: EncoderConfig::h264(config.resolution, config.bitrate_kbps()),
                    bitrate_kbps: config.bitrate_kbps(),
                    gop_size: config.gop_size(),
                    fps: config.fps,
                    input_format: h264_input_format,
                };

                if use_rkmpp_direct {
                    info!(
                        "Creating H264 encoder with RKMPP backend for {} direct input (codec: {})",
                        config.input_format, codec_name
                    );
                } else if let Some(ref backend) = config.encoder_backend {
                    info!(
                        "Creating H264 encoder with backend {:?} (codec: {})",
                        backend, codec_name
                    );
                }

                let encoder = H264Encoder::with_codec(encoder_config, &codec_name)?;

                info!("Created H264 encoder: {}", encoder.codec_name());
                Box::new(H264EncoderWrapper(encoder))
            }
            VideoEncoderType::H265 => {
                let codec_name = selected_codec_name.clone();

                let is_rkmpp = codec_name.contains("rkmpp");
                let direct_input_format = if is_rkmpp {
                    match pipeline_input_format {
                        PixelFormat::Yuyv => Some(H265InputFormat::Yuyv422),
                        PixelFormat::Yuv420 => Some(H265InputFormat::Yuv420p),
                        PixelFormat::Rgb24 => Some(H265InputFormat::Rgb24),
                        PixelFormat::Bgr24 => Some(H265InputFormat::Bgr24),
                        PixelFormat::Nv12 => Some(H265InputFormat::Nv12),
                        PixelFormat::Nv16 => Some(H265InputFormat::Nv16),
                        PixelFormat::Nv21 => Some(H265InputFormat::Nv21),
                        PixelFormat::Nv24 => Some(H265InputFormat::Nv24),
                        _ => None,
                    }
                } else if codec_name.contains("libx265") {
                    match pipeline_input_format {
                        PixelFormat::Yuv420 => Some(H265InputFormat::Yuv420p),
                        _ => None,
                    }
                } else {
                    None
                };

                let h265_input_format = if let Some(fmt) = direct_input_format {
                    fmt
                } else if codec_name.contains("libx265") {
                    H265InputFormat::Yuv420p
                } else {
                    H265InputFormat::Nv12
                };

                let encoder_config = H265Config {
                    base: EncoderConfig {
                        resolution: config.resolution,
                        input_format: config.input_format,
                        quality: config.bitrate_kbps(),
                        fps: config.fps,
                        gop_size: config.gop_size(),
                    },
                    bitrate_kbps: config.bitrate_kbps(),
                    gop_size: config.gop_size(),
                    fps: config.fps,
                    input_format: h265_input_format,
                };

                if use_rkmpp_direct {
                    info!(
                        "Creating H265 encoder with RKMPP backend for {} direct input (codec: {})",
                        config.input_format, codec_name
                    );
                } else if let Some(ref backend) = config.encoder_backend {
                    info!(
                        "Creating H265 encoder with backend {:?} (codec: {})",
                        backend, codec_name
                    );
                }

                let encoder = H265Encoder::with_codec(encoder_config, &codec_name)?;

                info!("Created H265 encoder: {}", encoder.codec_name());
                Box::new(H265EncoderWrapper(encoder))
            }
            VideoEncoderType::VP8 => {
                let encoder_config =
                    VP8Config::low_latency(config.resolution, config.bitrate_kbps());
                let codec_name = selected_codec_name.clone();
                if let Some(ref backend) = config.encoder_backend {
                    info!(
                        "Creating VP8 encoder with backend {:?} (codec: {})",
                        backend, codec_name
                    );
                }
                let encoder = VP8Encoder::with_codec(encoder_config, &codec_name)?;

                info!("Created VP8 encoder: {}", encoder.codec_name());
                Box::new(VP8EncoderWrapper(encoder))
            }
            VideoEncoderType::VP9 => {
                let encoder_config =
                    VP9Config::low_latency(config.resolution, config.bitrate_kbps());
                let codec_name = selected_codec_name.clone();
                if let Some(ref backend) = config.encoder_backend {
                    info!(
                        "Creating VP9 encoder with backend {:?} (codec: {})",
                        backend, codec_name
                    );
                }
                let encoder = VP9Encoder::with_codec(encoder_config, &codec_name)?;

                info!("Created VP9 encoder: {}", encoder.codec_name());
                Box::new(VP9EncoderWrapper(encoder))
            }
        };

        // Determine if encoder can take direct input without conversion
        let codec_name = encoder.codec_name();
        let use_direct_input = if codec_name.contains("rkmpp") {
            matches!(
                pipeline_input_format,
                PixelFormat::Yuyv
                    | PixelFormat::Yuv420
                    | PixelFormat::Rgb24
                    | PixelFormat::Bgr24
                    | PixelFormat::Nv12
                    | PixelFormat::Nv16
                    | PixelFormat::Nv21
                    | PixelFormat::Nv24
            )
        } else if codec_name.contains("libx264") {
            matches!(
                pipeline_input_format,
                PixelFormat::Nv12 | PixelFormat::Nv16 | PixelFormat::Nv21 | PixelFormat::Yuv420
            )
        } else {
            false
        };

        // Determine if encoder needs YUV420P (software encoders) or NV12 (hardware encoders)
        let needs_yuv420p = if codec_name.contains("libx264") {
            !matches!(
                pipeline_input_format,
                PixelFormat::Nv12 | PixelFormat::Nv16 | PixelFormat::Nv21 | PixelFormat::Yuv420
            )
        } else {
            codec_name.contains("libvpx") || codec_name.contains("libx265")
        };

        info!(
            "Encoder {} needs {} format",
            codec_name,
            if use_direct_input {
                "direct"
            } else if needs_yuv420p {
                "YUV420P"
            } else {
                "NV12"
            }
        );

        // Create converter or decoder based on input format and encoder needs
        info!(
            "Initializing input format handler for: {} -> {}",
            pipeline_input_format,
            if use_direct_input {
                "direct"
            } else if needs_yuv420p {
                "YUV420P"
            } else {
                "NV12"
            }
        );

        let (nv12_converter, yuv420p_converter) = if use_yuyv_direct {
            // RKMPP with YUYV direct input - skip all conversion
            info!("YUYV direct input enabled for RKMPP, skipping format conversion");
            (None, None)
        } else if use_direct_input {
            info!("Direct input enabled, skipping format conversion");
            (None, None)
        } else if needs_yuv420p {
            // Software encoder needs YUV420P
            match pipeline_input_format {
                PixelFormat::Yuv420 => {
                    info!("Using direct YUV420P input (no conversion)");
                    (None, None)
                }
                PixelFormat::Yuyv => {
                    info!("Using YUYV->YUV420P converter");
                    (
                        None,
                        Some(PixelConverter::yuyv_to_yuv420p(config.resolution)),
                    )
                }
                PixelFormat::Nv12 => {
                    info!("Using NV12->YUV420P converter");
                    (
                        None,
                        Some(PixelConverter::nv12_to_yuv420p(config.resolution)),
                    )
                }
                PixelFormat::Nv21 => {
                    info!("Using NV21->YUV420P converter");
                    (
                        None,
                        Some(PixelConverter::nv21_to_yuv420p(config.resolution)),
                    )
                }
                PixelFormat::Rgb24 => {
                    info!("Using RGB24->YUV420P converter");
                    (
                        None,
                        Some(PixelConverter::rgb24_to_yuv420p(config.resolution)),
                    )
                }
                PixelFormat::Bgr24 => {
                    info!("Using BGR24->YUV420P converter");
                    (
                        None,
                        Some(PixelConverter::bgr24_to_yuv420p(config.resolution)),
                    )
                }
                _ => {
                    return Err(AppError::VideoError(format!(
                        "Unsupported input format for software encoding: {}",
                        pipeline_input_format
                    )));
                }
            }
        } else {
            // Hardware encoder needs NV12
            match pipeline_input_format {
                PixelFormat::Nv12 => {
                    info!("Using direct NV12 input (no conversion)");
                    (None, None)
                }
                PixelFormat::Yuyv => {
                    info!("Using YUYV->NV12 converter");
                    (Some(Nv12Converter::yuyv_to_nv12(config.resolution)), None)
                }
                PixelFormat::Nv21 => {
                    info!("Using NV21->NV12 converter");
                    (Some(Nv12Converter::nv21_to_nv12(config.resolution)), None)
                }
                PixelFormat::Nv16 => {
                    info!("Using NV16->NV12 converter");
                    (Some(Nv12Converter::nv16_to_nv12(config.resolution)), None)
                }
                PixelFormat::Yuv420 => {
                    info!("Using YUV420P->NV12 converter");
                    (Some(Nv12Converter::yuv420_to_nv12(config.resolution)), None)
                }
                PixelFormat::Rgb24 => {
                    info!("Using RGB24->NV12 converter");
                    (Some(Nv12Converter::rgb24_to_nv12(config.resolution)), None)
                }
                PixelFormat::Bgr24 => {
                    info!("Using BGR24->NV12 converter");
                    (Some(Nv12Converter::bgr24_to_nv12(config.resolution)), None)
                }
                _ => {
                    return Err(AppError::VideoError(format!(
                        "Unsupported input format for hardware encoding: {}",
                        pipeline_input_format
                    )));
                }
            }
        };

        Ok(EncoderThreadState {
            encoder: Some(encoder),
            mjpeg_decoder,
            nv12_converter,
            yuv420p_converter,
            encoder_needs_yuv420p: needs_yuv420p,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_pipeline: None,
            #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
            ffmpeg_hw_enabled: false,
            fps: config.fps,
            codec: config.output_codec,
            input_format: config.input_format,
        })
    }

    /// Subscribe to encoded frames
    pub fn subscribe(&self) -> mpsc::Receiver<Arc<EncodedVideoFrame>> {
        let (tx, rx) = mpsc::channel(4);
        self.subscribers.write().push(tx);
        rx
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().iter().filter(|tx| !tx.is_closed()).count()
    }

    /// Report that a receiver has lagged behind
    ///
    /// Call this when a broadcast receiver detects it has fallen behind
    /// (e.g., when RecvError::Lagged is received).
    ///
    /// # Arguments
    ///
    /// * `_frames_lagged` - Number of frames the receiver has lagged (currently unused)
    pub async fn report_lag(&self, _frames_lagged: u64) {
        // No-op: backpressure control removed as it was not effective
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
                                let detail = if e.is_empty() { ffmpeg_hw_last_error() } else { e };
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

        self.clear_cmd_tx();

        info!("Switched to {} codec", codec);
        Ok(())
    }

    /// Start the pipeline
    pub async fn start(
        self: &Arc<Self>,
        mut frame_rx: broadcast::Receiver<VideoFrame>,
    ) -> Result<()> {
        if *self.running_rx.borrow() {
            warn!("Pipeline already running");
            return Ok(());
        }

        let config = self.config.read().await.clone();
        let mut encoder_state = Self::build_encoder_state(&config)?;
        let _ = self.running.send(true);
        self.running_flag.store(true, Ordering::Release);
        let gop_size = config.gop_size();
        info!(
            "Starting {} pipeline (GOP={})",
            config.output_codec, gop_size
        );

        let pipeline = self.clone();
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        {
            let mut guard = self.cmd_tx.write();
            *guard = Some(cmd_tx);
        }

        tokio::spawn(async move {
            let mut frame_count: u64 = 0;
            let mut last_fps_time = Instant::now();
            let mut fps_frame_count: u64 = 0;
            let mut running_rx = pipeline.running_rx.clone();

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
                                while let Ok(cmd) = cmd_rx.try_recv() {
                                    if let Err(e) = pipeline.apply_cmd(&mut encoder_state, cmd) {
                                        error!("Failed to apply pipeline command: {}", e);
                                    }
                                }
                                let subscriber_count = pipeline.subscriber_count();

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
                                            pipeline
                                                .running_flag
                                                .store(false, Ordering::Release);
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

                                match pipeline.encode_frame_sync(&mut encoder_state, &video_frame, frame_count) {
                                    Ok(Some(encoded_frame)) => {
                                        let encoded_arc = Arc::new(encoded_frame);
                                        pipeline.broadcast_encoded(encoded_arc).await;

                                        frame_count += 1;
                                        fps_frame_count += 1;
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        error!("Encoding failed: {}", e);
                                    }
                                }

                                // Update FPS every second (reduces lock contention)
                                let fps_elapsed = last_fps_time.elapsed();
                                if fps_elapsed >= Duration::from_secs(1) {
                                    let current_fps =
                                        fps_frame_count as f32 / fps_elapsed.as_secs_f32();
                                    fps_frame_count = 0;
                                    last_fps_time = Instant::now();

                                    // Single lock acquisition for FPS
                                    let mut s = pipeline.stats.lock().await;
                                    s.current_fps = current_fps;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                let _ = n;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                        }
                    }
                }
            }

            pipeline.clear_cmd_tx();
            pipeline.running_flag.store(false, Ordering::Release);
            info!("Video pipeline stopped");
        });

        Ok(())
    }

    /// Start the pipeline by owning capture + encode in a single loop.
    ///
    /// This avoids the raw-frame broadcast path and keeps capture and encode
    /// in the same thread for lower overhead.
    pub async fn start_with_device(
        self: &Arc<Self>,
        device_path: std::path::PathBuf,
        buffer_count: u32,
        _jpeg_quality: u8,
    ) -> Result<()> {
        if *self.running_rx.borrow() {
            warn!("Pipeline already running");
            return Ok(());
        }

        let config = self.config.read().await.clone();
        let mut encoder_state = Self::build_encoder_state(&config)?;
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
                            error!("Encoding failed: {}", e);
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
            std::thread::spawn(move || {
                let device = match Device::with_path(&device_path) {
                    Ok(d) => d,
                    Err(e) => {
                        error!("Failed to open device {:?}: {}", device_path, e);
                        let _ = pipeline.running.send(false);
                        pipeline.running_flag.store(false, Ordering::Release);
                        let _ = frame_seq_tx.send(1);
                        return;
                    }
                };

                let requested_format = Format::new(
                    config.resolution.width,
                    config.resolution.height,
                    config.input_format.to_fourcc(),
                );

                let actual_format = match device.set_format(&requested_format) {
                    Ok(f) => f,
                    Err(e) => {
                        error!("Failed to set capture format: {}", e);
                        let _ = pipeline.running.send(false);
                        pipeline.running_flag.store(false, Ordering::Release);
                        let _ = frame_seq_tx.send(1);
                        return;
                    }
                };

                let resolution = Resolution::new(actual_format.width, actual_format.height);
                let pixel_format =
                    PixelFormat::from_fourcc(actual_format.fourcc).unwrap_or(config.input_format);
                let stride = actual_format.stride;

                if config.fps > 0 {
                    if let Err(e) = device.set_params(&Parameters::with_fps(config.fps)) {
                        warn!("Failed to set hardware FPS: {}", e);
                    }
                }

                let mut stream = match MmapStream::with_buffers(
                    &device,
                    BufferType::VideoCapture,
                    buffer_count.max(1),
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to create capture stream: {}", e);
                        let _ = pipeline.running.send(false);
                        pipeline.running_flag.store(false, Ordering::Release);
                        let _ = frame_seq_tx.send(1);
                        return;
                    }
                };

                let mut no_subscribers_since: Option<Instant> = None;
                let grace_period = Duration::from_secs(AUTO_STOP_GRACE_PERIOD_SECS);
                let mut sequence: u64 = 0;
                let mut validate_counter: u64 = 0;

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

                    let (buf, meta) = match stream.next() {
                        Ok(frame_data) => frame_data,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::TimedOut {
                                warn!("Capture timeout - no signal?");
                            } else {
                                error!("Capture error: {}", e);
                            }
                            continue;
                        }
                    };

                    let frame_size = meta.bytesused as usize;
                    if frame_size < MIN_CAPTURE_FRAME_SIZE {
                        continue;
                    }

                    validate_counter = validate_counter.wrapping_add(1);
                    if pixel_format.is_compressed()
                        && validate_counter % JPEG_VALIDATE_INTERVAL == 0
                        && !VideoFrame::is_valid_jpeg_bytes(&buf[..frame_size])
                    {
                        continue;
                    }

                    let mut owned = buffer_pool.take(frame_size);
                    owned.resize(frame_size, 0);
                    owned[..frame_size].copy_from_slice(&buf[..frame_size]);
                    let frame = Arc::new(VideoFrame::from_pooled(
                        Arc::new(FrameBuffer::new(owned, Some(buffer_pool.clone()))),
                        resolution,
                        pixel_format,
                        stride,
                        sequence,
                    ));
                    sequence = sequence.wrapping_add(1);

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
                let detail = if e.is_empty() { ffmpeg_hw_last_error() } else { e };
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
            let decoder = state.mjpeg_decoder.as_mut().ok_or_else(|| {
                AppError::VideoError("MJPEG decoder not initialized".to_string())
            })?;
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

        let encode_result = if needs_yuv420p && state.yuv420p_converter.is_some() {
            // Software encoder with direct input conversion to YUV420P
            let conv = state.yuv420p_converter.as_mut().unwrap();
            let yuv420p_data = conv
                .convert(raw_frame)
                .map_err(|e| AppError::VideoError(format!("YUV420P conversion failed: {}", e)))?;
            encoder.encode_raw(yuv420p_data, pts_ms)
        } else if state.nv12_converter.is_some() {
            // Hardware encoder with input conversion to NV12
            let conv = state.nv12_converter.as_mut().unwrap();
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

    /// Stop the pipeline
    pub fn stop(&self) {
        if *self.running_rx.borrow() {
            let _ = self.running.send(false);
            self.running_flag.store(false, Ordering::Release);
            self.clear_cmd_tx();
            info!("Stopping video pipeline");
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
