//! RustDesk Connection Handler
//!
//! This module handles incoming connections from RustDesk clients.
//! It manages the connection lifecycle including:
//! - Connection establishment (P2P or via relay)
//! - Encrypted handshake
//! - Authentication
//! - Message routing (video, audio, input)
//! - Video frame streaming (shared with WebRTC)

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::{Bytes, BytesMut};
use parking_lot::RwLock;
use protobuf::Message as ProtobufMessage;
use sodiumoxide::crypto::box_;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{debug, error, info, warn};

use crate::audio::AudioController;
use crate::hid::{HidController, KeyEventType, KeyboardEvent, KeyboardModifiers};
use crate::video::encoder::registry::{EncoderRegistry, VideoEncoderType};
use crate::video::encoder::BitratePreset;
use crate::video::stream_manager::VideoStreamManager;

use super::bytes_codec::{read_frame, write_frame, write_frame_buffered};
use super::config::RustDeskConfig;
use super::crypto::{self, KeyPair, SigningKeyPair};
use super::frame_adapters::{AudioFrameAdapter, VideoCodec, VideoFrameAdapter};
use super::hid_adapter::{convert_key_event, convert_mouse_event, mouse_type};
use super::protocol::{
    decode_message, login_response, message, misc, Clipboard, ControlKey, DisplayInfo, Hash,
    HbbMessage, IdPk, KeyEvent, LoginRequest, LoginResponse, Misc, MouseEvent, OptionMessage,
    PeerInfo, PublicKey, SignedId, SupportedEncoding, TestDelay,
};

use sodiumoxide::crypto::secretbox;

/// Default screen dimensions for mouse coordinate conversion
const DEFAULT_SCREEN_WIDTH: u32 = 1920;
const DEFAULT_SCREEN_HEIGHT: u32 = 1080;

/// Default mouse event throttle interval (16ms ≈ 60Hz)
const DEFAULT_MOUSE_THROTTLE_MS: u64 = 16;

/// Input event throttler
///
/// Limits the rate of input events sent to HID devices to prevent EAGAIN errors.
/// USB HID devices typically poll at 100-125Hz, so sending events faster than
/// this rate will cause the device to return EAGAIN (resource temporarily unavailable).
struct InputThrottler {
    /// Last time a mouse move event was sent
    last_mouse_time: Instant,
    /// Minimum interval between mouse move events
    mouse_interval: Duration,
}

impl InputThrottler {
    /// Create a new input throttler with default intervals
    fn new() -> Self {
        Self {
            last_mouse_time: Instant::now() - Duration::from_millis(DEFAULT_MOUSE_THROTTLE_MS),
            mouse_interval: Duration::from_millis(DEFAULT_MOUSE_THROTTLE_MS),
        }
    }

    /// Check if a mouse move event should be sent
    /// Returns true if enough time has passed since the last event
    fn should_send_mouse_move(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_mouse_time) >= self.mouse_interval {
            self.last_mouse_time = now;
            true
        } else {
            false
        }
    }

    /// Force update the last mouse time (for button events that must be sent)
    fn mark_mouse_sent(&mut self) {
        self.last_mouse_time = Instant::now();
    }
}

/// Get system hostname
fn get_hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "One-KVM".to_string())
}

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Waiting for connection
    Pending,
    /// Handshake in progress
    Handshaking,
    /// Authenticated and active
    Active,
    /// Connection closed
    Closed,
    /// Error state
    Error(String),
}

/// Incoming connection from a RustDesk client
pub struct Connection {
    /// Connection ID
    id: u32,
    /// Our device ID (RustDesk ID)
    device_id: String,
    /// Peer ID (client's RustDesk ID)
    peer_id: String,
    /// Peer name
    peer_name: String,
    /// Connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Our signing keypair (Ed25519) for signing SignedId messages
    signing_keypair: SigningKeyPair,
    /// Temporary Curve25519 keypair for this connection (used for encryption)
    /// Generated fresh for each connection, public key goes in IdPk.pk
    temp_keypair: (box_::PublicKey, box_::SecretKey),
    /// Device password
    password: String,
    /// HID controller for keyboard/mouse events
    hid: Option<Arc<HidController>>,
    /// Audio controller for audio streaming
    audio: Option<Arc<AudioController>>,
    /// Video stream manager for frame subscription
    video_manager: Option<Arc<VideoStreamManager>>,
    /// Screen dimensions for mouse coordinate conversion
    screen_width: u32,
    screen_height: u32,
    /// Message sender to connection handler
    tx: mpsc::UnboundedSender<ConnectionMessage>,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    /// Video streaming task handle
    video_task: Option<tokio::task::JoinHandle<()>>,
    /// Audio streaming task handle
    audio_task: Option<tokio::task::JoinHandle<()>>,
    /// Session encryption key (negotiated during handshake)
    session_key: Option<secretbox::Key>,
    /// Encryption enabled flag
    encryption_enabled: bool,
    /// Encryption sequence number (for nonce generation)
    enc_seqnum: u64,
    /// Decryption sequence number (for nonce generation)
    dec_seqnum: u64,
    /// Negotiated video codec (after client capability exchange)
    negotiated_codec: Option<VideoEncoderType>,
    /// Video frame sender for restarting video after codec switch
    video_frame_tx: Option<mpsc::Sender<Bytes>>,
    /// Input event throttler to prevent HID device EAGAIN errors
    input_throttler: InputThrottler,
    /// Last measured round-trip delay in milliseconds (for TestDelay responses)
    last_delay: u32,
    /// Time when we last sent a TestDelay to the client (for RTT calculation)
    last_test_delay_sent: Option<Instant>,
    /// Last known CapsLock state from RustDesk modifiers (for detecting toggle)
    last_caps_lock: bool,
}

/// Messages sent to connection handler
#[derive(Debug)]
pub enum ConnectionMessage {
    /// Send video frame
    VideoFrame(Bytes),
    /// Send audio frame
    AudioFrame(Bytes),
    /// Send cursor data
    CursorData(Bytes),
    /// Close connection
    Close,
}

/// Messages received from client
#[derive(Debug)]
pub enum ClientMessage {
    /// Login request
    Login(LoginRequest),
    /// Key event
    KeyEvent(KeyEvent),
    /// Mouse event
    MouseEvent(MouseEvent),
    /// Clipboard
    Clipboard(Clipboard),
    /// Misc message
    Misc(Misc),
    /// Unknown/unhandled
    Unknown,
}

impl Connection {
    /// Create a new connection
    pub fn new(
        id: u32,
        config: &RustDeskConfig,
        signing_keypair: SigningKeyPair,
        hid: Option<Arc<HidController>>,
        audio: Option<Arc<AudioController>>,
        video_manager: Option<Arc<VideoStreamManager>>,
    ) -> (Self, mpsc::UnboundedReceiver<ConnectionMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, _) = broadcast::channel(1);

        // Generate fresh Curve25519 keypair for this connection
        // This is used for encrypting the symmetric key exchange
        let temp_keypair = box_::gen_keypair();

