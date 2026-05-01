//! Protobuf wrappers (`protos/` → `OUT_DIR`).

use protobuf::Message;

#[path = ""]
pub mod hbb {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

pub use hbb::rendezvous::{
    punch_hole_response, relay_response, rendezvous_message, ConfigUpdate, ConnType,
    FetchLocalAddr, HealthCheck, KeyExchange, LocalAddr, NatType, OnlineRequest, OnlineResponse,
    PeerDiscovery, PunchHole, PunchHoleRequest, PunchHoleResponse, PunchHoleSent, RegisterPeer,
    RegisterPeerResponse, RegisterPk, RegisterPkResponse, RelayResponse, RendezvousMessage,
    RequestRelay, SoftwareUpdate, TestNatRequest, TestNatResponse,
};

pub use hbb::message::{
    key_event, login_response, message, misc, AudioFormat, AudioFrame, Auth2FA, Clipboard,
    ControlKey, CursorData, CursorPosition, DisplayInfo, EncodedVideoFrame, EncodedVideoFrames,
    Features, Hash, IdPk, KeyEvent, LoginRequest, LoginResponse, Message as HbbMessage, Misc,
    MouseEvent, OptionMessage, PeerInfo, PublicKey, SignedId, SupportedDecoding, SupportedEncoding,
    SupportedResolutions, TestDelay, VideoFrame, WindowsSessions,
};

pub fn make_register_peer(id: &str, serial: i32) -> RendezvousMessage {
    let mut rp = RegisterPeer::new();
    rp.id = id.to_string();
    rp.serial = serial;

    let mut msg = RendezvousMessage::new();
    msg.set_register_peer(rp);
    msg
}

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

/// Use `id` (device id), not raw `pk`; hbbs fills `pk` when forwarding.
pub fn make_relay_response(
    uuid: &str,
    socket_addr: &[u8],
    relay_server: &str,
    device_id: &str,
) -> RendezvousMessage {
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

/// `socket_addr` must be the peer's mangled addr; `licence_key` required if hbbr uses `-k`.
pub fn make_request_relay(uuid: &str, licence_key: &str, socket_addr: &[u8]) -> RendezvousMessage {
    let mut rr = RequestRelay::new();
    rr.uuid = uuid.to_string();
    rr.licence_key = licence_key.to_string();
    rr.socket_addr = socket_addr.to_vec().into();

    let mut msg = RendezvousMessage::new();
    msg.set_request_relay(rr);
    msg
}

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

pub fn decode_rendezvous_message(buf: &[u8]) -> Result<RendezvousMessage, protobuf::Error> {
    RendezvousMessage::parse_from_bytes(buf)
}

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
