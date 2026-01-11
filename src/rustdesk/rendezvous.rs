//! RustDesk Rendezvous Mediator
//!
//! This module handles communication with the hbbs rendezvous server.
//! It registers the device ID and public key, handles punch hole requests,
//! and relay requests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use protobuf::Message;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::config::RustDeskConfig;
use super::crypto::{KeyPair, SigningKeyPair};
use super::protocol::{
    decode_rendezvous_message, make_punch_hole_sent, make_register_peer, make_register_pk,
    rendezvous_message, NatType, RendezvousMessage,
};

/// Registration interval in milliseconds
const REG_INTERVAL_MS: u64 = 12_000;

/// Minimum registration timeout
const MIN_REG_TIMEOUT_MS: u64 = 3_000;

/// Maximum registration timeout
const MAX_REG_TIMEOUT_MS: u64 = 30_000;

/// Timer interval for checking registration status
const TIMER_INTERVAL_MS: u64 = 300;

/// Rendezvous mediator status
#[derive(Debug, Clone, PartialEq)]
pub enum RendezvousStatus {
    Disconnected,
    Connecting,
    Connected,
    Registered,
    Error(String),
}

impl std::fmt::Display for RendezvousStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
            Self::Registered => write!(f, "registered"),
            Self::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// Callback for handling incoming connection requests
pub type ConnectionCallback = Arc<dyn Fn(ConnectionRequest) + Send + Sync>;

/// Incoming connection request from a RustDesk client
#[derive(Debug, Clone)]
pub struct ConnectionRequest {
    /// Peer socket address (encoded)
    pub socket_addr: Vec<u8>,
    /// Relay server to use
    pub relay_server: String,
    /// NAT type
    pub nat_type: NatType,
    /// Connection UUID
    pub uuid: String,
    /// Whether to use secure connection
    pub secure: bool,
}

/// Callback type for relay requests
/// Parameters: rendezvous_addr, relay_server, uuid, socket_addr (client's mangled address), device_id
pub type RelayCallback = Arc<dyn Fn(String, String, String, Vec<u8>, String) + Send + Sync>;

/// Callback type for P2P punch hole requests
/// Parameters: peer_addr (decoded), relay_callback_params (rendezvous_addr, relay_server, uuid, socket_addr, device_id)
/// Returns: should call relay callback if P2P fails
pub type PunchCallback =
    Arc<dyn Fn(Option<SocketAddr>, String, String, String, Vec<u8>, String) + Send + Sync>;

/// Callback type for intranet/local address connections
/// Parameters: rendezvous_addr, peer_socket_addr (mangled), local_addr, relay_server, device_id
pub type IntranetCallback = Arc<dyn Fn(String, Vec<u8>, SocketAddr, String, String) + Send + Sync>;

/// Rendezvous Mediator
///
/// Handles communication with hbbs rendezvous server:
/// - Registers device ID and public key
/// - Maintains keep-alive with server
/// - Handles punch hole and relay requests
pub struct RendezvousMediator {
    config: Arc<RwLock<RustDeskConfig>>,
    keypair: Arc<RwLock<Option<KeyPair>>>,
    signing_keypair: Arc<RwLock<Option<SigningKeyPair>>>,
    status: Arc<RwLock<RendezvousStatus>>,
    uuid: Arc<RwLock<[u8; 16]>>,
    uuid_needs_save: Arc<RwLock<bool>>,
    serial: Arc<RwLock<i32>>,
    key_confirmed: Arc<RwLock<bool>>,
    keep_alive_ms: Arc<RwLock<i32>>,
    relay_callback: Arc<RwLock<Option<RelayCallback>>>,
    punch_callback: Arc<RwLock<Option<PunchCallback>>>,
    intranet_callback: Arc<RwLock<Option<IntranetCallback>>>,
    listen_port: Arc<RwLock<u16>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RendezvousMediator {
    /// Create a new rendezvous mediator
    pub fn new(mut config: RustDeskConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        // Get or generate UUID from config (persisted)
        let (uuid, uuid_needs_save) = config.ensure_uuid();

        Self {
            config: Arc::new(RwLock::new(config)),
            keypair: Arc::new(RwLock::new(None)),
            signing_keypair: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(RendezvousStatus::Disconnected)),
            uuid: Arc::new(RwLock::new(uuid)),
            uuid_needs_save: Arc::new(RwLock::new(uuid_needs_save)),
            serial: Arc::new(RwLock::new(0)),
            key_confirmed: Arc::new(RwLock::new(false)),
            keep_alive_ms: Arc::new(RwLock::new(30_000)),
            relay_callback: Arc::new(RwLock::new(None)),
            punch_callback: Arc::new(RwLock::new(None)),
            intranet_callback: Arc::new(RwLock::new(None)),
            listen_port: Arc::new(RwLock::new(21118)),
            shutdown_tx,
        }
    }

