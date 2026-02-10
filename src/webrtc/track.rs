//! WebRTC track implementations for video and audio

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, watch};
use tracing::{debug, error, info};
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;

use crate::video::frame::VideoFrame;

/// Video track configuration
#[derive(Debug, Clone)]
pub struct VideoTrackConfig {
    /// Track ID
    pub track_id: String,
    /// Stream ID
    pub stream_id: String,
    /// Video codec
    pub codec: VideoCodecType,
    /// Clock rate
    pub clock_rate: u32,
    /// Target bitrate
    pub bitrate_kbps: u32,
}

impl Default for VideoTrackConfig {
    fn default() -> Self {
        Self {
            track_id: "video0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
            codec: VideoCodecType::H264,
            clock_rate: 90000,
            bitrate_kbps: 8000,
        }
    }
}

/// Video codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodecType {
    H264,
    VP8,
    VP9,
}

impl VideoCodecType {
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodecType::H264 => "video/H264",
            VideoCodecType::VP8 => "video/VP8",
            VideoCodecType::VP9 => "video/VP9",
        }
    }

    pub fn sdp_fmtp(&self) -> &'static str {
        match self {
            VideoCodecType::H264 => {
                "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
            }
            VideoCodecType::VP8 => "",
            VideoCodecType::VP9 => "profile-id=0",
        }
    }
}

/// Create RTP codec capability for video
pub fn video_codec_capability(codec: VideoCodecType, clock_rate: u32) -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: codec.mime_type().to_string(),
        clock_rate,
        channels: 0,
        sdp_fmtp_line: codec.sdp_fmtp().to_string(),
        rtcp_feedback: vec![],
    }
}

/// Create RTP codec capability for audio (Opus)
pub fn audio_codec_capability() -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: "audio/opus".to_string(),
        clock_rate: 48000,
        channels: 2,
        sdp_fmtp_line: "minptime=10;useinbandfec=1".to_string(),
        rtcp_feedback: vec![],
    }
}

/// Video track for WebRTC streaming
pub struct VideoTrack {
    config: VideoTrackConfig,
    /// RTP track
    track: Arc<TrackLocalStaticRTP>,
    /// Running flag
    running: Arc<watch::Sender<bool>>,
}

impl VideoTrack {
    /// Create a new video track
    pub fn new(config: VideoTrackConfig) -> Self {
        let capability = video_codec_capability(config.codec, config.clock_rate);

        let track = Arc::new(TrackLocalStaticRTP::new(
            capability,
            config.track_id.clone(),
            config.stream_id.clone(),
        ));

        let (running_tx, _) = watch::channel(false);

        Self {
            config,
            track,
            running: Arc::new(running_tx),
        }
    }

    /// Get the underlying RTP track
    pub fn rtp_track(&self) -> Arc<TrackLocalStaticRTP> {
        self.track.clone()
    }

    /// Start sending frames from a broadcast receiver
    pub async fn start_sending(&self, mut frame_rx: broadcast::Receiver<VideoFrame>) {
        let _ = self.running.send(true);
        let track = self.track.clone();
        let clock_rate = self.config.clock_rate;
        let mut running_rx = self.running.subscribe();

        info!("Starting video track sender");

        tokio::spawn(async move {
            let mut state = SendState::default();
            loop {
                tokio::select! {
                    result = frame_rx.recv() => {
                        match result {
                            Ok(frame) => {
                                if let Err(e) = Self::send_frame(
                                    &track,
                                    &frame,
                                    &mut state,
                                    clock_rate,
                                ).await {
                                    debug!("Failed to send frame: {}", e);
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                debug!("Video track lagged by {} frames", n);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                debug!("Frame channel closed");
                                break;
                            }
                        }
                    }
                    _ = running_rx.changed() => {
                        if !*running_rx.borrow() {
                            debug!("Video track stopped");
                            break;
                        }
                    }
                }
            }

            info!("Video track sender stopped");
        });
    }

    /// Stop sending
    pub fn stop(&self) {
        let _ = self.running.send(false);
    }

    /// Send a single frame as RTP packets
    async fn send_frame(
        track: &TrackLocalStaticRTP,
        frame: &VideoFrame,
        state: &mut SendState,
        clock_rate: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Calculate timestamp increment based on frame timing
        let now = Instant::now();
        let timestamp_increment = if let Some(last) = state.last_frame_time {
            let elapsed = now.duration_since(last);
            ((elapsed.as_secs_f64() * clock_rate as f64) as u32).min(clock_rate / 10)
        } else {
            clock_rate / 30 // Default to 30 fps
        };
        state.last_frame_time = Some(now);

        // Update timestamp
        state.timestamp = state.timestamp.wrapping_add(timestamp_increment);
        let _current_ts = state.timestamp;

        // For H.264, we need to packetize into RTP
        // This is a simplified implementation - real implementation needs proper NAL unit handling
        let data = frame.data();
        let max_payload_size = 1200; // MTU - headers

        let packet_count = data.len().div_ceil(max_payload_size);
        let mut bytes_sent = 0u64;

        for i in 0..packet_count {
            let start = i * max_payload_size;
            let end = ((i + 1) * max_payload_size).min(data.len());
            let _is_last = i == packet_count - 1;

            // Get sequence number
            let _seq_num = state.sequence_number;
            state.sequence_number = state.sequence_number.wrapping_add(1);

            // Build RTP packet payload
            // For simplicity, just send raw data - real implementation needs proper RTP packetization
            let payload = &data[start..end];
            bytes_sent += payload.len() as u64;

            // Write sample (the track handles RTP header construction)
            if let Err(e) = track.write(payload).await {
                error!("Failed to write RTP packet: {}", e);
                return Err(e.into());
            }
        }

        let _ = bytes_sent;

        Ok(())
    }
}

#[derive(Debug, Default)]
struct SendState {
    sequence_number: u16,
    timestamp: u32,
    last_frame_time: Option<Instant>,
}

/// Audio track configuration
#[derive(Debug, Clone)]
pub struct AudioTrackConfig {
    /// Track ID
    pub track_id: String,
    /// Stream ID
    pub stream_id: String,
    /// Sample rate
    pub sample_rate: u32,
    /// Channels
    pub channels: u8,
}

impl Default for AudioTrackConfig {
    fn default() -> Self {
        Self {
            track_id: "audio0".to_string(),
            stream_id: "one-kvm-stream".to_string(),
            sample_rate: 48000,
            channels: 2,
        }
    }
}

/// Audio track for WebRTC streaming
pub struct AudioTrack {
    #[allow(dead_code)]
    config: AudioTrackConfig,
    /// RTP track
    track: Arc<TrackLocalStaticRTP>,
    /// Running flag
    running: Arc<watch::Sender<bool>>,
}

impl AudioTrack {
    /// Create a new audio track
    pub fn new(config: AudioTrackConfig) -> Self {
        let capability = audio_codec_capability();

        let track = Arc::new(TrackLocalStaticRTP::new(
            capability,
            config.track_id.clone(),
            config.stream_id.clone(),
        ));

        let (running_tx, _) = watch::channel(false);

        Self {
            config,
            track,
            running: Arc::new(running_tx),
        }
    }

    /// Get the underlying RTP track
    pub fn rtp_track(&self) -> Arc<TrackLocalStaticRTP> {
        self.track.clone()
    }

    /// Stop sending
    pub fn stop(&self) {
        let _ = self.running.send(false);
    }
}
