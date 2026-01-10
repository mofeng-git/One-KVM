//! RTP packetization for H264 video
//!
//! This module provides H264 RTP packetization using the rtp crate's H264Payloader.
//! It handles:
//! - NAL unit parsing (Annex B start codes)
//! - SPS/PPS collection and STAP-A packetization
//! - Single NAL unit mode for small NALs
//! - FU-A fragmentation for large NALs
//!
//! IMPORTANT: Each NAL unit must be sent separately via write_sample(),
//! without Annex B start codes. The TrackLocalStaticSample handles
//! RTP packetization internally.

use bytes::Bytes;
use rtp::codecs::h264::H264Payloader;
use rtp::packetizer::Payloader;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, trace};
use webrtc::media::io::h264_reader::H264Reader;
use webrtc::media::Sample;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;

use crate::error::{AppError, Result};
use crate::video::format::Resolution;

/// Default MTU for RTP packets (conservative for most networks)
pub const RTP_MTU: usize = 1200;

/// H264 clock rate (always 90kHz per RFC 6184)
pub const H264_CLOCK_RATE: u32 = 90000;

/// H264 video track using TrackLocalStaticSample for proper packetization
pub struct H264VideoTrack {
    /// The underlying WebRTC track with automatic packetization
    track: Arc<TrackLocalStaticSample>,
    /// Track configuration
    config: H264VideoTrackConfig,
    /// H264 payloader for manual packetization (if needed)
    payloader: Mutex<H264Payloader>,
    /// Statistics
    stats: Mutex<H264TrackStats>,
    /// Cached SPS NAL unit for injection before IDR frames
    /// Some hardware encoders don't repeat SPS/PPS with every keyframe
    cached_sps: Mutex<Option<Bytes>>,
    /// Cached PPS NAL unit for injection before IDR frames
    cached_pps: Mutex<Option<Bytes>>,
}

/// H264 video track configuration
#[derive(Debug, Clone)]
pub struct H264VideoTrackConfig {
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
    /// H.264 profile-level-id (e.g., "42001f" for Baseline L3.1, "64001f" for High L3.1)
    /// If None, uses empty string to let browser negotiate
    /// Format: PPCCLL where PP=profile_idc, CC=constraint_flags, LL=level_idc
    pub profile_level_id: Option<String>,
}

impl Default for H264VideoTrackConfig {
    fn default() -> Self {
        Self {
            track_id: "video0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
            resolution: Resolution::HD720,
            bitrate_kbps: 8000,
            fps: 30,
            profile_level_id: None, // Let browser negotiate
        }
    }
}

/// H264 track statistics
#[derive(Debug, Clone, Default)]
pub struct H264TrackStats {
    /// Frames sent
    pub frames_sent: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Packets sent (RTP packets)
    pub packets_sent: u64,
    /// Key frames sent
    pub keyframes_sent: u64,
    /// Errors encountered
    pub errors: u64,
}

impl H264VideoTrack {
    /// Create a new H264 video track
    ///
    /// If `config.profile_level_id` is set, it will be used in SDP negotiation.
    /// Otherwise, uses empty fmtp line to let browser negotiate the best profile.
    pub fn new(config: H264VideoTrackConfig) -> Self {
        // Build sdp_fmtp_line based on profile_level_id
        let sdp_fmtp_line = if let Some(ref profile_level_id) = config.profile_level_id {
            // Use specified profile-level-id
            format!(
                "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id={}",
                profile_level_id
            )
        } else {
            // Let browser negotiate - empty string for maximum compatibility
            String::new()
        };

        let codec = RTCRtpCodecCapability {
            mime_type: "video/H264".to_string(),
            clock_rate: H264_CLOCK_RATE,
            channels: 0,
            sdp_fmtp_line,
            rtcp_feedback: vec![],
        };

        let track = Arc::new(TrackLocalStaticSample::new(
            codec,
            config.track_id.clone(),
            config.stream_id.clone(),
        ));

        Self {
            track,
            config,
            payloader: Mutex::new(H264Payloader::default()),
            stats: Mutex::new(H264TrackStats::default()),
            cached_sps: Mutex::new(None),
            cached_pps: Mutex::new(None),
        }
    }

