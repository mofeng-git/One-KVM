//! H.265/HEVC encoder using hwcodec (FFmpeg wrapper)
//!
//! Supports both hardware and software encoding:
//! - Hardware: VAAPI, NVENC, QSV, AMF, RKMPP, V4L2 M2M
//! - Software: libx265 (CPU-based, high CPU usage)
//!
//! Hardware encoding is preferred when available for better performance.

use bytes::Bytes;
use std::sync::Once;
use tracing::{debug, error, info, warn};

use hwcodec::common::{DataFormat, Quality, RateControl};
use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::encode::{EncodeContext, Encoder as HwEncoder};
use hwcodec::ffmpeg_ram::CodecInfo;

use super::registry::{EncoderBackend, EncoderRegistry, VideoEncoderType};
use super::traits::{EncodedFormat, EncodedFrame, Encoder, EncoderConfig};
use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

static INIT_LOGGING: Once = Once::new();

/// Initialize hwcodec logging (only once)
fn init_hwcodec_logging() {
    INIT_LOGGING.call_once(|| {
        debug!("hwcodec logging initialized for H265");
    });
}

/// H.265 encoder type (detected from hwcodec)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum H265EncoderType {
    /// NVIDIA NVENC
    Nvenc,
    /// Intel Quick Sync (QSV)
    Qsv,
    /// AMD AMF
    Amf,
    /// VAAPI (Linux generic)
    Vaapi,
    /// RKMPP (Rockchip)
    Rkmpp,
    /// V4L2 M2M (ARM generic)
    V4l2M2m,
    /// Software encoder (libx265)
    Software,
    /// No encoder available
    #[default]
    None,
}

impl std::fmt::Display for H265EncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            H265EncoderType::Nvenc => write!(f, "NVENC"),
            H265EncoderType::Qsv => write!(f, "QSV"),
            H265EncoderType::Amf => write!(f, "AMF"),
            H265EncoderType::Vaapi => write!(f, "VAAPI"),
            H265EncoderType::Rkmpp => write!(f, "RKMPP"),
            H265EncoderType::V4l2M2m => write!(f, "V4L2 M2M"),
            H265EncoderType::Software => write!(f, "Software"),
            H265EncoderType::None => write!(f, "None"),
        }
    }
}

impl From<EncoderBackend> for H265EncoderType {
    fn from(backend: EncoderBackend) -> Self {
        match backend {
            EncoderBackend::Nvenc => H265EncoderType::Nvenc,
            EncoderBackend::Qsv => H265EncoderType::Qsv,
            EncoderBackend::Amf => H265EncoderType::Amf,
            EncoderBackend::Vaapi => H265EncoderType::Vaapi,
            EncoderBackend::Rkmpp => H265EncoderType::Rkmpp,
            EncoderBackend::V4l2m2m => H265EncoderType::V4l2M2m,
            EncoderBackend::Software => H265EncoderType::Software,
        }
    }
}

/// Input pixel format for H265 encoder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum H265InputFormat {
    /// YUV420P (I420) - planar Y, U, V
    Yuv420p,
    /// NV12 - Y plane + interleaved UV plane (optimal for hardware encoders)
    #[default]
    Nv12,
    /// NV21 - Y plane + interleaved VU plane
    Nv21,
    /// NV16 - Y plane + interleaved UV plane (4:2:2)
    Nv16,
    /// NV24 - Y plane + interleaved UV plane (4:4:4)
    Nv24,
    /// YUYV422 - packed YUV 4:2:2 format (optimal for RKMPP direct input)
    Yuyv422,
    /// RGB24 - packed RGB format (RKMPP direct input)
    Rgb24,
    /// BGR24 - packed BGR format (RKMPP direct input)
    Bgr24,
}

