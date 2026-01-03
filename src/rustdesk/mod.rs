//! RustDesk Protocol Integration Module
//!
//! This module implements the RustDesk client protocol, enabling One-KVM devices
//! to be accessed via standard RustDesk clients through existing hbbs/hbbr servers.
//!
//! ## Architecture
//!
//! - `config`: Configuration types for RustDesk settings
//! - `protocol`: Protobuf message wrappers and serialization
//! - `crypto`: NaCl cryptography (key generation, encryption, signatures)
//! - `rendezvous`: Communication with hbbs rendezvous server
//! - `connection`: Client session handling
//! - `frame_adapters`: Video/audio frame conversion to RustDesk format
//! - `hid_adapter`: RustDesk HID events to One-KVM conversion

pub mod bytes_codec;
pub mod config;
pub mod connection;
pub mod crypto;
pub mod frame_adapters;
pub mod hid_adapter;
pub mod protocol;
pub mod punch;
pub mod rendezvous;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use protobuf::Message;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::audio::AudioController;
use crate::hid::HidController;
use crate::video::stream_manager::VideoStreamManager;

use self::config::RustDeskConfig;
use self::connection::ConnectionManager;
use self::protocol::{make_local_addr, make_relay_response, make_request_relay};
use self::rendezvous::{AddrMangle, RendezvousMediator, RendezvousStatus};

/// Relay connection timeout
const RELAY_CONNECT_TIMEOUT_MS: u64 = 10_000;

/// RustDesk service status
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    /// Service is stopped
    Stopped,
    /// Service is starting
    Starting,
    /// Service is running and registered with rendezvous server
    Running,
    /// Service encountered an error
    Error(String),
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// Default port for direct TCP connections (same as RustDesk)
const DIRECT_LISTEN_PORT: u16 = 21118;

/// RustDesk Service
///
/// Manages the RustDesk protocol integration, including:
/// - Registration with hbbs rendezvous server
/// - Accepting connections from RustDesk clients
/// - Streaming video/audio and receiving HID input
pub struct RustDeskService {
    config: Arc<RwLock<RustDeskConfig>>,
    status: Arc<RwLock<ServiceStatus>>,
    rendezvous: Arc<RwLock<Option<Arc<RendezvousMediator>>>>,
    rendezvous_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    tcp_listener_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    listen_port: Arc<RwLock<u16>>,
    connection_manager: Arc<ConnectionManager>,
    video_manager: Arc<VideoStreamManager>,
    hid: Arc<HidController>,
    audio: Arc<AudioController>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RustDeskService {
    /// Create a new RustDesk service instance
    pub fn new(
        config: RustDeskConfig,
        video_manager: Arc<VideoStreamManager>,
        hid: Arc<HidController>,
        audio: Arc<AudioController>,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let connection_manager = Arc::new(ConnectionManager::new(config.clone()));

        Self {
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(ServiceStatus::Stopped)),
            rendezvous: Arc::new(RwLock::new(None)),
            rendezvous_handle: Arc::new(RwLock::new(None)),
            tcp_listener_handle: Arc::new(RwLock::new(None)),
            listen_port: Arc::new(RwLock::new(DIRECT_LISTEN_PORT)),
            connection_manager,
            video_manager,
            hid,
            audio,
            shutdown_tx,
        }
    }

    /// Get the port for direct TCP connections
    pub fn listen_port(&self) -> u16 {
        *self.listen_port.read()
    }

    /// Get current service status
    pub fn status(&self) -> ServiceStatus {
        self.status.read().clone()
    }

    /// Get current configuration
    pub fn config(&self) -> RustDeskConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn update_config(&self, config: RustDeskConfig) {
        *self.config.write() = config;
    }

    /// Get rendezvous status
    pub fn rendezvous_status(&self) -> Option<RendezvousStatus> {
        self.rendezvous.read().as_ref().map(|r| r.status())
    }

    /// Get device ID
    pub fn device_id(&self) -> String {
        self.config.read().device_id.clone()
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connection_manager.connection_count()
    }

