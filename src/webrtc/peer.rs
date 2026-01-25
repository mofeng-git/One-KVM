//! WebRTC peer connection management

use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing::{debug, info};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice::mdns::MulticastDnsMode;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use super::config::WebRtcConfig;
use super::mdns::{default_mdns_host_name, mdns_mode};
use super::signaling::{ConnectionState, IceCandidate, SdpAnswer, SdpOffer};
use super::track::{VideoTrack, VideoTrackConfig};
use crate::error::{AppError, Result};
use crate::hid::datachannel::{parse_hid_message, HidChannelEvent};
use crate::hid::HidController;
use crate::video::frame::VideoFrame;

/// Peer connection wrapper with event handling
pub struct PeerConnection {
    /// Session ID
    pub session_id: String,
    /// WebRTC peer connection
    pc: Arc<RTCPeerConnection>,
    /// Video track
    video_track: Option<VideoTrack>,
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
}

impl PeerConnection {
    /// Create a new peer connection
    pub async fn new(config: &WebRtcConfig, session_id: String) -> Result<Self> {
        // Create media engine
        let mut media_engine = MediaEngine::default();

        // Register codecs
        media_engine
            .register_default_codecs()
            .map_err(|e| AppError::VideoError(format!("Failed to register codecs: {}", e)))?;

        // Create interceptor registry
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .map_err(|e| AppError::VideoError(format!("Failed to register interceptors: {}", e)))?;

        // Create API (with optional mDNS settings)
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

        // Build ICE servers
        let mut ice_servers = vec![];

        for stun_url in &config.stun_servers {
            ice_servers.push(RTCIceServer {
                urls: vec![stun_url.clone()],
                ..Default::default()
            });
        }

        for turn in &config.turn_servers {
            ice_servers.push(RTCIceServer {
                urls: turn.urls.clone(),
                username: turn.username.clone(),
                credential: turn.credential.clone(),
                ..Default::default()
            });
        }

        // Create peer connection configuration
        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        // Create peer connection
        let pc = api.new_peer_connection(rtc_config).await.map_err(|e| {
            AppError::VideoError(format!("Failed to create peer connection: {}", e))
        })?;

        let pc = Arc::new(pc);

        // Create state channel
        let (state_tx, state_rx) = watch::channel(ConnectionState::New);

        let peer_connection = Self {
            session_id,
            pc,
            video_track: None,
            data_channel: Arc::new(RwLock::new(None)),
            state: Arc::new(state_tx),
            state_rx,
            ice_candidates: Arc::new(Mutex::new(vec![])),
            hid_controller: None,
        };

        // Set up event handlers
        peer_connection.setup_event_handlers().await;

        Ok(peer_connection)
    }

