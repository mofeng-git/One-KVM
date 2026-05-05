//! One browser session: negotiated [`RTCPeerConnection`], outbound video/audio, HID DataChannel.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex, RwLock};
use tracing::{debug, info, warn};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice::mdns::MulticastDnsMode;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::rtp_transceiver::RTCPFeedback;

use super::config::WebRtcConfig;
use super::mdns::{default_mdns_host_name, mdns_mode};
use super::rtp::OpusAudioTrack;
use super::signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer};
use super::video_track::{UniversalVideoTrack, UniversalVideoTrackConfig, VideoCodec};
use crate::audio::OpusFrame;
use crate::error::{AppError, Result};
use crate::hid::datachannel::{parse_hid_message, HidChannelEvent};
use crate::hid::HidController;
use crate::video::types::{
    BitratePreset, EncodedVideoFrame, PixelFormat, Resolution, VideoEncoderType,
};
use std::sync::atomic::AtomicBool;

const MIME_TYPE_H265: &str = "video/H265";

fn h264_contains_parameter_sets(data: &[u8]) -> bool {
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let sc_len = if i + 4 <= data.len()
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

        let nal_start = i + sc_len;
        if nal_start < data.len() {
            let nal_type = data[nal_start] & 0x1F;
            if nal_type == 7 || nal_type == 8 {
                return true;
            }
        }
        i = nal_start.saturating_add(1);
    }

    let mut pos = 0usize;
    while pos + 4 <= data.len() {
        let nalu_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if nalu_len == 0 || pos + nalu_len > data.len() {
            break;
        }
        let nal_type = data[pos] & 0x1F;
        if nal_type == 7 || nal_type == 8 {
            return true;
        }
        pos += nalu_len;
    }

    false
}

#[derive(Debug, Clone)]
pub struct UniversalSessionConfig {
    pub webrtc: WebRtcConfig,
    pub codec: VideoEncoderType,
    pub resolution: Resolution,
    pub input_format: PixelFormat,
    pub bitrate_preset: BitratePreset,
    pub fps: u32,
    pub audio_enabled: bool,
}

impl Default for UniversalSessionConfig {
    fn default() -> Self {
        Self {
            webrtc: WebRtcConfig::default(),
            codec: VideoEncoderType::H264,
            resolution: Resolution::HD720,
            input_format: PixelFormat::Mjpeg,
            bitrate_preset: BitratePreset::Balanced,
            fps: 30,
            audio_enabled: false,
        }
    }
}

impl UniversalSessionConfig {
    pub fn with_codec(codec: VideoEncoderType) -> Self {
        Self {
            codec,
            ..Default::default()
        }
    }
}

fn encoder_type_to_video_codec(encoder_type: VideoEncoderType) -> VideoCodec {
    match encoder_type {
        VideoEncoderType::H264 => VideoCodec::H264,
        VideoEncoderType::H265 => VideoCodec::H265,
        VideoEncoderType::VP8 => VideoCodec::VP8,
        VideoEncoderType::VP9 => VideoCodec::VP9,
    }
}

pub struct UniversalSession {
    pub session_id: String,
    codec: VideoEncoderType,
    pc: Arc<RTCPeerConnection>,
    video_track: Arc<UniversalVideoTrack>,
    audio_track: Option<Arc<OpusAudioTrack>>,
    data_channel: Arc<RwLock<Option<Arc<RTCDataChannel>>>>,
    state: Arc<watch::Sender<ConnectionState>>,
    state_rx: watch::Receiver<ConnectionState>,
    ice_candidates: Arc<Mutex<Vec<IceCandidate>>>,
    hid_controller: Option<Arc<HidController>>,
    video_receiver_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    audio_receiver_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    fps: u32,
}