    /// Get the underlying WebRTC track for adding to peer connection
    pub fn track(&self) -> Arc<TrackLocalStaticSample> {
        self.track.clone()
    }

    /// Get track as TrackLocal for peer connection
    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        self.track.clone()
    }

    /// Get current statistics
    pub async fn stats(&self) -> H264TrackStats {
        self.stats.lock().await.clone()
    }

    /// Write an H264 encoded frame to the track
    ///
    /// The frame data should be H264 Annex B format (with start codes 0x00000001 or 0x000001).
    /// This is the format produced by hwcodec/FFmpeg encoders.
    ///
    /// IMPORTANT: Each NAL unit is sent separately via write_sample(), without start codes.
    /// This is required for proper WebRTC RTP packetization.
    /// See: https://github.com/webrtc-rs/webrtc/blob/master/examples/examples/play-from-disk-h264/
    ///
    /// # Arguments
    /// * `data` - H264 Annex B encoded frame data
    /// * `duration` - Frame duration (typically 1/fps seconds)
    /// * `is_keyframe` - Whether this is a keyframe (IDR frame)
    pub async fn write_frame(&self, data: &[u8], _duration: Duration, is_keyframe: bool) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // Use H264Reader to parse NAL units from Annex B data
        let cursor = Cursor::new(data);
        let mut h264_reader = H264Reader::new(cursor, 1024 * 1024);

        // Collect all NAL units first to check for SPS/PPS presence
        let mut nals: Vec<Bytes> = Vec::new();
        let mut has_sps = false;
        let mut has_pps = false;
        let mut has_idr = false;

        // Send each NAL unit separately (like official webrtc-rs example)
        // H264Reader returns NAL data WITHOUT start codes - this is what we need
        while let Ok(nal) = h264_reader.next_nal() {
            if nal.data.is_empty() {
                continue;
            }

            let nal_type = nal.data[0] & 0x1F;

            // Skip AUD NAL units (type 9) - not needed for WebRTC
            if nal_type == 9 {
                continue;
            }

            // Skip filler data (type 12)
            if nal_type == 12 {
                continue;
            }

            // Track NAL types
            match nal_type {
                5 => has_idr = true,
                7 => {
                    has_sps = true;
                    // Cache SPS for future injection
                    *self.cached_sps.lock().await = Some(nal.data.clone().freeze());
                }
                8 => {
                    has_pps = true;
                    // Cache PPS for future injection
                    *self.cached_pps.lock().await = Some(nal.data.clone().freeze());
                }
                _ => {}
            }

            trace!(
                "Sending NAL: type={} ({}) size={} bytes",
                nal_type,
                match nal_type {
                    1 => "Non-IDR slice",
                    5 => "IDR slice",
                    6 => "SEI",
                    7 => "SPS",
                    8 => "PPS",
                    _ => "Other",
                },
                nal.data.len()
            );

            nals.push(nal.data.freeze());
        }

        // Inject cached SPS/PPS before IDR if missing
        // This is critical for hardware encoders that don't repeat SPS/PPS
        if has_idr && (!has_sps || !has_pps) {
            let mut injected_nals: Vec<Bytes> = Vec::new();

            if !has_sps {
                if let Some(sps) = self.cached_sps.lock().await.clone() {
                    debug!("Injecting cached SPS before IDR frame");
                    injected_nals.push(sps);
                }
            }
            if !has_pps {
                if let Some(pps) = self.cached_pps.lock().await.clone() {
                    debug!("Injecting cached PPS before IDR frame");
                    injected_nals.push(pps);
                }
            }

            if !injected_nals.is_empty() {
                injected_nals.extend(nals);
                nals = injected_nals;
            }
        }

        let mut nal_count = 0;
        let mut total_bytes = 0u64;

        // Send NAL data directly WITHOUT start codes
        // TrackLocalStaticSample handles RTP packetization internally
        // Use duration=1s for each NAL like official webrtc-rs example
        for nal_data in nals {
            let sample = Sample {
                data: nal_data.clone(),
                duration: Duration::from_secs(1), // Like official example
                ..Default::default()
            };

            if let Err(e) = self.track.write_sample(&sample).await {
                // Only log periodically to avoid spam when no peer connected
                if nal_count % 100 == 0 {
                    debug!("Write sample failed (no peer?): {}", e);
                }
            }

            total_bytes += nal_data.len() as u64;
            nal_count += 1;
        }

        // Update statistics
        if nal_count > 0 {
            let mut stats = self.stats.lock().await;
            stats.frames_sent += 1;
            stats.bytes_sent += total_bytes;
            if is_keyframe {
                stats.keyframes_sent += 1;
            }
        }

        trace!(
            "Sent frame: {} NAL units, {} bytes, keyframe={}",
            nal_count,
            total_bytes,
            is_keyframe
        );

        Ok(())
    }

    /// Write frame with timestamp (for more precise timing control)
    pub async fn write_frame_with_timestamp(
        &self,
        data: &[u8],
        _pts_ms: u64,
        is_keyframe: bool,
    ) -> Result<()> {
        // Convert pts from milliseconds to frame duration
        // Assuming constant frame rate from config
        let duration = Duration::from_millis(1000 / self.config.fps as u64);
        self.write_frame(data, duration, is_keyframe).await
    }

    /// Manually packetize H264 data into RTP payloads
    ///
    /// This is useful if you need direct control over RTP packets
    /// (e.g., for sending via TrackLocalStaticRTP instead of TrackLocalStaticSample)
    pub async fn packetize(&self, data: &[u8], mtu: usize) -> Result<Vec<Bytes>> {
        let mut payloader = self.payloader.lock().await;
        let bytes = Bytes::copy_from_slice(data);

        payloader.payload(mtu, &bytes).map_err(|e| {
            AppError::VideoError(format!("H264 packetization failed: {}", e))
        })
    }

    /// Get configuration
    pub fn config(&self) -> &H264VideoTrackConfig {
        &self.config
    }
}