    /// Set the TCP listen port for direct connections
    pub fn set_listen_port(&self, port: u16) {
        let old_port = *self.listen_port.read();
        if old_port != port {
            *self.listen_port.write() = port;
            // Port changed, increment serial to notify server
            self.increment_serial();
        }
    }

    /// Get the TCP listen port
    pub fn listen_port(&self) -> u16 {
        *self.listen_port.read()
    }

    /// Increment the serial number to indicate local state change
    pub fn increment_serial(&self) {
        let mut serial = self.serial.write();
        *serial = serial.wrapping_add(1);
        debug!("Serial incremented to {}", *serial);
    }

    /// Get current serial number
    pub fn serial(&self) -> i32 {
        *self.serial.read()
    }

    /// Check if UUID needs to be saved to persistent storage
    pub fn uuid_needs_save(&self) -> bool {
        *self.uuid_needs_save.read()
    }

    /// Get the current config (with UUID set)
    pub fn config(&self) -> RustDeskConfig {
        self.config.read().clone()
    }

    /// Mark UUID as saved
    pub fn mark_uuid_saved(&self) {
        *self.uuid_needs_save.write() = false;
    }

    /// Set the callback for relay requests
    pub fn set_relay_callback(&self, callback: RelayCallback) {
        *self.relay_callback.write() = Some(callback);
    }

    /// Set the callback for P2P punch hole requests
    pub fn set_punch_callback(&self, callback: PunchCallback) {
        *self.punch_callback.write() = Some(callback);
    }

    /// Set the callback for intranet/local address connections
    pub fn set_intranet_callback(&self, callback: IntranetCallback) {
        *self.intranet_callback.write() = Some(callback);
    }

    /// Get current status
    pub fn status(&self) -> RendezvousStatus {
        self.status.read().clone()
    }

    /// Update configuration
    pub fn update_config(&self, config: RustDeskConfig) {
        *self.config.write() = config;
        // Config changed, increment serial to notify server
        self.increment_serial();
    }

    /// Initialize or get keypair (Curve25519 for encryption)
    pub fn ensure_keypair(&self) -> KeyPair {
        let mut keypair_guard = self.keypair.write();
        if keypair_guard.is_none() {
            let config = self.config.read();
            // Try to load from config first
            if let (Some(pk), Some(sk)) = (&config.public_key, &config.private_key) {
                if let Ok(kp) = KeyPair::from_base64(pk, sk) {
                    *keypair_guard = Some(kp.clone());
                    return kp;
                }
            }
            // Generate new keypair
            let kp = KeyPair::generate();
            *keypair_guard = Some(kp.clone());
            kp
        } else {
            keypair_guard.as_ref().unwrap().clone()
        }
    }

    /// Initialize or get signing keypair (Ed25519 for SignedId)
    pub fn ensure_signing_keypair(&self) -> SigningKeyPair {
        let mut signing_guard = self.signing_keypair.write();
        if signing_guard.is_none() {
            let config = self.config.read();
            // Try to load from config first
            if let (Some(pk), Some(sk)) = (&config.signing_public_key, &config.signing_private_key)
            {
                if let Ok(skp) = SigningKeyPair::from_base64(pk, sk) {
                    debug!("Loaded signing keypair from config");
                    *signing_guard = Some(skp.clone());
                    return skp;
                } else {
                    warn!("Failed to decode signing keypair from config, generating new one");
                }
            }
            // Generate new signing keypair
            let skp = SigningKeyPair::generate();
            debug!("Generated new signing keypair");
            *signing_guard = Some(skp.clone());
            skp
        } else {
            signing_guard.as_ref().unwrap().clone()
        }
    }