impl UniversalSession {
    pub async fn new(
        config: UniversalSessionConfig,
        session_id: String,
        _event_bus: Option<Arc<crate::events::EventBus>>,
    ) -> Result<Self> {
        info!(
            "Creating {} session: {} @ {}x{} (audio={})",
            config.codec,
            session_id,
            config.resolution.width,
            config.resolution.height,
            config.audio_enabled
        );

        let video_codec = encoder_type_to_video_codec(config.codec);
        let track_config = UniversalVideoTrackConfig {
            track_id: format!("video-{}", &session_id[..8.min(session_id.len())]),
            stream_id: "one-kvm-stream".to_string(),
            codec: video_codec,
            resolution: config.resolution,
            bitrate_kbps: config.bitrate_preset.bitrate_kbps(),
            fps: config.fps,
        };
        let video_track = Arc::new(UniversalVideoTrack::new(track_config));

        let audio_track = if config.audio_enabled {
            Some(Arc::new(OpusAudioTrack::new(
                &format!("audio-{}", &session_id[..8.min(session_id.len())]),
                "one-kvm-stream",
            )))
        } else {
            None
        };

        let mut media_engine = MediaEngine::default();

        // H265 is not registered by register_default_codecs.
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

            media_engine
                .register_codec(
                    RTCRtpCodecParameters {
                        capability: RTCRtpCodecCapability {
                            mime_type: MIME_TYPE_H265.to_owned(),
                            clock_rate: 90000,
                            channels: 0,
                            sdp_fmtp_line: "level-id=180;profile-id=1;tier-flag=0;tx-mode=SRST"
                                .to_owned(),
                            rtcp_feedback: video_rtcp_feedback.clone(),
                        },
                        payload_type: 49,
                        ..Default::default()
                    },
                    RTPCodecType::Video,
                )
                .map_err(|e| {
                    AppError::VideoError(format!("Failed to register H.265 codec: {}", e))
                })?;

            media_engine
                .register_codec(
                    RTCRtpCodecParameters {
                        capability: RTCRtpCodecCapability {
                            mime_type: MIME_TYPE_H265.to_owned(),
                            clock_rate: 90000,
                            channels: 0,
                            sdp_fmtp_line: "level-id=180;profile-id=2;tier-flag=0;tx-mode=SRST"
                                .to_owned(),
                            rtcp_feedback: video_rtcp_feedback,
                        },
                        payload_type: 51,
                        ..Default::default()
                    },
                    RTPCodecType::Video,
                )
                .map_err(|e| {
                    AppError::VideoError(format!(
                        "Failed to register H.265 codec (profile 2): {}",
                        e
                    ))
                })?;

            info!("Registered H.265/HEVC codec for session {}", session_id);
        }

        media_engine
            .register_default_codecs()
            .map_err(|e| AppError::VideoError(format!("Failed to register codecs: {}", e)))?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .map_err(|e| AppError::VideoError(format!("Failed to register interceptors: {}", e)))?;

        let mut setting_engine = SettingEngine::default();
        let mode = mdns_mode();
        setting_engine.set_ice_multicast_dns_mode(mode);
        if mode == MulticastDnsMode::QueryAndGather {
            setting_engine.set_multicast_dns_host_name(default_mdns_host_name(&session_id));
        }
        info!("WebRTC mDNS mode: {:?} (session {})", mode, session_id);