        let conn = Self {
            id,
            device_id: config.device_id.clone(),
            peer_id: String::new(),
            peer_name: String::new(),
            state: Arc::new(RwLock::new(ConnectionState::Pending)),
            signing_keypair,
            temp_keypair,
            password: config.device_password.clone(),
            hid,
            audio,
            video_manager,
            screen_width: DEFAULT_SCREEN_WIDTH,
            screen_height: DEFAULT_SCREEN_HEIGHT,
            tx,
            shutdown_tx,
            video_task: None,
            audio_task: None,
            session_key: None,
            encryption_enabled: false,
            enc_seqnum: 0,
            dec_seqnum: 0,
            negotiated_codec: None,
            video_frame_tx: None,
            input_throttler: InputThrottler::new(),
            last_delay: 0,
            last_test_delay_sent: None,
            last_caps_lock: false,
        };

        (conn, rx)
    }

    /// Get connection ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get connection state
    pub fn state(&self) -> ConnectionState {
        self.state.read().clone()
    }

    /// Get peer ID
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    /// Get message sender
    pub fn sender(&self) -> mpsc::UnboundedSender<ConnectionMessage> {
        self.tx.clone()
    }

    /// Handle an incoming TCP connection
    pub async fn handle_tcp(
        &mut self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<()> {
        info!("New connection from {}", peer_addr);
        *self.state.write() = ConnectionState::Handshaking;

        let (mut reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(writer));
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Send our SignedId first (this is what RustDesk protocol expects)
        // The SignedId contains our device ID and temporary public key
        let signed_id_msg = self.create_signed_id_message(&self.device_id.clone());
        let signed_id_bytes = signed_id_msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode SignedId: {}", e))?;
        debug!("Sending SignedId with device_id={}", self.device_id);
        self.send_framed_arc(&writer, &signed_id_bytes).await?;

        // Channel for receiving video frames to send (bounded to provide backpressure)
        let (video_tx, mut video_rx) = mpsc::channel::<Bytes>(4);
        let mut video_streaming = false;

        // Channel for receiving audio frames to send (bounded to provide backpressure)
        let (audio_tx, mut audio_rx) = mpsc::channel::<Bytes>(8);
        let mut audio_streaming = false;

        // Timer for sending TestDelay to measure round-trip latency
        // RustDesk clients display this delay information
        let mut test_delay_interval = tokio::time::interval(Duration::from_secs(1));
        test_delay_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // Pre-allocated buffer for framing (reused across sends to reduce allocations)
        // Typical H264 frame is 10-100KB, pre-allocate 128KB
        let mut frame_buf = BytesMut::with_capacity(128 * 1024);

        loop {
            tokio::select! {
                // Read framed message from client using RustDesk's variable-length encoding
                result = read_frame(&mut reader) => {
                    match result {
                        Ok(msg_buf) => {
                            if let Err(e) = self.handle_message_arc(&msg_buf, &writer, &video_tx, &mut video_streaming, &audio_tx, &mut audio_streaming).await {
                                error!("Error handling message: {}", e);
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            if self.state() == ConnectionState::Handshaking {
                                warn!("Connection closed by peer DURING HANDSHAKE - signature verification likely failed on client side");
                            } else {
                                info!("Connection closed by peer");
                            }
                            break;
                        }
                        Err(e) => {
                            error!("Read error: {}", e);
                            break;
                        }
                    }
                }

                // Send video frames (encrypted if session key is set)
                // Optimized path: inline encryption and use pre-allocated buffer
                Some(frame_data) = video_rx.recv() => {
                    let send_result = if let Some(ref key) = self.session_key {
                        // Encrypt the frame
                        self.enc_seqnum += 1;
                        let nonce = Self::get_nonce(self.enc_seqnum);
                        let ciphertext = secretbox::seal(&frame_data, &nonce, key);
                        // Send using pre-allocated buffer
                        let mut w = writer.lock().await;
                        write_frame_buffered(&mut *w, &ciphertext, &mut frame_buf).await
                    } else {
                        // No encryption, send plain
                        let mut w = writer.lock().await;
                        write_frame_buffered(&mut *w, &frame_data, &mut frame_buf).await
                    };

                    if let Err(e) = send_result {
                        error!("Error sending video frame: {}", e);
                        break;
                    }
                }

                // Send audio frames (encrypted if session key is set)
                Some(frame_data) = audio_rx.recv() => {
                    let send_result = if let Some(ref key) = self.session_key {
                        // Encrypt the frame
                        self.enc_seqnum += 1;
                        let nonce = Self::get_nonce(self.enc_seqnum);
                        let ciphertext = secretbox::seal(&frame_data, &nonce, key);
                        let mut w = writer.lock().await;
                        write_frame_buffered(&mut *w, &ciphertext, &mut frame_buf).await
                    } else {
                        // No encryption, send plain
                        let mut w = writer.lock().await;
                        write_frame_buffered(&mut *w, &frame_data, &mut frame_buf).await
                    };

                    if let Err(e) = send_result {
                        error!("Error sending audio frame: {}", e);
                        break;
                    }
                }

                // Send TestDelay periodically to measure latency
                _ = test_delay_interval.tick() => {
                    if self.state() == ConnectionState::Active && self.last_test_delay_sent.is_none() {
                        if let Err(e) = self.send_test_delay(&writer).await {
                            warn!("Failed to send TestDelay: {}", e);
                        }
                    }
                }

                // Shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Connection shutdown requested");
                    break;
                }
            }
        }

        // Stop video streaming task if running
        if let Some(task) = self.video_task.take() {
            task.abort();
        }

        // Stop audio streaming task if running
        if let Some(task) = self.audio_task.take() {
            task.abort();
        }

        *self.state.write() = ConnectionState::Closed;
        Ok(())
    }

    /// Send framed message using Arc<Mutex<OwnedWriteHalf>> with RustDesk's variable-length encoding
    async fn send_framed_arc(
        &self,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let mut w = writer.lock().await;
        write_frame(&mut *w, data).await?;
        Ok(())
    }

    /// Generate nonce from sequence number (RustDesk format)
    fn get_nonce(seqnum: u64) -> secretbox::Nonce {
        let mut nonce = secretbox::Nonce([0u8; 24]);
        nonce.0[..8].copy_from_slice(&seqnum.to_le_bytes());
        nonce
    }

    /// Send encrypted framed message if encryption is enabled
    /// RustDesk uses sequence-based nonce, NOT nonce prefix in message
    async fn send_encrypted_arc(
        &mut self,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
        data: &[u8],
    ) -> anyhow::Result<()> {
        if let Some(ref key) = self.session_key {
            // Increment encryption sequence number
            self.enc_seqnum += 1;
            let nonce = Self::get_nonce(self.enc_seqnum);
            // Encrypt the message - RustDesk only sends ciphertext, no nonce prefix
            let ciphertext = secretbox::seal(data, &nonce, key);
            self.send_framed_arc(writer, &ciphertext).await
        } else {
            // No encryption, send plain
            self.send_framed_arc(writer, data).await
        }
    }

    /// Handle incoming message with Arc writer
    /// Messages may be encrypted after session key negotiation
    async fn handle_message_arc(
        &mut self,
        data: &[u8],
        writer: &Arc<Mutex<OwnedWriteHalf>>,
        video_tx: &mpsc::Sender<Bytes>,
        video_streaming: &mut bool,
        audio_tx: &mpsc::Sender<Bytes>,
        audio_streaming: &mut bool,
    ) -> anyhow::Result<()> {
        // Try to decrypt if we have a session key
        // RustDesk uses sequence-based nonce, NOT nonce prefix in message
        let decrypted_data: Vec<u8>;
        let msg_data = if let Some(ref key) = self.session_key {
            // Increment decryption sequence number
            self.dec_seqnum += 1;
            let nonce = Self::get_nonce(self.dec_seqnum);
            match secretbox::open(data, &nonce, key) {
                Ok(decrypted) => {
                    decrypted_data = decrypted;
                    &decrypted_data[..]
                }
                Err(_) => {
                    // Decryption failed, try as plain message
                    // (PublicKey message is sent before encryption is enabled)
                    // Reset sequence number since this wasn't an encrypted message
                    self.dec_seqnum -= 1;
                    data
                }
            }
        } else {
            data
        };

        let msg = decode_message(msg_data)?;

        match msg.union {
            Some(message::Union::PublicKey(ref pk)) => {
                info!(
                    "Received PublicKey from peer: asymmetric_len={}, symmetric_len={}",
                    pk.asymmetric_value.len(),
                    pk.symmetric_value.len()
                );
                if pk.asymmetric_value.is_empty() && pk.symmetric_value.is_empty() {
                    warn!(
                        "Received EMPTY PublicKey - client may have failed signature verification!"
                    );
                }
                self.handle_peer_public_key(pk, writer).await?;
            }
            Some(message::Union::LoginRequest(lr)) => {
                debug!("Received login request from {}", lr.my_id);
                self.peer_id = lr.my_id.clone();
                self.peer_name = lr.my_name.clone();

                // Handle login and start video/audio streaming if successful
                if self.handle_login_request_arc(&lr, writer).await? {
                    // Store video_tx for potential codec switching
                    self.video_frame_tx = Some(video_tx.clone());
                    // Start video streaming
                    if !*video_streaming {
                        self.start_video_streaming(video_tx.clone());
                        *video_streaming = true;
                    }
                    // Start audio streaming
                    if !*audio_streaming {
                        self.start_audio_streaming(audio_tx.clone());
                        *audio_streaming = true;
                    }
                }
            }
            Some(message::Union::KeyEvent(ke)) => {
                if self.state() == ConnectionState::Active {
                    self.handle_key_event(&ke).await?;
                }
            }
            Some(message::Union::MouseEvent(me)) => {
                if self.state() == ConnectionState::Active {
                    self.handle_mouse_event(&me).await?;
                }
            }
            Some(message::Union::Clipboard(_cb)) => {
                if self.state() == ConnectionState::Active {
                    debug!("Received clipboard data");
                }
            }
            Some(message::Union::Misc(misc)) => {
                self.handle_misc_arc(&misc, writer).await?;
            }
            Some(message::Union::TestDelay(td)) => {
                self.handle_test_delay(&td, writer).await?;
            }
            Some(other) => {
                // Log the actual message type for debugging
                let type_name = match other {
                    message::Union::SignedId(ref si) => {
                        // Client sends SignedId as first message in handshake
                        // We should respond with our IdPk (ID + public key)
                        info!("Received SignedId from peer, id_len={}", si.id.len());
                        self.handle_signed_id(si, writer).await?;
                        return Ok(());
                    }
                    message::Union::Hash(_) => "Hash",
                    message::Union::VideoFrame(_) => "VideoFrame",
                    message::Union::CursorData(_) => "CursorData",
                    message::Union::CursorPosition(_) => "CursorPosition",
                    message::Union::CursorId(_) => "CursorId",
                    message::Union::AudioFrame(_) => "AudioFrame",
                    message::Union::FileAction(_) => "FileAction",
                    message::Union::FileResponse(_) => "FileResponse",
                    message::Union::SwitchSidesResponse(_) => "SwitchSidesResponse",
                    message::Union::PointerDeviceEvent(_) => "PointerDeviceEvent",
                    _ => "Other",
                };
                info!("Received unhandled message type: {}", type_name);
            }
            None => {
                debug!("Received empty message");
            }
        }

        Ok(())
    }

    /// Handle login request and return true if successful
    async fn handle_login_request_arc(
        &mut self,
        lr: &LoginRequest,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
    ) -> anyhow::Result<bool> {
        info!(
            "Login request from {} ({}), password_len={}",
            lr.my_id,
            lr.my_name,
            lr.password.len()
        );

        // Check if our server requires a password
        if !self.password.is_empty() {
            // Server requires password
            if lr.password.is_empty() {
                // Client sent empty password - tell them to enter password
                info!(
                    "Empty password from {}, requesting password input",
                    lr.my_id
                );
                let error_response = self.create_login_error_response("Empty Password");
                let response_bytes = error_response
                    .write_to_bytes()
                    .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
                self.send_encrypted_arc(writer, &response_bytes).await?;
                // Don't close connection - wait for retry with password
                return Ok(false);
            }

            // Verify the password
            if !self.verify_password(&lr.password) {
                warn!("Wrong password from {}", lr.my_id);
                let error_response = self.create_login_error_response("Wrong Password");
                let response_bytes = error_response
                    .write_to_bytes()
                    .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
                self.send_encrypted_arc(writer, &response_bytes).await?;
                // Don't close connection - wait for retry with correct password
                return Ok(false);
            }
        }

        // Password valid or no password required
        info!("Login successful for {}", lr.my_id);
        *self.state.write() = ConnectionState::Active;

        // Select the best available video codec
        // Priority: H264 > H265 > VP8 > VP9 (H264/H265 leverage hardware encoding)
        let negotiated = self.negotiate_video_codec();
        self.negotiated_codec = Some(negotiated);
        info!("Negotiated video codec: {:?}", negotiated);

        let response = self.create_login_response(true);
        let response_bytes = response
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        self.send_encrypted_arc(writer, &response_bytes).await?;
        Ok(true)
    }

    /// Negotiate video codec - select the best available encoder
    /// Priority: H264 > H265 > VP8 > VP9 (H264/H265 leverage hardware encoding on embedded devices)
    fn negotiate_video_codec(&self) -> VideoEncoderType {
        let registry = EncoderRegistry::global();

        // Check availability in priority order
        // H264 is preferred because it has the best hardware encoder support (RKMPP, VAAPI, etc.)
        // and most RustDesk clients support H264 hardware decoding
        if registry.is_format_available(VideoEncoderType::H264, false) {
            return VideoEncoderType::H264;
        }
        if registry.is_format_available(VideoEncoderType::H265, false) {
            return VideoEncoderType::H265;
        }
        if registry.is_format_available(VideoEncoderType::VP8, false) {
            return VideoEncoderType::VP8;
        }
        if registry.is_format_available(VideoEncoderType::VP9, false) {
            return VideoEncoderType::VP9;
        }

        // Fallback to H264 (should be available via hardware or software encoder)
        warn!("No video encoder available, defaulting to H264");
        VideoEncoderType::H264
    }

    /// Handle misc message with Arc writer
    async fn handle_misc_arc(
        &mut self,
        misc: &Misc,
        _writer: &Arc<Mutex<OwnedWriteHalf>>,
    ) -> anyhow::Result<()> {
        match &misc.union {
            Some(misc::Union::SwitchDisplay(sd)) => {
                debug!("Switch display request: {}", sd.display);
            }
            Some(misc::Union::Option(opt)) => {
                self.handle_option_message(opt).await?;
            }
            Some(misc::Union::RefreshVideo(refresh)) => {
                if *refresh {
                    debug!("Video refresh requested");
                    // TODO: Request keyframe from encoder
                }
            }
            Some(misc::Union::VideoReceived(received)) => {
                if *received {
                    debug!("Video received acknowledgement");
                }
            }
            _ => {
                debug!("Unhandled misc message");
            }
        }

        Ok(())
    }

    /// Handle Option message from client (includes codec and quality preferences)
    async fn handle_option_message(&mut self, opt: &OptionMessage) -> anyhow::Result<()> {
        // Handle image quality preset
        // RustDesk ImageQuality: NotSet=0, Low=2, Balanced=3, Best=4
        // Map to One-KVM BitratePreset: Low->Speed, Balanced->Balanced, Best->Quality
        let image_quality = opt.image_quality.value();
        if image_quality != 0 {
            let preset = match image_quality {
                2 => Some(BitratePreset::Speed),    // Low -> Speed (1 Mbps)
                3 => Some(BitratePreset::Balanced), // Balanced -> Balanced (4 Mbps)
                4 => Some(BitratePreset::Quality),  // Best -> Quality (8 Mbps)
                _ => None,
            };

            if let Some(preset) = preset {
                info!(
                    "Client requested quality preset: {:?} (image_quality={})",
                    preset, image_quality
                );
                if let Some(ref video_manager) = self.video_manager {
                    if let Err(e) = video_manager.set_bitrate_preset(preset).await {
                        warn!("Failed to set bitrate preset: {}", e);
                    }
                }
            }
        }

        // Check if client sent supported_decoding with a codec preference
        if let Some(ref supported_decoding) = opt.supported_decoding.as_ref() {
            let prefer = supported_decoding.prefer.value();
            debug!("Client codec preference: prefer={}", prefer);

            // Map RustDesk PreferCodec enum to our VideoEncoderType
            // From proto: Auto=0, VP9=1, H264=2, H265=3, VP8=4, AV1=5
            let requested_codec = match prefer {
                1 => Some(VideoEncoderType::VP9),
                2 => Some(VideoEncoderType::H264),
                3 => Some(VideoEncoderType::H265),
                4 => Some(VideoEncoderType::VP8),
                // Auto(0) or AV1(5) or unknown: use current or negotiate
                _ => None,
            };

            if let Some(new_codec) = requested_codec {
                // Check if this codec is different from current and available
                if self.negotiated_codec != Some(new_codec) {
                    let registry = EncoderRegistry::global();
                    if registry.is_format_available(new_codec, false) {
                        info!(
                            "Client requested codec switch: {:?} -> {:?}",
                            self.negotiated_codec, new_codec
                        );
                        // Switch codec
                        if let Err(e) = self.switch_video_codec(new_codec).await {
                            warn!("Failed to switch video codec: {}", e);
                        }
                    } else {
                        warn!(
                            "Client requested codec {:?} but it's not available",
                            new_codec
                        );
                    }
                }
            }
        }

        // Log custom_image_quality (accept but don't process)
        if opt.custom_image_quality > 0 {
            debug!(
                "Client sent custom_image_quality: {} (ignored)",
                opt.custom_image_quality
            );
        }
        if opt.custom_fps > 0 {
            debug!("Client requested FPS: {}", opt.custom_fps);
        }

        Ok(())
    }

    /// Switch video codec dynamically
    /// Stops current video task, changes codec, and restarts
    async fn switch_video_codec(&mut self, new_codec: VideoEncoderType) -> anyhow::Result<()> {
        // Stop current video streaming task
        if let Some(task) = self.video_task.take() {
            info!("Stopping video task for codec switch");
            task.abort();
            // Wait a bit for cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Update negotiated codec
        self.negotiated_codec = Some(new_codec);

        // Restart video streaming with new codec if we have a video_tx
        if let Some(ref video_tx) = self.video_frame_tx {
            info!("Restarting video streaming with codec {:?}", new_codec);
            self.start_video_streaming(video_tx.clone());
        } else {
            warn!("No video_tx available, cannot restart video streaming");
        }

        Ok(())
    }

    /// Start video streaming task
    fn start_video_streaming(&mut self, video_tx: mpsc::Sender<Bytes>) {
        let video_manager = match &self.video_manager {
            Some(vm) => vm.clone(),
            None => {
                warn!("No video manager available, skipping video streaming");
                return;
            }
        };

        let state = self.state.clone();
        let conn_id = self.id;
        let shutdown_tx = self.shutdown_tx.clone();
        let negotiated_codec = self.negotiated_codec.unwrap_or(VideoEncoderType::H264);

        let task = tokio::spawn(async move {
            info!(
                "Starting video streaming for connection {} with codec {:?}",
                conn_id, negotiated_codec
            );

            if let Err(e) = run_video_streaming(
                conn_id,
                video_manager,
                video_tx,
                state,
                shutdown_tx,
                negotiated_codec,
            )
            .await
            {
                error!("Video streaming error for connection {}: {}", conn_id, e);
            }

            info!("Video streaming stopped for connection {}", conn_id);
        });

        self.video_task = Some(task);
    }

    /// Start audio streaming task
    fn start_audio_streaming(&mut self, audio_tx: mpsc::Sender<Bytes>) {
        let audio_controller = match &self.audio {
            Some(ac) => ac.clone(),
            None => {
                debug!("No audio controller available, skipping audio streaming");
                return;
            }
        };

        let state = self.state.clone();
        let conn_id = self.id;
        let shutdown_tx = self.shutdown_tx.clone();

        let task = tokio::spawn(async move {
            info!("Starting audio streaming for connection {}", conn_id);

            if let Err(e) =
                run_audio_streaming(conn_id, audio_controller, audio_tx, state, shutdown_tx).await
            {
                error!("Audio streaming error for connection {}: {}", conn_id, e);
            }

            info!("Audio streaming stopped for connection {}", conn_id);
        });

        self.audio_task = Some(task);
    }

    /// Create SignedId message for initial handshake
    ///
    /// RustDesk protocol:
    /// - IdPk contains device ID and a fresh Curve25519 public key for this connection
    /// - The IdPk is signed with Ed25519 to prove ownership of the device
    /// - Client verifies the Ed25519 signature using public key from hbbs
    /// - Client then encrypts symmetric key using the Curve25519 public key from IdPk
    fn create_signed_id_message(&self, device_id: &str) -> HbbMessage {
        // Create IdPk with our device ID and temporary Curve25519 public key
        // IMPORTANT: Use the fresh Curve25519 public key, NOT Ed25519!
        // The client will use this directly for encryption (no conversion needed)
        let pk_bytes = self.temp_keypair.0.as_ref();
        let mut id_pk = IdPk::new();
        id_pk.id = device_id.to_string();
        id_pk.pk = pk_bytes.to_vec().into();

        // Encode IdPk to bytes
        let id_pk_bytes = id_pk.write_to_bytes().unwrap_or_default();

        // Sign the IdPk bytes with Ed25519
        // RustDesk's sign::sign() prepends the 64-byte signature to the message
        let signed_id_pk = self.signing_keypair.sign(&id_pk_bytes);

        let mut signed_id = SignedId::new();
        signed_id.id = signed_id_pk.into();

        let mut msg = HbbMessage::new();
        msg.union = Some(message::Union::SignedId(signed_id));
        msg
    }

    /// Handle peer's public key and negotiate session encryption
    /// After successful negotiation, send Hash message for password authentication
    async fn handle_peer_public_key(
        &mut self,
        pk: &PublicKey,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
    ) -> anyhow::Result<()> {
        // RustDesk's PublicKey message has two parts:
        // - asymmetric_value: The peer's temporary Curve25519 public key (32 bytes)
        // - symmetric_value: The sealed symmetric key (encrypted with our Curve25519 public key from IdPk)

        if pk.asymmetric_value.len() == 32 && !pk.symmetric_value.is_empty() {
            // Client sent us an encrypted symmetric key
            debug!(
                "Received encrypted handshake: pk={} bytes, sealed_key={} bytes",
                pk.asymmetric_value.len(),
                pk.symmetric_value.len()
            );

            // Decrypt the symmetric key using our temporary Curve25519 keypair
            // The client encrypted it using our Curve25519 public key from IdPk
            match crypto::decrypt_symmetric_key(
                &pk.asymmetric_value,
                &pk.symmetric_value,
                &self.temp_keypair.1,
            ) {
                Ok(session_key) => {
                    info!("Session key negotiated successfully");
                    self.session_key = Some(session_key);
                    self.encryption_enabled = true;
                }
                Err(e) => {
                    warn!(
                        "Failed to decrypt session key: {:?}, falling back to unencrypted",
                        e
                    );
                    // Continue without encryption - some clients may not support it
                    self.encryption_enabled = false;
                }
            }
        } else if pk.asymmetric_value.is_empty() {
            // Client doesn't want encryption
            debug!("Client requested unencrypted session");
            self.encryption_enabled = false;
        } else {
            // Just received a public key without symmetric key
            // This might be an older client or a different handshake mode
            debug!(
                "Received public key without symmetric value: {} bytes",
                pk.asymmetric_value.len()
            );
            self.encryption_enabled = false;
        }

        // Send Hash message for password authentication
        // This tells the client what salt to use for password hashing
        // Must be encrypted if session key was negotiated
        let hash_msg = self.create_hash_message();
        let hash_bytes = hash_msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        debug!(
            "Sending Hash message for password authentication (encrypted={})",
            self.encryption_enabled
        );
        self.send_encrypted_arc(writer, &hash_bytes).await?;

        Ok(())
    }

    /// Handle SignedId from peer
    ///
    /// When we receive a SignedId from the client, it means the client is also trying
    /// to authenticate. We should respond with our own SignedId if we haven't already,
    /// or proceed with the connection.
    async fn handle_signed_id(
        &mut self,
        si: &SignedId,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
    ) -> anyhow::Result<()> {
        // The SignedId contains a signed IdPk message
        // Try to parse the IdPk from the signed data
        // Note: The signature is at the beginning (64 bytes for Ed25519) followed by the message
        let signed_data = &si.id;

        // RustDesk's sign::sign() prepends the signature to the message
        // Ed25519 signature is 64 bytes
        let id_pk_bytes = if signed_data.len() > 64 {
            // Skip the signature and parse the IdPk
            &signed_data[64..]
        } else {
            // Might be unsigned, try parsing directly
            &signed_data[..]
        };

        if let Ok(id_pk) = IdPk::parse_from_bytes(id_pk_bytes) {
            info!(
                "Received SignedId from peer: id={}, pk_len={}",
                id_pk.id,
                id_pk.pk.len()
            );

            // Store the peer's ID
            if !id_pk.id.is_empty() {
                self.peer_id = id_pk.id.clone();
            }

            // If the peer sent a public key, we could use it for encryption
            // For now, just acknowledge
            debug!("Peer ID from SignedId: {}", self.peer_id);
        } else {
            warn!("Failed to parse IdPk from SignedId");
        }

        // If we haven't sent our SignedId yet, send it now
        // (This handles the case where client sends SignedId before we do)
        let signed_id_msg = self.create_signed_id_message(&self.device_id.clone());
        let signed_id_bytes = signed_id_msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        self.send_framed_arc(writer, &signed_id_bytes).await?;

        Ok(())
    }

    /// Verify password
    fn verify_password(&self, provided: &[u8]) -> bool {
        // RustDesk password verification:
        // We send Hash { salt: device_id, challenge: "" } to client
        // The client calculates: SHA256(SHA256(password + salt) + challenge)
        // See create_hash_message() for the salt and challenge we use
        //
        // Empty password case
        if provided.is_empty() {
            return self.password.is_empty();
        }

        if self.password.is_empty() {
            return false;
        }

        // The client calculates: SHA256(SHA256(password + salt) + challenge)
        // where salt is our device_id and challenge is empty
        let expected_hash = crypto::hash_password_double(&self.password, &self.device_id, "");

        // Try comparison with double hash
        if provided == expected_hash.as_slice() {
            debug!("Password verified with double hash");
            return true;
        }

        // Also try single hash for compatibility
        let expected_hash_single = crypto::hash_password(&self.password, &self.device_id);
        if provided == expected_hash_single.as_slice() {
            debug!("Password verified with single hash");
            return true;
        }

        // Log what we received vs expected for debugging
        debug!(
            "Password mismatch: provided_len={}, expected_double_len={}, expected_single_len={}",
            provided.len(),
            expected_hash.len(),
            expected_hash_single.len()
        );

        false
    }

    /// Create login response with dynamically detected encoder capabilities
    fn create_login_response(&self, success: bool) -> HbbMessage {
        if success {
            // Dynamically detect available encoders
            let registry = EncoderRegistry::global();

            // Check which encoders are available (include software fallback)
            let h264_available = registry.is_format_available(VideoEncoderType::H264, false);
            let h265_available = registry.is_format_available(VideoEncoderType::H265, false);
            let vp8_available = registry.is_format_available(VideoEncoderType::VP8, false);
            let vp9_available = registry.is_format_available(VideoEncoderType::VP9, false);

            info!(
                "Server encoding capabilities: H264={}, H265={}, VP8={}, VP9={}",
                h264_available, h265_available, vp8_available, vp9_available
            );

            let mut display_info = DisplayInfo::new();
            display_info.x = 0;
            display_info.y = 0;
            display_info.width = 1920;
            display_info.height = 1080;
            display_info.name = "KVM Display".to_string();
            display_info.online = true;
            display_info.cursor_embedded = false;
            display_info.scale = 1.0;

            let mut encoding = SupportedEncoding::new();
            encoding.h264 = h264_available;
            encoding.h265 = h265_available;
            encoding.vp8 = vp8_available;
            encoding.av1 = false; // AV1 not supported yet

            let mut peer_info = PeerInfo::new();
            peer_info.username = "one-kvm".to_string();
            peer_info.hostname = get_hostname();
            peer_info.platform = "Linux".to_string();
            peer_info.displays.push(display_info);
            peer_info.current_display = 0;
            peer_info.sas_enabled = false;
            peer_info.version = env!("CARGO_PKG_VERSION").to_string();
            peer_info.encoding = protobuf::MessageField::some(encoding);

            let mut login_response = LoginResponse::new();
            login_response.union = Some(login_response::Union::PeerInfo(peer_info));
            login_response.enable_trusted_devices = false;

            let mut msg = HbbMessage::new();
            msg.union = Some(message::Union::LoginResponse(login_response));
            msg
        } else {
            let mut login_response = LoginResponse::new();
            login_response.union =
                Some(login_response::Union::Error("Invalid password".to_string()));
            login_response.enable_trusted_devices = false;

            let mut msg = HbbMessage::new();
            msg.union = Some(message::Union::LoginResponse(login_response));
            msg
        }
    }

    /// Create login error response with specific error message
    /// RustDesk client recognizes specific error strings:
    /// - "Empty Password" -> prompts for password input
    /// - "Wrong Password" -> prompts for password re-entry
    fn create_login_error_response(&self, error: &str) -> HbbMessage {
        let mut login_response = LoginResponse::new();
        login_response.union = Some(login_response::Union::Error(error.to_string()));
        login_response.enable_trusted_devices = false;

        let mut msg = HbbMessage::new();
        msg.union = Some(message::Union::LoginResponse(login_response));
        msg
    }

    /// Create Hash message for password authentication
    /// The client will hash the password with the salt and send it back in LoginRequest
    fn create_hash_message(&self) -> HbbMessage {
        // Use device_id as salt for simplicity (RustDesk uses Config::get_salt())
        // The challenge field is not used for our password verification
        let mut hash = Hash::new();
        hash.salt = self.device_id.clone();
        hash.challenge = String::new();

        let mut msg = HbbMessage::new();
        msg.union = Some(message::Union::Hash(hash));
        msg
    }

    /// Handle TestDelay message for round-trip latency measurement
    ///
    /// RustDesk uses TestDelay for bidirectional latency measurement:
    /// 1. Server sends TestDelay with from_client=false, records send time
    /// 2. Client echoes back the same TestDelay
    /// 3. Server calculates RTT and stores in last_delay
    /// 4. Server includes last_delay in next TestDelay for client display
    async fn handle_test_delay(
        &mut self,
        td: &TestDelay,
        writer: &Arc<Mutex<OwnedWriteHalf>>,
    ) -> anyhow::Result<()> {
        if td.from_client {
            // Client initiated the delay test, respond with the same time
            let mut test_delay = TestDelay::new();
            test_delay.time = td.time;
            test_delay.from_client = false;
            test_delay.last_delay = self.last_delay;
            test_delay.target_bitrate = 0; // We don't do adaptive bitrate yet

            let mut response = HbbMessage::new();
            response.union = Some(message::Union::TestDelay(test_delay));

            let data = response
                .write_to_bytes()
                .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
            self.send_encrypted_arc(writer, &data).await?;

            debug!(
                "TestDelay response sent: time={}, last_delay={}ms",
                td.time, self.last_delay
            );
        } else {
            // This is a response to our TestDelay - calculate RTT
            if let Some(sent_time) = self.last_test_delay_sent.take() {
                let rtt_ms = sent_time.elapsed().as_millis() as u32;
                self.last_delay = rtt_ms;

                debug!(
                    "TestDelay RTT measured: {}ms (from echoed time={})",
                    rtt_ms, td.time
                );
            }
        }

        Ok(())
    }

    /// Send TestDelay message to client for latency measurement
    ///
    /// The client will echo this back, allowing us to calculate RTT.
    /// The measured delay is then included in future TestDelay messages
    /// for the client to display.
    async fn send_test_delay(&mut self, writer: &Arc<Mutex<OwnedWriteHalf>>) -> anyhow::Result<()> {
        // Get current time in milliseconds since epoch
        let time_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut test_delay = TestDelay::new();
        test_delay.time = time_ms;
        test_delay.from_client = false;
        test_delay.last_delay = self.last_delay;
        test_delay.target_bitrate = 0;

        let mut msg = HbbMessage::new();
        msg.union = Some(message::Union::TestDelay(test_delay));

        let data = msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        self.send_encrypted_arc(writer, &data).await?;

        // Record when we sent this, so we can calculate RTT when client echoes back
        self.last_test_delay_sent = Some(Instant::now());

        debug!(
            "TestDelay sent: time={}, last_delay={}ms",
            time_ms, self.last_delay
        );
        Ok(())
    }

    /// Handle key event
    async fn handle_key_event(&mut self, ke: &KeyEvent) -> anyhow::Result<()> {
        debug!(
            "Key event: down={}, press={}, chr={:?}, modifiers={:?}",
            ke.down, ke.press, ke.union, ke.modifiers
        );

        // Check for CapsLock state change in modifiers
        // RustDesk doesn't send CapsLock key events, only includes it in modifiers
        let caps_lock_in_modifiers = ke.modifiers.iter().any(|m| {
            use protobuf::Enum;
            m.value() == ControlKey::CapsLock.value()
        });

        if caps_lock_in_modifiers != self.last_caps_lock {
            self.last_caps_lock = caps_lock_in_modifiers;
            // Send CapsLock key press (down + up) to toggle state on target
            if let Some(ref hid) = self.hid {
                debug!(
                    "CapsLock state changed to {}, sending CapsLock key",
                    caps_lock_in_modifiers
                );
                let caps_down = KeyboardEvent {
                    event_type: KeyEventType::Down,
                    key: 0x39, // USB HID CapsLock
                    modifiers: KeyboardModifiers::default(),
                    is_usb_hid: true,
                };
                let caps_up = KeyboardEvent {
                    event_type: KeyEventType::Up,
                    key: 0x39,
                    modifiers: KeyboardModifiers::default(),
                    is_usb_hid: true,
                };
                if let Err(e) = hid.send_keyboard(caps_down).await {
                    warn!("Failed to send CapsLock down: {}", e);
                }
                if let Err(e) = hid.send_keyboard(caps_up).await {
                    warn!("Failed to send CapsLock up: {}", e);
                }
            }
        }

        // Convert RustDesk key event to One-KVM key event
        if let Some(kb_event) = convert_key_event(ke) {
            debug!(
                "Converted to HID: key=0x{:02X}, event_type={:?}, modifiers={:02X}",
                kb_event.key,
                kb_event.event_type,
                kb_event.modifiers.to_hid_byte()
            );
            // Send to HID controller if available
            if let Some(ref hid) = self.hid {
                if let Err(e) = hid.send_keyboard(kb_event).await {
                    warn!("Failed to send keyboard event: {}", e);
                }
            } else {
                debug!("HID controller not available, skipping key event");
            }
        } else {
            warn!("Could not convert key event to HID: chr={:?}", ke.union);
        }

        Ok(())
    }

    /// Handle mouse event with throttling
    ///
    /// Pure move events (no button/scroll) are throttled to prevent HID EAGAIN errors.
    /// Button down/up and scroll events are always sent immediately.
    async fn handle_mouse_event(&mut self, me: &MouseEvent) -> anyhow::Result<()> {
        // Parse RustDesk mask format: (button << 3) | event_type
        let event_type = me.mask & 0x07;

        // Check if this is a pure move event (no button/scroll)
        let is_pure_move = event_type == mouse_type::MOVE;

        // For pure move events, apply throttling
        if is_pure_move && !self.input_throttler.should_send_mouse_move() {
            // Skip this move event to prevent HID EAGAIN
            return Ok(());
        }

        debug!("Mouse event: x={}, y={}, mask={}", me.x, me.y, me.mask);

        // Convert RustDesk mouse event to One-KVM mouse events
        let mouse_events = convert_mouse_event(me, self.screen_width, self.screen_height);

        // Send to HID controller if available
        if let Some(ref hid) = self.hid {
            for event in mouse_events {
                if let Err(e) = hid.send_mouse(event).await {
                    warn!("Failed to send mouse event: {}", e);
                }
            }
            // Mark that we sent a mouse event (for non-move events)
            if !is_pure_move {
                self.input_throttler.mark_mouse_sent();
            }
        } else {
            debug!("HID controller not available, skipping mouse event");
        }

        Ok(())
    }

    /// Close the connection
    pub fn close(&self) {
        let _ = self.shutdown_tx.send(());
        *self.state.write() = ConnectionState::Closed;
    }
}

/// Lightweight connection info for tracking active connections
pub struct ConnectionInfo {
    /// Connection ID
    pub id: u32,
    /// Connection state (shared with Connection)
    pub state: Arc<RwLock<ConnectionState>>,
}

impl ConnectionInfo {
    /// Get connection state
    pub fn state(&self) -> ConnectionState {
        self.state.read().clone()
    }
}

/// Connection manager
pub struct ConnectionManager {
    /// Active connection info
    connections: Arc<RwLock<Vec<Arc<RwLock<ConnectionInfo>>>>>,
    /// Next connection ID
    next_id: Arc<RwLock<u32>>,
    /// Configuration
    config: Arc<RwLock<RustDeskConfig>>,
    /// Keypair for encryption (Curve25519)
    keypair: Arc<RwLock<Option<KeyPair>>>,
    /// Signing keypair for Ed25519 signatures (SignedId messages)
    signing_keypair: Arc<RwLock<Option<SigningKeyPair>>>,
    /// HID controller for keyboard/mouse
    hid: Arc<RwLock<Option<Arc<HidController>>>>,
    /// Audio controller for audio streaming
    audio: Arc<RwLock<Option<Arc<AudioController>>>>,
    /// Video stream manager for frame subscription
    video_manager: Arc<RwLock<Option<Arc<VideoStreamManager>>>>,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(config: RustDeskConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(Vec::new())),
            next_id: Arc::new(RwLock::new(1)),
            config: Arc::new(RwLock::new(config)),
            keypair: Arc::new(RwLock::new(None)),
            signing_keypair: Arc::new(RwLock::new(None)),
            hid: Arc::new(RwLock::new(None)),
            audio: Arc::new(RwLock::new(None)),
            video_manager: Arc::new(RwLock::new(None)),
        }
    }

    /// Set HID controller
    pub fn set_hid(&self, hid: Arc<HidController>) {
        *self.hid.write() = Some(hid);
    }

    /// Set audio controller
    pub fn set_audio(&self, audio: Arc<AudioController>) {
        *self.audio.write() = Some(audio);
    }

    /// Set video stream manager
    pub fn set_video_manager(&self, video_manager: Arc<VideoStreamManager>) {
        *self.video_manager.write() = Some(video_manager);
    }

    /// Set keypair
    pub fn set_keypair(&self, keypair: KeyPair) {
        *self.keypair.write() = Some(keypair);
    }

    /// Get or generate keypair
    pub fn ensure_keypair(&self) -> KeyPair {
        let mut kp = self.keypair.write();
        if kp.is_none() {
            *kp = Some(KeyPair::generate());
        }
        kp.as_ref().unwrap().clone()
    }

    /// Set signing keypair (Ed25519)
    pub fn set_signing_keypair(&self, signing_keypair: SigningKeyPair) {
        *self.signing_keypair.write() = Some(signing_keypair);
    }

    /// Get or generate signing keypair (Ed25519)
    pub fn ensure_signing_keypair(&self) -> SigningKeyPair {
        let mut skp = self.signing_keypair.write();
        if skp.is_none() {
            warn!("ConnectionManager: signing_keypair not set, generating new one! This may cause signature verification failure.");
            *skp = Some(SigningKeyPair::generate());
        }
        skp.as_ref().unwrap().clone()
    }

    /// Accept a new connection
    pub async fn accept_connection(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> anyhow::Result<u32> {
        let id = {
            let mut next = self.next_id.write();
            let id = *next;
            *next += 1;
            id
        };

        let config = self.config.read().clone();
        let signing_keypair = self.ensure_signing_keypair();
        let hid = self.hid.read().clone();
        let audio = self.audio.read().clone();
        let video_manager = self.video_manager.read().clone();
        let (mut conn, _rx) =
            Connection::new(id, &config, signing_keypair, hid, audio, video_manager);

        // Track connection state for external access
        let state = conn.state.clone();
        self.connections
            .write()
            .push(Arc::new(RwLock::new(ConnectionInfo { id, state })));

        // Spawn connection handler - Connection is moved, not locked
        tokio::spawn(async move {
            if let Err(e) = conn.handle_tcp(stream, peer_addr).await {
                error!("Connection {} error: {}", id, e);
            }
        });

        Ok(id)
    }

    /// Get active connection count
    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    /// Mark all connections as closed (actual connection tasks will detect this)
    pub fn close_all(&self) {
        let connections = self.connections.read();
        for conn_info in connections.iter() {
            *conn_info.read().state.write() = ConnectionState::Closed;
        }
    }
}

