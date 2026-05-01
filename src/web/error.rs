use crate::error::AppError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            success: false,
            message: self.to_string(),
        };

        tracing::error!(
            error_type = std::any::type_name_of_val(&self),
            error_message = %body.message,
            "Request failed"
        );

        // Always return 200 OK - success/failure is indicated by the success field
        (StatusCode::OK, Json(body)).into_response()
    }
}