    /// Get the device ID
    pub fn device_id(&self) -> String {
        self.config.read().device_id.clone()
    }

    /// Start the rendezvous mediator
    pub async fn start(&self) -> anyhow::Result<()> {
        let config = self.config.read().clone();
        let effective_server = config.effective_rendezvous_server();
        debug!(
            "RendezvousMediator.start(): enabled={}, server='{}'",
            config.enabled, effective_server
        );
        if !config.enabled || effective_server.is_empty() {
            info!(
                "Rendezvous mediator not starting: enabled={}, server='{}'",
                config.enabled, effective_server
            );
            return Ok(());
        }

        *self.status.write() = RendezvousStatus::Connecting;

        let addr = config.rendezvous_addr();
        info!(
            "Starting rendezvous mediator for {} to {}",
            config.device_id, addr
        );

        // Resolve server address
        let server_addr: SocketAddr = tokio::net::lookup_host(&addr)
            .await?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to resolve {}", addr))?;

        // Create UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;

        info!("Connected to rendezvous server at {}", server_addr);
        *self.status.write() = RendezvousStatus::Connected;

        // Start registration loop
        self.registration_loop(socket).await
    }

    /// Main registration loop
    async fn registration_loop(&self, socket: UdpSocket) -> anyhow::Result<()> {
        let mut timer = interval(Duration::from_millis(TIMER_INTERVAL_MS));
        let mut recv_buf = vec![0u8; 65535];
        let mut last_register_sent: Option<Instant> = None;
        let mut last_register_resp: Option<Instant> = None;
        let mut reg_timeout = MIN_REG_TIMEOUT_MS;
        let mut fails = 0;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                // Handle incoming messages
                result = socket.recv(&mut recv_buf) => {
                    match result {
                        Ok(len) => {
                            if let Ok(msg) = decode_rendezvous_message(&recv_buf[..len]) {
                                self.handle_response(&socket, msg, &mut last_register_resp, &mut fails, &mut reg_timeout).await?;
                            } else {
                                debug!("Failed to decode rendezvous message");
                            }
                        }
                        Err(e) => {
                            error!("Failed to receive from socket: {}", e);
                            *self.status.write() = RendezvousStatus::Error(e.to_string());
                            break;
                        }
                    }
                }

                // Periodic registration
                _ = timer.tick() => {
                    let now = Instant::now();
                    let expired = last_register_resp
                        .map(|x| x.elapsed().as_millis() as u64 >= REG_INTERVAL_MS)
                        .unwrap_or(true);
                    let timeout = last_register_sent
                        .map(|x| x.elapsed().as_millis() as u64 >= reg_timeout)
                        .unwrap_or(false);

                    if timeout && reg_timeout < MAX_REG_TIMEOUT_MS {
                        reg_timeout += MIN_REG_TIMEOUT_MS;
                        fails += 1;
                        if fails >= 4 {
                            warn!("Registration timeout, {} consecutive failures", fails);
                        }
                    }

                    if timeout || (last_register_sent.is_none() && expired) {
                        self.send_register(&socket).await?;
                        last_register_sent = Some(now);
                    }
                }

                // Shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Rendezvous mediator shutting down");
                    break;
                }
            }
        }

        *self.status.write() = RendezvousStatus::Disconnected;
        Ok(())
    }

    /// Send registration message
    async fn send_register(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        let key_confirmed = *self.key_confirmed.read();

        if !key_confirmed {
            // Send RegisterPk with public key
            self.send_register_pk(socket).await
        } else {
            // Send RegisterPeer heartbeat
            self.send_register_peer(socket).await
        }
    }

    /// Send RegisterPeer message
    async fn send_register_peer(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        let id = self.device_id();
        let serial = *self.serial.read();

        let msg = make_register_peer(&id, serial);
        let bytes = msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        socket.send(&bytes).await?;
        Ok(())
    }

    /// Send RegisterPk message
    /// Uses the Ed25519 signing public key for registration
    async fn send_register_pk(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        let id = self.device_id();
        // Use signing public key (Ed25519) for RegisterPk
        // This is what clients will use to verify our SignedId signature
        let signing_keypair = self.ensure_signing_keypair();
        let pk = signing_keypair.public_key_bytes();
        let uuid = *self.uuid.read();

        debug!("Sending RegisterPk: id={}", id);
        let msg = make_register_pk(&id, &uuid, pk, "");
        let bytes = msg
            .write_to_bytes()
            .map_err(|e| anyhow::anyhow!("Failed to encode: {}", e))?;
        socket.send(&bytes).await?;
        Ok(())
    }

    /// Handle FetchLocalAddr - send to callback for proper TCP handling
    ///
    /// The intranet callback will:
    /// 1. Open a TCP connection to the rendezvous server
    /// 2. Send LocalAddr message
    /// 3. Accept the peer connection over that same TCP stream
    async fn send_local_addr(
        &self,
        _udp_socket: &UdpSocket,
        peer_socket_addr: &[u8],
        relay_server: &str,
    ) -> anyhow::Result<()> {
        let id = self.device_id();

        // Get our actual local IP addresses for same-LAN connection
        let local_addrs = get_local_addresses();
        if local_addrs.is_empty() {
            debug!("No local addresses available for LocalAddr response");
            return Ok(());
        }

        // Get the rendezvous server address for TCP connection
        let config = self.config.read().clone();
        let rendezvous_addr = config.rendezvous_addr();

        // Use TCP listen port for direct connections
        let listen_port = self.listen_port();

        // Use the first local IP
        let local_ip = local_addrs[0];
        let local_sock_addr = SocketAddr::new(local_ip, listen_port);

        info!(
            "FetchLocalAddr: calling intranet callback with local_addr={}, rendezvous={}",
            local_sock_addr, rendezvous_addr
        );

        // Call the intranet callback if set
        if let Some(callback) = self.intranet_callback.read().as_ref() {
            callback(
                rendezvous_addr,
                peer_socket_addr.to_vec(),
                local_sock_addr,
                relay_server.to_string(),
                id,
            );
        } else {
            warn!("No intranet callback set, cannot handle FetchLocalAddr properly");
        }

        Ok(())
    }

    /// Handle response from rendezvous server
    async fn handle_response(
        &self,
        socket: &UdpSocket,
        msg: RendezvousMessage,
        last_resp: &mut Option<Instant>,
        fails: &mut i32,
        reg_timeout: &mut u64,
    ) -> anyhow::Result<()> {
        *last_resp = Some(Instant::now());
        *fails = 0;
        *reg_timeout = MIN_REG_TIMEOUT_MS;

        match msg.union {
            Some(rendezvous_message::Union::RegisterPeerResponse(rpr)) => {
                if rpr.request_pk {
                    // Server wants us to register our public key
                    info!("Server requested public key registration");
                    *self.key_confirmed.write() = false;
                    self.send_register_pk(socket).await?;
                }
                *self.status.write() = RendezvousStatus::Registered;
            }
            Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
                info!("Received RegisterPkResponse: result={:?}", rpr.result);
                match rpr.result.value() {
                    0 => {
                        // OK
                        info!("✓ Public key registered successfully with server");
                        *self.key_confirmed.write() = true;
                        // Increment serial after successful registration
                        self.increment_serial();
                        *self.status.write() = RendezvousStatus::Registered;
                    }
                    2 => {
                        // UUID_MISMATCH
                        warn!("UUID mismatch, need to re-register");
                        *self.key_confirmed.write() = false;
                    }
                    3 => {
                        // ID_EXISTS
                        error!("Device ID already exists on server");
                        *self.status.write() =
                            RendezvousStatus::Error("Device ID already exists".to_string());
                    }
                    4 => {
                        // TOO_FREQUENT
                        warn!("Registration too frequent");
                    }
                    5 => {
                        // INVALID_ID_FORMAT
                        error!("Invalid device ID format");
                        *self.status.write() =
                            RendezvousStatus::Error("Invalid ID format".to_string());
                    }
                    _ => {
                        error!("Unknown RegisterPkResponse result: {:?}", rpr.result);
                    }
                }

                if rpr.keep_alive > 0 {
                    *self.keep_alive_ms.write() = rpr.keep_alive * 1000;
                    debug!("Keep alive set to {}ms", rpr.keep_alive * 1000);
                }
            }
            Some(rendezvous_message::Union::PunchHole(ph)) => {
                // Decode the peer's socket address
                let peer_addr = if !ph.socket_addr.is_empty() {
                    AddrMangle::decode(&ph.socket_addr)
                } else {
                    None
                };

                info!(
                    "Received PunchHole request: peer_addr={:?}, socket_addr_len={}, relay_server={}, nat_type={:?}",
                    peer_addr, ph.socket_addr.len(), ph.relay_server, ph.nat_type
                );

                // Send PunchHoleSent to acknowledge
                // IMPORTANT: socket_addr in PunchHoleSent should be the PEER's address (from PunchHole),
                // not our own address. This is how RustDesk protocol works.
                let id = self.device_id();

                info!(
                    "Sending PunchHoleSent: id={}, peer_addr={:?}, relay_server={}",
                    id, peer_addr, ph.relay_server
                );

                let msg = make_punch_hole_sent(
                    &ph.socket_addr.to_vec(), // Use peer's socket_addr, not ours
                    &id,
                    &ph.relay_server,
                    ph.nat_type.enum_value().unwrap_or(NatType::UNKNOWN_NAT),
                    env!("CARGO_PKG_VERSION"),
                );
                let bytes = msg.write_to_bytes().unwrap_or_default();
                if let Err(e) = socket.send(&bytes).await {
                    warn!("Failed to send PunchHoleSent: {}", e);
                } else {
                    info!("Sent PunchHoleSent response successfully");
                }

                // Try P2P direct connection first, fall back to relay if needed
                if !ph.relay_server.is_empty() {
                    let relay_server = if ph.relay_server.contains(':') {
                        ph.relay_server.clone()
                    } else {
                        format!("{}:21117", ph.relay_server)
                    };
                    // Generate a standard UUID v4 for relay pairing
                    // This must match the format used by RustDesk client
                    let uuid = uuid::Uuid::new_v4().to_string();
                    let config = self.config.read().clone();
                    let rendezvous_addr = config.rendezvous_addr();
                    let device_id = config.device_id.clone();

                    // Use punch callback if set (tries P2P first, then relay)
                    // Otherwise fall back to relay callback directly
                    if let Some(callback) = self.punch_callback.read().as_ref() {
                        callback(
                            peer_addr,
                            rendezvous_addr,
                            relay_server,
                            uuid,
                            ph.socket_addr.to_vec(),
                            device_id,
                        );
                    } else if let Some(callback) = self.relay_callback.read().as_ref() {
                        callback(
                            rendezvous_addr,
                            relay_server,
                            uuid,
                            ph.socket_addr.to_vec(),
                            device_id,
                        );
                    }
                }
            }
            Some(rendezvous_message::Union::RequestRelay(rr)) => {
                info!(
                    "Received RequestRelay: relay_server={}, uuid={}, secure={}",
                    rr.relay_server, rr.uuid, rr.secure
                );
                // Call the relay callback to handle the connection
                if let Some(callback) = self.relay_callback.read().as_ref() {
                    let relay_server = if rr.relay_server.contains(':') {
                        rr.relay_server.clone()
                    } else {
                        format!("{}:21117", rr.relay_server)
                    };
                    let config = self.config.read().clone();
                    let rendezvous_addr = config.rendezvous_addr();
                    let device_id = config.device_id.clone();
                    callback(
                        rendezvous_addr,
                        relay_server,
                        rr.uuid.clone(),
                        rr.socket_addr.to_vec(),
                        device_id,
                    );
                }
            }
            Some(rendezvous_message::Union::FetchLocalAddr(fla)) => {
                // Decode the peer address for logging
                let peer_addr = AddrMangle::decode(&fla.socket_addr);
                info!(
                    "Received FetchLocalAddr request: peer_addr={:?}, socket_addr_len={}, relay_server={}",
                    peer_addr, fla.socket_addr.len(), fla.relay_server
                );
                // Respond with our local address for same-LAN direct connection
                self.send_local_addr(socket, &fla.socket_addr, &fla.relay_server)
                    .await?;
            }
            Some(rendezvous_message::Union::ConfigureUpdate(cu)) => {
                info!("Received ConfigureUpdate, serial={}", cu.serial);
                *self.serial.write() = cu.serial;
            }
            Some(other) => {
                // Log the actual message type for debugging
                let type_name = match other {
                    rendezvous_message::Union::PunchHoleRequest(_) => "PunchHoleRequest",
                    rendezvous_message::Union::PunchHoleResponse(_) => "PunchHoleResponse",
                    rendezvous_message::Union::SoftwareUpdate(_) => "SoftwareUpdate",
                    rendezvous_message::Union::TestNatRequest(_) => "TestNatRequest",
                    rendezvous_message::Union::TestNatResponse(_) => "TestNatResponse",
                    rendezvous_message::Union::PeerDiscovery(_) => "PeerDiscovery",
                    rendezvous_message::Union::OnlineRequest(_) => "OnlineRequest",
                    rendezvous_message::Union::OnlineResponse(_) => "OnlineResponse",
                    rendezvous_message::Union::KeyExchange(_) => "KeyExchange",
                    rendezvous_message::Union::Hc(_) => "HealthCheck",
                    rendezvous_message::Union::RelayResponse(_) => "RelayResponse",
                    _ => "Other",
                };
                info!("Received unhandled rendezvous message type: {}", type_name);
            }
            None => {
                debug!("Received empty rendezvous message");
            }
        }

        Ok(())
    }

    /// Stop the rendezvous mediator
    pub fn stop(&self) {
        info!("Stopping rendezvous mediator");
        let _ = self.shutdown_tx.send(());
        *self.status.write() = RendezvousStatus::Disconnected;
    }

    /// Get a shutdown receiver
    pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
}