/// H.265 encoder configuration
#[derive(Debug, Clone)]
pub struct H265Config {
    /// Base encoder config
    pub base: EncoderConfig,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// GOP size (keyframe interval)
    pub gop_size: u32,
    /// Frame rate
    pub fps: u32,
    /// Input pixel format
    pub input_format: H265InputFormat,
}

impl Default for H265Config {
    fn default() -> Self {
        Self {
            base: EncoderConfig::default(),
            bitrate_kbps: 8000,
            gop_size: 30,
            fps: 30,
            input_format: H265InputFormat::Nv12,
        }
    }
}

impl H265Config {
    /// Create config for low latency streaming with NV12 input
    pub fn low_latency(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig {
                resolution,
                input_format: PixelFormat::Nv12,
                quality: bitrate_kbps,
                fps: 30,
                gop_size: 30,
            },
            bitrate_kbps,
            gop_size: 30,
            fps: 30,
            input_format: H265InputFormat::Nv12,
        }
    }

    /// Create config for low latency streaming with YUYV422 input (optimal for RKMPP direct input)
    pub fn low_latency_yuyv422(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig {
                resolution,
                input_format: PixelFormat::Yuyv,
                quality: bitrate_kbps,
                fps: 30,
                gop_size: 30,
            },
            bitrate_kbps,
            gop_size: 30,
            fps: 30,
            input_format: H265InputFormat::Yuyv422,
        }
    }

    /// Create config for quality streaming
    pub fn quality(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig {
                resolution,
                input_format: PixelFormat::Nv12,
                quality: bitrate_kbps,
                fps: 30,
                gop_size: 60,
            },
            bitrate_kbps,
            gop_size: 60,
            fps: 30,
            input_format: H265InputFormat::Nv12,
        }
    }

    /// Set input format
    pub fn with_input_format(mut self, format: H265InputFormat) -> Self {
        self.input_format = format;
        self
    }
}

/// Get available H265 hardware encoders from hwcodec
pub fn get_available_h265_encoders(width: u32, height: u32) -> Vec<CodecInfo> {
    init_hwcodec_logging();

    let ctx = EncodeContext {
        name: String::new(),
        mc_name: None,
        width: width as i32,
        height: height as i32,
        pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
        align: 1,
        fps: 30,
        gop: 30,
        rc: RateControl::RC_CBR,
        quality: Quality::Quality_Default,
        kbs: 2000,
        q: 23,
        thread_count: 1,
    };

    let all_encoders = HwEncoder::available_encoders(ctx, None);

    // Include both hardware and software H265 encoders
    all_encoders
        .into_iter()
        .filter(|e| e.format == DataFormat::H265)
        .collect()
}

/// Detect best available H.265 encoder (hardware preferred, software fallback)
pub fn detect_best_h265_encoder(width: u32, height: u32) -> (H265EncoderType, Option<String>) {
    let encoders = get_available_h265_encoders(width, height);

    if encoders.is_empty() {
        warn!("No H.265 encoders available");
        return (H265EncoderType::None, None);
    }

    // Prefer hardware encoders over software (libx265)
    // Hardware priority: NVENC > QSV > AMF > VAAPI > RKMPP > V4L2 M2M > Software
    let codec = encoders
        .iter()
        .find(|e| !e.name.contains("libx265"))
        .or_else(|| encoders.first())
        .unwrap();

    let encoder_type = if codec.name.contains("nvenc") {
        H265EncoderType::Nvenc
    } else if codec.name.contains("qsv") {
        H265EncoderType::Qsv
    } else if codec.name.contains("amf") {
        H265EncoderType::Amf
    } else if codec.name.contains("vaapi") {
        H265EncoderType::Vaapi
    } else if codec.name.contains("rkmpp") {
        H265EncoderType::Rkmpp
    } else if codec.name.contains("v4l2m2m") {
        H265EncoderType::V4l2M2m
    } else {
        H265EncoderType::Software // Default to software for unknown
    };

    info!("Selected H.265 encoder: {} ({})", codec.name, encoder_type);
    (encoder_type, Some(codec.name.clone()))
}

