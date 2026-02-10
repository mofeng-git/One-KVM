//! H.264 encoder using hwcodec (rustdesk's FFmpeg wrapper)
//!
//! Supports multiple encoder backends via FFmpeg:
//! - VAAPI (Intel/AMD/NVIDIA on Linux)
//! - NVENC (NVIDIA)
//! - AMF (AMD)
//! - Software (libx264)
//!
//! The encoder is selected automatically based on availability.

use bytes::Bytes;
use std::sync::Once;
use tracing::{debug, error, info, warn};

use hwcodec::common::{Quality, RateControl};
use hwcodec::ffmpeg::AVPixelFormat;
use hwcodec::ffmpeg_ram::encode::{EncodeContext, Encoder as HwEncoder};
use hwcodec::ffmpeg_ram::CodecInfo;

use super::traits::{EncodedFormat, EncodedFrame, Encoder, EncoderConfig};
use crate::error::{AppError, Result};
use crate::video::format::{PixelFormat, Resolution};

static INIT_LOGGING: Once = Once::new();

/// Initialize hwcodec logging (only once)
fn init_hwcodec_logging() {
    INIT_LOGGING.call_once(|| {
        // hwcodec uses the `log` crate, which will work with our tracing subscriber
        debug!("hwcodec logging initialized");
    });
}

/// H.264 encoder type (detected from hwcodec)
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum H264EncoderType {
    /// NVIDIA NVENC
    Nvenc,
    /// Intel Quick Sync (QSV)
    Qsv,
    /// AMD AMF
    Amf,
    /// VAAPI (Linux generic)
    Vaapi,
    /// RKMPP (Rockchip) - requires hwcodec extension
    Rkmpp,
    /// V4L2 M2M (ARM generic) - requires hwcodec extension
    V4l2M2m,
    /// Software encoding (libx264/openh264)
    Software,
    /// No encoder available
    #[default]
    None,
}

impl std::fmt::Display for H264EncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            H264EncoderType::Nvenc => write!(f, "NVENC"),
            H264EncoderType::Qsv => write!(f, "QSV"),
            H264EncoderType::Amf => write!(f, "AMF"),
            H264EncoderType::Vaapi => write!(f, "VAAPI"),
            H264EncoderType::Rkmpp => write!(f, "RKMPP"),
            H264EncoderType::V4l2M2m => write!(f, "V4L2 M2M"),
            H264EncoderType::Software => write!(f, "Software"),
            H264EncoderType::None => write!(f, "None"),
        }
    }
}


/// Map codec name to encoder type
fn codec_name_to_type(name: &str) -> H264EncoderType {
    if name.contains("nvenc") {
        H264EncoderType::Nvenc
    } else if name.contains("qsv") {
        H264EncoderType::Qsv
    } else if name.contains("amf") {
        H264EncoderType::Amf
    } else if name.contains("vaapi") {
        H264EncoderType::Vaapi
    } else if name.contains("rkmpp") {
        H264EncoderType::Rkmpp
    } else if name.contains("v4l2m2m") {
        H264EncoderType::V4l2M2m
    } else {
        H264EncoderType::Software
    }
}

/// Input pixel format for H264 encoder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum H264InputFormat {
    /// YUV420P (I420) - planar Y, U, V
    Yuv420p,
    /// NV12 - Y plane + interleaved UV plane (optimal for VAAPI)
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


/// H.264 encoder configuration
#[derive(Debug, Clone)]
pub struct H264Config {
    /// Base encoder config
    pub base: EncoderConfig,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// GOP size (keyframe interval)
    pub gop_size: u32,
    /// Frame rate
    pub fps: u32,
    /// Input pixel format
    pub input_format: H264InputFormat,
}

impl Default for H264Config {
    fn default() -> Self {
        Self {
            base: EncoderConfig::default(),
            bitrate_kbps: 1000,
            gop_size: 30,
            fps: 30,
            input_format: H264InputFormat::Nv12,
        }
    }
}