/// Opus audio track using TrackLocalStaticSample
pub struct OpusAudioTrack {
    /// The underlying WebRTC track
    track: Arc<TrackLocalStaticSample>,
    /// Statistics
    stats: Mutex<OpusTrackStats>,
}

/// Opus track statistics
#[derive(Debug, Clone, Default)]
pub struct OpusTrackStats {
    /// Packets sent
    pub packets_sent: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Errors
    pub errors: u64,
}

impl OpusAudioTrack {
    /// Create a new Opus audio track
    pub fn new(track_id: &str, stream_id: &str) -> Self {
        let codec = RTCRtpCodecCapability {
            mime_type: "audio/opus".to_string(),
            clock_rate: 48000,
            channels: 2,
            sdp_fmtp_line: "minptime=10;useinbandfec=1".to_string(),
            rtcp_feedback: vec![],
        };

        let track = Arc::new(TrackLocalStaticSample::new(
            codec,
            track_id.to_string(),
            stream_id.to_string(),
        ));

        Self {
            track,
            stats: Mutex::new(OpusTrackStats::default()),
        }
    }

    /// Get the underlying WebRTC track
    pub fn track(&self) -> Arc<TrackLocalStaticSample> {
        self.track.clone()
    }

    /// Get track as TrackLocal
    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        self.track.clone()
    }

    /// Get statistics
    pub async fn stats(&self) -> OpusTrackStats {
        self.stats.lock().await.clone()
    }

    /// Write Opus encoded audio data
    ///
    /// # Arguments
    /// * `data` - Opus encoded packet
    /// * `samples` - Number of audio samples in this packet (typically 960 for 20ms at 48kHz)
    pub async fn write_packet(&self, data: &[u8], samples: u32) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        // Opus frame duration based on samples
        // 48000 Hz, so duration = samples / 48000 seconds
        let duration = Duration::from_micros((samples as u64 * 1_000_000) / 48000);

        let sample = Sample {
            data: Bytes::copy_from_slice(data),
            duration,
            ..Default::default()
        };

        match self.track.write_sample(&sample).await {
            Ok(_) => {
                let mut stats = self.stats.lock().await;
                stats.packets_sent += 1;
                stats.bytes_sent += data.len() as u64;
                Ok(())
            }
            Err(e) => {
                let mut stats = self.stats.lock().await;
                stats.errors += 1;
                error!("Failed to write Opus sample: {}", e);
                Err(AppError::WebRtcError(format!("Failed to write audio sample: {}", e)))
            }
        }
    }
}