/// Check if H265 hardware encoding is available
pub fn is_h265_available() -> bool {
    let registry = EncoderRegistry::global();
    registry.is_format_available(VideoEncoderType::H265, true)
}

/// Encoded frame from hwcodec (cloned for ownership)
#[derive(Debug, Clone)]
pub struct HwEncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

/// H.265 encoder using hwcodec (hardware only)
pub struct H265Encoder {
    /// hwcodec encoder instance
    inner: HwEncoder,
    /// Encoder configuration
    config: H265Config,
    /// Detected encoder type
    encoder_type: H265EncoderType,
    /// Codec name
    codec_name: String,
    /// Frame counter
    frame_count: u64,
    /// Required buffer length from hwcodec
    buffer_length: i32,
}

impl H265Encoder {
    /// Create a new H.265 encoder with automatic hardware codec detection
    ///
    /// Returns an error if no hardware encoder is available.
    pub fn new(config: H265Config) -> Result<Self> {
        init_hwcodec_logging();

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        // Detect best hardware encoder
        let (encoder_type, codec_name) = detect_best_h265_encoder(width, height);

        if encoder_type == H265EncoderType::None {
            return Err(AppError::VideoError(
                "No H.265 encoder available. Please ensure FFmpeg is built with libx265 support."
                    .to_string(),
            ));
        }

        let codec_name = codec_name.unwrap();
        Self::with_codec(config, &codec_name)
    }

    /// Create encoder with specific codec name
    pub fn with_codec(config: H265Config, codec_name: &str) -> Result<Self> {
        init_hwcodec_logging();

        // Determine if this is a software encoder
        let is_software = codec_name.contains("libx265");

        // Warn about software encoder performance
        if is_software {
            warn!(
                "Using software H.265 encoder (libx265) - high CPU usage expected. \
                Hardware encoder is recommended for better performance."
            );
        }

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        // Software encoders (libx265) require YUV420P, hardware encoders use NV12 or YUYV422
        let (pixfmt, actual_input_format) = if is_software {
            (AVPixelFormat::AV_PIX_FMT_YUV420P, H265InputFormat::Yuv420p)
        } else {
            match config.input_format {
                H265InputFormat::Nv12 => (AVPixelFormat::AV_PIX_FMT_NV12, H265InputFormat::Nv12),
                H265InputFormat::Nv21 => (AVPixelFormat::AV_PIX_FMT_NV21, H265InputFormat::Nv21),
                H265InputFormat::Nv16 => (AVPixelFormat::AV_PIX_FMT_NV16, H265InputFormat::Nv16),
                H265InputFormat::Nv24 => (AVPixelFormat::AV_PIX_FMT_NV24, H265InputFormat::Nv24),
                H265InputFormat::Yuv420p => {
                    (AVPixelFormat::AV_PIX_FMT_YUV420P, H265InputFormat::Yuv420p)
                }
                H265InputFormat::Yuyv422 => {
                    (AVPixelFormat::AV_PIX_FMT_YUYV422, H265InputFormat::Yuyv422)
                }
                H265InputFormat::Rgb24 => (AVPixelFormat::AV_PIX_FMT_RGB24, H265InputFormat::Rgb24),
                H265InputFormat::Bgr24 => (AVPixelFormat::AV_PIX_FMT_BGR24, H265InputFormat::Bgr24),
            }
        };

        info!(
            "Creating H.265 encoder: {} at {}x{} @ {} kbps (input: {:?})",
            codec_name, width, height, config.bitrate_kbps, actual_input_format
        );

        let ctx = EncodeContext {
            name: codec_name.to_string(),
            mc_name: None,
            width: width as i32,
            height: height as i32,
            pixfmt,
            align: 1,
            fps: config.fps as i32,
            gop: config.gop_size as i32,
            rc: RateControl::RC_CBR,
            quality: Quality::Quality_Default,
            kbs: config.bitrate_kbps as i32,
            q: 23,
            thread_count: 1,
        };

        let inner = HwEncoder::new(ctx).map_err(|_| {
            AppError::VideoError(format!("Failed to create H.265 encoder: {}", codec_name))
        })?;

        let buffer_length = inner.length;
        let backend = EncoderBackend::from_codec_name(codec_name);
        let encoder_type = H265EncoderType::from(backend);

        // Update config to reflect actual input format used
        let mut config = config;
        config.input_format = actual_input_format;

        info!(
            "H.265 encoder created: {} (type: {}, buffer_length: {})",
            codec_name, encoder_type, buffer_length
        );

        Ok(Self {
            inner,
            config,
            encoder_type,
            codec_name: codec_name.to_string(),
            frame_count: 0,
            buffer_length,
        })
    }