impl H264Config {
    /// Create config for low latency streaming with NV12 input (optimal for VAAPI)
    pub fn low_latency(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig::h264(resolution, bitrate_kbps),
            bitrate_kbps,
            gop_size: 30,
            fps: 30,
            input_format: H264InputFormat::Nv12,
        }
    }

    /// Create config for low latency streaming with YUV420P input
    pub fn low_latency_yuv420p(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig::h264(resolution, bitrate_kbps),
            bitrate_kbps,
            gop_size: 30,
            fps: 30,
            input_format: H264InputFormat::Yuv420p,
        }
    }

    /// Create config for low latency streaming with YUYV422 input (optimal for RKMPP direct input)
    pub fn low_latency_yuyv422(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig::h264(resolution, bitrate_kbps),
            bitrate_kbps,
            gop_size: 30,
            fps: 30,
            input_format: H264InputFormat::Yuyv422,
        }
    }

    /// Create config for quality streaming
    pub fn quality(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            base: EncoderConfig::h264(resolution, bitrate_kbps),
            bitrate_kbps,
            gop_size: 60,
            fps: 30,
            input_format: H264InputFormat::Nv12,
        }
    }

    /// Set input format
    pub fn with_input_format(mut self, format: H264InputFormat) -> Self {
        self.input_format = format;
        self
    }
}

/// Get available H264 encoders from hwcodec
pub fn get_available_encoders(width: u32, height: u32) -> Vec<CodecInfo> {
    init_hwcodec_logging();

    let ctx = EncodeContext {
        name: String::new(),
        mc_name: None,
        width: width as i32,
        height: height as i32,
        pixfmt: AVPixelFormat::AV_PIX_FMT_YUV420P,
        align: 1,
        fps: 30,
        gop: 30,
        rc: RateControl::RC_CBR,
        quality: Quality::Quality_Low, // Use low quality preset for fastest encoding (ultrafast)
        kbs: 2000,
        q: 23,
        thread_count: 4,
    };

    HwEncoder::available_encoders(ctx, None)
}

/// Detect best available H.264 encoder
pub fn detect_best_encoder(width: u32, height: u32) -> (H264EncoderType, Option<String>) {
    let encoders = get_available_encoders(width, height);

    if encoders.is_empty() {
        warn!("No H.264 encoders available from hwcodec");
        return (H264EncoderType::None, None);
    }

    // Find H264 encoder (not H265)
    for codec in &encoders {
        if codec.format == hwcodec::common::DataFormat::H264 {
            let encoder_type = codec_name_to_type(&codec.name);
            info!("Best H.264 encoder: {} ({})", codec.name, encoder_type);
            return (encoder_type, Some(codec.name.clone()));
        }
    }

    (H264EncoderType::None, None)
}

/// Encoded frame from hwcodec (cloned for ownership)
#[derive(Debug, Clone)]
pub struct HwEncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

/// H.264 encoder using hwcodec
pub struct H264Encoder {
    /// hwcodec encoder instance
    inner: HwEncoder,
    /// Encoder configuration
    config: H264Config,
    /// Detected encoder type
    encoder_type: H264EncoderType,
    /// Codec name
    codec_name: String,
    /// Frame counter
    frame_count: u64,
    /// YUV420P buffer for input (reserved for future use)
    #[allow(dead_code)]
    yuv_buffer: Vec<u8>,
    /// Required YUV buffer length from hwcodec
    yuv_length: i32,
}

impl H264Encoder {
    /// Create a new H.264 encoder with automatic codec detection
    pub fn new(config: H264Config) -> Result<Self> {
        init_hwcodec_logging();

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        // Detect best encoder
        let (_encoder_type, codec_name) = detect_best_encoder(width, height);

        let codec_name = codec_name
            .ok_or_else(|| AppError::VideoError("No H.264 encoder available".to_string()))?;

        Self::with_codec(config, &codec_name)
    }

