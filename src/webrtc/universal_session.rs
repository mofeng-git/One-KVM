//! Universal WebRTC session with multi-codec support
//!
//! Provides WebRTC sessions that can use any supported video codec (H264, H265, VP8, VP9).
//! Replaces the H264-only H264Session with a more flexible implementation.

use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing::{debug, info, trace, warn};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType};
use webrtc::rtp_transceiver::RTCPFeedback;

use super::config::WebRtcConfig;
use super::rtp::OpusAudioTrack;
use super::signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer};
use super::video_track::{UniversalVideoTrack, UniversalVideoTrackConfig, VideoCodec};
use crate::audio::OpusFrame;
use crate::error::{AppError, Result};
use crate::hid::datachannel::{parse_hid_message, HidChannelEvent};
use crate::hid::HidController;
use crate::video::encoder::registry::VideoEncoderType;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::shared_video_pipeline::EncodedVideoFrame;

/// H.265/HEVC MIME type (RFC 7798)
const MIME_TYPE_H265: &str = "video/H265";

/// Universal WebRTC session configuration
#[derive(Debug, Clone)]
pub struct UniversalSessionConfig {
    /// WebRTC configuration
    pub webrtc: WebRtcConfig,
    /// Video codec type
    pub codec: VideoEncoderType,
    /// Input resolution
    pub resolution: Resolution,
    /// Input pixel format
    pub input_format: PixelFormat,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Target FPS
    pub fps: u32,
    /// GOP size
    pub gop_size: u32,
    /// Enable audio track
    pub audio_enabled: bool,
}

impl Default for UniversalSessionConfig {
    fn default() -> Self {
        Self {
            webrtc: WebRtcConfig::default(),
            codec: VideoEncoderType::H264,
            resolution: Resolution::HD720,
            input_format: PixelFormat::Mjpeg,
            bitrate_kbps: 1000,
            fps: 30,
            gop_size: 30,
            audio_enabled: false,
        }
    }
}

impl UniversalSessionConfig {
    /// Create config for specific codec
    pub fn with_codec(codec: VideoEncoderType) -> Self {
        Self {
            codec,
            ..Default::default()
        }
    }
}

/// Convert VideoEncoderType to VideoCodec
fn encoder_type_to_video_codec(encoder_type: VideoEncoderType) -> VideoCodec {
    match encoder_type {
        VideoEncoderType::H264 => VideoCodec::H264,
        VideoEncoderType::H265 => VideoCodec::H265,
        VideoEncoderType::VP8 => VideoCodec::VP8,
        VideoEncoderType::VP9 => VideoCodec::VP9,
    }
}

/// Universal WebRTC session
///
/// Receives pre-encoded video frames and sends via WebRTC.
/// Supports H264, H265, VP8, VP9 codecs.
pub struct UniversalSession {
    /// Session ID
    pub session_id: String,
    /// Video codec type
    codec: VideoEncoderType,
    /// WebRTC peer connection
    pc: Arc<RTCPeerConnection>,
    /// Video track for RTP packetization
    video_track: Arc<UniversalVideoTrack>,
    /// Opus audio track (optional)
    audio_track: Option<Arc<OpusAudioTrack>>,
    /// Data channel for HID events
    data_channel: Arc<RwLock<Option<Arc<RTCDataChannel>>>>,
    /// Connection state
    state: Arc<watch::Sender<ConnectionState>>,
    /// State receiver
    state_rx: watch::Receiver<ConnectionState>,
    /// ICE candidates gathered
    ice_candidates: Arc<Mutex<Vec<IceCandidate>>>,
    /// HID controller reference
    hid_controller: Option<Arc<HidController>>,
    /// Video frame receiver handle
    video_receiver_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Audio frame receiver handle
    audio_receiver_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// FPS configuration
    fps: u32,
}

