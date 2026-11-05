//! Opus outbound track. Video RTP lives in [`crate::webrtc::video_track`].

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
