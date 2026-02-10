//! VP9 encoder using hwcodec (FFmpeg wrapper)
//!
//! Supports both hardware and software encoding:
//! - Hardware: VAAPI (Intel on Linux)
//! - Software: libvpx-vp9 (CPU-based, high CPU usage)
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
        debug!("hwcodec logging initialized for VP9");
    });
}

/// VP9 encoder type (detected from hwcodec)
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum VP9EncoderType {
    /// VAAPI (Intel on Linux)
    Vaapi,
    /// Software encoder (libvpx-vp9)
    Software,
    /// No encoder available
    #[default]
    None,
}

impl std::fmt::Display for VP9EncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VP9EncoderType::Vaapi => write!(f, "VAAPI"),
            VP9EncoderType::Software => write!(f, "Software"),
            VP9EncoderType::None => write!(f, "None"),
        }
    }
}


impl From<EncoderBackend> for VP9EncoderType {
    fn from(backend: EncoderBackend) -> Self {
        match backend {
            EncoderBackend::Vaapi => VP9EncoderType::Vaapi,
            EncoderBackend::Software => VP9EncoderType::Software,
            _ => VP9EncoderType::None,
        }
    }
}

/// Input pixel format for VP9 encoder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum VP9InputFormat {
    /// YUV420P (I420) - planar Y, U, V
    Yuv420p,
    /// NV12 - Y plane + interleaved UV plane
    #[default]
    Nv12,
}


/// VP9 encoder configuration
#[derive(Debug, Clone)]
pub struct VP9Config {
    /// Base encoder config
    pub base: EncoderConfig,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// GOP size (keyframe interval)
    pub gop_size: u32,
    /// Frame rate
    pub fps: u32,
    /// Input pixel format
    pub input_format: VP9InputFormat,
}

impl Default for VP9Config {
    fn default() -> Self {
        Self {
            base: EncoderConfig::default(),
            bitrate_kbps: 8000,
            gop_size: 30,
            fps: 30,
            input_format: VP9InputFormat::Nv12,
        }
    }
}

impl VP9Config {
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
            input_format: VP9InputFormat::Nv12,
        }
    }

    /// Set input format
    pub fn with_input_format(mut self, format: VP9InputFormat) -> Self {
        self.input_format = format;
        self
    }
}

/// Get available VP9 hardware encoders from hwcodec
pub fn get_available_vp9_encoders(width: u32, height: u32) -> Vec<CodecInfo> {
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

    // Include both hardware and software VP9 encoders
    all_encoders
        .into_iter()
        .filter(|e| e.format == DataFormat::VP9)
        .collect()
}

/// Detect best available VP9 encoder (hardware preferred, software fallback)
pub fn detect_best_vp9_encoder(width: u32, height: u32) -> (VP9EncoderType, Option<String>) {
    let encoders = get_available_vp9_encoders(width, height);

    if encoders.is_empty() {
        warn!("No VP9 encoders available");
        return (VP9EncoderType::None, None);
    }

    // Prefer hardware encoders (VAAPI) over software (libvpx-vp9)
    let codec = encoders
        .iter()
        .find(|e| e.name.contains("vaapi"))
        .or_else(|| encoders.first())
        .unwrap();

    let encoder_type = if codec.name.contains("vaapi") {
        VP9EncoderType::Vaapi
    } else {
        VP9EncoderType::Software // Default to software for unknown
    };

    info!("Selected VP9 encoder: {} ({})", codec.name, encoder_type);
    (encoder_type, Some(codec.name.clone()))
}

/// Check if VP9 hardware encoding is available
pub fn is_vp9_available() -> bool {
    let registry = EncoderRegistry::global();
    registry.is_format_available(VideoEncoderType::VP9, true)
}

/// Encoded frame from hwcodec (cloned for ownership)
#[derive(Debug, Clone)]
pub struct HwEncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

/// VP9 encoder using hwcodec (hardware only - VAAPI)
pub struct VP9Encoder {
    /// hwcodec encoder instance
    inner: HwEncoder,
    /// Encoder configuration
    config: VP9Config,
    /// Detected encoder type
    encoder_type: VP9EncoderType,
    /// Codec name
    codec_name: String,
    /// Frame counter
    frame_count: u64,
    /// Required buffer length from hwcodec
    buffer_length: i32,
}

impl VP9Encoder {
    /// Create a new VP9 encoder with automatic hardware codec detection
    ///
    /// Returns an error if no hardware encoder is available.
    /// VP9 hardware encoding requires Intel VAAPI support.
    pub fn new(config: VP9Config) -> Result<Self> {
        init_hwcodec_logging();

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        let (encoder_type, codec_name) = detect_best_vp9_encoder(width, height);

        if encoder_type == VP9EncoderType::None {
            return Err(AppError::VideoError(
                "No VP9 encoder available. Please ensure FFmpeg is built with libvpx support."
                    .to_string(),
            ));
        }

        let codec_name = codec_name.unwrap();
        Self::with_codec(config, &codec_name)
    }

