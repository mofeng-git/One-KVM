//! Unified video track supporting H264, H265, VP8, VP9
//!
//! This module provides a unified video track implementation that supports
//! multiple video codecs with proper RTP packetization.
//!
//! # Supported Codecs
//!
//! - **H264**: NAL unit parsing with SPS/PPS caching (RFC 6184)
//! - **H265**: NAL unit parsing with VPS/SPS/PPS caching (RFC 7798)
//! - **VP8**: Direct frame sending (RFC 7741)
//! - **VP9**: Direct frame sending (draft-ietf-payload-vp9)
//!
//! # Architecture
//!
//! For NAL-based codecs (H264/H265):
//! - Parse NAL units from Annex B format
//! - Cache parameter sets (SPS/PPS/VPS) for injection
//! - Send each NAL unit via TrackLocalStaticSample
//!
//! For VP8/VP9:
//! - Send raw encoded frames directly
//! - webrtc-rs handles RTP packetization internally

use bytes::Bytes;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, trace, warn};
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::media::Sample;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::error::{AppError, Result};
use crate::video::format::Resolution;

/// Video codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    H264,
    H265,
    VP8,
    VP9,
}

impl VideoCodec {
    /// Get MIME type for this codec
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "video/H264",
            VideoCodec::H265 => "video/H265",
            VideoCodec::VP8 => "video/VP8",
            VideoCodec::VP9 => "video/VP9",
        }
    }

    /// Get clock rate (always 90kHz for video)
    pub fn clock_rate(&self) -> u32 {
        90000
    }

    /// Get SDP fmtp line for this codec
    pub fn sdp_fmtp_line(&self) -> String {
        match self {
            VideoCodec::H264 => {
                "level-asymmetry-allowed=1;packetization-mode=1".to_string()
            }
            VideoCodec::H265 => {
                // H265 fmtp parameters
                String::new()
            }
            VideoCodec::VP8 => String::new(),
            VideoCodec::VP9 => String::new(),
        }
    }

    /// Check if codec uses NAL units (H264/H265)
    pub fn uses_nal_units(&self) -> bool {
        matches!(self, VideoCodec::H264 | VideoCodec::H265)
    }
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoCodec::H264 => write!(f, "H264"),
            VideoCodec::H265 => write!(f, "H265"),
            VideoCodec::VP8 => write!(f, "VP8"),
            VideoCodec::VP9 => write!(f, "VP9"),
        }
    }
}

/// Unified video track configuration
#[derive(Debug, Clone)]
pub struct UnifiedVideoTrackConfig {
    /// Video codec
    pub codec: VideoCodec,
    /// Track ID
    pub track_id: String,
    /// Stream ID
    pub stream_id: String,
    /// Resolution
    pub resolution: Resolution,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Frames per second
    pub fps: u32,
}

impl Default for UnifiedVideoTrackConfig {
    fn default() -> Self {
        Self {
            codec: VideoCodec::H264,
            track_id: "video0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
            resolution: Resolution::HD720,
            bitrate_kbps: 8000,
            fps: 30,
        }
    }
}

/// Unified video track statistics
#[derive(Debug, Clone, Default)]
pub struct UnifiedVideoTrackStats {
    pub frames_sent: u64,
    pub bytes_sent: u64,
    pub keyframes_sent: u64,
    pub errors: u64,
}

/// Cached NAL parameter sets for H264
struct H264ParameterSets {
    sps: Option<Bytes>,
    pps: Option<Bytes>,
}

/// Cached NAL parameter sets for H265
struct H265ParameterSets {
    vps: Option<Bytes>,
    sps: Option<Bytes>,
    pps: Option<Bytes>,
}

/// NAL type constants for H264
mod h264_nal {
    pub const NON_IDR_SLICE: u8 = 1;
    pub const IDR_SLICE: u8 = 5;
    pub const SEI: u8 = 6;
    pub const SPS: u8 = 7;
    pub const PPS: u8 = 8;
    pub const AUD: u8 = 9;
    pub const FILLER: u8 = 12;
}

/// NAL type constants for H265
mod h265_nal {
    pub const IDR_W_RADL: u8 = 19;
    pub const IDR_N_LP: u8 = 20;
    pub const CRA_NUT: u8 = 21;
    pub const VPS: u8 = 32;
    pub const SPS: u8 = 33;
    pub const PPS: u8 = 34;
    pub const AUD: u8 = 35;
    pub const FD_NUT: u8 = 38; // Filler data

    /// Check if NAL type is an IDR frame
    pub fn is_idr(nal_type: u8) -> bool {
        nal_type == IDR_W_RADL || nal_type == IDR_N_LP || nal_type == CRA_NUT
    }
}