    /// Set up peer connection event handlers
    async fn setup_event_handlers(&self) {
        let state = self.state.clone();
        let session_id = self.session_id.clone();

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

                    info!("Peer {} connection state: {}", session_id, new_state);
                    let _ = state.send(new_state);
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

        // Data channel handler - note: HID processing is done when hid_controller is set
        let data_channel = self.data_channel.clone();
        self.pc
            .on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let data_channel = data_channel.clone();

                Box::pin(async move {
                    info!("Data channel opened: {}", dc.label());

                    // Store data channel
                    *data_channel.write().await = Some(dc.clone());

                    // Message handler logs messages; HID processing requires set_hid_controller()
                    dc.on_message(Box::new(move |msg: DataChannelMessage| {
                        debug!("DataChannel message: {} bytes", msg.data.len());
                        Box::pin(async {})
                    }));
                })
            }));
    }

    /// Set HID controller for processing DataChannel messages
    pub fn set_hid_controller(&mut self, hid: Arc<HidController>) {
        let hid_clone = hid.clone();
        let data_channel = self.data_channel.clone();

        // Set up message handler with HID processing
        let pc = self.pc.clone();
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let data_channel = data_channel.clone();
            let hid = hid_clone.clone();
            let label = dc.label().to_string();

            Box::pin(async move {
                // Handle both reliable (hid) and unreliable (hid-unreliable) channels
                let is_hid_channel = label == "hid" || label == "hid-unreliable";

                if is_hid_channel {
                    info!(
                        "HID DataChannel opened: {} (unreliable: {})",
                        label,
                        label == "hid-unreliable"
                    );

                    // Store the reliable data channel for sending responses
                    if label == "hid" {
                        *data_channel.write().await = Some(dc.clone());
                    }

                    // Set up message handler with HID processing
                    // Both channels use the same HID processing logic
                    dc.on_message(Box::new(move |msg: DataChannelMessage| {
                        let hid = hid.clone();

                        tokio::spawn(async move {
                            debug!("DataChannel HID message: {} bytes", msg.data.len());

                            // Parse and process HID message
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

                        // Return empty future (actual work is spawned above)
                        Box::pin(async {})
                    }));
                } else {
                    info!("Non-HID DataChannel opened: {}", label);
                }
            })
        }));

        self.hid_controller = Some(hid);
    }

    /// Add video track to the connection
    pub async fn add_video_track(&mut self, config: VideoTrackConfig) -> Result<()> {
        let video_track = VideoTrack::new(config);

        // Add track to peer connection
        self.pc
            .add_track(video_track.rtp_track())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to add video track: {}", e)))?;

        self.video_track = Some(video_track);
        info!("Video track added to peer connection");

        Ok(())
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

    /// Handle SDP offer and create answer
    pub async fn handle_offer(&self, offer: SdpOffer) -> Result<SdpAnswer> {
        // Parse and set remote description
        let sdp = RTCSessionDescription::offer(offer.sdp)
            .map_err(|e| AppError::VideoError(format!("Invalid SDP offer: {}", e)))?;

        self.pc.set_remote_description(sdp).await.map_err(|e| {
            AppError::VideoError(format!("Failed to set remote description: {}", e))
        })?;

        // Create answer
        let answer = self
            .pc
            .create_answer(None)
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to create answer: {}", e)))?;

        // Set local description
        self.pc
            .set_local_description(answer.clone())
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to set local description: {}", e)))?;

        // Wait a bit for ICE candidates to gather
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Get gathered ICE candidates
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

    /// Start sending video frames
    pub async fn start_video(&self, frame_rx: broadcast::Receiver<VideoFrame>) {
        if let Some(ref track) = self.video_track {
            track.start_sending(frame_rx).await;
        }
    }

    /// Send HID data via data channel
    pub async fn send_hid_data(&self, data: &[u8]) -> Result<()> {
        let dc = self.data_channel.read().await;

        if let Some(ref channel) = *dc {
            channel
                .send(&bytes::Bytes::copy_from_slice(data))
                .await
                .map_err(|e| AppError::VideoError(format!("Failed to send HID data: {}", e)))?;
        }

        Ok(())
    }

    /// Close the connection
    pub async fn close(&self) -> Result<()> {
        // Reset HID state to release any held keys/buttons
        if let Some(ref hid) = self.hid_controller {
            if let Err(e) = hid.reset().await {
                tracing::warn!(
                    "Failed to reset HID on peer {} close: {}",
                    self.session_id,
                    e
                );
            } else {
                tracing::debug!("HID reset on peer {} close", self.session_id);
            }
        }

        if let Some(ref track) = self.video_track {
            track.stop();
        }

        self.pc
            .close()
            .await
            .map_err(|e| AppError::VideoError(format!("Failed to close peer connection: {}", e)))?;

        Ok(())
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Manager for multiple peer connections
pub struct PeerConnectionManager {
    config: WebRtcConfig,
    /// Active peer connections
    peers: Arc<RwLock<Vec<Arc<Mutex<PeerConnection>>>>>,
    /// Frame broadcast sender (to distribute to all peers)
    frame_tx: broadcast::Sender<VideoFrame>,
    /// HID controller for DataChannel HID processing
    hid_controller: Option<Arc<HidController>>,
}

impl PeerConnectionManager {
    /// Create a new peer connection manager
    pub fn new(config: WebRtcConfig) -> Self {
        let (frame_tx, _) = broadcast::channel(16);

        Self {
            config,
            peers: Arc::new(RwLock::new(vec![])),
            frame_tx,
            hid_controller: None,
        }
    }

    /// Create a new peer connection manager with HID controller
    pub fn with_hid(config: WebRtcConfig, hid: Arc<HidController>) -> Self {
        let (frame_tx, _) = broadcast::channel(16);

        Self {
            config,
            peers: Arc::new(RwLock::new(vec![])),
            frame_tx,
            hid_controller: Some(hid),
        }
    }

    /// Set HID controller
    pub fn set_hid_controller(&mut self, hid: Arc<HidController>) {
        self.hid_controller = Some(hid);
    }

    /// Create a new peer connection
    pub async fn create_peer(&self) -> Result<Arc<Mutex<PeerConnection>>> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let mut peer = PeerConnection::new(&self.config, session_id).await?;

        // Add video track
        peer.add_video_track(VideoTrackConfig::default()).await?;

        // Set HID controller if available
        // Note: We DON'T create a data channel here - the frontend creates it.
        // The server receives it via on_data_channel callback set in set_hid_controller().
        if self.config.enable_datachannel {
            if let Some(ref hid) = self.hid_controller {
                peer.set_hid_controller(hid.clone());
            }
        }

        let peer = Arc::new(Mutex::new(peer));

        // Add to peers list
        self.peers.write().await.push(peer.clone());

        // Start sending video when connected
        let frame_rx = self.frame_tx.subscribe();
        let peer_clone = peer.clone();
        tokio::spawn(async move {
            let peer = peer_clone.lock().await;
            let mut state_rx = peer.state_watch();
            drop(peer);

            // Wait for connection
            while state_rx.changed().await.is_ok() {
                if *state_rx.borrow() == ConnectionState::Connected {
                    let peer = peer_clone.lock().await;
                    peer.start_video(frame_rx).await;
                    break;
                }
            }
        });

        Ok(peer)
    }

    /// Get frame sender (for video streamer to push frames)
    pub fn frame_sender(&self) -> broadcast::Sender<VideoFrame> {
        self.frame_tx.clone()
    }

    /// Remove closed connections
    pub async fn cleanup(&self) {
        let mut peers = self.peers.write().await;
        let mut to_remove = vec![];

        for (i, peer) in peers.iter().enumerate() {
            let p = peer.lock().await;
            if matches!(p.state(), ConnectionState::Closed | ConnectionState::Failed) {
                to_remove.push(i);
            }
        }

        for i in to_remove.into_iter().rev() {
            peers.remove(i);
        }
    }

    /// Get active peer count
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    /// Close all connections
    pub async fn close_all(&self) {
        let peers = self.peers.read().await;
        for peer in peers.iter() {
            let p = peer.lock().await;
            let _ = p.close().await;
        }
    }
}