impl UniversalSession {
    /// Create a new universal WebRTC session
    pub async fn new(config: UniversalSessionConfig, session_id: String) -> Result<Self> {
        info!(
            "Creating {} session: {} @ {}x{} (audio={})",
            config.codec,
            session_id,
            config.resolution.width,
            config.resolution.height,
            config.audio_enabled
        );

        // Create video track with appropriate codec
        let video_codec = encoder_type_to_video_codec(config.codec);
        let track_config = UniversalVideoTrackConfig {
            track_id: format!("video-{}", &session_id[..8.min(session_id.len())]),
            stream_id: "one-kvm-stream".to_string(),
            codec: video_codec,
            resolution: config.resolution,
            bitrate_kbps: config.bitrate_kbps,
            fps: config.fps,
        };
        let video_track = Arc::new(UniversalVideoTrack::new(track_config));

        // Create Opus audio track if enabled
        let audio_track = if config.audio_enabled {
            Some(Arc::new(OpusAudioTrack::new(
                &format!("audio-{}", &session_id[..8.min(session_id.len())]),
                "one-kvm-stream",
            )))
        } else {
            None
        };

        // Create media engine
        let mut media_engine = MediaEngine::default();

        // Register H.265/HEVC codec (not included in default codecs)
        // According to RFC 7798, H.265 uses MIME type video/H265
        if config.codec == VideoEncoderType::H265 {
            let video_rtcp_feedback = vec![
                RTCPFeedback {
                    typ: "goog-remb".to_owned(),
                    parameter: "".to_owned(),
                },
                RTCPFeedback {
                    typ: "ccm".to_owned(),
                    parameter: "fir".to_owned(),
                },
                RTCPFeedback {
                    typ: "nack".to_owned(),
                    parameter: "".to_owned(),
                },
                RTCPFeedback {
                    typ: "nack".to_owned(),
                    parameter: "pli".to_owned(),
                },
            ];

            // Register H.265 with profile-id=1 (Main profile) - matches Chrome's offer
            // Chrome sends: level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST
            media_engine
                .register_codec(
                    RTCRtpCodecParameters {
                        capability: RTCRtpCodecCapability {
                            mime_type: MIME_TYPE_H265.to_owned(),
                            clock_rate: 90000,
                            channels: 0,
                            // Match browser's fmtp format for profile-id=1
                            sdp_fmtp_line: "level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST".to_owned(),
                            rtcp_feedback: video_rtcp_feedback.clone(),
                        },
                        payload_type: 49, // Use same payload type as browser
                        ..Default::default()
                    },
                    RTPCodecType::Video,
                )
                .map_err(|e| AppError::VideoError(format!("Failed to register H.265 codec: {}", e)))?;

            // Also register profile-id=2 (Main 10) variant
            media_engine
                .register_codec(
                    RTCRtpCodecParameters {
                        capability: RTCRtpCodecCapability {
                            mime_type: MIME_TYPE_H265.to_owned(),
                            clock_rate: 90000,
                            channels: 0,
                            sdp_fmtp_line: "level-id=180;profile-id=2;tier-flag=0;tx-mode=SRST".to_owned(),
                            rtcp_feedback: video_rtcp_feedback,
                        },
                        payload_type: 51,
                        ..Default::default()
                    },
                    RTPCodecType::Video,
                )
                .map_err(|e| AppError::VideoError(format!("Failed to register H.265 codec (profile 2): {}", e)))?;

            info!("Registered H.265/HEVC codec for session {}", session_id);
        }

        media_engine
            .register_default_codecs()
            .map_err(|e| AppError::VideoError(format!("Failed to register codecs: {}", e)))?;

        // Create interceptor registry
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .map_err(|e| AppError::VideoError(format!("Failed to register interceptors: {}", e)))?;

        // Create API
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        // Build ICE servers
        let mut ice_servers = vec![];
        for stun_url in &config.webrtc.stun_servers {
            ice_servers.push(RTCIceServer {
                urls: vec![stun_url.clone()],
                ..Default::default()
            });
        }
        for turn in &config.webrtc.turn_servers {
            // Skip TURN servers without credentials (webrtc-rs requires them)
            if turn.username.is_empty() || turn.credential.is_empty() {
                warn!(
                    "Skipping TURN server {} - credentials required but missing",
                    turn.url
                );
                continue;
            }
            ice_servers.push(RTCIceServer {
                urls: vec![turn.url.clone()],
                username: turn.username.clone(),
                credential: turn.credential.clone(),
                ..Default::default()
            });
        }

        // Create peer connection
        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let pc = api
            .new_peer_connection(rtc_config)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to create peer connection: {}", e)))?;

        let pc = Arc::new(pc);

        // Add video track to peer connection
        pc.add_track(video_track.as_track_local())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to add video track: {}", e)))?;

        info!(
            "{} video track added to peer connection (session {})",
            config.codec, session_id
        );

        // Add Opus audio track if enabled
        if let Some(ref audio) = audio_track {
            pc.add_track(audio.as_track_local())
                .await
                .map_err(|e| AppError::AudioError(format!("Failed to add audio track: {}", e)))?;
            info!("Opus audio track added to peer connection (session {})", session_id);
        }

        // Create state channel
        let (state_tx, state_rx) = watch::channel(ConnectionState::New);

        let session = Self {
            session_id,
            codec: config.codec,
            pc,
            video_track,
            audio_track,
            data_channel: Arc::new(RwLock::new(None)),
            state: Arc::new(state_tx),
            state_rx,
            ice_candidates: Arc::new(Mutex::new(vec![])),
            hid_controller: None,
            video_receiver_handle: Mutex::new(None),
            audio_receiver_handle: Mutex::new(None),
            fps: config.fps,
        };

        // Set up event handlers
        session.setup_event_handlers().await;

        Ok(session)
    }