/// Unified video track supporting multiple codecs
pub struct UnifiedVideoTrack {
    /// The underlying WebRTC track
    track: Arc<TrackLocalStaticSample>,
    /// Track configuration
    config: UnifiedVideoTrackConfig,
    /// Statistics
    stats: Mutex<UnifiedVideoTrackStats>,
    /// H264 parameter set cache
    h264_params: Mutex<H264ParameterSets>,
    /// H265 parameter set cache
    h265_params: Mutex<H265ParameterSets>,
}

impl UnifiedVideoTrack {
    /// Create a new unified video track
    pub fn new(config: UnifiedVideoTrackConfig) -> Self {
        let codec_capability = RTCRtpCodecCapability {
            mime_type: config.codec.mime_type().to_string(),
            clock_rate: config.codec.clock_rate(),
            channels: 0,
            sdp_fmtp_line: config.codec.sdp_fmtp_line(),
            rtcp_feedback: vec![],
        };

        let track = Arc::new(TrackLocalStaticSample::new(
            codec_capability,
            config.track_id.clone(),
            config.stream_id.clone(),
        ));

        Self {
            track,
            config,
            stats: Mutex::new(UnifiedVideoTrackStats::default()),
            h264_params: Mutex::new(H264ParameterSets { sps: None, pps: None }),
            h265_params: Mutex::new(H265ParameterSets { vps: None, sps: None, pps: None }),
        }
    }

    /// Create track for H264
    pub fn h264(track_id: &str, stream_id: &str, resolution: Resolution, fps: u32) -> Self {
        Self::new(UnifiedVideoTrackConfig {
            codec: VideoCodec::H264,
            track_id: track_id.to_string(),
            stream_id: stream_id.to_string(),
            resolution,
            fps,
            ..Default::default()
        })
    }

    /// Create track for H265
    pub fn h265(track_id: &str, stream_id: &str, resolution: Resolution, fps: u32) -> Self {
        Self::new(UnifiedVideoTrackConfig {
            codec: VideoCodec::H265,
            track_id: track_id.to_string(),
            stream_id: stream_id.to_string(),
            resolution,
            fps,
            ..Default::default()
        })
    }

    /// Create track for VP8
    pub fn vp8(track_id: &str, stream_id: &str, resolution: Resolution, fps: u32) -> Self {
        Self::new(UnifiedVideoTrackConfig {
            codec: VideoCodec::VP8,
            track_id: track_id.to_string(),
            stream_id: stream_id.to_string(),
            resolution,
            fps,
            ..Default::default()
        })
    }

    /// Create track for VP9
    pub fn vp9(track_id: &str, stream_id: &str, resolution: Resolution, fps: u32) -> Self {
        Self::new(UnifiedVideoTrackConfig {
            codec: VideoCodec::VP9,
            track_id: track_id.to_string(),
            stream_id: stream_id.to_string(),
            resolution,
            fps,
            ..Default::default()
        })
    }

    /// Get the underlying track for peer connection
    pub fn track(&self) -> Arc<TrackLocalStaticSample> {
        self.track.clone()
    }

