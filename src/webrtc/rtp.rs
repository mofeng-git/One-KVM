//! Opus outbound track plus H.264 Annex B helpers (SPS/PPS, keyframe scan). Video RTP lives in [`crate::webrtc::video_track`].

use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tracing::error;
use webrtc::media::Sample;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::error::{AppError, Result};

pub const RTP_MTU: usize = 1200;

pub const H264_CLOCK_RATE: u32 = 90000;

pub struct OpusAudioTrack {
    track: Arc<TrackLocalStaticSample>,
}

impl OpusAudioTrack {
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

        Self { track }
    }

    pub fn track(&self) -> Arc<TrackLocalStaticSample> {
        self.track.clone()
    }

    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        self.track.clone()
    }

    pub async fn write_packet(&self, data: &[u8], samples: u32) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let duration = Duration::from_micros((samples as u64 * 1_000_000) / 48000);

        let sample = Sample {
            data: Bytes::copy_from_slice(data),
            duration,
            ..Default::default()
        };

        self.track.write_sample(&sample).await.map_err(|e| {
            error!("Failed to write Opus sample: {}", e);
            AppError::WebRtcError(format!("Failed to write audio sample: {}", e))
        })
    }
}

/// Strips AUD (9) and filler (12) NALs; some WebRTC stacks dislike AUD.
pub fn strip_aud_nal_units(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
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

        if nal_type != 9 && nal_type != 12 {
            result.extend_from_slice(&data[start_code_pos..nal_end]);
        }

        i = nal_end;
    }

    if result.is_empty() && !data.is_empty() {
        return data.to_vec();
    }

    result
}

pub fn extract_sps_pps(data: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut sps: Option<Vec<u8>> = None;
    let mut pps: Option<Vec<u8>> = None;
    let mut i = 0;

    while i < data.len() {
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

pub fn has_sps_pps(data: &[u8]) -> bool {
    let mut has_sps = false;
    let mut has_pps = false;
    let mut i = 0;

    while i < data.len() {
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

        i = nal_start + 1;
    }

    has_sps && has_pps
}

pub fn is_h264_keyframe(data: &[u8]) -> bool {
    let mut i = 0;
    while i < data.len() {
        if i + 3 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            let nal_start = if data[i + 2] == 1 {
                i + 3
            } else if i + 4 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                i + 4
            } else {
                i += 1;
                continue;
            };

            if nal_start < data.len() {
                let nal_type = data[nal_start] & 0x1F;
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

/// `profile-level-id` hex for SDP (`42001f` etc.); expects SPS NAL RBSP without start code.
pub fn parse_profile_level_id_from_sps(sps: &[u8]) -> Option<String> {
    if sps.len() < 4 {
        return None;
    }

    let profile_idc = sps[1];
    let constraint_set_flags = sps[2];
    let level_idc = sps[3];

    Some(format!(
        "{:02x}{:02x}{:02x}",
        profile_idc, constraint_set_flags, level_idc
    ))
}

pub fn extract_profile_level_id(data: &[u8]) -> Option<String> {
    let (sps, _) = extract_sps_pps(data);
    sps.and_then(|sps_data| parse_profile_level_id_from_sps(&sps_data))
}

pub mod profiles {
    pub const CONSTRAINED_BASELINE_31: &str = "42e01f";
    pub const BASELINE_31: &str = "42001f";
    pub const MAIN_31: &str = "4d001f";
    pub const HIGH_31: &str = "64001f";
    pub const HIGH_40: &str = "640028";
    pub const HIGH_51: &str = "640033";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_h264_keyframe() {
        let idr_frame = vec![0x00, 0x00, 0x00, 0x01, 0x65];
        assert!(is_h264_keyframe(&idr_frame));

        let idr_frame_3 = vec![0x00, 0x00, 0x01, 0x65];
        assert!(is_h264_keyframe(&idr_frame_3));

        let p_frame = vec![0x00, 0x00, 0x00, 0x01, 0x41];
        assert!(!is_h264_keyframe(&p_frame));

        let sps = vec![0x00, 0x00, 0x00, 0x01, 0x67];
        assert!(!is_h264_keyframe(&sps));

        let multi_nal = vec![
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x01, 0x68, 0xce,
            0x38, 0x80, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84,
        ];
        assert!(is_h264_keyframe(&multi_nal));
    }
}
