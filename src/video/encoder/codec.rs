//! WebRTC Video Codec abstraction layer
//!
//! This module provides a unified interface for video codecs used in WebRTC streaming.
//! It supports multiple codec types (H264, VP8, VP9, H265) with a common API.
//!
//! # Architecture
//!
//! ```text
//! VideoCodec (trait)
//!     |
//!     +-- H264Codec (current implementation)
//!     +-- VP8Codec (reserved)
//!     +-- VP9Codec (reserved)
//!     +-- H265Codec (reserved)
//! ```

use bytes::Bytes;
use std::time::Duration;

use crate::error::Result;
use crate::video::format::Resolution;

/// Supported video codec types for WebRTC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoCodecType {
    /// H.264/AVC - widely supported, good compression
    H264,
    /// VP8 - royalty-free, good browser support
    VP8,
    /// VP9 - better compression than VP8
    VP9,
    /// H.265/HEVC - best compression, limited browser support
    H265,
}

impl VideoCodecType {
    /// Get the codec name for SDP
    pub fn sdp_name(&self) -> &'static str {
        match self {
            VideoCodecType::H264 => "H264",
            VideoCodecType::VP8 => "VP8",
            VideoCodecType::VP9 => "VP9",
            VideoCodecType::H265 => "H265",
        }
    }

    /// Get the default RTP payload type
    pub fn default_payload_type(&self) -> u8 {
        match self {
            VideoCodecType::H264 => 96,
            VideoCodecType::VP8 => 97,
            VideoCodecType::VP9 => 98,
            VideoCodecType::H265 => 99,
        }
    }

    /// Get the RTP clock rate (always 90000 for video)
    pub fn clock_rate(&self) -> u32 {
        90000
    }

    /// Get the MIME type
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodecType::H264 => "video/H264",
            VideoCodecType::VP8 => "video/VP8",
            VideoCodecType::VP9 => "video/VP9",
            VideoCodecType::H265 => "video/H265",
        }
    }
}

impl std::fmt::Display for VideoCodecType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sdp_name())
    }
}

/// Encoded video frame for WebRTC transmission
#[derive(Debug, Clone)]
pub struct CodecFrame {
    /// Encoded data (Annex B format for H264/H265, raw for VP8/VP9)
    pub data: Bytes,
    /// Presentation timestamp in milliseconds
    pub pts_ms: i64,
    /// Whether this is a keyframe (IDR for H264, key frame for VP8/VP9)
    pub is_keyframe: bool,
    /// Codec type
    pub codec: VideoCodecType,
    /// Frame sequence number
    pub sequence: u64,
    /// Frame duration
    pub duration: Duration,
}

impl CodecFrame {
    /// Create a new H264 frame
    pub fn h264(data: Bytes, pts_ms: i64, is_keyframe: bool, sequence: u64, fps: u32) -> Self {
        Self {
            data,
            pts_ms,
            is_keyframe,
            codec: VideoCodecType::H264,
            sequence,
            duration: Duration::from_millis(1000 / fps as u64),
        }
    }

    /// Create a new VP8 frame
    pub fn vp8(data: Bytes, pts_ms: i64, is_keyframe: bool, sequence: u64, fps: u32) -> Self {
        Self {
            data,
            pts_ms,
            is_keyframe,
            codec: VideoCodecType::VP8,
            sequence,
            duration: Duration::from_millis(1000 / fps as u64),
        }
    }

    /// Create a new VP9 frame
    pub fn vp9(data: Bytes, pts_ms: i64, is_keyframe: bool, sequence: u64, fps: u32) -> Self {
        Self {
            data,
            pts_ms,
            is_keyframe,
            codec: VideoCodecType::VP9,
            sequence,
            duration: Duration::from_millis(1000 / fps as u64),
        }
    }

    /// Create a new H265 frame
    pub fn h265(data: Bytes, pts_ms: i64, is_keyframe: bool, sequence: u64, fps: u32) -> Self {
        Self {
            data,
            pts_ms,
            is_keyframe,
            codec: VideoCodecType::H265,
            sequence,
            duration: Duration::from_millis(1000 / fps as u64),
        }
    }

    /// Get frame size in bytes
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if frame is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Video codec configuration
#[derive(Debug, Clone)]
pub struct VideoCodecConfig {
    /// Codec type
    pub codec: VideoCodecType,
    /// Target resolution
    pub resolution: Resolution,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Target FPS
    pub fps: u32,
    /// GOP size (keyframe interval in frames)
    pub gop_size: u32,
    /// Profile (codec-specific)
    pub profile: Option<String>,
    /// Level (codec-specific)
    pub level: Option<String>,
}

impl Default for VideoCodecConfig {
    fn default() -> Self {
        Self {
            codec: VideoCodecType::H264,
            resolution: Resolution::HD720,
            bitrate_kbps: 8000,
            fps: 30,
            gop_size: 30,
            profile: None,
            level: None,
        }
    }
}

impl VideoCodecConfig {
    /// Create H264 config with common settings
    pub fn h264(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodecType::H264,
            resolution,
            bitrate_kbps,
            fps,
            gop_size: fps, // 1 second GOP
            profile: Some("baseline".to_string()),
            level: Some("3.1".to_string()),
        }
    }

