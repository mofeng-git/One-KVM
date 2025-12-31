//! RustDesk Protocol Messages
//!
//! This module provides the compiled protobuf messages for the RustDesk protocol.
//! Messages are generated from rendezvous.proto and message.proto at build time.

use prost::Message;

// Include the generated protobuf code
pub mod hbb {
    include!(concat!(env!("OUT_DIR"), "/hbb.rs"));
}

// Re-export commonly used types (except Message which conflicts with prost::Message)
pub use hbb::{
    ConnType, ConfigUpdate, FetchLocalAddr, HealthCheck, KeyExchange, LocalAddr, NatType,
    OnlineRequest, OnlineResponse, PeerDiscovery, PunchHole, PunchHoleRequest, PunchHoleResponse,
    PunchHoleSent, RegisterPeer, RegisterPeerResponse, RegisterPk, RegisterPkResponse,
    RelayResponse, RendezvousMessage, RequestRelay, SoftwareUpdate, TestNatRequest,
    TestNatResponse,
};

// Re-export message.proto types
pub use hbb::{
    AudioFormat, AudioFrame, Auth2Fa, Clipboard, CursorData, CursorPosition, EncodedVideoFrame,
    EncodedVideoFrames, Hash, IdPk, KeyEvent, LoginRequest, LoginResponse, MouseEvent, Misc,
    OptionMessage, PeerInfo, PublicKey, SignedId, SupportedDecoding, VideoFrame,
};

/// Trait for encoding/decoding protobuf messages
pub trait ProtobufMessage: Message + Default {
    /// Encode the message to bytes
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf).expect("Failed to encode message");
        buf
    }

    /// Decode from bytes
    fn decode_from_slice(buf: &[u8]) -> Result<Self, prost::DecodeError> {
        Self::decode(buf)
    }
}

// Implement for all generated message types
impl<T: Message + Default> ProtobufMessage for T {}

/// Helper to create a RendezvousMessage with RegisterPeer
pub fn make_register_peer(id: &str, serial: i32) -> RendezvousMessage {
    RendezvousMessage {
        union: Some(hbb::rendezvous_message::Union::RegisterPeer(RegisterPeer {
            id: id.to_string(),
            serial,
        })),
    }
}

/// Helper to create a RendezvousMessage with RegisterPk
pub fn make_register_pk(id: &str, uuid: &[u8], pk: &[u8], old_id: &str) -> RendezvousMessage {
    RendezvousMessage {
        union: Some(hbb::rendezvous_message::Union::RegisterPk(RegisterPk {
            id: id.to_string(),
            uuid: uuid.to_vec(),
            pk: pk.to_vec(),
            old_id: old_id.to_string(),
        })),
    }
}

/// Helper to create a PunchHoleSent message
pub fn make_punch_hole_sent(
    socket_addr: &[u8],
    id: &str,
    relay_server: &str,
    nat_type: NatType,
    version: &str,
) -> RendezvousMessage {
    RendezvousMessage {
        union: Some(hbb::rendezvous_message::Union::PunchHoleSent(PunchHoleSent {
            socket_addr: socket_addr.to_vec(),
            id: id.to_string(),
            relay_server: relay_server.to_string(),
            nat_type: nat_type.into(),
            version: version.to_string(),
        })),
    }
}

/// Helper to create a RelayResponse message (sent to relay server)
pub fn make_relay_response(uuid: &str, _pk: Option<&[u8]>) -> RendezvousMessage {
    RendezvousMessage {
        union: Some(hbb::rendezvous_message::Union::RelayResponse(RelayResponse {
            socket_addr: vec![],
            uuid: uuid.to_string(),
            relay_server: String::new(),
            ..Default::default()
        })),
    }
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
    RendezvousMessage {
        union: Some(hbb::rendezvous_message::Union::LocalAddr(LocalAddr {
            socket_addr: socket_addr.to_vec(),
            local_addr: local_addr.to_vec(),
            relay_server: relay_server.to_string(),
            id: id.to_string(),
            version: version.to_string(),
        })),
    }
}

/// Decode a RendezvousMessage from bytes
pub fn decode_rendezvous_message(buf: &[u8]) -> Result<RendezvousMessage, prost::DecodeError> {
    RendezvousMessage::decode(buf)
}

/// Decode a Message (session message) from bytes
pub fn decode_message(buf: &[u8]) -> Result<hbb::Message, prost::DecodeError> {
    hbb::Message::decode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message as ProstMessage;

    #[test]
    fn test_register_peer_encoding() {
        let msg = make_register_peer("123456789", 1);
        let encoded = ProstMessage::encode_to_vec(&msg);
        assert!(!encoded.is_empty());

        let decoded = decode_rendezvous_message(&encoded).unwrap();
        match decoded.union {
            Some(hbb::rendezvous_message::Union::RegisterPeer(rp)) => {
                assert_eq!(rp.id, "123456789");
                assert_eq!(rp.serial, 1);
            }
            _ => panic!("Expected RegisterPeer message"),
        }
    }

    #[test]
    fn test_register_pk_encoding() {
        let uuid = [1u8; 16];
        let pk = [2u8; 32];
        let msg = make_register_pk("123456789", &uuid, &pk, "");
        let encoded = ProstMessage::encode_to_vec(&msg);
        assert!(!encoded.is_empty());

        let decoded = decode_rendezvous_message(&encoded).unwrap();
        match decoded.union {
            Some(hbb::rendezvous_message::Union::RegisterPk(rpk)) => {
                assert_eq!(rpk.id, "123456789");
                assert_eq!(rpk.uuid.len(), 16);
                assert_eq!(rpk.pk.len(), 32);
            }
            _ => panic!("Expected RegisterPk message"),
        }
    }
}
