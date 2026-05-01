use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Not authenticated")]
    Unauthorized,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Video error: {0}")]
    VideoError(String),

    /// No input signal while opening capture; `kind` is `SignalStatus` as string (`from_str`).
    #[error("Capture has no valid signal: {kind}")]
    CaptureNoSignal { kind: String },

    #[error("Audio error: {0}")]
    AudioError(String),

    #[error("HID error [{backend}]: {reason} (code: {error_code})")]
    HidError {
        backend: String,
        reason: String,
        error_code: String,
    },

    #[error("WebRTC error: {0}")]
    WebRtcError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Persistence(err.to_string())
    }
}