    /// Create with auto-detected encoder
    pub fn auto(resolution: Resolution, bitrate_kbps: u32) -> Result<Self> {
        let config = H265Config::low_latency(resolution, bitrate_kbps);
        Self::new(config)
    }

    /// Get encoder type
    pub fn encoder_type(&self) -> &H265EncoderType {
        &self.encoder_type
    }

    /// Get codec name
    pub fn codec_name(&self) -> &str {
        &self.codec_name
    }

    /// Update bitrate dynamically
    pub fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        self.inner
            .set_bitrate(bitrate_kbps as i32)
            .map_err(|_| AppError::VideoError("Failed to set H.265 bitrate".to_string()))?;
        self.config.bitrate_kbps = bitrate_kbps;
        debug!("H.265 bitrate updated to {} kbps", bitrate_kbps);
        Ok(())
    }

    /// Request next frame to be a keyframe (IDR)
    pub fn request_keyframe(&mut self) {
        self.inner.request_keyframe();
        debug!("H265 keyframe requested");
    }

    /// Encode raw frame data (NV12 or YUV420P depending on config)
    pub fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        if data.len() < self.buffer_length as usize {
            return Err(AppError::VideoError(format!(
                "Frame data too small: {} < {}",
                data.len(),
                self.buffer_length
            )));
        }

        self.frame_count += 1;

        // Debug log every 30 frames (1 second at 30fps)
        if self.frame_count % 30 == 1 {
            debug!(
                "[H265] Encoding frame #{}: input_size={}, pts_ms={}, codec={}",
                self.frame_count,
                data.len(),
                pts_ms,
                self.codec_name
            );
        }

        match self.inner.encode(data, pts_ms) {
            Ok(frames) => {
                // Zero-copy: drain frames from hwcodec buffer instead of cloning
                let owned_frames: Vec<HwEncodeFrame> = frames
                    .drain(..)
                    .map(|f| HwEncodeFrame {
                        data: f.data, // Move, not clone
                        pts: f.pts,
                        key: f.key,
                    })
                    .collect();

                // Log encoded output
                if !owned_frames.is_empty() {
                    let total_size: usize = owned_frames.iter().map(|f| f.data.len()).sum();
                    let keyframe = owned_frames.iter().any(|f| f.key == 1);

                    if keyframe || self.frame_count % 30 == 1 {
                        debug!(
                            "[H265] Encoded frame #{}: output_size={}, keyframe={}, frame_count={}",
                            self.frame_count,
                            total_size,
                            keyframe,
                            owned_frames.len()
                        );

                        // Log first few bytes of keyframe for debugging
                        if keyframe && !owned_frames[0].data.is_empty() {
                            let preview_len = owned_frames[0].data.len().min(32);
                            debug!(
                                "[H265] Keyframe data preview: {:02x?}",
                                &owned_frames[0].data[..preview_len]
                            );
                        }
                    }
                } else {
                    warn!(
                        "[H265] Encoder returned empty frame list for frame #{}",
                        self.frame_count
                    );
                }

                Ok(owned_frames)
            }
            Err(e) => {
                error!("[H265] Encode failed at frame #{}: {}", self.frame_count, e);
                Err(AppError::VideoError(format!("H.265 encode failed: {}", e)))
            }
        }
    }

    /// Encode NV12 data
    pub fn encode_nv12(&mut self, nv12_data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        self.encode_raw(nv12_data, pts_ms)
    }

    /// Get input format
    pub fn input_format(&self) -> H265InputFormat {
        self.config.input_format
    }

    /// Get buffer info (linesize, offset, length)
    pub fn buffer_info(&self) -> (Vec<i32>, Vec<i32>, i32) {
        (
            self.inner.linesize.clone(),
            self.inner.offset.clone(),
            self.inner.length,
        )
    }
}