/// AddrMangle - RustDesk's address encoding scheme
///
/// Certain routers and firewalls scan packets and modify IP addresses.
/// This encoding mangles the address to avoid detection.
pub struct AddrMangle;

impl AddrMangle {
    /// Encode a SocketAddr to bytes using RustDesk's mangle algorithm
    pub fn encode(addr: SocketAddr) -> Vec<u8> {
        // Try to convert IPv6-mapped IPv4 to plain IPv4
        let addr = try_into_v4(addr);

        match addr {
            SocketAddr::V4(addr_v4) => {
                use std::time::{SystemTime, UNIX_EPOCH};

                let tm = (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_micros() as u32) as u128;
                let ip = u32::from_le_bytes(addr_v4.ip().octets()) as u128;
                let port = addr.port() as u128;
                let v = ((ip + tm) << 49) | (tm << 17) | (port + (tm & 0xFFFF));
                let bytes = v.to_le_bytes();

                // Remove trailing zeros
                let mut n_padding = 0;
                for i in bytes.iter().rev() {
                    if *i == 0u8 {
                        n_padding += 1;
                    } else {
                        break;
                    }
                }
                bytes[..(16 - n_padding)].to_vec()
            }
            SocketAddr::V6(addr_v6) => {
                let mut x = addr_v6.ip().octets().to_vec();
                let port: [u8; 2] = addr_v6.port().to_le_bytes();
                x.push(port[0]);
                x.push(port[1]);
                x
            }
        }
    }

