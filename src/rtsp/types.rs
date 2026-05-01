use rand::Rng;
use rtsp_types as rtsp;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RtspServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

impl fmt::Display for RtspServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error(err) => write!(f, "error: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RtspRequest {
    pub method: rtsp::Method,
    pub uri: String,
    pub version: rtsp::Version,
    pub headers: HashMap<String, String>,
}

pub(crate) struct RtspConnectionState {
    pub session_id: String,
    pub setup_done: bool,
    pub interleaved_channel: u8,
}

impl RtspConnectionState {
    pub fn new() -> Self {
        Self {
            session_id: generate_session_id(),
            setup_done: false,
            interleaved_channel: 0,
        }
    }
}

pub(crate) fn generate_session_id() -> String {
    let mut rng = rand::rng();
    let value: u64 = rng.random();
    format!("{:016x}", value)
}