    /// Create encoder with specific codec name
    pub fn with_codec(config: H264Config, codec_name: &str) -> Result<Self> {
        init_hwcodec_logging();

        let width = config.base.resolution.width;
        let height = config.base.resolution.height;

        // Select pixel format based on config
        let pixfmt = match config.input_format {
            H264InputFormat::Nv12 => AVPixelFormat::AV_PIX_FMT_NV12,
            H264InputFormat::Nv21 => AVPixelFormat::AV_PIX_FMT_NV21,
            H264InputFormat::Nv16 => AVPixelFormat::AV_PIX_FMT_NV16,
            H264InputFormat::Nv24 => AVPixelFormat::AV_PIX_FMT_NV24,
            H264InputFormat::Yuv420p => AVPixelFormat::AV_PIX_FMT_YUV420P,
            H264InputFormat::Yuyv422 => AVPixelFormat::AV_PIX_FMT_YUYV422,
            H264InputFormat::Rgb24 => AVPixelFormat::AV_PIX_FMT_RGB24,
            H264InputFormat::Bgr24 => AVPixelFormat::AV_PIX_FMT_BGR24,
        };

        info!(
            "Creating H.264 encoder: {} at {}x{} @ {} kbps (input: {:?})",
            codec_name, width, height, config.bitrate_kbps, config.input_format
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
            quality: Quality::Quality_Low, // Use low quality preset for fastest encoding (lowest latency)
            kbs: config.bitrate_kbps as i32,
            q: 23,
            thread_count: 4, // Use 4 threads for better performance
        };

        let inner = HwEncoder::new(ctx).map_err(|_| {
            AppError::VideoError(format!("Failed to create encoder: {}", codec_name))
        })?;

        let yuv_length = inner.length;
        let encoder_type = codec_name_to_type(codec_name);

        info!(
            "H.264 encoder created: {} (type: {}, buffer_length: {}, input_format: {:?})",
            codec_name, encoder_type, yuv_length, config.input_format
        );

        Ok(Self {
            inner,
            config,
            encoder_type,
            codec_name: codec_name.to_string(),
            frame_count: 0,
            yuv_buffer: vec![0u8; yuv_length as usize],
            yuv_length,
        })
    }

    /// Create with auto-detected encoder
    pub fn auto(resolution: Resolution, bitrate_kbps: u32) -> Result<Self> {
        let config = H264Config::low_latency(resolution, bitrate_kbps);
        Self::new(config)
    }

    /// Get encoder type
    pub fn encoder_type(&self) -> &H264EncoderType {
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
            .map_err(|_| AppError::VideoError("Failed to set bitrate".to_string()))?;
        self.config.bitrate_kbps = bitrate_kbps;
        debug!("Bitrate updated to {} kbps", bitrate_kbps);
        Ok(())
    }

    /// Request next frame to be a keyframe (IDR)
    pub fn request_keyframe(&mut self) {
        self.inner.request_keyframe();
        debug!("H264 keyframe requested");
    }

    /// Encode raw frame data (YUV420P or NV12 depending on config)
    pub fn encode_raw(&mut self, data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        if data.len() < self.yuv_length as usize {
            return Err(AppError::VideoError(format!(
                "Frame data too small: {} < {}",
                data.len(),
                self.yuv_length
            )));
        }

        self.frame_count += 1;

        match self.inner.encode(data, pts_ms) {
            Ok(frames) => {
                // Zero-copy: drain frames from hwcodec buffer instead of cloning
                // hwcodec returns &mut Vec, so we can take ownership via drain
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
                error!("Encode failed: {}", e);
                Err(AppError::VideoError(format!("Encode failed: {}", e)))
            }
        }
    }

