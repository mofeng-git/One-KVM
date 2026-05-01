//! HBBS UDP registration; punch / relay / intranet callbacks.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use protobuf::Message;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::utils::bind_udp_socket;

use super::config::RustDeskConfig;
use super::crypto::{KeyPair, SigningKeyPair};
use super::protocol::{
    decode_rendezvous_message, make_punch_hole_sent, make_register_peer, make_register_pk,
    rendezvous_message, NatType, RendezvousMessage,
};

const REG_INTERVAL_MS: u64 = 12_000;

const MIN_REG_TIMEOUT_MS: u64 = 3_000;

const MAX_REG_TIMEOUT_MS: u64 = 30_000;

const TIMER_INTERVAL_MS: u64 = 300;

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

pub type RelayCallback = Arc<dyn Fn(String, String, String, Vec<u8>, String) + Send + Sync>;

pub type PunchCallback =
    Arc<dyn Fn(Option<SocketAddr>, String, String, String, Vec<u8>, String) + Send + Sync>;

pub type IntranetCallback = Arc<dyn Fn(String, Vec<u8>, SocketAddr, String, String) + Send + Sync>;

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
    pub fn new(mut config: RustDeskConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

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

    pub fn set_listen_port(&self, port: u16) {
        let old_port = *self.listen_port.read();
        if old_port != port {
            *self.listen_port.write() = port;
            self.increment_serial();
        }
    }

    pub fn listen_port(&self) -> u16 {
        *self.listen_port.read()
    }

    pub fn increment_serial(&self) {
        let mut serial = self.serial.write();
        *serial = serial.wrapping_add(1);
        debug!("Serial incremented to {}", *serial);
    }

    pub fn serial(&self) -> i32 {
        *self.serial.read()
    }

    pub fn uuid_needs_save(&self) -> bool {
        *self.uuid_needs_save.read()
    }

    pub fn config(&self) -> RustDeskConfig {
        self.config.read().clone()
    }

    pub fn mark_uuid_saved(&self) {
        *self.uuid_needs_save.write() = false;
    }

    pub fn set_relay_callback(&self, callback: RelayCallback) {
        *self.relay_callback.write() = Some(callback);
    }

    pub fn set_punch_callback(&self, callback: PunchCallback) {
        *self.punch_callback.write() = Some(callback);
    }

    pub fn set_intranet_callback(&self, callback: IntranetCallback) {
        *self.intranet_callback.write() = Some(callback);
    }

    pub fn status(&self) -> RendezvousStatus {
        self.status.read().clone()
    }

    pub fn update_config(&self, config: RustDeskConfig) {
        *self.config.write() = config;
        self.increment_serial();
    }

    pub fn ensure_keypair(&self) -> KeyPair {
        let mut keypair_guard = self.keypair.write();
        if keypair_guard.is_none() {
            let config = self.config.read();
            if let (Some(pk), Some(sk)) = (&config.public_key, &config.private_key) {
                if let Ok(kp) = KeyPair::from_base64(pk, sk) {
                    *keypair_guard = Some(kp.clone());
                    return kp;
                }
            }
            let kp = KeyPair::generate();
            *keypair_guard = Some(kp.clone());
            kp
        } else {
            keypair_guard.as_ref().unwrap().clone()
        }
    }

    pub fn ensure_signing_keypair(&self) -> SigningKeyPair {
        let mut signing_guard = self.signing_keypair.write();
        if signing_guard.is_none() {
            let config = self.config.read();
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
            let skp = SigningKeyPair::generate();
            debug!("Generated new signing keypair");
            *signing_guard = Some(skp.clone());
            skp
        } else {
            signing_guard.as_ref().unwrap().clone()
        }
    }

    pub fn device_id(&self) -> String {
        self.config.read().device_id.clone()
    }

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

        let server_addr: SocketAddr = tokio::net::lookup_host(&addr)
            .await?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to resolve {}", addr))?;

        let bind_addr = match server_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let std_socket = bind_udp_socket(bind_addr)?;
        let socket = UdpSocket::from_std(std_socket)?;
        socket.connect(server_addr).await?;

        info!("Connected to rendezvous server at {}", server_addr);
        *self.status.write() = RendezvousStatus::Connected;

        self.registration_loop(socket).await
    }

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

                _ = shutdown_rx.recv() => {
                    info!("Rendezvous mediator shutting down");
                    break;
                }
            }
        }

        *self.status.write() = RendezvousStatus::Disconnected;
        Ok(())
    }

    async fn send_register(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        let key_confirmed = *self.key_confirmed.read();

        if !key_confirmed {
            self.send_register_pk(socket).await
        } else {
            self.send_register_peer(socket).await
        }
    }

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

    async fn send_register_pk(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        let id = self.device_id();
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

    async fn send_local_addr(
        &self,
        _udp_socket: &UdpSocket,
        peer_socket_addr: &[u8],
        relay_server: &str,
    ) -> anyhow::Result<()> {
        let id = self.device_id();

        let local_addrs = get_local_addresses();
        if local_addrs.is_empty() {
            debug!("No local addresses available for LocalAddr response");
            return Ok(());
        }

        let config = self.config.read().clone();
        let rendezvous_addr = config.rendezvous_addr();

        let listen_port = self.listen_port();

        let local_ip = local_addrs[0];
        let local_sock_addr = SocketAddr::new(local_ip, listen_port);

        info!(
            "FetchLocalAddr: calling intranet callback with local_addr={}, rendezvous={}",
            local_sock_addr, rendezvous_addr
        );

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
                        info!("✓ Public key registered successfully with server");
                        *self.key_confirmed.write() = true;
                        self.increment_serial();
                        *self.status.write() = RendezvousStatus::Registered;
                    }
                    2 => {
                        warn!("UUID mismatch, need to re-register");
                        *self.key_confirmed.write() = false;
                    }
                    3 => {
                        error!("Device ID already exists on server");
                        *self.status.write() =
                            RendezvousStatus::Error("Device ID already exists".to_string());
                    }
                    4 => {
                        warn!("Registration too frequent");
                    }
                    5 => {
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
                let config = self.config.read().clone();
                let effective_relay_server =
                    select_relay_server(config.relay_server.as_deref(), &ph.relay_server);

                let peer_addr = if !ph.socket_addr.is_empty() {
                    AddrMangle::decode(&ph.socket_addr)
                } else {
                    None
                };

                info!(
                    "Received PunchHole request: peer_addr={:?}, socket_addr_len={}, relay_server={}, effective_relay_server={}, nat_type={:?}",
                    peer_addr,
                    ph.socket_addr.len(),
                    ph.relay_server,
                    effective_relay_server.as_deref().unwrap_or(""),
                    ph.nat_type
                );

                // IMPORTANT: socket_addr in PunchHoleSent should be the PEER's address (from PunchHole),
                let id = self.device_id();

                info!(
                    "Sending PunchHoleSent: id={}, peer_addr={:?}, relay_server={}",
                    id,
                    peer_addr,
                    effective_relay_server
                        .as_deref()
                        .unwrap_or(ph.relay_server.as_str())
                );

                let msg = make_punch_hole_sent(
                    &ph.socket_addr, // Use peer's socket_addr, not ours
                    &id,
                    effective_relay_server
                        .as_deref()
                        .unwrap_or(ph.relay_server.as_str()),
                    ph.nat_type.enum_value().unwrap_or(NatType::UNKNOWN_NAT),
                    env!("CARGO_PKG_VERSION"),
                );
                let bytes = msg.write_to_bytes().unwrap_or_default();
                if let Err(e) = socket.send(&bytes).await {
                    warn!("Failed to send PunchHoleSent: {}", e);
                } else {
                    info!("Sent PunchHoleSent response successfully");
                }

                if let Some(relay_server) = effective_relay_server {
                    let uuid = uuid::Uuid::new_v4().to_string();
                    let rendezvous_addr = config.rendezvous_addr();
                    let device_id = config.device_id.clone();

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
                } else {
                    debug!("No relay server available for PunchHole, skipping relay fallback");
                }
            }
            Some(rendezvous_message::Union::RequestRelay(rr)) => {
                let config = self.config.read().clone();
                let effective_relay_server =
                    select_relay_server(config.relay_server.as_deref(), &rr.relay_server);

                info!(
                    "Received RequestRelay: relay_server={}, effective_relay_server={}, uuid={}, secure={}",
                    rr.relay_server,
                    effective_relay_server.as_deref().unwrap_or(""),
                    rr.uuid,
                    rr.secure
                );
                if let Some(callback) = self.relay_callback.read().as_ref() {
                    if let Some(relay_server) = effective_relay_server {
                        let rendezvous_addr = config.rendezvous_addr();
                        let device_id = config.device_id.clone();
                        callback(
                            rendezvous_addr,
                            relay_server,
                            rr.uuid.clone(),
                            rr.socket_addr.to_vec(),
                            device_id,
                        );
                    } else {
                        debug!("No relay server available for RequestRelay callback");
                    }
                }
            }
            Some(rendezvous_message::Union::FetchLocalAddr(fla)) => {
                let config = self.config.read().clone();
                let effective_relay_server =
                    select_relay_server(config.relay_server.as_deref(), &fla.relay_server)
                        .unwrap_or_default();

                let peer_addr = AddrMangle::decode(&fla.socket_addr);
                info!(
                    "Received FetchLocalAddr request: peer_addr={:?}, socket_addr_len={}, relay_server={}, effective_relay_server={}",
                    peer_addr,
                    fla.socket_addr.len(),
                    fla.relay_server,
                    effective_relay_server
                );
                self.send_local_addr(socket, &fla.socket_addr, &effective_relay_server)
                    .await?;
            }
            Some(rendezvous_message::Union::ConfigureUpdate(cu)) => {
                info!("Received ConfigureUpdate, serial={}", cu.serial);
                *self.serial.write() = cu.serial;
            }
            Some(other) => {
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

    pub fn stop(&self) {
        info!("Stopping rendezvous mediator");
        let _ = self.shutdown_tx.send(());
        *self.status.write() = RendezvousStatus::Disconnected;
    }

    pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
}

/// RustDesk mangled socket encoding.
pub struct AddrMangle;

fn normalize_relay_server(server: &str) -> Option<String> {
    let trimmed = server.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains(':') {
        Some(trimmed.to_string())
    } else {
        Some(format!("{}:21117", trimmed))
    }
}

fn select_relay_server(local_relay: Option<&str>, server_relay: &str) -> Option<String> {
    local_relay
        .and_then(normalize_relay_server)
        .or_else(|| normalize_relay_server(server_relay))
}

impl AddrMangle {
    pub fn encode(addr: SocketAddr) -> Vec<u8> {
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

    pub fn decode(bytes: &[u8]) -> Option<SocketAddr> {
        use std::convert::TryInto;
        use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4};

        if bytes.len() > 16 {
            if bytes.len() != 18 {
                return None;
            }
            let tmp: [u8; 2] = bytes[16..].try_into().ok()?;
            let port = u16::from_le_bytes(tmp);
            let tmp: [u8; 16] = bytes[..16].try_into().ok()?;
            let ip = Ipv6Addr::from(tmp);
            return Some(SocketAddr::new(std::net::IpAddr::V6(ip), port));
        }

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

fn is_virtual_interface(name: &str) -> bool {
    name.starts_with("docker")
        || name.starts_with("br-")
        || name.starts_with("veth")
        || name.starts_with("cni")
        || name.starts_with("flannel")
        || name.starts_with("calico")
        || name.starts_with("weave")
        || name.starts_with("virbr")
        || name.starts_with("lxcbr")
        || name.starts_with("lxdbr")
        || name.starts_with("tun")
        || name.starts_with("tap")
}

fn is_docker_ip(ip: &std::net::IpAddr) -> bool {
    if let std::net::IpAddr::V4(ipv4) = ip {
        let octets = ipv4.octets();
        if octets[0] == 172 && octets[1] == 17 {
            return true;
        }
        if octets[0] == 172 && (18..=31).contains(&octets[1]) {
            return true;
        }
        if octets[0] == 10 && octets[1] == 0 && octets[2] < 10 {
            return true;
        }
    }
    false
}

fn get_local_addresses() -> Vec<std::net::IpAddr> {
    let mut addrs = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if let Ok(interfaces) = std::fs::read_dir("/sys/class/net") {
            for entry in interfaces.flatten() {
                let iface_name = entry.file_name().to_string_lossy().to_string();
                if iface_name == "lo" || is_virtual_interface(&iface_name) {
                    continue;
                }

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

    if addrs.is_empty() {
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if socket.connect("8.8.8.8:53").is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip();
                    if !ip.is_loopback() && !is_docker_ip(&ip) {
                        addrs.push(ip);
                    }
                }
            }
        }
    }

    addrs
}

#[cfg(test)]
mod tests {
    use super::{normalize_relay_server, select_relay_server};

    #[test]
    fn test_normalize_relay_server() {
        assert_eq!(normalize_relay_server(""), None);
        assert_eq!(normalize_relay_server("   "), None);
        assert_eq!(
            normalize_relay_server("relay.example.com"),
            Some("relay.example.com:21117".to_string())
        );
        assert_eq!(
            normalize_relay_server("relay.example.com:22117"),
            Some("relay.example.com:22117".to_string())
        );
    }

    #[test]
    fn test_select_relay_server_prefers_local() {
        assert_eq!(
            select_relay_server(Some("local.example.com:21117"), "server.example.com:21117"),
            Some("local.example.com:21117".to_string())
        );

        assert_eq!(
            select_relay_server(Some("local.example.com"), "server.example.com:21117"),
            Some("local.example.com:21117".to_string())
        );

        assert_eq!(
            select_relay_server(Some("   "), "server.example.com"),
            Some("server.example.com:21117".to_string())
        );

        assert_eq!(
            select_relay_server(None, "server.example.com:21117"),
            Some("server.example.com:21117".to_string())
        );

        assert_eq!(select_relay_server(None, ""), None);
    }
}
