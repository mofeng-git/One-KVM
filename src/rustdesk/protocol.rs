//! RustDesk Protocol Messages
//!
//! This module provides the compiled protobuf messages for the RustDesk protocol.
//! Messages are generated from rendezvous.proto and message.proto at build time.
//! Uses protobuf-rust (same as RustDesk server) for full compatibility.

use protobuf::Message;

// Include the generated protobuf code
#[path = ""]
pub mod hbb {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

// Re-export commonly used types
pub use hbb::rendezvous::{
    rendezvous_message, relay_response, punch_hole_response,
    ConnType, ConfigUpdate, FetchLocalAddr, HealthCheck, KeyExchange, LocalAddr, NatType,
    OnlineRequest, OnlineResponse, PeerDiscovery, PunchHole, PunchHoleRequest, PunchHoleResponse,
    PunchHoleSent, RegisterPeer, RegisterPeerResponse, RegisterPk, RegisterPkResponse,
    RelayResponse, RendezvousMessage, RequestRelay, SoftwareUpdate, TestNatRequest,
    TestNatResponse,
};

// Re-export message.proto types
pub use hbb::message::{
    message, misc, login_response, key_event,
    AudioFormat, AudioFrame, Auth2FA, Clipboard, CursorData, CursorPosition, EncodedVideoFrame,
    EncodedVideoFrames, Hash, IdPk, KeyEvent, LoginRequest, LoginResponse, MouseEvent, Misc,
    OptionMessage, PeerInfo, PublicKey, SignedId, SupportedDecoding, VideoFrame, TestDelay,
    Features, SupportedResolutions, WindowsSessions, Message as HbbMessage, ControlKey,
    DisplayInfo, SupportedEncoding,
};

/// Helper to create a RendezvousMessage with RegisterPeer
pub fn make_register_peer(id: &str, serial: i32) -> RendezvousMessage {
    let mut rp = RegisterPeer::new();
    rp.id = id.to_string();
    rp.serial = serial;

    let mut msg = RendezvousMessage::new();
    msg.set_register_peer(rp);
    msg
}

/// Helper to create a RendezvousMessage with RegisterPk
pub fn make_register_pk(id: &str, uuid: &[u8], pk: &[u8], old_id: &str) -> RendezvousMessage {
    let mut rpk = RegisterPk::new();
    rpk.id = id.to_string();
    rpk.uuid = uuid.to_vec().into();
    rpk.pk = pk.to_vec().into();
    rpk.old_id = old_id.to_string();

    let mut msg = RendezvousMessage::new();
    msg.set_register_pk(rpk);
    msg
}

/// Helper to create a PunchHoleSent message
pub fn make_punch_hole_sent(
    socket_addr: &[u8],
    id: &str,
    relay_server: &str,
    nat_type: NatType,
    version: &str,
) -> RendezvousMessage {
    let mut phs = PunchHoleSent::new();
    phs.socket_addr = socket_addr.to_vec().into();
    phs.id = id.to_string();
    phs.relay_server = relay_server.to_string();
    phs.nat_type = nat_type.into();
    phs.version = version.to_string();

    let mut msg = RendezvousMessage::new();
    msg.set_punch_hole_sent(phs);
    msg
}

/// Helper to create a RelayResponse message (sent to rendezvous server)
/// IMPORTANT: The union field should be `Id` (our device ID), NOT `Pk`.
/// The rendezvous server will look up our registered public key using this ID,
/// sign it with the server's private key, and set the `pk` field before forwarding to client.
pub fn make_relay_response(uuid: &str, socket_addr: &[u8], relay_server: &str, device_id: &str) -> RendezvousMessage {
    let mut rr = RelayResponse::new();
    rr.socket_addr = socket_addr.to_vec().into();
    rr.uuid = uuid.to_string();
    rr.relay_server = relay_server.to_string();
    rr.version = env!("CARGO_PKG_VERSION").to_string();
    rr.set_id(device_id.to_string());

    let mut msg = RendezvousMessage::new();
    msg.set_relay_response(rr);
    msg
}

/// Helper to create a RequestRelay message (sent to relay server to identify ourselves)
///
/// The `licence_key` is required if the relay server is configured with a key.
/// If the key doesn't match, the relay server will silently reject the connection.
///
/// IMPORTANT: `socket_addr` is the peer's encoded socket address (from FetchLocalAddr/RelayResponse).
/// The relay server uses this to match the two peers connecting to the same relay session.
pub fn make_request_relay(uuid: &str, licence_key: &str, socket_addr: &[u8]) -> RendezvousMessage {
    let mut rr = RequestRelay::new();
    rr.uuid = uuid.to_string();
    rr.licence_key = licence_key.to_string();
    rr.socket_addr = socket_addr.to_vec().into();

    let mut msg = RendezvousMessage::new();
    msg.set_request_relay(rr);
    msg
}

/// Helper to create a LocalAddr response message
/// This is sent in response to FetchLocalAddr when a peer on the same LAN wants to connect
pub fn make_local_addr(
    socket_addr: &[u8],
    local_addr: &[u8],
    relay_server: &str,
    id: &str,
    version: &str,
) -> RendezvousMessage {
    let mut la = LocalAddr::new();
    la.socket_addr = socket_addr.to_vec().into();
    la.local_addr = local_addr.to_vec().into();
    la.relay_server = relay_server.to_string();
    la.id = id.to_string();
    la.version = version.to_string();

    let mut msg = RendezvousMessage::new();
    msg.set_local_addr(la);
    msg
}

/// Decode a RendezvousMessage from bytes
pub fn decode_rendezvous_message(buf: &[u8]) -> Result<RendezvousMessage, protobuf::Error> {
    RendezvousMessage::parse_from_bytes(buf)
}

/// Decode a Message (session message) from bytes
pub fn decode_message(buf: &[u8]) -> Result<hbb::message::Message, protobuf::Error> {
    hbb::message::Message::parse_from_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_peer_encoding() {
        let msg = make_register_peer("123456789", 1);
        let encoded = msg.write_to_bytes().unwrap();
        assert!(!encoded.is_empty());

        let decoded = decode_rendezvous_message(&encoded).unwrap();
        assert!(decoded.has_register_peer());
        let rp = decoded.register_peer();
        assert_eq!(rp.id, "123456789");
        assert_eq!(rp.serial, 1);
    }

    #[test]
    fn test_register_pk_encoding() {
        let uuid = [1u8; 16];
        let pk = [2u8; 32];
        let msg = make_register_pk("123456789", &uuid, &pk, "");
        let encoded = msg.write_to_bytes().unwrap();
        assert!(!encoded.is_empty());

        let decoded = decode_rendezvous_message(&encoded).unwrap();
        assert!(decoded.has_register_pk());
        let rpk = decoded.register_pk();
        assert_eq!(rpk.id, "123456789");
        assert_eq!(rpk.uuid.len(), 16);
        assert_eq!(rpk.pk.len(), 32);
    }

    #[test]
    fn test_relay_response_encoding() {
        let socket_addr = vec![1, 2, 3, 4, 5, 6];
        let msg = make_relay_response("test-uuid", &socket_addr, "relay.example.com", "123456789");
        let encoded = msg.write_to_bytes().unwrap();
        assert!(!encoded.is_empty());

        let decoded = decode_rendezvous_message(&encoded).unwrap();
        assert!(decoded.has_relay_response());
        let rr = decoded.relay_response();
        assert_eq!(rr.uuid, "test-uuid");
        assert_eq!(rr.relay_server, "relay.example.com");
        // Check the oneof union field contains Id
        assert_eq!(rr.id(), "123456789");
    }
}