    /// Encode YUV420P data (legacy method, use encode_raw for new code)
    pub fn encode_yuv420p(&mut self, yuv_data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        self.encode_raw(yuv_data, pts_ms)
    }

    /// Encode NV12 data
    pub fn encode_nv12(&mut self, nv12_data: &[u8], pts_ms: i64) -> Result<Vec<HwEncodeFrame>> {
        self.encode_raw(nv12_data, pts_ms)
    }

    /// Get input format
    pub fn input_format(&self) -> H264InputFormat {
        self.config.input_format
    }

    /// Get YUV buffer info (linesize, offset, length)
    pub fn yuv_info(&self) -> (Vec<i32>, Vec<i32>, i32) {
        (
            self.inner.linesize.clone(),
            self.inner.offset.clone(),
            self.inner.length,
        )
    }
}

// SAFETY: H264Encoder contains hwcodec::ffmpeg_ram::encode::Encoder which has raw pointers
// that are not Send by default. However, we ensure that H264Encoder is only used from
// a single task/thread at a time (encoding is sequential), so this is safe.
// The raw pointers are internal FFmpeg context that doesn't escape the encoder.
unsafe impl Send for H264Encoder {}

impl Encoder for H264Encoder {
    fn name(&self) -> &str {
        &self.codec_name
    }

    fn output_format(&self) -> EncodedFormat {
        EncodedFormat::H264
    }

    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame> {
        // Assume input is YUV420P
        let pts_ms = (sequence * 1000 / self.config.fps as u64) as i64;

        let mut frames = self.encode_yuv420p(data, pts_ms)?;

        if frames.is_empty() {
            // Encoder needs more frames (shouldn't happen with our config)
            warn!("Encoder returned no frames");
            return Err(AppError::VideoError(
                "Encoder returned no frames".to_string(),
            ));
        }

        // Take ownership of the first frame (zero-copy)
        let frame = frames.remove(0);
        let key_frame = frame.key == 1;

        Ok(EncodedFrame::h264(
            Bytes::from(frame.data), // Move Vec into Bytes (zero-copy)
            self.config.base.resolution,
            key_frame,
            sequence,
            frame.pts as u64,
            frame.pts as u64,
        ))
    }

    fn flush(&mut self) -> Result<Vec<EncodedFrame>> {
        // hwcodec doesn't have explicit flush, return empty
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
        // Check if the format matches our configured input format
        match self.config.input_format {
            H264InputFormat::Nv12 => matches!(format, PixelFormat::Nv12),
            H264InputFormat::Nv21 => matches!(format, PixelFormat::Nv21),
            H264InputFormat::Nv16 => matches!(format, PixelFormat::Nv16),
            H264InputFormat::Nv24 => matches!(format, PixelFormat::Nv24),
            H264InputFormat::Yuv420p => matches!(format, PixelFormat::Yuv420),
            H264InputFormat::Yuyv422 => matches!(format, PixelFormat::Yuyv),
            H264InputFormat::Rgb24 => matches!(format, PixelFormat::Rgb24),
            H264InputFormat::Bgr24 => matches!(format, PixelFormat::Bgr24),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_encoder() {
        let (encoder_type, codec_name) = detect_best_encoder(1280, 720);
        println!("Detected encoder: {:?} ({:?})", encoder_type, codec_name);
    }

    #[test]
    fn test_available_encoders() {
        let encoders = get_available_encoders(1280, 720);
        println!("Available encoders:");
        for enc in &encoders {
            println!("  - {} ({:?})", enc.name, enc.format);
        }
    }

    #[test]
    fn test_create_encoder() {
        let config = H264Config::low_latency(Resolution::HD720, 2000);
        match H264Encoder::new(config) {
            Ok(encoder) => {
                println!(
                    "Created encoder: {} ({})",
                    encoder.codec_name(),
                    encoder.encoder_type()
                );
            }
            Err(e) => {
                println!("Failed to create encoder: {}", e);
            }
        }
    }
}
