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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = status_code(&self);
        let body = ErrorResponse {
            success: false,
            code: error_code(&self),
            message: public_message(&self),
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
        AppError::Conflict(_) => StatusCode::CONFLICT,
        AppError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        AppError::Msd(error) => msd_status_code(error.code()),
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_code(error: &AppError) -> Option<&'static str> {
    match error {
        AppError::Msd(error) => Some(error.code().as_str()),
        _ => None,
    }
}

fn public_message(error: &AppError) -> String {
    match error {
        AppError::Msd(error) => error.code().message().to_string(),
        _ => error.to_string(),
    }
}

pub(crate) fn msd_status_code(code: crate::error::MsdErrorCode) -> StatusCode {
    use crate::error::MsdErrorCode::*;
    match code {
        MsdUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        MsdResourceNotFound | MsdDriveNotInitialized => StatusCode::NOT_FOUND,
        MsdOperationInProgress
        | MsdResourceAlreadyExists
        | MsdMediaSlotsFull
        | MsdMediaAlreadyMounted
        | MsdMediaInUse
        | MsdDriveConnected
        | MsdMediumRemovalPrevented => StatusCode::CONFLICT,
        MsdInvalidRequest
        | MsdImageTooLarge
        | MsdInvalidUrl
        | MsdDriveFilesystemUnsupported
        | MsdDriveSizeInvalid
        | MsdStorageSpaceUnavailable
        | MsdStorageFull
        | MsdStorageReadOnly
        | MsdStoragePermissionDenied => StatusCode::BAD_REQUEST,
        MsdOperationFailed
        | MsdRemoteDownloadFailed
        | MsdDownloadIncomplete
        | MsdDisconnectFailed => StatusCode::INTERNAL_SERVER_ERROR,
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
        assert_eq!(
            status_code(&AppError::Conflict("exists".to_string())),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_code(&AppError::RateLimited("limited".to_string())),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            status_code(&AppError::from(
                crate::error::MsdErrorCode::MsdMediumRemovalPrevented
            )),
            StatusCode::CONFLICT
        );
        assert_eq!(
            error_code(&AppError::from(
                crate::error::MsdErrorCode::MsdMediumRemovalPrevented
            )),
            Some("MSD_MEDIUM_REMOVAL_PREVENTED")
        );
    }

    #[test]
    fn maps_internal_errors_to_server_error() {
        assert_eq!(
            status_code(&AppError::Internal("failed".to_string())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn every_msd_error_has_a_stable_code_message_and_status() {
        use crate::error::MsdErrorCode::*;
        let cases = [
            (MsdUnavailable, StatusCode::SERVICE_UNAVAILABLE),
            (MsdOperationInProgress, StatusCode::CONFLICT),
            (MsdOperationFailed, StatusCode::INTERNAL_SERVER_ERROR),
            (MsdInvalidRequest, StatusCode::BAD_REQUEST),
            (MsdResourceNotFound, StatusCode::NOT_FOUND),
            (MsdResourceAlreadyExists, StatusCode::CONFLICT),
            (MsdMediaSlotsFull, StatusCode::CONFLICT),
            (MsdMediaAlreadyMounted, StatusCode::CONFLICT),
            (MsdMediaInUse, StatusCode::CONFLICT),
            (MsdImageTooLarge, StatusCode::BAD_REQUEST),
            (MsdInvalidUrl, StatusCode::BAD_REQUEST),
            (MsdRemoteDownloadFailed, StatusCode::INTERNAL_SERVER_ERROR),
            (MsdDownloadIncomplete, StatusCode::INTERNAL_SERVER_ERROR),
            (MsdDriveNotInitialized, StatusCode::NOT_FOUND),
            (MsdDriveConnected, StatusCode::CONFLICT),
            (MsdDriveFilesystemUnsupported, StatusCode::BAD_REQUEST),
            (MsdDriveSizeInvalid, StatusCode::BAD_REQUEST),
            (MsdStorageSpaceUnavailable, StatusCode::BAD_REQUEST),
            (MsdStorageFull, StatusCode::BAD_REQUEST),
            (MsdStorageReadOnly, StatusCode::BAD_REQUEST),
            (MsdStoragePermissionDenied, StatusCode::BAD_REQUEST),
            (MsdMediumRemovalPrevented, StatusCode::CONFLICT),
            (MsdDisconnectFailed, StatusCode::INTERNAL_SERVER_ERROR),
        ];
        assert_eq!(cases.len(), crate::error::MsdErrorCode::ALL.len());
        for (code, expected_status) in cases {
            let error = AppError::from(code);
            assert_eq!(error_code(&error), Some(code.as_str()));
            assert_eq!(public_message(&error), code.message());
            assert!(!code.message().contains('/'));
            assert_eq!(msd_status_code(code), expected_status);
        }
    }

    #[tokio::test]
    async fn msd_response_contains_only_the_public_error_contract() {
        let response =
            AppError::from(crate::error::MsdErrorCode::MsdOperationFailed).into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["success"], false);
        assert_eq!(json["code"], "MSD_OPERATION_FAILED");
        assert_eq!(
            json["message"],
            crate::error::MsdErrorCode::MsdOperationFailed.message()
        );
        assert_eq!(json.as_object().unwrap().len(), 3);
    }
}