    /// Set up peer connection event handlers
    async fn setup_event_handlers(&self) {
        let state = self.state.clone();
        let session_id = self.session_id.clone();
        let codec = self.codec;

        // Connection state change handler
        self.pc
            .on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
                let state = state.clone();
                let session_id = session_id.clone();

                Box::pin(async move {
                    let new_state = match s {
                        RTCPeerConnectionState::New => ConnectionState::New,
                        RTCPeerConnectionState::Connecting => ConnectionState::Connecting,
                        RTCPeerConnectionState::Connected => ConnectionState::Connected,
                        RTCPeerConnectionState::Disconnected => ConnectionState::Disconnected,
                        RTCPeerConnectionState::Failed => ConnectionState::Failed,
                        RTCPeerConnectionState::Closed => ConnectionState::Closed,
                        _ => return,
                    };

                    info!("{} session {} state: {}", codec, session_id, new_state);
                    let _ = state.send(new_state);
                })
            }));

        // ICE connection state handler
        let session_id_ice = self.session_id.clone();
        self.pc
            .on_ice_connection_state_change(Box::new(move |state| {
                let session_id = session_id_ice.clone();
                Box::pin(async move {
                    info!("[ICE] Session {} connection state: {:?}", session_id, state);
                })
            }));

        // ICE gathering state handler
        let session_id_gather = self.session_id.clone();
        self.pc
            .on_ice_gathering_state_change(Box::new(move |state| {
                let session_id = session_id_gather.clone();
                Box::pin(async move {
                    debug!("[ICE] Session {} gathering state: {:?}", session_id, state);
                })
            }));

        // ICE candidate handler
        let ice_candidates = self.ice_candidates.clone();
        self.pc
            .on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
                let ice_candidates = ice_candidates.clone();

                Box::pin(async move {
                    if let Some(c) = candidate {
                        let candidate_str = c.to_json().map(|j| j.candidate).unwrap_or_default();
                        debug!("ICE candidate: {}", candidate_str);

                        let mut candidates = ice_candidates.lock().await;
                        candidates.push(IceCandidate {
                            candidate: candidate_str,
                            sdp_mid: c.to_json().ok().and_then(|j| j.sdp_mid),
                            sdp_mline_index: c.to_json().ok().and_then(|j| j.sdp_mline_index),
                            username_fragment: None,
                        });
                    }
                })
            }));

        // Data channel handler
        let data_channel = self.data_channel.clone();
        self.pc
            .on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let data_channel = data_channel.clone();

                Box::pin(async move {
                    info!("Data channel opened: {}", dc.label());
                    *data_channel.write().await = Some(dc.clone());

                    dc.on_message(Box::new(move |msg: DataChannelMessage| {
                        debug!("DataChannel message: {} bytes", msg.data.len());
                        Box::pin(async {})
                    }));
                })
            }));
    }

    /// Set HID controller for DataChannel HID processing
    pub fn set_hid_controller(&mut self, hid: Arc<HidController>) {
        let hid_clone = hid.clone();
        let data_channel = self.data_channel.clone();

        self.pc
            .on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let data_channel = data_channel.clone();
                let hid = hid_clone.clone();

                Box::pin(async move {
                    info!("Data channel with HID support: {}", dc.label());
                    *data_channel.write().await = Some(dc.clone());

                    dc.on_message(Box::new(move |msg: DataChannelMessage| {
                        let hid = hid.clone();

                        Box::pin(async move {
                            if let Some(event) = parse_hid_message(&msg.data) {
                                match event {
                                    HidChannelEvent::Keyboard(kb_event) => {
                                        if let Err(e) = hid.send_keyboard(kb_event).await {
                                            debug!("Failed to send keyboard event: {}", e);
                                        }
                                    }
                                    HidChannelEvent::Mouse(ms_event) => {
                                        if let Err(e) = hid.send_mouse(ms_event).await {
                                            debug!("Failed to send mouse event: {}", e);
                                        }
                                    }
                                }
                            }
                        })
                    }));
                })
            }));

        self.hid_controller = Some(hid);
    }

    /// Create data channel for HID events
    pub async fn create_data_channel(&self, label: &str) -> Result<()> {
        let dc = self
            .pc
            .create_data_channel(label, None)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to create data channel: {}", e)))?;

        *self.data_channel.write().await = Some(dc);
        info!("Data channel '{}' created", label);

        Ok(())
    }

    /// Start receiving encoded video frames from shared pipeline
    ///
    /// The `on_connected` callback is called when ICE connection is established,
    /// allowing the caller to request a keyframe at the right time.
    pub async fn start_from_video_pipeline<F>(
        &self,
        mut frame_rx: broadcast::Receiver<EncodedVideoFrame>,
        on_connected: F,
    )
    where
        F: FnOnce() + Send + 'static,
    {
        info!("Starting {} session {} with shared encoder", self.codec, self.session_id);

        let video_track = self.video_track.clone();
        let mut state_rx = self.state_rx.clone();
        let session_id = self.session_id.clone();
        let _fps = self.fps;
        let expected_codec = self.codec;

        let handle = tokio::spawn(async move {
            info!("Video receiver waiting for connection for session {}", session_id);

            // Wait for Connected state before sending frames
            loop {
                let current_state = *state_rx.borrow();
                if current_state == ConnectionState::Connected {
                    break;
                }
                if matches!(current_state, ConnectionState::Closed | ConnectionState::Failed) {
                    info!("Session {} closed before connecting", session_id);
                    return;
                }
                if state_rx.changed().await.is_err() {
                    return;
                }
            }

            info!("Video receiver started for session {} (ICE connected)", session_id);

            // Request keyframe now that connection is established
            on_connected();

            let mut frames_sent: u64 = 0;

            loop {
                tokio::select! {
                    biased;

                    result = state_rx.changed() => {
                        if result.is_err() {
                            break;
                        }
                        let state = *state_rx.borrow();
                        if matches!(state, ConnectionState::Closed | ConnectionState::Failed | ConnectionState::Disconnected) {
                            info!("Session {} closed, stopping receiver", session_id);
                            break;
                        }
                    }

                    result = frame_rx.recv() => {
                        match result {
                            Ok(encoded_frame) => {
                                // Verify codec matches
                                let frame_codec = match encoded_frame.codec {
                                    VideoEncoderType::H264 => VideoEncoderType::H264,
                                    VideoEncoderType::H265 => VideoEncoderType::H265,
                                    VideoEncoderType::VP8 => VideoEncoderType::VP8,
                                    VideoEncoderType::VP9 => VideoEncoderType::VP9,
                                };

                                if frame_codec != expected_codec {
                                    trace!("Skipping frame with codec {:?}, expected {:?}", frame_codec, expected_codec);
                                    continue;
                                }

                                // Debug log for H265 frames
                                if expected_codec == VideoEncoderType::H265 {
                                    if encoded_frame.is_keyframe || frames_sent % 30 == 0 {
                                        debug!(
                                            "[Session-H265] Received frame #{}: size={}, keyframe={}, seq={}",
                                            frames_sent,
                                            encoded_frame.data.len(),
                                            encoded_frame.is_keyframe,
                                            encoded_frame.sequence
                                        );
                                    }
                                }

                                // Send encoded frame via RTP
                                if let Err(e) = video_track
                                    .write_frame(&encoded_frame.data, encoded_frame.is_keyframe)
                                    .await
                                {
                                    if frames_sent % 100 == 0 {
                                        debug!("Failed to write frame to track: {}", e);
                                    }
                                } else {
                                    frames_sent += 1;

                                    // Log successful H265 frame send
                                    if expected_codec == VideoEncoderType::H265 && (encoded_frame.is_keyframe || frames_sent % 30 == 0) {
                                        debug!(
                                            "[Session-H265] Frame #{} sent successfully",
                                            frames_sent
                                        );
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Session {} lagged by {} frames", session_id, n);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                info!("Video frame channel closed for session {}", session_id);
                                break;
                            }
                        }
                    }
                }
            }

            info!("Video receiver stopped for session {} (sent {} frames)", session_id, frames_sent);
        });

        *self.video_receiver_handle.lock().await = Some(handle);
    }

    /// Start receiving Opus audio frames
    pub async fn start_audio_from_opus(&self, mut opus_rx: broadcast::Receiver<OpusFrame>) {
        let audio_track = match &self.audio_track {
            Some(track) => track.clone(),
            None => {
                debug!("Audio track not enabled for session {}", self.session_id);
                return;
            }
        };

        info!("Starting audio receiver for session {}", self.session_id);

        let mut state_rx = self.state_rx.clone();
        let session_id = self.session_id.clone();

        let handle = tokio::spawn(async move {
            // Wait for Connected state before sending audio
            loop {
                let current_state = *state_rx.borrow();
                if current_state == ConnectionState::Connected {
                    break;
                }
                if matches!(current_state, ConnectionState::Closed | ConnectionState::Failed) {
                    info!("Session {} closed before audio could start", session_id);
                    return;
                }
                if state_rx.changed().await.is_err() {
                    return;
                }
            }

            info!("Audio receiver started for session {} (ICE connected)", session_id);

            let mut packets_sent: u64 = 0;

            loop {
                tokio::select! {
                    biased;

                    result = state_rx.changed() => {
                        if result.is_err() {
                            break;
                        }
                        let state = *state_rx.borrow();
                        if matches!(state, ConnectionState::Closed | ConnectionState::Failed | ConnectionState::Disconnected) {
                            info!("Session {} closed, stopping audio receiver", session_id);
                            break;
                        }
                    }

                    result = opus_rx.recv() => {
                        match result {
                            Ok(opus_frame) => {
                                // 20ms at 48kHz = 960 samples
                                let samples = 960u32;
                                if let Err(e) = audio_track.write_packet(&opus_frame.data, samples).await {
                                    if packets_sent % 100 == 0 {
                                        debug!("Failed to write audio packet: {}", e);
                                    }
                                } else {
                                    packets_sent += 1;
                                    trace!(
                                        "Session {} sent audio packet {}: {} bytes",
                                        session_id,
                                        packets_sent,
                                        opus_frame.data.len()
                                    );
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Session {} audio lagged by {} packets", session_id, n);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                info!("Opus channel closed for session {}", session_id);
                                break;
                            }
                        }
                    }
                }
            }

            info!("Audio receiver stopped for session {} (sent {} packets)", session_id, packets_sent);
        });

        *self.audio_receiver_handle.lock().await = Some(handle);
    }

    /// Check if audio is enabled for this session
    pub fn has_audio(&self) -> bool {
        self.audio_track.is_some()
    }

    /// Get codec type
    pub fn codec(&self) -> VideoEncoderType {
        self.codec
    }

    /// Handle SDP offer and create answer
    pub async fn handle_offer(&self, offer: SdpOffer) -> Result<SdpAnswer> {
        // Log offer for debugging H.265 codec negotiation
        if self.codec == VideoEncoderType::H265 {
            let has_h265 = offer.sdp.to_lowercase().contains("h265")
                || offer.sdp.to_lowercase().contains("hevc");
            info!(
                "[SDP] Session {} offer contains H.265: {}",
                self.session_id,
                has_h265
            );
            if !has_h265 {
                warn!("[SDP] Browser offer does not include H.265 codec! Session may fail.");
            }
        }

        let sdp = RTCSessionDescription::offer(offer.sdp)
            .map_err(|e| AppError::VideoError(format!("Invalid SDP offer: {}", e)))?;

        self.pc
            .set_remote_description(sdp)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to set remote description: {}", e)))?;

        let answer = self
            .pc
            .create_answer(None)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to create answer: {}", e)))?;

        // Log answer for debugging
        if self.codec == VideoEncoderType::H265 {
            let has_h265 = answer.sdp.to_lowercase().contains("h265")
                || answer.sdp.to_lowercase().contains("hevc");
            info!(
                "[SDP] Session {} answer contains H.265: {}",
                self.session_id,
                has_h265
            );
            if !has_h265 {
                warn!("[SDP] Answer does not include H.265! Codec negotiation may have failed.");
            }
        }

        self.pc
            .set_local_description(answer.clone())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to set local description: {}", e)))?;

        // Wait for ICE candidates
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let candidates = self.ice_candidates.lock().await.clone();
        Ok(SdpAnswer::with_candidates(answer.sdp, candidates))
    }

    /// Add ICE candidate
    pub async fn add_ice_candidate(&self, candidate: IceCandidate) -> Result<()> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let init = RTCIceCandidateInit {
            candidate: candidate.candidate,
            sdp_mid: candidate.sdp_mid,
            sdp_mline_index: candidate.sdp_mline_index,
            username_fragment: candidate.username_fragment,
        };

        self.pc
            .add_ice_candidate(init)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to add ICE candidate: {}", e)))?;

        Ok(())
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        *self.state_rx.borrow()
    }

    /// Subscribe to state changes
    pub fn state_watch(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    /// Close the session
    pub async fn close(&self) -> Result<()> {
        // Stop video receiver
        if let Some(handle) = self.video_receiver_handle.lock().await.take() {
            handle.abort();
        }

        // Stop audio receiver
        if let Some(handle) = self.audio_receiver_handle.lock().await.take() {
            handle.abort();
        }

        // Close peer connection
        self.pc
            .close()
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to close peer connection: {}", e)))?;

        let _ = self.state.send(ConnectionState::Closed);

        info!("{} session {} closed", self.codec, self.session_id);
        Ok(())
    }
}

/// Session info for listing
#[derive(Debug, Clone)]
pub struct UniversalSessionInfo {
    pub session_id: String,
    pub codec: VideoEncoderType,
    pub created_at: std::time::Instant,
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universal_session_config_default() {
        let config = UniversalSessionConfig::default();
        assert_eq!(config.codec, VideoEncoderType::H264);
        assert_eq!(config.resolution, Resolution::HD720);
    }

    #[test]
    fn test_encoder_type_to_video_codec() {
        assert_eq!(encoder_type_to_video_codec(VideoEncoderType::H264), VideoCodec::H264);
        assert_eq!(encoder_type_to_video_codec(VideoEncoderType::H265), VideoCodec::H265);
        assert_eq!(encoder_type_to_video_codec(VideoEncoderType::VP8), VideoCodec::VP8);
        assert_eq!(encoder_type_to_video_codec(VideoEncoderType::VP9), VideoCodec::VP9);
    }
}
