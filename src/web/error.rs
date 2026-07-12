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
        let status = status_code(&self);
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

fn status_code(error: &AppError) -> StatusCode {
    match error {
        AppError::AuthError(_) | AppError::Unauthorized => StatusCode::UNAUTHORIZED,
        AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_client_and_availability_errors_to_http_statuses() {
        assert_eq!(
            status_code(&AppError::BadRequest("invalid".to_string())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_code(&AppError::AuthError("invalid".to_string())),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_code(&AppError::NotFound("missing".to_string())),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_code(&AppError::ServiceUnavailable("offline".to_string())),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn maps_internal_errors_to_server_error() {
        assert_eq!(
            status_code(&AppError::Internal("failed".to_string())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
