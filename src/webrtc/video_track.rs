//! Universal video track for WebRTC streaming
//!
//! Supports multiple codecs: H264, H265, VP8, VP9
//!
//! # Architecture
//!
//! ```text
//! Encoded Frame (H264/H265/VP8/VP9)
//!        |
//!        v
//! UniversalVideoTrack
//!   - H264/VP8/VP9: TrackLocalStaticSample (built-in payloader)
//!   - H265: TrackLocalStaticRTP (rtp crate HevcPayloader)
//!        |
//!        v
//! WebRTC PeerConnection
//! ```

use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, trace, warn};
use webrtc::media::Sample;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

// Use our custom H265Payloader that handles ALL NAL types correctly
// The rtp crate's HevcPayloader has bugs:
// 1. It drops the IDR frame after emitting the AP packet
// 2. It ignores NAL type 20 (IDR_N_LP)
use super::h265_payloader::H265Payloader;

use crate::error::Result;
use crate::video::format::Resolution;

/// Default MTU for RTP packets
const RTP_MTU: usize = 1200;

/// Video codec type for WebRTC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    /// H.264/AVC
    H264,
    /// H.265/HEVC
    H265,
    /// VP8
    VP8,
    /// VP9
    VP9,
}

impl VideoCodec {
    /// Get MIME type for SDP
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "video/H264",
            VideoCodec::H265 => "video/H265",
            VideoCodec::VP8 => "video/VP8",
            VideoCodec::VP9 => "video/VP9",
        }
    }

    /// Get RTP clock rate (always 90kHz for video)
    pub fn clock_rate(&self) -> u32 {
        90000
    }

    /// Get default RTP payload type
    pub fn default_payload_type(&self) -> u8 {
        match self {
            VideoCodec::H264 => 96,
            VideoCodec::VP8 => 97,
            VideoCodec::VP9 => 98,
            VideoCodec::H265 => 99,
        }
    }

    /// Get SDP fmtp parameters
    pub fn sdp_fmtp(&self) -> String {
        match self {
            VideoCodec::H264 => {
                "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f".to_string()
            }
            VideoCodec::H265 => {
                // Match Chrome's H.265 fmtp format: level-id=180 (Level 6.0), profile-id=1 (Main), tier-flag=0, tx-mode=SRST
                "level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST".to_string()
            }
            VideoCodec::VP8 => String::new(),
            VideoCodec::VP9 => "profile-id=0".to_string(),
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::H265 => "H.265/HEVC",
            VideoCodec::VP8 => "VP8",
            VideoCodec::VP9 => "VP9",
        }
    }
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Universal video track configuration
#[derive(Debug, Clone)]
pub struct UniversalVideoTrackConfig {
    /// Track ID
    pub track_id: String,
    /// Stream ID
    pub stream_id: String,
    /// Video codec
    pub codec: VideoCodec,
    /// Resolution
    pub resolution: Resolution,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Frames per second
    pub fps: u32,
}

impl Default for UniversalVideoTrackConfig {
    fn default() -> Self {
        Self {
            track_id: "video0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
            codec: VideoCodec::H264,
            resolution: Resolution::HD720,
            bitrate_kbps: 8000,
            fps: 30,
        }
    }
}