    /// Start the RustDesk service
    pub async fn start(&self) -> anyhow::Result<()> {
        let config = self.config.read().clone();

        if !config.enabled {
            info!("RustDesk service is disabled");
            return Ok(());
        }

        if !config.is_valid() {
            warn!("RustDesk configuration is incomplete");
            return Ok(());
        }

        if self.status() == ServiceStatus::Running {
            warn!("RustDesk service is already running");
            return Ok(());
        }

        *self.status.write() = ServiceStatus::Starting;
        info!(
            "Starting RustDesk service with ID: {} -> {}",
            config.device_id,
            config.rendezvous_addr()
        );

        // Initialize crypto
        if let Err(e) = crypto::init() {
            error!("Failed to initialize crypto: {}", e);
            *self.status.write() = ServiceStatus::Error(e.to_string());
            return Err(e.into());
        }

        // Create and start rendezvous mediator with relay callback
        let mediator = Arc::new(RendezvousMediator::new(config.clone()));

        // Set the keypair on connection manager (Curve25519 for encryption)
        let keypair = mediator.ensure_keypair();
        self.connection_manager.set_keypair(keypair);

        // Set the signing keypair on connection manager (Ed25519 for SignedId)
        let signing_keypair = mediator.ensure_signing_keypair();
        self.connection_manager.set_signing_keypair(signing_keypair);

        // Set the HID controller on connection manager
        self.connection_manager.set_hid(self.hid.clone());

        // Set the audio controller on connection manager for audio streaming
        self.connection_manager.set_audio(self.audio.clone());

        // Set the video manager on connection manager for video streaming
        self.connection_manager.set_video_manager(self.video_manager.clone());

        *self.rendezvous.write() = Some(mediator.clone());

        // Start TCP listener BEFORE the rendezvous mediator to ensure port is set correctly
        // This prevents race condition where mediator starts registration with wrong port
        let (tcp_handle, listen_port) = self.start_tcp_listener_with_port().await?;
        *self.tcp_listener_handle.write() = Some(tcp_handle);

        // Set the listen port on mediator before starting the registration loop
        mediator.set_listen_port(listen_port);

        // Create relay request handler
        let connection_manager = self.connection_manager.clone();
        let video_manager = self.video_manager.clone();
        let hid = self.hid.clone();
        let audio = self.audio.clone();
        let service_config = self.config.clone();

        // Set the punch callback on the mediator (tries P2P first, then relay)
        let connection_manager_punch = self.connection_manager.clone();
        let video_manager_punch = self.video_manager.clone();
        let hid_punch = self.hid.clone();
        let audio_punch = self.audio.clone();
        let service_config_punch = self.config.clone();

        mediator.set_punch_callback(Arc::new(move |peer_addr, rendezvous_addr, relay_server, uuid, socket_addr, device_id| {
            let conn_mgr = connection_manager_punch.clone();
            let video = video_manager_punch.clone();
            let hid = hid_punch.clone();
            let audio = audio_punch.clone();
            let config = service_config_punch.clone();

            tokio::spawn(async move {
                // Get relay_key from config, or use public server's relay_key if using public server
                let relay_key = {
                    let cfg = config.read();
                    cfg.relay_key.clone().unwrap_or_else(|| {
                        if cfg.is_using_public_server() {
                            crate::secrets::rustdesk::RELAY_KEY.to_string()
                        } else {
                            String::new()
                        }
                    })
                };

                // Try P2P direct connection first
                if let Some(addr) = peer_addr {
                    info!("Attempting P2P direct connection to {}", addr);
                    match punch::try_direct_connection(addr).await {
                        punch::PunchResult::DirectConnection(stream) => {
                            info!("P2P direct connection succeeded to {}", addr);
                            if let Err(e) = conn_mgr.accept_connection(stream, addr).await {
                                error!("Failed to accept P2P connection: {}", e);
                            }
                            return;
                        }
                        punch::PunchResult::NeedRelay => {
                            info!("P2P direct connection failed, falling back to relay");
                        }
                    }
                }

                // Fall back to relay
                if let Err(e) = handle_relay_request(
                    &rendezvous_addr,
                    &relay_server,
                    &uuid,
                    &socket_addr,
                    &device_id,
                    &relay_key,
                    conn_mgr,
                    video,
                    hid,
                    audio,
                ).await {
                    error!("Failed to handle relay request: {}", e);
                }
            });
        }));

        // Set the relay callback on the mediator
        mediator.set_relay_callback(Arc::new(move |rendezvous_addr, relay_server, uuid, socket_addr, device_id| {
            let conn_mgr = connection_manager.clone();
            let video = video_manager.clone();
            let hid = hid.clone();
            let audio = audio.clone();
            let config = service_config.clone();

            tokio::spawn(async move {
                // Get relay_key from config, or use public server's relay_key if using public server
                let relay_key = {
                    let cfg = config.read();
                    cfg.relay_key.clone().unwrap_or_else(|| {
                        if cfg.is_using_public_server() {
                            crate::secrets::rustdesk::RELAY_KEY.to_string()
                        } else {
                            String::new()
                        }
                    })
                };

                if let Err(e) = handle_relay_request(
                    &rendezvous_addr,
                    &relay_server,
                    &uuid,
                    &socket_addr,
                    &device_id,
                    &relay_key,
                    conn_mgr,
                    video,
                    hid,
                    audio,
                ).await {
                    error!("Failed to handle relay request: {}", e);
                }
            });
        }));

        // Set the intranet callback on the mediator for same-LAN connections
        let connection_manager2 = self.connection_manager.clone();
        mediator.set_intranet_callback(Arc::new(move |rendezvous_addr, peer_socket_addr, local_addr, relay_server, device_id| {
            let conn_mgr = connection_manager2.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_intranet_request(
                    &rendezvous_addr,
                    &peer_socket_addr,
                    local_addr,
                    &relay_server,
                    &device_id,
                    conn_mgr,
                ).await {
                    error!("Failed to handle intranet request: {}", e);
                }
            });
        }));

        // Spawn rendezvous task
        let status = self.status.clone();
        let handle = tokio::spawn(async move {
            loop {
                match mediator.start().await {
                    Ok(_) => {
                        info!("Rendezvous mediator stopped normally");
                        break;
                    }
                    Err(e) => {
                        error!("Rendezvous mediator error: {}", e);
                        *status.write() = ServiceStatus::Error(e.to_string());
                        // Wait before retry
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        *status.write() = ServiceStatus::Starting;
                    }
                }
            }
        });

        *self.rendezvous_handle.write() = Some(handle);

        *self.status.write() = ServiceStatus::Running;

        Ok(())
    }

    /// Start TCP listener for direct peer connections
    /// Returns the join handle and the port that was bound
    async fn start_tcp_listener_with_port(&self) -> anyhow::Result<(JoinHandle<()>, u16)> {
        // Try to bind to the default port, or find an available port
        let listener = match TcpListener::bind(format!("0.0.0.0:{}", DIRECT_LISTEN_PORT)).await {
            Ok(l) => l,
            Err(_) => {
                // Try binding to port 0 to get an available port
                TcpListener::bind("0.0.0.0:0").await?
            }
        };

        let local_addr = listener.local_addr()?;
        let listen_port = local_addr.port();
        *self.listen_port.write() = listen_port;
        info!("RustDesk TCP listener started on {}", local_addr);

        let connection_manager = self.connection_manager.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer_addr)) => {
                                info!("Accepted direct connection from {}", peer_addr);
                                let conn_mgr = connection_manager.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = conn_mgr.accept_connection(stream, peer_addr).await {
                                        error!("Failed to handle direct connection from {}: {}", peer_addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                error!("TCP accept error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("TCP listener shutting down");
                        break;
                    }
                }
            }
        });

        Ok((handle, listen_port))
    }

    /// Stop the RustDesk service
    pub async fn stop(&self) -> anyhow::Result<()> {
        if self.status() == ServiceStatus::Stopped {
            return Ok(());
        }

        info!("Stopping RustDesk service");

        // Send shutdown signal (this will stop the TCP listener)
        let _ = self.shutdown_tx.send(());

        // Close all connections
        self.connection_manager.close_all();

        // Stop rendezvous mediator
        if let Some(mediator) = self.rendezvous.read().as_ref() {
            mediator.stop();
        }

        // Wait for rendezvous task to finish
        if let Some(handle) = self.rendezvous_handle.write().take() {
            handle.abort();
        }

        // Wait for TCP listener task to finish
        if let Some(handle) = self.tcp_listener_handle.write().take() {
            handle.abort();
        }

        *self.rendezvous.write() = None;
        *self.status.write() = ServiceStatus::Stopped;

        Ok(())
    }

    /// Restart the service with new configuration
    pub async fn restart(&self, config: RustDeskConfig) -> anyhow::Result<()> {
        self.stop().await?;
        self.update_config(config);
        self.start().await
    }

    /// Save keypair and UUID to config
    /// Returns the updated config if changes were made
    pub fn save_credentials(&self) -> Option<RustDeskConfig> {
        if let Some(mediator) = self.rendezvous.read().as_ref() {
            let kp = mediator.ensure_keypair();
            let skp = mediator.ensure_signing_keypair();
            let mut config = self.config.write();
            let mut changed = false;

            // Save encryption keypair (Curve25519)
            let pk = kp.public_key_base64();
            let sk = kp.secret_key_base64();
            if config.public_key.as_ref() != Some(&pk) || config.private_key.as_ref() != Some(&sk) {
                config.public_key = Some(pk);
                config.private_key = Some(sk);
                changed = true;
            }

            // Save signing keypair (Ed25519)
            let signing_pk = skp.public_key_base64();
            let signing_sk = skp.secret_key_base64();
            if config.signing_public_key.as_ref() != Some(&signing_pk) || config.signing_private_key.as_ref() != Some(&signing_sk) {
                config.signing_public_key = Some(signing_pk);
                config.signing_private_key = Some(signing_sk);
                changed = true;
            }

            // Save UUID if it was newly generated
            if mediator.uuid_needs_save() {
                let mediator_config = mediator.config();
                if let Some(uuid) = mediator_config.uuid {
                    if config.uuid.as_ref() != Some(&uuid) {
                        config.uuid = Some(uuid);
                        changed = true;
                    }
                }
                mediator.mark_uuid_saved();
            }

            if changed {
                return Some(config.clone());
            }
        }
        None
    }

    /// Save keypair to config (deprecated, use save_credentials instead)
    #[deprecated(note = "Use save_credentials instead")]
    pub fn save_keypair(&self) {
        let _ = self.save_credentials();
    }
}