/// Strip AUD (Access Unit Delimiter) NAL units from H264 Annex B data
/// AUD (NAL type 9) can cause decoding issues in some browser WebRTC implementations
/// Also strips filler data (NAL type 12) and SEI (NAL type 6) for cleaner output
pub fn strip_aud_nal_units(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        // Find start code (3 or 4 bytes)
        let (start_code_pos, start_code_len) = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            (i, 4)
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            (i, 3)
        } else {
            i += 1;
            continue;
        };

        let nal_start = start_code_pos + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        // Find next start code to determine NAL unit end
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

        // Skip AUD (9), filler (12), and optionally SEI (6)
        // Keep SPS (7), PPS (8), IDR (5), non-IDR slice (1)
        if nal_type != 9 && nal_type != 12 {
            // Include this NAL unit with start code
            result.extend_from_slice(&data[start_code_pos..nal_end]);
        }

        i = nal_end;
    }

    // If nothing was stripped, return original data
    if result.is_empty() && !data.is_empty() {
        return data.to_vec();
    }

    result
}

/// Extract SPS and PPS NAL units from H264 Annex B data
/// Returns (SPS data without start code, PPS data without start code)
pub fn extract_sps_pps(data: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut sps: Option<Vec<u8>> = None;
    let mut pps: Option<Vec<u8>> = None;
    let mut i = 0;

    while i < data.len() {
        // Find start code (3 or 4 bytes)
        let start_code_len = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            4
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            3
        } else {
            i += 1;
            continue;
        };

        let nal_start = i + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        // Find next start code to determine NAL unit end
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

        // Extract SPS (NAL type 7) and PPS (NAL type 8) without start codes
        match nal_type {
            7 => {
                sps = Some(data[nal_start..nal_end].to_vec());
            }
            8 => {
                pps = Some(data[nal_start..nal_end].to_vec());
            }
            _ => {}
        }

        i = nal_end;
    }

    (sps, pps)
}

/// Check if H264 Annex B data contains SPS and PPS NAL units
pub fn has_sps_pps(data: &[u8]) -> bool {
    let mut has_sps = false;
    let mut has_pps = false;
    let mut i = 0;

    while i < data.len() {
        // Find start code (3 or 4 bytes)
        let start_code_len = if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            4
        } else if i + 3 <= data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            3
        } else {
            i += 1;
            continue;
        };

        let nal_start = i + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let nal_type = data[nal_start] & 0x1F;

        match nal_type {
            7 => has_sps = true,
            8 => has_pps = true,
            _ => {}
        }

        if has_sps && has_pps {
            return true;
        }

        // Move past start code to next position
        i = nal_start + 1;
    }

    has_sps && has_pps
}

/// Check if H264 data contains a keyframe (IDR NAL unit)
pub fn is_h264_keyframe(data: &[u8]) -> bool {
    // Look for IDR NAL unit (type 5)
    // NAL units start with 0x00 0x00 0x01 or 0x00 0x00 0x00 0x01
    let mut i = 0;
    while i < data.len() {
        // Find start code
        if i + 3 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            let (nal_start, _start_code_len) = if data[i + 2] == 1 {
                (i + 3, 3)
            } else if i + 4 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                (i + 4, 4)
            } else {
                i += 1;
                continue;
            };

            if nal_start < data.len() {
                let nal_type = data[nal_start] & 0x1F;
                // IDR = 5, SPS = 7, PPS = 8
                if nal_type == 5 {
                    return true;
                }
            }
            i = nal_start;
        } else {
            i += 1;
        }
    }
    false
}