    /// Create encoder with specific codec name
    pub fn with_codec(config: VP9Config, codec_name: &str) -> Result<Self> {
        init_hwcodec_logging();

        // Determine if this is a software encoder
        let is_software = codec_name.contains("libvpx");

        // Warn about software encoder performance
        if is_software {
            warn!(
                "Using software VP9 encoder (libvpx-vp9) - high CPU usage expected. \
                Hardware encoder is recommended for better performance."
            );
        }

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        // Software encoders (libvpx-vp9) require YUV420P, hardware (VAAPI) uses NV12
        let (pixfmt, actual_input_format) = if is_software {
            (AVPixelFormat::AV_PIX_FMT_YUV420P, VP9InputFormat::Yuv420p)
        } else {
            match config.input_format {
                VP9InputFormat::Nv12 => (AVPixelFormat::AV_PIX_FMT_NV12, VP9InputFormat::Nv12),
                VP9InputFormat::Yuv420p => {
                    (AVPixelFormat::AV_PIX_FMT_YUV420P, VP9InputFormat::Yuv420p)
                }
            }
        };

        info!(
            "Creating VP9 encoder: {} at {}x{} @ {} kbps (input: {:?})",
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
            q: 31,
            thread_count: 4, // VP9 benefits from multi-threading
        };

        let inner = HwEncoder::new(ctx).map_err(|_| {
            AppError::VideoError(format!("Failed to create VP9 encoder: {}", codec_name))
        })?;

        let buffer_length = inner.length;
        let backend = EncoderBackend::from_codec_name(codec_name);
        let encoder_type = VP9EncoderType::from(backend);

        // Update config to reflect actual input format used
        let mut config = config;
        config.input_format = actual_input_format;

        info!(
            "VP9 encoder created: {} (type: {}, buffer_length: {})",
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
        let config = VP9Config::low_latency(resolution, bitrate_kbps);
        Self::new(config)
    }

    /// Get encoder type
    pub fn encoder_type(&self) -> &VP9EncoderType {
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
            .map_err(|_| AppError::VideoError("Failed to set VP9 bitrate".to_string()))?;
        self.config.bitrate_kbps = bitrate_kbps;
        debug!("VP9 bitrate updated to {} kbps", bitrate_kbps);
        Ok(())
    }

    /// Encode raw frame data
    pub fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        if data.len() < self.buffer_length as usize {
            return Err(AppError::VideoError(format!(
                "Frame data too small: {} < {}",
                data.len(),
                self.buffer_length
            )));
        }

        self.frame_count += 1;

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
                Ok(owned_frames)
            }
            Err(e) => {
                error!("VP9 encode failed: {}", e);
                Err(AppError::VideoError(format!("VP9 encode failed: {}", e)))
            }
        }
    }

    /// Encode NV12 data
    pub fn encode_nv12(&mut self, nv12_data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        self.encode_raw(nv12_data, pts_ms)
    }

    /// Get input format
    pub fn input_format(&self) -> VP9InputFormat {
        self.config.input_format
    }

    /// Get buffer info
    pub fn buffer_info(&self) -> (Vec<i32>, Vec<i32>, i32) {
        (
            self.inner.linesize.clone(),
            self.inner.offset.clone(),
            self.inner.length,
        )
    }
}

// SAFETY: VP9Encoder contains hwcodec::ffmpeg_ram::encode::Encoder which has raw pointers
// that are not Send by default. However, we ensure that VP9Encoder is only used from
// a single task/thread at a time (encoding is sequential), so this is safe.
unsafe impl Send for VP9Encoder {}

impl Encoder for VP9Encoder {
    fn name(&self) -> &str {
        &self.codec_name
    }

    fn output_format(&self) -> EncodedFormat {
        EncodedFormat::Vp9
    }

    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        let pts_ms = (sequence * 1000 / self.config.fps as u64) as i64;

        let mut frames = self.encode_raw(data, pts_ms)?;

        if frames.is_empty() {
            warn!("VP9 encoder returned no frames");
            return Err(AppError::VideoError(
                "VP9 encoder returned no frames".to_string(),
            ));
        }

        // Take ownership of the first frame (zero-copy)
        let frame = frames.remove(0);
        let key_frame = frame.key == 1;

        Ok(EncodedFrame {
            data: Bytes::from(frame.data), // Move Vec into Bytes (zero-copy)
            format: EncodedFormat::Vp9,
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
            VP9InputFormat::Nv12 => matches!(format, PixelFormat::Nv12),
            VP9InputFormat::Yuv420p => matches!(format, PixelFormat::Yuv420),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_vp9_encoder() {
        let (encoder_type, codec_name) = detect_best_vp9_encoder(1280, 720);
        println!(
            "Detected VP9 encoder: {:?} ({:?})",
            encoder_type, codec_name
        );
    }

    #[test]
    fn test_available_vp9_encoders() {
        let encoders = get_available_vp9_encoders(1280, 720);
        println!("Available VP9 hardware encoders:");
        for enc in &encoders {
            println!("  - {} ({:?})", enc.name, enc.format);
        }
    }

    #[test]
    fn test_vp9_availability() {
        let available = is_vp9_available();
        println!("VP9 hardware encoding available: {}", available);
    }
}