    /// Decode bytes to SocketAddr using RustDesk's mangle algorithm
    pub fn decode(bytes: &[u8]) -> Option<SocketAddr> {
        use std::convert::TryInto;
        use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4};

        if bytes.len() > 16 {
            // IPv6 format: 16 bytes IP + 2 bytes port
            if bytes.len() != 18 {
                return None;
            }
            let tmp: [u8; 2] = bytes[16..].try_into().ok()?;
            let port = u16::from_le_bytes(tmp);
            let tmp: [u8; 16] = bytes[..16].try_into().ok()?;
            let ip = Ipv6Addr::from(tmp);
            return Some(SocketAddr::new(std::net::IpAddr::V6(ip), port));
        }

        // IPv4 mangled format
        let mut padded = [0u8; 16];
        padded[..bytes.len()].copy_from_slice(bytes);
        let number = u128::from_le_bytes(padded);
        let tm = (number >> 17) & (u32::MAX as u128);
        let ip = (((number >> 49).wrapping_sub(tm)) as u32).to_le_bytes();
        let port = ((number & 0xFFFFFF).wrapping_sub(tm & 0xFFFF)) as u16;
        Some(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]),
            port,
        )))
    }
}

/// Try to convert IPv6-mapped IPv4 address to plain IPv4
fn try_into_v4(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V6(v6) if !addr.ip().is_loopback() => {
            if let Some(ipv4) = v6.ip().to_ipv4_mapped() {
                return SocketAddr::new(std::net::IpAddr::V4(ipv4), v6.port());
            }
        }
        _ => {}
    }
    addr
}