/// Parse profile-level-id from SPS NAL unit data (without start code)
///
/// Returns a 6-character hex string like "42001f" (Baseline L3.1) or "64001f" (High L3.1)
///
/// SPS structure (first 4 bytes after NAL header):
/// - Byte 0: NAL header (0x67 for SPS)
/// - Byte 1: profile_idc (0x42=Baseline, 0x4D=Main, 0x64=High)
/// - Byte 2: constraint_set_flags
/// - Byte 3: level_idc (0x1f=3.1, 0x28=4.0, 0x33=5.1)
pub fn parse_profile_level_id_from_sps(sps: &[u8]) -> Option<String> {
    // SPS NAL must be at least 4 bytes: NAL header + profile_idc + constraints + level_idc
    if sps.len() < 4 {
        return None;
    }

    // First byte is NAL header, skip it
    let profile_idc = sps[1];
    let constraint_set_flags = sps[2];
    let level_idc = sps[3];

    Some(format!(
        "{:02x}{:02x}{:02x}",
        profile_idc, constraint_set_flags, level_idc
    ))
}

/// Extract profile-level-id from H264 Annex B data (containing SPS)
///
/// This function finds the SPS NAL unit and extracts the profile-level-id.
/// Useful for determining the actual encoder output profile.
///
/// # Example
/// ```ignore
/// let h264_data = encoder.encode(&yuv)?;
/// if let Some(profile_level_id) = extract_profile_level_id(&h264_data) {
///     println!("Encoder outputs profile-level-id: {}", profile_level_id);
///     // Use this to configure H264VideoTrackConfig
/// }
/// ```
pub fn extract_profile_level_id(data: &[u8]) -> Option<String> {
    let (sps, _) = extract_sps_pps(data);
    sps.and_then(|sps_data| parse_profile_level_id_from_sps(&sps_data))
}

/// Common H.264 profile-level-id values
pub mod profiles {
    /// Constrained Baseline Profile Level 3.1 - Maximum browser compatibility
    pub const CONSTRAINED_BASELINE_31: &str = "42e01f";
    /// Baseline Profile Level 3.1
    pub const BASELINE_31: &str = "42001f";
    /// Main Profile Level 3.1
    pub const MAIN_31: &str = "4d001f";
    /// High Profile Level 3.1 - Hardware encoders typically output this
    pub const HIGH_31: &str = "64001f";
    /// High Profile Level 4.0
    pub const HIGH_40: &str = "640028";
    /// High Profile Level 5.1
    pub const HIGH_51: &str = "640033";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_h264_keyframe() {
        // IDR frame with 4-byte start code
        let idr_frame = vec![0x00, 0x00, 0x00, 0x01, 0x65]; // NAL type 5 = IDR
        assert!(is_h264_keyframe(&idr_frame));

        // IDR frame with 3-byte start code
        let idr_frame_3 = vec![0x00, 0x00, 0x01, 0x65];
        assert!(is_h264_keyframe(&idr_frame_3));

        // Non-IDR frame (P-frame, NAL type 1)
        let p_frame = vec![0x00, 0x00, 0x00, 0x01, 0x41];
        assert!(!is_h264_keyframe(&p_frame));

        // SPS (NAL type 7) - not a keyframe by itself
        let sps = vec![0x00, 0x00, 0x00, 0x01, 0x67];
        assert!(!is_h264_keyframe(&sps));

        // Multiple NAL units with IDR
        let multi_nal = vec![
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, // SPS
            0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x38, 0x80, // PPS
            0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, // IDR
        ];
        assert!(is_h264_keyframe(&multi_nal));
    }

    #[test]
    fn test_h264_track_config_default() {
        let config = H264VideoTrackConfig::default();
        assert_eq!(config.fps, 30);
        assert_eq!(config.bitrate_kbps, 8000);
        assert_eq!(config.resolution, Resolution::HD720);
    }
}
