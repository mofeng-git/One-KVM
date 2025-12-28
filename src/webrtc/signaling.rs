//! WebRTC signaling types and messages

use serde::{Deserialize, Serialize};

/// Signaling message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SignalingMessage {
    /// SDP Offer from client
    Offer(SdpOffer),
    /// SDP Answer from server
    Answer(SdpAnswer),
    /// ICE candidate
    Candidate(IceCandidate),
    /// Connection error
    Error(SignalingError),
    /// Connection closed
    Close,
}

/// SDP Offer from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpOffer {
    /// SDP content
    pub sdp: String,
}

impl SdpOffer {
    pub fn new(sdp: impl Into<String>) -> Self {
        Self { sdp: sdp.into() }
    }
}

/// SDP Answer from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpAnswer {
    /// SDP content
    pub sdp: String,
    /// ICE candidates gathered during answer creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ice_candidates: Option<Vec<IceCandidate>>,
}

impl SdpAnswer {
    pub fn new(sdp: impl Into<String>) -> Self {
        Self {
            sdp: sdp.into(),
            ice_candidates: None,
        }
    }

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

/// ICE candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    /// Candidate string
    pub candidate: String,
    /// SDP mid (media ID)
    #[serde(rename = "sdpMid")]
    pub sdp_mid: Option<String>,
    /// SDP mline index
    #[serde(rename = "sdpMLineIndex")]
    pub sdp_mline_index: Option<u16>,
    /// Username fragment
    #[serde(rename = "usernameFragment")]
    pub username_fragment: Option<String>,
}

impl IceCandidate {
    pub fn new(candidate: impl Into<String>) -> Self {
        Self {
            candidate: candidate.into(),
            sdp_mid: None,
            sdp_mline_index: None,
            username_fragment: None,
        }
    }

    pub fn with_mid(mut self, mid: impl Into<String>, index: u16) -> Self {
        self.sdp_mid = Some(mid.into());
        self.sdp_mline_index = Some(index);
        self
    }
}

/// Signaling error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingError {
    /// Error code
    pub code: u32,
    /// Error message
    pub message: String,
}

impl SignalingError {
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_offer() -> Self {
        Self::new(400, "Invalid SDP offer")
    }

    pub fn connection_failed() -> Self {
        Self::new(500, "Connection failed")
    }

    pub fn media_error() -> Self {
        Self::new(502, "Media error")
    }
}

/// WebRTC offer request (from HTTP API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferRequest {
    /// SDP offer
    pub sdp: String,
    /// Client ID (optional, for tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

/// WebRTC answer response (from HTTP API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerResponse {
    /// SDP answer
    pub sdp: String,
    /// Session ID for this connection
    pub session_id: String,
    /// ICE candidates
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

/// ICE candidate request (trickle ICE)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidateRequest {
    /// Session ID
    pub session_id: String,
    /// ICE candidate
    pub candidate: IceCandidate,
}

/// Connection state notification
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