/// Handle relay request from rendezvous server
///
/// Correct flow based on RustDesk protocol:
/// 1. Connect to RENDEZVOUS server (not relay!)
/// 2. Send RelayResponse with client's socket_addr
/// 3. Connect to RELAY server
/// 4. Accept connection without waiting for response
async fn handle_relay_request(
    rendezvous_addr: &str,
    relay_server: &str,
    uuid: &str,
    socket_addr: &[u8],
    device_id: &str,
    relay_key: &str,
    connection_manager: Arc<ConnectionManager>,
    _video_manager: Arc<VideoStreamManager>,
    _hid: Arc<HidController>,
    _audio: Arc<AudioController>,
) -> anyhow::Result<()> {
    info!("Handling relay request: rendezvous={}, relay={}, uuid={}", rendezvous_addr, relay_server, uuid);

    // Step 1: Connect to RENDEZVOUS server and send RelayResponse
    let rendezvous_socket_addr: SocketAddr = tokio::net::lookup_host(rendezvous_addr)
        .await?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve rendezvous server: {}", rendezvous_addr))?;

    let mut rendezvous_stream = tokio::time::timeout(
        Duration::from_millis(RELAY_CONNECT_TIMEOUT_MS),
        TcpStream::connect(rendezvous_socket_addr),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Rendezvous connection timeout"))??;

    debug!("Connected to rendezvous server at {}", rendezvous_socket_addr);

    // Send RelayResponse to rendezvous server with client's socket_addr
    // IMPORTANT: Include our device ID so rendezvous server can look up and sign our public key
    let relay_response = make_relay_response(uuid, socket_addr, relay_server, device_id);
    let bytes = relay_response.write_to_bytes().map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
    bytes_codec::write_frame(&mut rendezvous_stream, &bytes).await?;
    debug!("Sent RelayResponse to rendezvous server for uuid={}", uuid);

    // Close rendezvous connection - we don't need to wait for response
    drop(rendezvous_stream);

    // Step 2: Connect to RELAY server and send RequestRelay to identify ourselves
    let relay_addr: SocketAddr = tokio::net::lookup_host(relay_server)
        .await?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve relay server: {}", relay_server))?;

    let mut stream = tokio::time::timeout(
        Duration::from_millis(RELAY_CONNECT_TIMEOUT_MS),
        TcpStream::connect(relay_addr),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Relay connection timeout"))??;

    info!("Connected to relay server at {}", relay_addr);

    // Send RequestRelay to relay server with our uuid, licence_key, and peer's socket_addr
    // The licence_key is required if the relay server is configured with -k option
    // The socket_addr is CRITICAL - the relay server uses it to match us with the peer
    let request_relay = make_request_relay(uuid, relay_key, socket_addr);
    let bytes = request_relay.write_to_bytes().map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
    bytes_codec::write_frame(&mut stream, &bytes).await?;
    debug!("Sent RequestRelay to relay server for uuid={}", uuid);

    // Decode peer address for logging
    let peer_addr = rendezvous::AddrMangle::decode(socket_addr).unwrap_or(relay_addr);

    // Step 3: Accept connection - relay server bridges the connection
    connection_manager.accept_connection(stream, peer_addr).await?;
    info!("Relay connection established for uuid={}, peer={}", uuid, peer_addr);

    Ok(())
}

/// Handle intranet/same-LAN connection request
///
/// When the server determines that the client and peer are on the same intranet
/// (same public IP or both on LAN), it sends FetchLocalAddr to the peer.
/// The peer must:
/// 1. Open a TCP connection to the rendezvous server
/// 2. Send LocalAddr with our local address
/// 3. Accept the peer connection over that same TCP stream
async fn handle_intranet_request(
    rendezvous_addr: &str,
    peer_socket_addr: &[u8],
    local_addr: SocketAddr,
    relay_server: &str,
    device_id: &str,
    connection_manager: Arc<ConnectionManager>,
) -> anyhow::Result<()> {
    info!(
        "Handling intranet request: rendezvous={}, local_addr={}, device_id={}",
        rendezvous_addr, local_addr, device_id
    );

    // Decode peer address for logging
    let peer_addr = AddrMangle::decode(peer_socket_addr);
    debug!("Peer address from FetchLocalAddr: {:?}", peer_addr);

    // Connect to rendezvous server via TCP with timeout
    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(rendezvous_addr),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Timeout connecting to rendezvous server"))??;

    info!("Connected to rendezvous server for intranet: {}", rendezvous_addr);

    // Build LocalAddr message with our local address (mangled)
    let local_addr_bytes = AddrMangle::encode(local_addr);
    let msg = make_local_addr(
        peer_socket_addr,
        &local_addr_bytes,
        relay_server,
        device_id,
        env!("CARGO_PKG_VERSION"),
    );
    let bytes = msg.write_to_bytes().map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;

    // Send LocalAddr using RustDesk's variable-length framing
    bytes_codec::write_frame(&mut stream, &bytes).await?;

    info!("Sent LocalAddr to rendezvous server, waiting for peer connection");

    // Now the rendezvous server will forward this to the client,
    // and the client will connect to us through this same TCP stream.
    // The server proxies the connection between client and peer.

    // Get peer address for logging/connection tracking
    let effective_peer_addr = peer_addr.unwrap_or_else(|| {
        // If we can't decode the peer address, use the rendezvous server address
        rendezvous_addr.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap())
    });

    // Accept the connection - the stream is now a proxied connection to the client
    connection_manager.accept_connection(stream, effective_peer_addr).await?;
    info!("Intranet connection established via rendezvous server proxy");

    Ok(())
}