/// Run video streaming loop for a connection
///
/// This function subscribes to the shared video encoding pipeline (used by WebRTC)
/// and forwards encoded frames to the RustDesk client. This avoids duplicate encoding
/// when both WebRTC and RustDesk clients are connected.
///
/// When the pipeline is restarted (e.g., due to bitrate/codec change), this function
/// will automatically re-subscribe to the new pipeline.
async fn run_video_streaming(
    conn_id: u32,
    video_manager: Arc<VideoStreamManager>,
    video_tx: mpsc::Sender<Bytes>,
    state: Arc<RwLock<ConnectionState>>,
    shutdown_tx: broadcast::Sender<()>,
    negotiated_codec: VideoEncoderType,
) -> anyhow::Result<()> {
    use crate::video::encoder::VideoCodecType;

    // Convert VideoEncoderType to VideoCodecType for the pipeline
    let webrtc_codec = match negotiated_codec {
        VideoEncoderType::H264 => VideoCodecType::H264,
        VideoEncoderType::H265 => VideoCodecType::H265,
        VideoEncoderType::VP8 => VideoCodecType::VP8,
        VideoEncoderType::VP9 => VideoCodecType::VP9,
    };

    // Set the video codec on the shared pipeline before subscribing
    info!(
        "Setting video codec to {:?} for connection {}",
        negotiated_codec, conn_id
    );
    if let Err(e) = video_manager.set_video_codec(webrtc_codec).await {
        error!("Failed to set video codec: {}", e);
        // Continue anyway, will use whatever codec the pipeline already has
    }

    // Create video frame adapter for RustDesk protocol
    let codec = match negotiated_codec {
        VideoEncoderType::H264 => VideoCodec::H264,
        VideoEncoderType::H265 => VideoCodec::H265,
        VideoEncoderType::VP8 => VideoCodec::VP8,
        VideoEncoderType::VP9 => VideoCodec::VP9,
    };
    let mut video_adapter = VideoFrameAdapter::new(codec);

    let mut shutdown_rx = shutdown_tx.subscribe();
    let mut encoded_count: u64 = 0;
    let mut last_log_time = Instant::now();

    info!(
        "Started shared video streaming for connection {} (codec: {:?})",
        conn_id, codec
    );

    // Outer loop: handles pipeline restarts by re-subscribing
    'subscribe_loop: loop {
        // Check if connection is still active before subscribing
        if *state.read() != ConnectionState::Active {
            debug!("Connection {} no longer active, stopping video", conn_id);
            break;
        }

        // Subscribe to the shared video encoding pipeline
        let mut encoded_frame_rx = match video_manager.subscribe_encoded_frames().await {
            Some(rx) => rx,
            None => {
                // Pipeline not ready yet, wait and retry
                debug!(
                    "No encoded frame source available for connection {}, retrying...",
                    conn_id
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue 'subscribe_loop;
            }
        };

        // Log encoding config
        if let Some(config) = video_manager.get_encoding_config().await {
            info!(
                "RustDesk connection {} subscribed to video pipeline: {:?} {}x{} @ {}",
                conn_id,
                config.output_codec,
                config.resolution.width,
                config.resolution.height,
                config.bitrate_preset
            );
        }

        // Inner loop: receives frames from current subscription
        loop {
            // Check if connection is still active
            if *state.read() != ConnectionState::Active {
                debug!("Connection {} no longer active, stopping video", conn_id);
                break 'subscribe_loop;
            }

            tokio::select! {
                biased;

                _ = shutdown_rx.recv() => {
                    debug!("Shutdown signal received, stopping video for connection {}", conn_id);
                    break 'subscribe_loop;
                }

                result = encoded_frame_rx.recv() => {
                    match result {
                        Ok(frame) => {
                            // Convert EncodedVideoFrame to RustDesk VideoFrame message
                            // Use zero-copy version: Bytes.clone() only increments refcount
                            let msg_bytes = video_adapter.encode_frame_bytes_zero_copy(
                                frame.data.clone(),
                                frame.is_keyframe,
                                frame.pts_ms as u64,
                            );

                            // Send to connection (blocks if channel is full, providing backpressure)
                            if video_tx.send(msg_bytes).await.is_err() {
                                debug!("Video channel closed for connection {}", conn_id);
                                break 'subscribe_loop;
                            }

                            encoded_count += 1;

                            // Log stats periodically
                            if last_log_time.elapsed().as_secs() >= 10 {
                                info!(
                                    "Video streaming stats for connection {}: {} frames forwarded",
                                    conn_id, encoded_count
                                );
                                last_log_time = Instant::now();
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("Connection {} lagged {} encoded frames", conn_id, n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Pipeline was restarted (e.g., bitrate/codec change)
                            // Re-subscribe to the new pipeline
                            info!("Video pipeline closed for connection {}, re-subscribing...", conn_id);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            continue 'subscribe_loop;
                        }
                    }
                }
            }
        }
    }

    info!(
        "Video streaming ended for connection {}: {} total frames forwarded",
        conn_id, encoded_count
    );

    Ok(())
}

/// Run audio streaming loop for a connection
///
/// This function subscribes to the audio controller's Opus stream
/// and forwards encoded audio frames to the RustDesk client.
async fn run_audio_streaming(
    conn_id: u32,
    audio_controller: Arc<AudioController>,
    audio_tx: mpsc::Sender<Bytes>,
    state: Arc<RwLock<ConnectionState>>,
    shutdown_tx: broadcast::Sender<()>,
) -> anyhow::Result<()> {
    // Audio format: 48kHz stereo Opus
    let mut audio_adapter = AudioFrameAdapter::new(48000, 2);

    let mut shutdown_rx = shutdown_tx.subscribe();
    let mut frame_count: u64 = 0;
    let mut last_log_time = Instant::now();

    info!("Started audio streaming for connection {}", conn_id);

    // Outer loop: handles pipeline restarts by re-subscribing
    'subscribe_loop: loop {
        // Check if connection is still active before subscribing
        if *state.read() != ConnectionState::Active {
            debug!("Connection {} no longer active, stopping audio", conn_id);
            break;
        }

        // Subscribe to the audio Opus stream
        let mut opus_rx = match audio_controller.subscribe_opus_async().await {
            Some(rx) => rx,
            None => {
                // Audio not available, wait and retry
                debug!(
                    "No audio source available for connection {}, retrying...",
                    conn_id
                );
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue 'subscribe_loop;
            }
        };

        info!(
            "RustDesk connection {} subscribed to audio pipeline",
            conn_id
        );

        // Send audio format message once before sending frames
        if !audio_adapter.format_sent() {
            let format_msg = audio_adapter.create_format_message();
            let format_bytes = Bytes::from(format_msg.write_to_bytes().unwrap_or_default());
            if audio_tx.send(format_bytes).await.is_err() {
                debug!("Audio channel closed for connection {}", conn_id);
                break 'subscribe_loop;
            }
            debug!("Sent audio format message for connection {}", conn_id);
        }

        // Inner loop: receives frames from current subscription
        loop {
            // Check if connection is still active
            if *state.read() != ConnectionState::Active {
                debug!("Connection {} no longer active, stopping audio", conn_id);
                break 'subscribe_loop;
            }

            tokio::select! {
                biased;

                _ = shutdown_rx.recv() => {
                    debug!("Shutdown signal received, stopping audio for connection {}", conn_id);
                    break 'subscribe_loop;
                }

                result = opus_rx.recv() => {
                    match result {
                        Ok(opus_frame) => {
                            // Convert OpusFrame to RustDesk AudioFrame message
                            let msg_bytes = audio_adapter.encode_opus_bytes(&opus_frame.data);

                            // Send to connection (blocks if channel is full, providing backpressure)
                            if audio_tx.send(msg_bytes).await.is_err() {
                                debug!("Audio channel closed for connection {}", conn_id);
                                break 'subscribe_loop;
                            }

                            frame_count += 1;

                            // Log stats periodically
                            if last_log_time.elapsed().as_secs() >= 30 {
                                info!(
                                    "Audio streaming stats for connection {}: {} frames forwarded",
                                    conn_id, frame_count
                                );
                                last_log_time = Instant::now();
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("Connection {} lagged {} audio frames", conn_id, n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Pipeline was restarted
                            info!("Audio pipeline closed for connection {}, re-subscribing...", conn_id);
                            audio_adapter.reset();
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            continue 'subscribe_loop;
                        }
                    }
                }
            }
        }
    }

    info!(
        "Audio streaming ended for connection {}: {} total frames forwarded",
        conn_id, frame_count
    );

    Ok(())
}
