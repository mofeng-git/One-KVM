//! Send H.26x as RTP and VP8/VP9 through sample tracks.

use bytes::Bytes;
use rtp::codecs::h264::H264Payloader;
use rtp::packetizer::Payloader;
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

use crate::error::{AppError, Result};
use crate::video::codec::h264_bitstream;
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
            VideoCodec::H264 => h264_bitstream::fallback_webrtc_fmtp_line(),
            VideoCodec::H265 => "level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST".to_string(),
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
    pub sdp_fmtp_line: Option<String>,
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
            sdp_fmtp_line: None,
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

struct H264RtpClock {
    timestamp: u32,
    last_pts_ms: Option<i64>,
    increment: u32,
    started: bool,
}

impl H264RtpClock {
    fn next(&mut self, pts_ms: Option<i64>) -> u32 {
        if self.started {
            let ticks = match (self.last_pts_ms, pts_ms) {
                (Some(previous), Some(current)) => {
                    current.saturating_sub(previous).saturating_mul(90).max(1) as u32
                }
                _ => self.increment,
            };
            self.timestamp = self.timestamp.wrapping_add(ticks);
        }
        self.started = true;
        if let Some(pts) = pts_ms {
            self.last_pts_ms = Some(self.last_pts_ms.map_or(pts, |last| last.max(pts)));
        } else {
            self.last_pts_ms = None;
        }
        self.timestamp
    }
}