impl UniversalVideoTrackConfig {
    /// Create H264 config
    pub fn h264(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::H264,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

    /// Create H265 config
    pub fn h265(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::H265,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

    /// Create VP8 config
    pub fn vp8(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::VP8,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

    /// Create VP9 config
    pub fn vp9(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::VP9,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }
}

/// Track statistics
#[derive(Debug, Clone, Default)]
pub struct VideoTrackStats {
    /// Frames sent
    pub frames_sent: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Keyframes sent
    pub keyframes_sent: u64,
    /// Errors
    pub errors: u64,
}

/// Track type wrapper to support different underlying track implementations
enum TrackType {
    /// Sample-based track with built-in payloader (H264, VP8, VP9)
    Sample(Arc<TrackLocalStaticSample>),
    /// RTP-based track with custom payloader (H265)
    Rtp(Arc<TrackLocalStaticRTP>),
}

/// H265-specific RTP state
struct H265RtpState {
    /// H265 payloader (custom implementation that handles all NAL types)
    payloader: H265Payloader,
    /// Current sequence number
    sequence_number: u16,
    /// Current RTP timestamp
    timestamp: u32,
    /// Timestamp increment per frame (90000 / fps)
    timestamp_increment: u32,
}

/// Universal video track supporting H264/H265/VP8/VP9
pub struct UniversalVideoTrack {
    /// Underlying WebRTC track (Sample or RTP based)
    track: TrackType,
    /// Codec type
    codec: VideoCodec,
    /// Configuration
    config: UniversalVideoTrackConfig,
    /// Statistics
    stats: Mutex<VideoTrackStats>,
    /// H265 RTP state (only used for H265)
    h265_state: Option<Mutex<H265RtpState>>,
}

impl UniversalVideoTrack {
    /// Create a new universal video track
    pub fn new(config: UniversalVideoTrackConfig) -> Self {
        let codec_capability = RTCRtpCodecCapability {
            mime_type: config.codec.mime_type().to_string(),
            clock_rate: config.codec.clock_rate(),
            channels: 0,
            sdp_fmtp_line: config.codec.sdp_fmtp(),
            rtcp_feedback: vec![],
        };

        // Use different track types for different codecs
        let (track, h265_state) = if config.codec == VideoCodec::H265 {
            // H265 uses TrackLocalStaticRTP with official rtp crate HevcPayloader
            let rtp_track = Arc::new(TrackLocalStaticRTP::new(
                codec_capability,
                config.track_id.clone(),
                config.stream_id.clone(),
            ));

            // Create H265 RTP state with custom H265Payloader
            let h265_state = H265RtpState {
                payloader: H265Payloader::new(),
                sequence_number: rand::random::<u16>(),
                timestamp: rand::random::<u32>(),
                timestamp_increment: 90000 / config.fps.max(1),
            };

            (TrackType::Rtp(rtp_track), Some(Mutex::new(h265_state)))
        } else {
            // H264/VP8/VP9 use TrackLocalStaticSample with built-in payloader
            let sample_track = Arc::new(TrackLocalStaticSample::new(
                codec_capability,
                config.track_id.clone(),
                config.stream_id.clone(),
            ));

            (TrackType::Sample(sample_track), None)
        };

        Self {
            track,
            codec: config.codec,
            config,
            stats: Mutex::new(VideoTrackStats::default()),
            h265_state,
        }
    }

    /// Get track as TrackLocal for peer connection
    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        match &self.track {
            TrackType::Sample(t) => t.clone(),
            TrackType::Rtp(t) => t.clone(),
        }
    }

    /// Get codec type
    pub fn codec(&self) -> VideoCodec {
        self.codec
    }

    /// Get configuration
    pub fn config(&self) -> &UniversalVideoTrackConfig {
        &self.config
    }

    /// Get current statistics
    pub async fn stats(&self) -> VideoTrackStats {
        self.stats.lock().await.clone()
    }

    /// Write an encoded frame to the track
    ///
    /// Handles codec-specific processing:
    /// - H264/H265: NAL unit parsing, parameter caching
    /// - VP8/VP9: Direct frame sending
    pub async fn write_frame_bytes(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        match self.codec {
            VideoCodec::H264 => self.write_h264_frame(data, is_keyframe).await,
            VideoCodec::H265 => self.write_h265_frame(data, is_keyframe).await,
            VideoCodec::VP8 => self.write_vp8_frame(data, is_keyframe).await,
            VideoCodec::VP9 => self.write_vp9_frame(data, is_keyframe).await,
        }
    }

    pub async fn write_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        self.write_frame_bytes(Bytes::copy_from_slice(data), is_keyframe)
            .await
    }

    /// Write H264 frame (Annex B format)
    ///
    /// Sends the entire Annex B frame as a single Sample to allow the
    /// H264Payloader to aggregate SPS+PPS into STAP-A packets.
    async fn write_h264_frame(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        // Send entire Annex B frame as one Sample
        // The H264Payloader in rtp crate will:
        // 1. Parse NAL units from Annex B format
        // 2. Cache SPS and PPS
        // 3. Aggregate SPS+PPS+IDR into STAP-A when possible
        // 4. Fragment large NALs using FU-A
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
        let data_len = data.len();
        let sample = Sample {
            data,
            duration: frame_duration,
            ..Default::default()
        };

        match &self.track {
            TrackType::Sample(track) => {
                if let Err(e) = track.write_sample(&sample).await {
                    debug!("H264 write_sample failed: {}", e);
                }
            }
            TrackType::Rtp(_) => {
                warn!("H264 should not use RTP track");
            }
        }

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data_len as u64;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        Ok(())
    }

    /// Write H265 frame (Annex B format)
    ///
    /// Pass raw Annex B data directly to the official HevcPayloader.
    /// The payloader handles NAL parsing, VPS/SPS/PPS caching, AP generation, and FU fragmentation.
    async fn write_h265_frame(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        // Pass raw Annex B data directly to the official HevcPayloader
        self.send_h265_rtp(data, is_keyframe).await
    }

    /// Write VP8 frame
    async fn write_vp8_frame(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        // VP8 frames are sent directly without NAL parsing
        // Calculate frame duration based on configured FPS
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
        let data_len = data.len();
        let sample = Sample {
            data,
            duration: frame_duration,
            ..Default::default()
        };

        match &self.track {
            TrackType::Sample(track) => {
                if let Err(e) = track.write_sample(&sample).await {
                    debug!("VP8 write_sample failed: {}", e);
                }
            }
            TrackType::Rtp(_) => {
                warn!("VP8 should not use RTP track");
            }
        }

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data_len as u64;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        Ok(())
    }

    /// Write VP9 frame
    async fn write_vp9_frame(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        // VP9 frames are sent directly without NAL parsing
        // Calculate frame duration based on configured FPS
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
        let data_len = data.len();
        let sample = Sample {
            data,
            duration: frame_duration,
            ..Default::default()
        };

        match &self.track {
            TrackType::Sample(track) => {
                if let Err(e) = track.write_sample(&sample).await {
                    debug!("VP9 write_sample failed: {}", e);
                }
            }
            TrackType::Rtp(_) => {
                warn!("VP9 should not use RTP track");
            }
        }

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data_len as u64;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        Ok(())
    }

    /// Send H265 NAL units via custom H265Payloader
    async fn send_h265_rtp(&self, payload: Bytes, is_keyframe: bool) -> Result<()> {
        let rtp_track = match &self.track {
            TrackType::Rtp(t) => t,
            TrackType::Sample(_) => {
                warn!("send_h265_rtp called but track is Sample type");
                return Ok(());
            }
        };

        let h265_state = match &self.h265_state {
            Some(s) => s,
            None => {
                warn!("send_h265_rtp called but h265_state is None");
                return Ok(());
            }
        };

        // Minimize lock hold time: only hold lock during payload generation and state update
        let (payloads, timestamp, seq_start, num_payloads) = {
            let mut state = h265_state.lock().await;

            // Use custom H265Payloader to fragment the data
            let payloads = state.payloader.payload(RTP_MTU, &payload);

            if payloads.is_empty() {
                return Ok(());
            }

            let timestamp = state.timestamp;
            let num_payloads = payloads.len();
            let seq_start = state.sequence_number;

            // Pre-increment sequence number and timestamp
            state.sequence_number = state.sequence_number.wrapping_add(num_payloads as u16);
            state.timestamp = state.timestamp.wrapping_add(state.timestamp_increment);

            (payloads, timestamp, seq_start, num_payloads)
        }; // Lock released here, before network I/O

        let mut total_bytes = 0u64;

        // Send RTP packets without holding the lock
        for (i, payload_data) in payloads.into_iter().enumerate() {
            let seq = seq_start.wrapping_add(i as u16);
            let is_last = i == num_payloads - 1;

            // Build RTP packet
            let packet = rtp::packet::Packet {
                header: rtp::header::Header {
                    version: 2,
                    padding: false,
                    extension: false,
                    marker: is_last,
                    payload_type: 49,
                    sequence_number: seq,
                    timestamp,
                    ssrc: 0,
                    ..Default::default()
                },
                payload: payload_data.clone(),
            };

            if let Err(e) = rtp_track.write_rtp(&packet).await {
                trace!("H265 write_rtp failed: {}", e);
            }

            total_bytes += payload_data.len() as u64;
        }

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += total_bytes;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec_properties() {
        assert_eq!(VideoCodec::H264.mime_type(), "video/H264");
        assert_eq!(VideoCodec::H265.mime_type(), "video/H265");
        assert_eq!(VideoCodec::VP8.mime_type(), "video/VP8");
        assert_eq!(VideoCodec::VP9.mime_type(), "video/VP9");

        assert_eq!(VideoCodec::H264.clock_rate(), 90000);
        assert_eq!(VideoCodec::H265.clock_rate(), 90000);
    }

    #[test]
    fn test_config_creation() {
        let h264_config = UniversalVideoTrackConfig::h264(Resolution::HD1080, 4000, 30);
        assert_eq!(h264_config.codec, VideoCodec::H264);
        assert_eq!(h264_config.bitrate_kbps, 4000);

        let h265_config = UniversalVideoTrackConfig::h265(Resolution::HD720, 2000, 30);
        assert_eq!(h265_config.codec, VideoCodec::H265);
    }
}
