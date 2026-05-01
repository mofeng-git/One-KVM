//! Multiplex H264/VP8/VP9 (`TrackLocalStaticSample`) vs H265 (`TrackLocalStaticRTP` + [`H265Payloader`]).

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

// rtp `HevcPayloader` mishandles AP+IDR and NAL 20 (`IDR_N_LP`).
use super::h265_payloader::H265Payloader;

use crate::error::Result;
use crate::video::types::Resolution;

const RTP_MTU: usize = 1200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoCodec {
    H264,
    H265,
    VP8,
    VP9,
}

impl VideoCodec {
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "video/H264",
            VideoCodec::H265 => "video/H265",
            VideoCodec::VP8 => "video/VP8",
            VideoCodec::VP9 => "video/VP9",
        }
    }

    pub fn clock_rate(&self) -> u32 {
        90000
    }

    pub fn default_payload_type(&self) -> u8 {
        match self {
            VideoCodec::H264 => 96,
            VideoCodec::VP8 => 97,
            VideoCodec::VP9 => 98,
            VideoCodec::H265 => 99,
        }
    }

    pub fn sdp_fmtp(&self) -> String {
        match self {
            VideoCodec::H264 => {
                "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f".to_string()
            }
            VideoCodec::H265 => {
                "level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST".to_string()
            }
            VideoCodec::VP8 => String::new(),
            VideoCodec::VP9 => "profile-id=0".to_string(),
        }
    }

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

#[derive(Debug, Clone)]
pub struct UniversalVideoTrackConfig {
    pub track_id: String,
    pub stream_id: String,
    pub codec: VideoCodec,
    pub resolution: Resolution,
    pub bitrate_kbps: u32,
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
    pub fn h264(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::H264,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

    pub fn h265(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::H265,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

    pub fn vp8(resolution: Resolution, bitrate_kbps: u32, fps: u32) -> Self {
        Self {
            codec: VideoCodec::VP8,
            resolution,
            bitrate_kbps,
            fps,
            ..Default::default()
        }
    }

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

enum TrackType {
    Sample(Arc<TrackLocalStaticSample>),
    Rtp(Arc<TrackLocalStaticRTP>),
}

struct H265RtpState {
    payloader: H265Payloader,
    sequence_number: u16,
    timestamp: u32,
    timestamp_increment: u32,
}

pub struct UniversalVideoTrack {
    track: TrackType,
    codec: VideoCodec,
    config: UniversalVideoTrackConfig,
    h265_state: Option<Mutex<H265RtpState>>,
}

impl UniversalVideoTrack {
    pub fn new(config: UniversalVideoTrackConfig) -> Self {
        let codec_capability = RTCRtpCodecCapability {
            mime_type: config.codec.mime_type().to_string(),
            clock_rate: config.codec.clock_rate(),
            channels: 0,
            sdp_fmtp_line: config.codec.sdp_fmtp(),
            rtcp_feedback: vec![],
        };

        let (track, h265_state) = if config.codec == VideoCodec::H265 {
            let rtp_track = Arc::new(TrackLocalStaticRTP::new(
                codec_capability,
                config.track_id.clone(),
                config.stream_id.clone(),
            ));

            let h265_state = H265RtpState {
                payloader: H265Payloader::new(),
                sequence_number: rand::random::<u16>(),
                timestamp: rand::random::<u32>(),
                timestamp_increment: 90000 / config.fps.max(1),
            };

            (TrackType::Rtp(rtp_track), Some(Mutex::new(h265_state)))
        } else {
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
            h265_state,
        }
    }

    pub fn as_track_local(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        match &self.track {
            TrackType::Sample(t) => t.clone(),
            TrackType::Rtp(t) => t.clone(),
        }
    }

    pub fn codec(&self) -> VideoCodec {
        self.codec
    }

    pub fn config(&self) -> &UniversalVideoTrackConfig {
        &self.config
    }

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

    /// One Annex-B AU per sample so the stack can STAP/FU internally.
    async fn write_h264_frame(&self, data: Bytes, _is_keyframe: bool) -> Result<()> {
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
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

        Ok(())
    }

    async fn write_h265_frame(&self, data: Bytes, is_keyframe: bool) -> Result<()> {
        self.send_h265_rtp(data, is_keyframe).await
    }

    async fn write_vp8_frame(&self, data: Bytes, _is_keyframe: bool) -> Result<()> {
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
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

        Ok(())
    }

    async fn write_vp9_frame(&self, data: Bytes, _is_keyframe: bool) -> Result<()> {
        let frame_duration = Duration::from_micros(1_000_000 / self.config.fps.max(1) as u64);
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

        Ok(())
    }

    async fn send_h265_rtp(&self, payload: Bytes, _is_keyframe: bool) -> Result<()> {
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

        // Lock only around payloader + seq/ts bump, not RTP write.
        let (payloads, timestamp, seq_start, num_payloads) = {
            let mut state = h265_state.lock().await;

            let payloads = state.payloader.payload(RTP_MTU, &payload);

            if payloads.is_empty() {
                return Ok(());
            }

            let timestamp = state.timestamp;
            let num_payloads = payloads.len();
            let seq_start = state.sequence_number;

            state.sequence_number = state.sequence_number.wrapping_add(num_payloads as u16);
            state.timestamp = state.timestamp.wrapping_add(state.timestamp_increment);

            (payloads, timestamp, seq_start, num_payloads)
        };

        for (i, payload_data) in payloads.into_iter().enumerate() {
            let seq = seq_start.wrapping_add(i as u16);
            let is_last = i == num_payloads - 1;

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