    /// Get track as TrackLocal for peer connection
    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        self.track.clone()
    }

    /// Get current codec
    pub fn codec(&self) -> VideoCodec {
        self.config.codec
    }

    /// Get statistics
    pub async fn stats(&self) -> UnifiedVideoTrackStats {
        self.stats.lock().await.clone()
    }

    /// Write an encoded frame to the track
    ///
    /// The frame data should be in the appropriate format for the codec:
    /// - H264/H265: Annex B format (with start codes)
    /// - VP8/VP9: Raw encoded frame
    pub async fn write_frame(&self, data: &[u8], _duration: Duration, is_keyframe: bool) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        match self.config.codec {
            VideoCodec::H264 => self.write_h264_frame(data, is_keyframe).await,
            VideoCodec::H265 => self.write_h265_frame(data, is_keyframe).await,
            VideoCodec::VP8 => self.write_vp8_frame(data, is_keyframe).await,
            VideoCodec::VP9 => self.write_vp9_frame(data, is_keyframe).await,
        }
    }

    /// Write H264 frame (Annex B format)
    async fn write_h264_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        let cursor = Cursor::new(data);
        let mut reader = H264Reader::new(cursor, 1024 * 1024);

        let mut nals: Vec<Bytes> = Vec::new();
        let mut has_sps = false;
        let mut has_pps = false;
        let mut has_idr = false;

        // Parse NAL units
        while let Ok(nal) = reader.next_nal() {
            if nal.data.is_empty() {
                continue;
            }

            let nal_type = nal.data[0] & 0x1F;

            // Skip AUD and filler NAL units
            if nal_type == h264_nal::AUD || nal_type == h264_nal::FILLER {
                continue;
            }

            match nal_type {
                h264_nal::IDR_SLICE => has_idr = true,
                h264_nal::SPS => {
                    has_sps = true;
                    *self.h264_params.lock().await = H264ParameterSets {
                        sps: Some(nal.data.clone().freeze()),
                        pps: self.h264_params.lock().await.pps.clone(),
                    };
                }
                h264_nal::PPS => {
                    has_pps = true;
                    let mut params = self.h264_params.lock().await;
                    params.pps = Some(nal.data.clone().freeze());
                }
                _ => {}
            }

            nals.push(nal.data.freeze());
        }

        // Inject cached SPS/PPS before IDR if missing
        if has_idr && (!has_sps || !has_pps) {
            let params = self.h264_params.lock().await;
            let mut injected: Vec<Bytes> = Vec::new();

            if !has_sps {
                if let Some(ref sps) = params.sps {
                    debug!("Injecting cached H264 SPS");
                    injected.push(sps.clone());
                }
            }
            if !has_pps {
                if let Some(ref pps) = params.pps {
                    debug!("Injecting cached H264 PPS");
                    injected.push(pps.clone());
                }
            }

            if !injected.is_empty() {
                injected.extend(nals);
                nals = injected;
            }
        }

        // Send NAL units
        self.send_nal_units(nals, is_keyframe).await
    }

    /// Write H265 frame (Annex B format)
    async fn write_h265_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        let mut nals: Vec<Bytes> = Vec::new();
        let mut has_vps = false;
        let mut has_sps = false;
        let mut has_pps = false;
        let mut has_idr = false;

        // Parse H265 NAL units manually (H264Reader works for both since format is similar)
        let mut i = 0;
        while i < data.len() {
            // Find start code
            let (start_code_len, nal_start) = if i + 4 <= data.len()
                && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 1
            {
                (4, i + 4)
            } else if i + 3 <= data.len()
                && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1
            {
                (3, i + 3)
            } else {
                i += 1;
                continue;
            };

            if nal_start >= data.len() {
                break;
            }

            // Find end of NAL unit (next start code or end of data)
            let mut nal_end = data.len();
            let mut j = nal_start + 1;
            while j + 3 <= data.len() {
                if (data[j] == 0 && data[j + 1] == 0 && data[j + 2] == 1)
                    || (j + 4 <= data.len() && data[j] == 0 && data[j + 1] == 0
                        && data[j + 2] == 0 && data[j + 3] == 1)
                {
                    nal_end = j;
                    break;
                }
                j += 1;
            }

            let nal_data = &data[nal_start..nal_end];
            if nal_data.is_empty() {
                i = nal_end;
                continue;
            }

            // H265 NAL type: (first_byte >> 1) & 0x3F
            let nal_type = (nal_data[0] >> 1) & 0x3F;

            // Skip AUD and filler
            if nal_type == h265_nal::AUD || nal_type == h265_nal::FD_NUT {
                i = nal_end;
                continue;
            }

            match nal_type {
                h265_nal::VPS => {
                    has_vps = true;
                    let mut params = self.h265_params.lock().await;
                    params.vps = Some(Bytes::copy_from_slice(nal_data));
                }
                h265_nal::SPS => {
                    has_sps = true;
                    let mut params = self.h265_params.lock().await;
                    params.sps = Some(Bytes::copy_from_slice(nal_data));
                }
                h265_nal::PPS => {
                    has_pps = true;
                    let mut params = self.h265_params.lock().await;
                    params.pps = Some(Bytes::copy_from_slice(nal_data));
                }
                _ if h265_nal::is_idr(nal_type) => {
                    has_idr = true;
                }
                _ => {}
            }

            trace!("H265 NAL: type={} size={}", nal_type, nal_data.len());
            nals.push(Bytes::copy_from_slice(nal_data));
            i = nal_end;
        }

        // Inject cached VPS/SPS/PPS before IDR if missing
        if has_idr && (!has_vps || !has_sps || !has_pps) {
            let params = self.h265_params.lock().await;
            let mut injected: Vec<Bytes> = Vec::new();

            if !has_vps {
                if let Some(ref vps) = params.vps {
                    debug!("Injecting cached H265 VPS");
                    injected.push(vps.clone());
                }
            }
            if !has_sps {
                if let Some(ref sps) = params.sps {
                    debug!("Injecting cached H265 SPS");
                    injected.push(sps.clone());
                }
            }
            if !has_pps {
                if let Some(ref pps) = params.pps {
                    debug!("Injecting cached H265 PPS");
                    injected.push(pps.clone());
                }
            }

            if !injected.is_empty() {
                injected.extend(nals);
                nals = injected;
            }
        }

        self.send_nal_units(nals, is_keyframe).await
    }

    /// Write VP8 frame (raw encoded)
    async fn write_vp8_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        // VP8 frames are sent directly
        let sample = Sample {
            data: Bytes::copy_from_slice(data),
            duration: Duration::from_secs(1),
            ..Default::default()
        };

        if let Err(e) = self.track.write_sample(&sample).await {
            debug!("VP8 write_sample failed: {}", e);
        }

        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data.len() as u64;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        trace!("VP8 frame: {} bytes, keyframe={}", data.len(), is_keyframe);
        Ok(())
    }

    /// Write VP9 frame (raw encoded)
    async fn write_vp9_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        // VP9 frames are sent directly
        let sample = Sample {
            data: Bytes::copy_from_slice(data),
            duration: Duration::from_secs(1),
            ..Default::default()
        };

        if let Err(e) = self.track.write_sample(&sample).await {
            debug!("VP9 write_sample failed: {}", e);
        }

        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.bytes_sent += data.len() as u64;
        if is_keyframe {
            stats.keyframes_sent += 1;
        }

        trace!("VP9 frame: {} bytes, keyframe={}", data.len(), is_keyframe);
        Ok(())
    }

    /// Send NAL units via track (for H264/H265)
    async fn send_nal_units(&self, nals: Vec<Bytes>, is_keyframe: bool) -> Result<()> {
        let mut total_bytes = 0u64;
        let mut nal_count = 0;

        for nal_data in nals {
            let sample = Sample {
                data: nal_data.clone(),
                duration: Duration::from_secs(1),
                ..Default::default()
            };

            if let Err(e) = self.track.write_sample(&sample).await {
                if nal_count % 100 == 0 {
                    debug!("write_sample failed (no peer?): {}", e);
                }
            }

            total_bytes += nal_data.len() as u64;
            nal_count += 1;
        }

        if nal_count > 0 {
            let mut stats = self.stats.lock().await;
            stats.frames_sent += 1;
            stats.bytes_sent += total_bytes;
            if is_keyframe {
                stats.keyframes_sent += 1;
            }
        }

        trace!("Sent {} NAL units, {} bytes, keyframe={}", nal_count, total_bytes, is_keyframe);
        Ok(())
    }

    /// Get configuration
    pub fn config(&self) -> &UnifiedVideoTrackConfig {
        &self.config
    }
}