struct H264TrackState {
    payloader: H264Payloader,
    sequence_number: u16,
    clock: H264RtpClock,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

pub struct UniversalVideoTrack {
    track: TrackType,
    codec: VideoCodec,
    config: UniversalVideoTrackConfig,
    h265_state: Option<Mutex<H265RtpState>>,
    h264_state: Mutex<H264TrackState>,
}

impl UniversalVideoTrack {
    pub fn new(config: UniversalVideoTrackConfig) -> Self {
        let codec_capability = RTCRtpCodecCapability {
            mime_type: config.codec.mime_type().to_string(),
            clock_rate: config.codec.clock_rate(),
            channels: 0,
            sdp_fmtp_line: config
                .sdp_fmtp_line
                .clone()
                .unwrap_or_else(|| config.codec.sdp_fmtp()),
            rtcp_feedback: vec![],
        };

        let (track, h265_state) = if matches!(config.codec, VideoCodec::H264 | VideoCodec::H265) {
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

            (
                TrackType::Rtp(rtp_track),
                (config.codec == VideoCodec::H265).then(|| Mutex::new(h265_state)),
            )
        } else {
            let sample_track = Arc::new(TrackLocalStaticSample::new(
                codec_capability,
                config.track_id.clone(),
                config.stream_id.clone(),
            ));

            (TrackType::Sample(sample_track), None)
        };

        let h264_state = H264TrackState {
            sps: None,
            pps: None,
            payloader: H264Payloader::default(),
            sequence_number: rand::random(),
            clock: H264RtpClock {
                timestamp: rand::random(),
                last_pts_ms: None,
                increment: 90000 / config.fps.max(1),
                started: false,
            },
        };
        Self {
            track,
            codec: config.codec,
            config,
            h265_state,
            h264_state: Mutex::new(h264_state),
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
        self.write_frame_bytes_at(data, is_keyframe, None).await
    }

    /// Use the encoder's monotonic presentation time for H.264. Counting frames
    /// compresses the RTP timeline whenever capture/encoding misses a frame.
    pub async fn write_frame_bytes_at(
        &self,
        data: Bytes,
        is_keyframe: bool,
        pts_ms: Option<i64>,
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        match self.codec {
            VideoCodec::H264 => self.write_h264_frame(data, pts_ms).await,
            VideoCodec::H265 => self.write_h265_frame(data, is_keyframe).await,
            VideoCodec::VP8 => self.write_vp8_frame(data, is_keyframe).await,
            VideoCodec::VP9 => self.write_vp9_frame(data, is_keyframe).await,
        }
    }

    pub async fn write_frame(&self, data: &[u8], is_keyframe: bool) -> Result<()> {
        self.write_frame_bytes(Bytes::copy_from_slice(data), is_keyframe)
            .await
    }

    /// Keep the stack's H.264 STAP/FU payloader, with explicit RTP timestamps.
    async fn write_h264_frame(&self, data: Bytes, pts_ms: Option<i64>) -> Result<()> {
        let normalized = h264_bitstream::normalize_for_webrtc(data.as_ref());
        let mut data = Bytes::from(normalized);

        let idr = h264_bitstream::is_keyframe(data.as_ref());
        let has_parameter_sets = h264_bitstream::has_sps_pps(data.as_ref());

        let mut state = self.h264_state.lock().await;
        {
            let (sps, pps) = h264_bitstream::extract_sps_pps(data.as_ref());
            if let Some(sps) = sps {
                state.sps = Some(sps);
            }
            if let Some(pps) = pps {
                state.pps = Some(pps);
            }

            if idr && !has_parameter_sets {
                if let (Some(sps), Some(pps)) = (&state.sps, &state.pps) {
                    let mut with_parameter_sets =
                        Vec::with_capacity(data.len() + sps.len() + pps.len() + 8);
                    with_parameter_sets.extend_from_slice(&[0, 0, 0, 1]);
                    with_parameter_sets.extend_from_slice(sps);
                    with_parameter_sets.extend_from_slice(&[0, 0, 0, 1]);
                    with_parameter_sets.extend_from_slice(pps);
                    with_parameter_sets.extend_from_slice(data.as_ref());
                    data = Bytes::from(with_parameter_sets);
                }
            }
        }

        let TrackType::Rtp(track) = &self.track else {
            return Err(AppError::WebRtcError("H264 RTP track missing".to_owned()));
        };
        let payloads = state
            .payloader
            .payload(RTP_MTU - 12, &data)
            .map_err(|e| AppError::WebRtcError(format!("H264 packetization failed: {e}")))?;
        if payloads.is_empty() {
            return Ok(());
        }
        let timestamp = state.clock.next(pts_ms);
        let count = payloads.len();
        // Serialize complete access units, including their sequence allocation.
        for (index, payload) in payloads.into_iter().enumerate() {
            let sequence_number = state.sequence_number;
            state.sequence_number = state.sequence_number.wrapping_add(1);
            let packet = rtp::packet::Packet {
                header: rtp::header::Header {
                    version: 2,
                    marker: index + 1 == count,
                    sequence_number,
                    timestamp,
                    ..Default::default()
                },
                payload,
            };
            track
                .write_rtp(&packet)
                .await
                .map_err(|e| AppError::WebRtcError(format!("H264 RTP write failed: {e}")))?;
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
                    return Err(AppError::WebRtcError(format!(
                        "VP8 write_sample failed: {}",
                        e
                    )));
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
                    return Err(AppError::WebRtcError(format!(
                        "VP9 write_sample failed: {}",
                        e
                    )));
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
                return Err(AppError::WebRtcError(format!(
                    "H265 write_rtp failed: {}",
                    e
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_clock(timestamp: u32) -> H264RtpClock {
        H264RtpClock {
            timestamp,
            last_pts_ms: None,
            increment: 3000,
            started: false,
        }
    }

    #[test]
    fn h264_timestamps_preserve_variable_frame_intervals() {
        let mut clock = test_clock(1000);
        assert_eq!(clock.next(Some(100)), 1000);
        assert_eq!(clock.next(Some(133)), 3970);
        // Missing input frames must leave a gap in media time too.
        assert_eq!(clock.next(Some(300)), 19000);
        assert_eq!(clock.next(Some(333)), 21970);
    }

    #[test]
    fn h264_timestamps_wrap_and_handle_legacy_or_repeated_pts() {
        let mut clock = test_clock(u32::MAX - 89);
        assert_eq!(clock.next(Some(0)), u32::MAX - 89);
        assert_eq!(clock.next(Some(1)), 0);
        assert_eq!(clock.next(Some(1)), 1);
        assert_eq!(clock.next(Some(0)), 2);
        assert_eq!(clock.next(Some(2)), 92);
        assert_eq!(clock.next(None), 3092);
        assert_eq!(clock.next(None), 6092);
    }

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
