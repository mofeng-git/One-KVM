//! Encoder traits and common types

use bytes::Bytes;
use std::time::Instant;

use crate::video::format::{PixelFormat, Resolution};
use crate::error::Result;

/// Encoder configuration
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// Target resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Output quality (1-100 for JPEG, bitrate kbps for H264)
    pub quality: u32,
    /// Target frame rate
    pub fps: u32,
    /// Keyframe interval (for H264)
    pub gop_size: u32,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            resolution: Resolution::HD1080,
            input_format: PixelFormat::Yuyv,
            quality: 80,
            fps: 30,
            gop_size: 30,
        }
    }
}

impl EncoderConfig {
    pub fn jpeg(resolution: Resolution, quality: u32) -> Self {
        Self {
            resolution,
            input_format: PixelFormat::Yuyv,
            quality,
            fps: 30,
            gop_size: 1,
        }
    }

    pub fn h264(resolution: Resolution, bitrate_kbps: u32) -> Self {
        Self {
            resolution,
            input_format: PixelFormat::Yuyv,
            quality: bitrate_kbps,
            fps: 30,
            gop_size: 30,
        }
    }
}

/// Encoded frame output
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// Encoded data
    pub data: Bytes,
    /// Output format (JPEG, H264, etc.)
    pub format: EncodedFormat,
    /// Resolution
    pub resolution: Resolution,
    /// Whether this is a key frame
    pub key_frame: bool,
    /// Frame sequence number
    pub sequence: u64,
    /// Encoding timestamp
    pub timestamp: Instant,
    /// Presentation timestamp (for video sync)
    pub pts: u64,
    /// Decode timestamp (for B-frames)
    pub dts: u64,
}

impl EncodedFrame {
    pub fn jpeg(data: Bytes, resolution: Resolution, sequence: u64) -> Self {
        Self {
            data,
            format: EncodedFormat::Jpeg,
            resolution,
            key_frame: true,
            sequence,
            timestamp: Instant::now(),
            pts: sequence,
            dts: sequence,
        }
    }

    pub fn h264(
        data: Bytes,
        resolution: Resolution,
        key_frame: bool,
        sequence: u64,
        pts: u64,
        dts: u64,
    ) -> Self {
        Self {
            data,
            format: EncodedFormat::H264,
            resolution,
            key_frame,
            sequence,
            timestamp: Instant::now(),
            pts,
            dts,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Encoded output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedFormat {
    Jpeg,
    H264,
    H265,
    Vp8,
    Vp9,
    Av1,
}

impl std::fmt::Display for EncodedFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodedFormat::Jpeg => write!(f, "JPEG"),
            EncodedFormat::H264 => write!(f, "H.264"),
            EncodedFormat::H265 => write!(f, "H.265"),
            EncodedFormat::Vp8 => write!(f, "VP8"),
            EncodedFormat::Vp9 => write!(f, "VP9"),
            EncodedFormat::Av1 => write!(f, "AV1"),
        }
    }
}

/// Generic encoder trait
/// Note: Not Sync because some encoders (like turbojpeg) are not thread-safe
pub trait Encoder: Send {
    /// Get encoder name
    fn name(&self) -> &str;

    /// Get output format
    fn output_format(&self) -> EncodedFormat;

    /// Encode a raw frame
    fn encode(&mut self, data: &[u8], sequence: u64) -> Result<EncodedFrame>;

    /// Flush any pending frames
    fn flush(&mut self) -> Result<Vec<EncodedFrame>> {
        Ok(vec![])
    }

    /// Reset encoder state
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    /// Get current configuration
    fn config(&self) -> &EncoderConfig;

    /// Check if encoder supports the given input format
    fn supports_format(&self, format: PixelFormat) -> bool;
}

/// Encoder factory for creating encoders
pub trait EncoderFactory: Send + Sync {
    /// Create an encoder with the given configuration
    fn create(&self, config: EncoderConfig) -> Result<Box<dyn Encoder>>;

    /// Get encoder type name
    fn encoder_type(&self) -> &str;

    /// Check if this encoder is available on the system
    fn is_available(&self) -> bool;

    /// Get encoder priority (higher = preferred)
    fn priority(&self) -> u32;
}