/// Check if VP8 frame is a keyframe
pub fn is_vp8_keyframe(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    // VP8 keyframe detection: first byte bit 0 is 0 for keyframe
    (data[0] & 0x01) == 0
}

/// Check if VP9 frame is a keyframe
pub fn is_vp9_keyframe(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    // VP9 keyframe detection: bit 2 of first byte is 0 for keyframe
    (data[0] & 0x04) == 0
}

/// Check if H265 frame contains IDR NAL unit
pub fn is_h265_keyframe(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        // Find start code
        let nal_start = if i + 4 <= data.len()
            && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 0 && data[i + 3] == 1
        {
            i + 4
        } else if i + 3 <= data.len()
            && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1
        {
            i + 3
        } else {
            i += 1;
            continue;
        };

        if nal_start >= data.len() {
            break;
        }

        // H265 NAL type
        let nal_type = (data[nal_start] >> 1) & 0x3F;
        if h265_nal::is_idr(nal_type) {
            return true;
        }

        i = nal_start + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec_mime_types() {
        assert_eq!(VideoCodec::H264.mime_type(), "video/H264");
        assert_eq!(VideoCodec::H265.mime_type(), "video/H265");
        assert_eq!(VideoCodec::VP8.mime_type(), "video/VP8");
        assert_eq!(VideoCodec::VP9.mime_type(), "video/VP9");
    }

    #[test]
    fn test_h265_nal_type() {
        // H265 NAL type is (first_byte >> 1) & 0x3F
        // VPS: type 32 = 0x40 >> 1 = 32
        let vps_header = 0x40u8; // VPS type 32
        let nal_type = (vps_header >> 1) & 0x3F;
        assert_eq!(nal_type, 32);

        // IDR_W_RADL: type 19
        let idr_header = 0x26u8; // type 19 = 0x13 << 1 = 0x26
        let nal_type = (idr_header >> 1) & 0x3F;
        assert_eq!(nal_type, 19);
    }

    #[test]
    fn test_vp8_keyframe_detection() {
        // VP8 keyframe: bit 0 is 0
        assert!(is_vp8_keyframe(&[0x00]));
        assert!(!is_vp8_keyframe(&[0x01]));
    }
}