    /// Create VP8 config
    pub fn vp8(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodecType::VP8,
            resolution,
            bitrate_kbps,
            fps,
            gop_size: fps,
            profile: None,
            level: None,
        }
    }

    /// Create VP9 config
    pub fn vp9(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodecType::VP9,
            resolution,
            bitrate_kbps,
            fps,
            gop_size: fps,
            profile: None,
            level: None,
        }
    }

    /// Create H265 config
    pub fn h265(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodecType::H265,
            resolution,
            bitrate_kbps,
            fps,
            gop_size: fps,
            profile: Some("main".to_string()),
            level: Some("4.0".to_string()),
        }
    }
}

/// WebRTC video codec trait
///
/// This trait defines the interface for video codecs used in WebRTC streaming.
/// Implementations should handle format conversion internally if needed.
pub trait VideoCodec: Send {
    /// Get codec type
    fn codec_type(&self) -> VideoCodecType;

    /// Get codec name for display
    fn codec_name(&self) -> &'static str;

    /// Get RTP payload type
    fn payload_type(&self) -> u8 {
        self.codec_type().default_payload_type()
    }

    /// Get SDP fmtp parameters (codec-specific)
    ///
    /// For H264: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
    /// For VP8/VP9: None or empty
    fn sdp_fmtp(&self) -> Option<String>;

    /// Encode a raw frame (NV12 format expected)
    ///
    /// # Arguments
    /// * `frame` - Raw frame data in NV12 format
    /// * `pts_ms` - Presentation timestamp in milliseconds
    ///
    /// # Returns
    /// * `Ok(Some(frame))` - Encoded frame
    /// * `Ok(None)` - Encoder is buffering (no output yet)
    /// * `Err(e)` - Encoding error
    fn encode(&mut self, frame: &[u8], pts_ms: i64) -> Result<Option<CodecFrame>>;

    /// Set target bitrate dynamically
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;

    /// Request a keyframe on next encode
    fn request_keyframe(&mut self);

    /// Get current configuration
    fn config(&self) -> &VideoCodecConfig;

    /// Flush any pending frames
    fn flush(&mut self) -> Result<Vec<CodecFrame>> {
        Ok(vec![])
    }

    /// Reset encoder state
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Video codec factory trait
///
/// Used to create codec instances and query available codecs.
pub trait VideoCodecFactory: Send + Sync {
    /// Create a codec with the given configuration
    fn create(&self, config: VideoCodecConfig) -> Result<Box<dyn VideoCodec>>;

    /// Get supported codec types
    fn supported_codecs(&self) -> Vec<VideoCodecType>;

    /// Check if a specific codec is available
    fn is_codec_available(&self, codec: VideoCodecType) -> bool {
        self.supported_codecs().contains(&codec)
    }

    /// Get the best available codec (based on priority)
    fn best_codec(&self) -> Option<VideoCodecType> {
        // Priority: H264 > VP8 > VP9 > H265
        let supported = self.supported_codecs();
        if supported.contains(&VideoCodecType::H264) {
            Some(VideoCodecType::H264)
        } else if supported.contains(&VideoCodecType::VP8) {
            Some(VideoCodecType::VP8)
        } else if supported.contains(&VideoCodecType::VP9) {
            Some(VideoCodecType::VP9)
        } else if supported.contains(&VideoCodecType::H265) {
            Some(VideoCodecType::H265)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_type_properties() {
        assert_eq!(VideoCodecType::H264.sdp_name(), "H264");
        assert_eq!(VideoCodecType::H264.default_payload_type(), 96);
        assert_eq!(VideoCodecType::H264.clock_rate(), 90000);
        assert_eq!(VideoCodecType::H264.mime_type(), "video/H264");
    }

    #[test]
    fn test_codec_frame_creation() {
        let data = Bytes::from(vec![0x00, 0x00, 0x00, 0x01, 0x65]);
        let frame = CodecFrame::h264(data.clone(), 1000, true, 1, 30);

        assert_eq!(frame.codec, VideoCodecType::H264);
        assert!(frame.is_keyframe);
        assert_eq!(frame.pts_ms, 1000);
        assert_eq!(frame.sequence, 1);
        assert_eq!(frame.len(), 5);
    }

    #[test]
    fn test_codec_config_default() {
        let config = VideoCodecConfig::default();
        assert_eq!(config.codec, VideoCodecType::H264);
        assert_eq!(config.bitrate_kbps, 2000);
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_codec_config_h264() {
        let config = VideoCodecConfig::h264(Resolution::HD1080, 4000, 60);
        assert_eq!(config.codec, VideoCodecType::H264);
        assert_eq!(config.bitrate_kbps, 4000);
        assert_eq!(config.fps, 60);
        assert_eq!(config.gop_size, 60);
    }
}