/// Check if an interface name belongs to Docker or other virtual networks
fn is_virtual_interface(name: &str) -> bool {
    // Docker interfaces
    name.starts_with("docker")
        || name.starts_with("br-")
        || name.starts_with("veth")
        // Kubernetes/container interfaces
        || name.starts_with("cni")
        || name.starts_with("flannel")
        || name.starts_with("calico")
        || name.starts_with("weave")
        // Virtual bridge interfaces
        || name.starts_with("virbr")
        || name.starts_with("lxcbr")
        || name.starts_with("lxdbr")
        // VPN interfaces (usually not useful for LAN discovery)
        || name.starts_with("tun")
        || name.starts_with("tap")
}

/// Check if an IP address is in a Docker/container private range
fn is_docker_ip(ip: &std::net::IpAddr) -> bool {
    if let std::net::IpAddr::V4(ipv4) = ip {
        let octets = ipv4.octets();
        // Docker default bridge: 172.17.0.0/16
        if octets[0] == 172 && octets[1] == 17 {
            return true;
        }
        // Docker user-defined networks: 172.18-31.0.0/16
        if octets[0] == 172 && (18..=31).contains(&octets[1]) {
            return true;
        }
        // Docker overlay networks: 10.0.0.0/8 (common range)
        // Note: 10.x.x.x is also used for corporate LANs, so we only filter
        // specific Docker-like patterns (10.0.x.x with small third octet)
        if octets[0] == 10 && octets[1] == 0 && octets[2] < 10 {
            return true;
        }
    }
    false
}

