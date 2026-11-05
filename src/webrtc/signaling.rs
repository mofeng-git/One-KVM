//! SDP / ICE JSON types used by HTTP and WebSocket handlers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpOffer {
    pub sdp: String,
}

impl SdpOffer {
    pub fn new(sdp: impl Into<String>) -> Self {
        Self { sdp: sdp.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpAnswer {
    pub sdp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ice_candidates: Option<Vec<IceCandidate>>,
}

impl SdpAnswer {
    pub fn with_candidates(sdp: impl Into<String>, candidates: Vec<IceCandidate>) -> Self {
        Self {
            sdp: sdp.into(),
            ice_candidates: if candidates.is_empty() {
                None
            } else {
                Some(candidates)
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate: String,
    #[serde(rename = "sdpMid")]
    pub sdp_mid: Option<String>,
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_mline_index: Option<u16>,
    #[serde(rename = "usernameFragment")]
    pub username_fragment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferRequest {
    pub sdp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerResponse {
    pub sdp: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ice_candidates: Vec<IceCandidate>,
}

impl AnswerResponse {
    pub fn new(
        sdp: impl Into<String>,
        session_id: impl Into<String>,
        ice_candidates: Vec<IceCandidate>,
    ) -> Self {
        Self {
            sdp: sdp.into(),
            session_id: session_id.into(),
            ice_candidates,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidateRequest {
    pub session_id: String,
    pub candidate: IceCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::New => write!(f, "new"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Failed => write!(f, "failed"),
            ConnectionState::Closed => write!(f, "closed"),
        }
    }
}