// SAFETY: H265Encoder contains hwcodec::ffmpeg_ram::encode::Encoder which has raw pointers
// that are not Send by default. However, we ensure that H265Encoder is only used from
// a single task/thread at a time (encoding is sequential), so this is safe.
unsafe impl Send for H265Encoder {}

impl Encoder for H265Encoder {
    fn name(&self) -> &str {
        &self.codec_name
    }

    fn output_format(&self) -> EncodedFormat {
        EncodedFormat::H265
    }

    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let pts_ms = (sequence * 1000 / self.config.fps as u64) as i64;

        let mut frames = self.encode_raw(data, pts_ms)?;

        if frames.is_empty() {
            warn!("H.265 encoder returned no frames");
            return Err(AppError::VideoError(
                "H.265 encoder returned no frames".to_string(),
            ));
        }

        // Take ownership of the first frame (zero-copy)
        let frame = frames.remove(0);
        let key_frame = frame.key == 1;

        Ok(EncodedFrame {
            data: Bytes::from(frame.data), // Move Vec into Bytes (zero-copy)
            format: EncodedFormat::H265,
            resolution: self.config.base.resolution,
            key_frame,
            sequence,
            timestamp: std::time::Instant::now(),
            pts: frame.pts as u64,
            dts: frame.pts as u64,
        })
    }

    fn flush(&mut self) -> Result<Vec<EncodedFrame>> {
        Ok(vec![])
    }

    fn reset(&mut self) -> Result<()> {
        self.frame_count = 0;
        Ok(())
    }

    fn config(&self) -> &EncoderConfig {
        &self.config.base
    }

    fn supports_format(&self, format: PixelFormat) -> bool {
        match self.config.input_format {
            H265InputFormat::Nv12 => matches!(format, PixelFormat::Nv12),
            H265InputFormat::Nv21 => matches!(format, PixelFormat::Nv21),
            H265InputFormat::Nv16 => matches!(format, PixelFormat::Nv16),
            H265InputFormat::Nv24 => matches!(format, PixelFormat::Nv24),
            H265InputFormat::Yuv420p => matches!(format, PixelFormat::Yuv420),
            H265InputFormat::Yuyv422 => matches!(format, PixelFormat::Yuyv),
            H265InputFormat::Rgb24 => matches!(format, PixelFormat::Rgb24),
            H265InputFormat::Bgr24 => matches!(format, PixelFormat::Bgr24),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_h265_encoder() {
        let (encoder_type, codec_name) = detect_best_h265_encoder(1280, 720);
        println!(
            "Detected H.265 encoder: {:?} ({:?})",
            encoder_type, codec_name
        );
    }

    #[test]
    fn test_available_h265_encoders() {
        let encoders = get_available_h265_encoders(1280, 720);
        println!("Available H.265 hardware encoders:");
        for enc in &encoders {
            println!("  - {} ({:?})", enc.name, enc.format);
        }
    }

    #[test]
    fn test_h265_availability() {
        let available = is_h265_available();
        println!("H.265 hardware encoding available: {}", available);
    }
}
