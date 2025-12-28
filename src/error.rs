use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Application-wide error type
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    AuthError(String),

    #[error("Not authenticated")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

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

    #[error("Video device lost [{device}]: {reason}")]
    VideoDeviceLost { device: String, reason: String },

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

/// Error response body (unified success format)
#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        // Always return 200 OK - success/failure is indicated by the success field
        StatusCode::OK
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = ErrorResponse {
            success: false,
            message: self.to_string(),
        };

        tracing::error!(
            error_type = std::any::type_name_of_val(&self),
            error_message = %body.message,
            "Request failed"
        );

        (status, Json(body)).into_response()
    }
}

/// Result type alias for handlers
pub type Result<T> = std::result::Result<T, AppError>;