        let api = APIBuilder::new()
            .with_setting_engine(setting_engine)
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

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
                    "Skipping TURN server {:?} - credentials required but missing",
                    turn.urls
                );
                continue;
            }
            ice_servers.push(RTCIceServer {
                urls: turn.urls.clone(),
                username: turn.username.clone(),
                credential: turn.credential.clone(),
            });
        }

        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let pc = api.new_peer_connection(rtc_config).await.map_err(|e| {
            AppError::VideoError(format!("Failed to create peer connection: {}", e))
        })?;

        let pc = Arc::new(pc);

        pc.add_track(video_track.as_track_local())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to add video track: {}", e)))?;

        info!(
            "{} video track added to peer connection (session {})",
            config.codec, session_id
        );

        if let Some(ref audio) = audio_track {
            pc.add_track(audio.as_track_local())
                .await
                .map_err(|e| AppError::AudioError(format!("Failed to add audio track: {}", e)))?;
            info!(
                "Opus audio track added to peer connection (session {})",
                session_id
            );
        }

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

        session.setup_event_handlers().await;

        Ok(session)
    }

    async fn setup_event_handlers(&self) {
        let state = self.state.clone();
        let session_id = self.session_id.clone();
        let codec = self.codec;
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
                    if matches!(
                        (*state.borrow(), new_state),
                        (
                            ConnectionState::Connected,
                            ConnectionState::New | ConnectionState::Connecting
                        )
                    ) {
                        return;
                    }
                    let _ = state.send(new_state);
                })
            }));

        let state_for_ice = self.state.clone();
        let session_id_ice = self.session_id.clone();
        self.pc
            .on_ice_connection_state_change(Box::new(move |ice_state| {
                let state = state_for_ice.clone();
                let session_id = session_id_ice.clone();
                Box::pin(async move {
                    info!(
                        "[ICE] Session {} connection state: {:?}",
                        session_id, ice_state
                    );

                    let new_state = match ice_state {
                        RTCIceConnectionState::Connected | RTCIceConnectionState::Completed => {
                            ConnectionState::Connected
                        }
                        RTCIceConnectionState::Disconnected => ConnectionState::Disconnected,
                        RTCIceConnectionState::Failed => ConnectionState::Failed,
                        RTCIceConnectionState::Closed => ConnectionState::Closed,
                        _ => return,
                    };

                    let _ = state.send(new_state);
                })
            }));

        let ice_candidates = self.ice_candidates.clone();
        self.pc
            .on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
                let ice_candidates = ice_candidates.clone();

                Box::pin(async move {
                    if let Some(c) = candidate {
                        let candidate_json = c.to_json().ok();
                        let candidate_str = candidate_json
                            .as_ref()
                            .map(|j| j.candidate.clone())
                            .unwrap_or_default();
                        let candidate = IceCandidate {
                            candidate: candidate_str,
                            sdp_mid: candidate_json.as_ref().and_then(|j| j.sdp_mid.clone()),
                            sdp_mline_index: candidate_json
                                .as_ref()
                                .and_then(|j| j.sdp_mline_index),
                            username_fragment: candidate_json
                                .as_ref()
                                .and_then(|j| j.username_fragment.clone()),
                        };

                        let mut candidates = ice_candidates.lock().await;
                        candidates.push(candidate.clone());
                        drop(candidates);
                    }
                })
            }));

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

                        // webrtc-rs won't poll this future; spawn HID work for latency.
                        tokio::spawn(async move {
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
                                    HidChannelEvent::Consumer(consumer_event) => {
                                        if let Err(e) = hid.send_consumer(consumer_event).await {
                                            debug!("Failed to send consumer event: {}", e);
                                        }
                                    }
                                }
                            }
                        });

                        Box::pin(async {})
                    }));
                })
            }));

        self.hid_controller = Some(hid);
    }

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

    /// `on_connected` runs once ICE is up (e.g. request a keyframe).
    pub async fn start_from_video_pipeline(
        &self,
        mut frame_rx: tokio::sync::mpsc::Receiver<std::sync::Arc<EncodedVideoFrame>>,
        request_keyframe: Arc<dyn Fn() + Send + Sync + 'static>,
    ) {
        if let Some(handle) = self.video_receiver_handle.lock().await.take() {
            handle.abort();
        }
        info!(
            "Starting {} session {} with shared encoder",
            self.codec, self.session_id
        );

        let video_track = self.video_track.clone();
        let mut state_rx = self.state_rx.clone();
        let session_id = self.session_id.clone();
        let _fps = self.fps;
        let expected_codec = self.codec;
        let send_in_flight = Arc::new(AtomicBool::new(false));

        let handle = tokio::spawn(async move {
            info!(
                "Video receiver waiting for connection for session {}",
                session_id
            );

            loop {
                let current_state = *state_rx.borrow();
                if current_state == ConnectionState::Connected {
                    break;
                }
                if matches!(
                    current_state,
                    ConnectionState::Closed | ConnectionState::Failed
                ) {
                    info!("Session {} closed before connecting", session_id);
                    return;
                }
                if state_rx.changed().await.is_err() {
                    return;
                }
            }

            info!(
                "Video receiver started for session {} (ICE connected)",
                session_id
            );

            request_keyframe();
            let mut waiting_for_keyframe = true;
            let mut last_sequence: Option<u64> = None;
            let mut last_keyframe_request = Instant::now() - Duration::from_secs(1);

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
                        let encoded_frame = match result {
                            Some(frame) => frame,
                            None => {
                                info!("Video frame channel closed for session {}", session_id);
                                break;
                            }
                        };

                        let frame_codec = encoded_frame.codec;

                        if frame_codec != expected_codec {
                            continue;
                        }

                        if expected_codec == VideoEncoderType::H265
                                && (encoded_frame.is_keyframe || frames_sent.is_multiple_of(30)) {
                                debug!(
                                    "[Session-H265] Received frame #{}: size={}, keyframe={}, seq={}",
                                    frames_sent,
                                    encoded_frame.data.len(),
                                    encoded_frame.is_keyframe,
                                    encoded_frame.sequence
                                );
                            }

                        let mut gap_detected = false;
                        if let Some(prev) = last_sequence {
                            if encoded_frame.sequence > prev.saturating_add(1) {
                                gap_detected = true;
                            }
                        }

                        if waiting_for_keyframe || gap_detected {
                            if encoded_frame.is_keyframe {
                                waiting_for_keyframe = false;
                            } else {
                                if gap_detected {
                                    waiting_for_keyframe = true;
                                }

                                // Some H264 encoders output SPS/PPS in a separate non-keyframe AU
                                // before IDR. Keep this frame so browser can decode the next IDR.
                                let forward_h264_parameter_frame = waiting_for_keyframe
                                    && expected_codec == VideoEncoderType::H264
                                    && h264_contains_parameter_sets(encoded_frame.data.as_ref());

                                let now = Instant::now();
                                if now.duration_since(last_keyframe_request)
                                    >= Duration::from_millis(200)
                                {
                                    request_keyframe();
                                    last_keyframe_request = now;
                                }
                                if !forward_h264_parameter_frame {
                                    continue;
                                }
                            }
                        }

                        let _ = send_in_flight;

                        let send_result = video_track
                            .write_frame_bytes(
                                encoded_frame.data.clone(),
                                encoded_frame.is_keyframe,
                            )
                            .await;
                        let _ = send_in_flight;

                        match send_result {
                            Ok(()) => {
                                frames_sent += 1;
                                last_sequence = Some(encoded_frame.sequence);
                            }
                            Err(e) => {
                                warn!(
                                    "Session {} failed to write video frame: sequence={}, keyframe={}, bytes={}, error={}",
                                    session_id,
                                    encoded_frame.sequence,
                                    encoded_frame.is_keyframe,
                                    encoded_frame.data.len(),
                                    e
                                );
                            }
                        }
                    }
                }
            }

            info!(
                "Video receiver stopped for session {} (sent {} frames)",
                session_id, frames_sent
            );
        });

        {
            let mut guard = self.video_receiver_handle.lock().await;
            if let Some(old) = guard.take() {
                old.abort();
            }
            *guard = Some(handle);
        }
    }

    pub async fn start_audio_from_opus(
        &self,
        mut opus_rx: tokio::sync::mpsc::Receiver<std::sync::Arc<OpusFrame>>,
    ) {
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
            loop {
                let current_state = *state_rx.borrow();
                if current_state == ConnectionState::Connected {
                    break;
                }
                if matches!(
                    current_state,
                    ConnectionState::Closed | ConnectionState::Failed
                ) {
                    info!("Session {} closed before audio could start", session_id);
                    return;
                }
                if state_rx.changed().await.is_err() {
                    return;
                }
            }

            info!(
                "Audio receiver started for session {} (ICE connected)",
                session_id
            );

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
                        let opus_frame = match result {
                            Some(frame) => frame,
                            None => {
                                info!("Opus mpsc closed for session {}", session_id);
                                break;
                            }
                        };

                        let samples = 960u32;
                        if let Err(e) = audio_track.write_packet(&opus_frame.data, samples).await {
                            if packets_sent.is_multiple_of(100) {
                                debug!("Failed to write audio packet: {}", e);
                            }
                        } else {
                            packets_sent += 1;
                        }
                    }
                }
            }

            info!(
                "Audio receiver stopped for session {} (sent {} packets)",
                session_id, packets_sent
            );
        });

        {
            let mut guard = self.audio_receiver_handle.lock().await;
            if let Some(old) = guard.take() {
                old.abort();
            }
            *guard = Some(handle);
        }
    }

    pub fn has_audio(&self) -> bool {
        self.audio_track.is_some()
    }

    pub fn codec(&self) -> VideoEncoderType {
        self.codec
    }

    pub async fn handle_offer(&self, offer: SdpOffer) -> Result<SdpAnswer> {
        if self.codec == VideoEncoderType::H265 {
            let has_h265 = offer.sdp.to_lowercase().contains("h265")
                || offer.sdp.to_lowercase().contains("hevc");
            info!(
                "[SDP] Session {} offer contains H.265: {}",
                self.session_id, has_h265
            );
            if !has_h265 {
                warn!("[SDP] Browser offer does not include H.265 codec! Session may fail.");
            }
        }

        let sdp = RTCSessionDescription::offer(offer.sdp)
            .map_err(|e| AppError::VideoError(format!("Invalid SDP offer: {}", e)))?;

        self.pc.set_remote_description(sdp).await.map_err(|e| {
            AppError::VideoError(format!("Failed to set remote description: {}", e))
        })?;

        let answer = self
            .pc
            .create_answer(None)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to create answer: {}", e)))?;

        if self.codec == VideoEncoderType::H265 {
            let has_h265 = answer.sdp.to_lowercase().contains("h265")
                || answer.sdp.to_lowercase().contains("hevc");
            info!(
                "[SDP] Session {} answer contains H.265: {}",
                self.session_id, has_h265
            );
            if !has_h265 {
                warn!("[SDP] Answer does not include H.265! Codec negotiation may have failed.");
            }
        }

        self.pc
            .set_local_description(answer.clone())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to set local description: {}", e)))?;

        tokio::time::sleep(Duration::from_millis(500)).await;
        let candidates = self.ice_candidates.lock().await.clone();

        Ok(SdpAnswer::with_candidates(answer.sdp, candidates))
    }

    pub async fn add_ice_candidate(&self, candidate: IceCandidate) -> Result<()> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let init = RTCIceCandidateInit {
            candidate: candidate.candidate,
            sdp_mid: candidate.sdp_mid,
            sdp_mline_index: candidate.sdp_mline_index,
            username_fragment: candidate.username_fragment,
        };

        if let Err(e) = self.pc.add_ice_candidate(init).await {
            warn!(
                "[ICE] Session {} failed to add remote candidate: {}",
                self.session_id, e
            );
            return Err(AppError::VideoError(format!(
                "Failed to add ICE candidate: {}",
                e
            )));
        }

        Ok(())
    }

    pub fn state(&self) -> ConnectionState {
        *self.state_rx.borrow()
    }

    pub fn state_watch(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    pub async fn close(&self) -> Result<()> {
        if let Some(handle) = self.video_receiver_handle.lock().await.take() {
            handle.abort();
        }

        if let Some(handle) = self.audio_receiver_handle.lock().await.take() {
            handle.abort();
        }

        self.pc
            .close()
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to close peer connection: {}", e)))?;

        let _ = self.state.send(ConnectionState::Closed);

        info!("{} session {} closed", self.codec, self.session_id);
        Ok(())
    }
}

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
        assert_eq!(
            encoder_type_to_video_codec(VideoEncoderType::H264),
            VideoCodec::H264
        );
        assert_eq!(
            encoder_type_to_video_codec(VideoEncoderType::H265),
            VideoCodec::H265
        );
        assert_eq!(
            encoder_type_to_video_codec(VideoEncoderType::VP8),
            VideoCodec::VP8
        );
        assert_eq!(
            encoder_type_to_video_codec(VideoEncoderType::VP9),
            VideoCodec::VP9
        );
    }
}