/// Get local IP addresses (non-loopback, non-Docker)
fn get_local_addresses() -> Vec<std::net::IpAddr> {
    let mut addrs = Vec::new();

    // Use pnet or network-interface crate if available, otherwise use simple method
    #[cfg(target_os = "linux")]
    {
        if let Ok(interfaces) = std::fs::read_dir("/sys/class/net") {
            for entry in interfaces.flatten() {
                let iface_name = entry.file_name().to_string_lossy().to_string();
                // Skip loopback and virtual interfaces
                if iface_name == "lo" || is_virtual_interface(&iface_name) {
                    continue;
                }

                // Try to get IP via command (simple approach)
                if let Ok(output) = std::process::Command::new("ip")
                    .args(["-4", "addr", "show", &iface_name])
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if let Some(inet_pos) = line.find("inet ") {
                            let ip_part = &line[inet_pos + 5..];
                            if let Some(slash_pos) = ip_part.find('/') {
                                if let Ok(ip) = ip_part[..slash_pos].parse::<std::net::IpAddr>() {
                                    // Skip loopback and Docker IPs
                                    if !ip.is_loopback() && !is_docker_ip(&ip) {
                                        addrs.push(ip);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: try to get default route interface IP
    if addrs.is_empty() {
        // Try using DNS lookup to get local IP (connects to external server)
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            // Connect to a public DNS server (doesn't actually send data)
            if socket.connect("8.8.8.8:53").is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip();
                    // Skip loopback and Docker IPs
                    if !ip.is_loopback() && !is_docker_ip(&ip) {
                        addrs.push(ip);
                    }
                }
            }
        }
    }

    addrs
}
