use serde::Serialize;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MsdErrorCode {
    MsdUnavailable,
    MsdOperationInProgress,
    MsdOperationFailed,
    MsdInvalidRequest,
    MsdResourceNotFound,
    MsdResourceAlreadyExists,
    MsdMediaSlotsFull,
    MsdMediaAlreadyMounted,
    MsdMediaInUse,
    MsdImageTooLarge,
    MsdInvalidUrl,
    MsdRemoteDownloadFailed,
    MsdDownloadIncomplete,
    MsdDriveNotInitialized,
    MsdDriveConnected,
    MsdDriveFilesystemUnsupported,
    MsdDriveSizeInvalid,
    MsdStorageSpaceUnavailable,
    MsdStorageFull,
    MsdStorageReadOnly,
    MsdStoragePermissionDenied,
    MsdMediumRemovalPrevented,
    MsdDisconnectFailed,
}

impl MsdErrorCode {
    pub const ALL: [Self; 23] = [
        Self::MsdUnavailable,
        Self::MsdOperationInProgress,
        Self::MsdOperationFailed,
        Self::MsdInvalidRequest,
        Self::MsdResourceNotFound,
        Self::MsdResourceAlreadyExists,
        Self::MsdMediaSlotsFull,
        Self::MsdMediaAlreadyMounted,
        Self::MsdMediaInUse,
        Self::MsdImageTooLarge,
        Self::MsdInvalidUrl,
        Self::MsdRemoteDownloadFailed,
        Self::MsdDownloadIncomplete,
        Self::MsdDriveNotInitialized,
        Self::MsdDriveConnected,
        Self::MsdDriveFilesystemUnsupported,
        Self::MsdDriveSizeInvalid,
        Self::MsdStorageSpaceUnavailable,
        Self::MsdStorageFull,
        Self::MsdStorageReadOnly,
        Self::MsdStoragePermissionDenied,
        Self::MsdMediumRemovalPrevented,
        Self::MsdDisconnectFailed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MsdUnavailable => "MSD_UNAVAILABLE",
            Self::MsdOperationInProgress => "MSD_OPERATION_IN_PROGRESS",
            Self::MsdOperationFailed => "MSD_OPERATION_FAILED",
            Self::MsdInvalidRequest => "MSD_INVALID_REQUEST",
            Self::MsdResourceNotFound => "MSD_RESOURCE_NOT_FOUND",
            Self::MsdResourceAlreadyExists => "MSD_RESOURCE_ALREADY_EXISTS",
            Self::MsdMediaSlotsFull => "MSD_MEDIA_SLOTS_FULL",
            Self::MsdMediaAlreadyMounted => "MSD_MEDIA_ALREADY_MOUNTED",
            Self::MsdMediaInUse => "MSD_MEDIA_IN_USE",
            Self::MsdImageTooLarge => "MSD_IMAGE_TOO_LARGE",
            Self::MsdInvalidUrl => "MSD_INVALID_URL",
            Self::MsdRemoteDownloadFailed => "MSD_REMOTE_DOWNLOAD_FAILED",
            Self::MsdDownloadIncomplete => "MSD_DOWNLOAD_INCOMPLETE",
            Self::MsdDriveNotInitialized => "MSD_DRIVE_NOT_INITIALIZED",
            Self::MsdDriveConnected => "MSD_DRIVE_CONNECTED",
            Self::MsdDriveFilesystemUnsupported => "MSD_DRIVE_FILESYSTEM_UNSUPPORTED",
            Self::MsdDriveSizeInvalid => "MSD_DRIVE_SIZE_INVALID",
            Self::MsdStorageSpaceUnavailable => "MSD_STORAGE_SPACE_UNAVAILABLE",
            Self::MsdStorageFull => "MSD_STORAGE_FULL",
            Self::MsdStorageReadOnly => "MSD_STORAGE_READ_ONLY",
            Self::MsdStoragePermissionDenied => "MSD_STORAGE_PERMISSION_DENIED",
            Self::MsdMediumRemovalPrevented => "MSD_MEDIUM_REMOVAL_PREVENTED",
            Self::MsdDisconnectFailed => "MSD_DISCONNECT_FAILED",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::MsdUnavailable => "Virtual media service is unavailable.",
            Self::MsdOperationInProgress => "Another virtual media operation is in progress.",
            Self::MsdOperationFailed => "The virtual media operation failed.",
            Self::MsdInvalidRequest => "The virtual media request is invalid.",
            Self::MsdResourceNotFound => "The requested virtual media resource was not found.",
            Self::MsdResourceAlreadyExists => "The virtual media resource already exists.",
            Self::MsdMediaSlotsFull => "All virtual media slots are in use.",
            Self::MsdMediaAlreadyMounted => "The virtual medium is already mounted.",
            Self::MsdMediaInUse => "The virtual medium is currently in use.",
            Self::MsdImageTooLarge => "The virtual media image is too large.",
            Self::MsdInvalidUrl => "The download URL is invalid.",
            Self::MsdRemoteDownloadFailed => "The remote image download failed.",
            Self::MsdDownloadIncomplete => "The remote image download was incomplete.",
            Self::MsdDriveNotInitialized => "The virtual drive is not initialized.",
            Self::MsdDriveConnected => "The virtual drive is connected to the controlled computer.",
            Self::MsdDriveFilesystemUnsupported => {
                "Web file management does not support this virtual drive format."
            }
            Self::MsdDriveSizeInvalid => "The virtual drive size is invalid.",
            Self::MsdStorageSpaceUnavailable => {
                "Available virtual media storage space could not be determined."
            }
            Self::MsdStorageFull => "Virtual media storage does not have enough free space.",
            Self::MsdStorageReadOnly => "Virtual media storage is read-only.",
            Self::MsdStoragePermissionDenied => {
                "Permission to access virtual media storage was denied."
            }
            Self::MsdMediumRemovalPrevented => {
                "The controlled computer prevented removal of the virtual medium."
            }
            Self::MsdDisconnectFailed => "The virtual medium could not be disconnected.",
        }
    }

    pub const fn redfish_key(self) -> &'static str {
        match self {
            Self::MsdUnavailable => "MsdUnavailable",
            Self::MsdOperationInProgress => "MsdOperationInProgress",
            Self::MsdOperationFailed => "MsdOperationFailed",
            Self::MsdInvalidRequest => "MsdInvalidRequest",
            Self::MsdResourceNotFound => "MsdResourceNotFound",
            Self::MsdResourceAlreadyExists => "MsdResourceAlreadyExists",
            Self::MsdMediaSlotsFull => "MsdMediaSlotsFull",
            Self::MsdMediaAlreadyMounted => "MsdMediaAlreadyMounted",
            Self::MsdMediaInUse => "MsdMediaInUse",
            Self::MsdImageTooLarge => "MsdImageTooLarge",
            Self::MsdInvalidUrl => "MsdInvalidUrl",
            Self::MsdRemoteDownloadFailed => "MsdRemoteDownloadFailed",
            Self::MsdDownloadIncomplete => "MsdDownloadIncomplete",
            Self::MsdDriveNotInitialized => "MsdDriveNotInitialized",
            Self::MsdDriveConnected => "MsdDriveConnected",
            Self::MsdDriveFilesystemUnsupported => "MsdDriveFilesystemUnsupported",
            Self::MsdDriveSizeInvalid => "MsdDriveSizeInvalid",
            Self::MsdStorageSpaceUnavailable => "MsdStorageSpaceUnavailable",
            Self::MsdStorageFull => "MsdStorageFull",
            Self::MsdStorageReadOnly => "MsdStorageReadOnly",
            Self::MsdStoragePermissionDenied => "MsdStoragePermissionDenied",
            Self::MsdMediumRemovalPrevented => "MsdMediumRemovalPrevented",
            Self::MsdDisconnectFailed => "MsdDisconnectFailed",
        }
    }

    pub const fn severity(self) -> &'static str {
        match self {
            Self::MsdUnavailable | Self::MsdOperationFailed | Self::MsdDisconnectFailed => {
                "Critical"
            }
            _ => "Warning",
        }
    }

    pub const fn resolution(self) -> &'static str {
        match self {
            Self::MsdUnavailable => "Enable or restore the virtual media service, then retry.",
            Self::MsdOperationInProgress => {
                "Wait for the current virtual media operation to finish, then retry."
            }
            Self::MsdResourceNotFound | Self::MsdDriveNotInitialized => {
                "Verify that the requested virtual media resource exists, then retry."
            }
            Self::MsdResourceAlreadyExists => {
                "Use a different resource name or remove the existing resource, then retry."
            }
            Self::MsdMediaSlotsFull => "Eject an inserted virtual medium, then retry.",
            Self::MsdMediaAlreadyMounted => {
                "Eject the existing virtual medium before mounting it again."
            }
            Self::MsdMediaInUse | Self::MsdDriveConnected | Self::MsdMediumRemovalPrevented => {
                "Eject or unmount the virtual medium on the controlled computer, then retry."
            }
            Self::MsdImageTooLarge | Self::MsdDriveSizeInvalid => {
                "Use a supported image or virtual drive size, then retry."
            }
            Self::MsdInvalidUrl | Self::MsdInvalidRequest => "Correct the request and retry.",
            Self::MsdRemoteDownloadFailed | Self::MsdDownloadIncomplete => {
                "Verify the remote server and network connection, then retry."
            }
            Self::MsdDriveFilesystemUnsupported => {
                "Mount the drive on the controlled computer, or use a supported format for web file management."
            }
            Self::MsdStorageSpaceUnavailable => {
                "Verify that virtual media storage is available, then retry."
            }
            Self::MsdStorageFull => {
                "Free space in virtual media storage or select a smaller image, then retry."
            }
            Self::MsdStorageReadOnly => "Make virtual media storage writable, then retry.",
            Self::MsdStoragePermissionDenied => {
                "Correct virtual media storage permissions, then retry."
            }
            Self::MsdOperationFailed | Self::MsdDisconnectFailed => {
                "Retry the operation. If the problem persists, check the One-KVM system logs."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsdError {
    code: MsdErrorCode,
}

impl MsdError {
    pub const fn new(code: MsdErrorCode) -> Self {
        Self { code }
    }

    pub const fn code(self) -> MsdErrorCode {
        self.code
    }
}

impl fmt::Display for MsdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.message())
    }
}

impl std::error::Error for MsdError {}

impl From<MsdErrorCode> for AppError {
    fn from(code: MsdErrorCode) -> Self {
        Self::Msd(MsdError::new(code))
    }
}

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

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Too many attempts: {0}")]
    RateLimited(String),

    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Msd(#[from] MsdError),

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

#[cfg(test)]
mod tests {
    use super::MsdErrorCode::*;

    #[test]
    fn msd_codes_and_messages_are_stable() {
        let cases = [
            (
                MsdUnavailable,
                "MSD_UNAVAILABLE",
                "Virtual media service is unavailable.",
            ),
            (
                MsdOperationInProgress,
                "MSD_OPERATION_IN_PROGRESS",
                "Another virtual media operation is in progress.",
            ),
            (
                MsdOperationFailed,
                "MSD_OPERATION_FAILED",
                "The virtual media operation failed.",
            ),
            (
                MsdInvalidRequest,
                "MSD_INVALID_REQUEST",
                "The virtual media request is invalid.",
            ),
            (
                MsdResourceNotFound,
                "MSD_RESOURCE_NOT_FOUND",
                "The requested virtual media resource was not found.",
            ),
            (
                MsdResourceAlreadyExists,
                "MSD_RESOURCE_ALREADY_EXISTS",
                "The virtual media resource already exists.",
            ),
            (
                MsdMediaSlotsFull,
                "MSD_MEDIA_SLOTS_FULL",
                "All virtual media slots are in use.",
            ),
            (
                MsdMediaAlreadyMounted,
                "MSD_MEDIA_ALREADY_MOUNTED",
                "The virtual medium is already mounted.",
            ),
            (
                MsdMediaInUse,
                "MSD_MEDIA_IN_USE",
                "The virtual medium is currently in use.",
            ),
            (
                MsdImageTooLarge,
                "MSD_IMAGE_TOO_LARGE",
                "The virtual media image is too large.",
            ),
            (
                MsdInvalidUrl,
                "MSD_INVALID_URL",
                "The download URL is invalid.",
            ),
            (
                MsdRemoteDownloadFailed,
                "MSD_REMOTE_DOWNLOAD_FAILED",
                "The remote image download failed.",
            ),
            (
                MsdDownloadIncomplete,
                "MSD_DOWNLOAD_INCOMPLETE",
                "The remote image download was incomplete.",
            ),
            (
                MsdDriveNotInitialized,
                "MSD_DRIVE_NOT_INITIALIZED",
                "The virtual drive is not initialized.",
            ),
            (
                MsdDriveConnected,
                "MSD_DRIVE_CONNECTED",
                "The virtual drive is connected to the controlled computer.",
            ),
            (
                MsdDriveFilesystemUnsupported,
                "MSD_DRIVE_FILESYSTEM_UNSUPPORTED",
                "Web file management does not support this virtual drive format.",
            ),
            (
                MsdDriveSizeInvalid,
                "MSD_DRIVE_SIZE_INVALID",
                "The virtual drive size is invalid.",
            ),
            (
                MsdStorageSpaceUnavailable,
                "MSD_STORAGE_SPACE_UNAVAILABLE",
                "Available virtual media storage space could not be determined.",
            ),
            (
                MsdStorageFull,
                "MSD_STORAGE_FULL",
                "Virtual media storage does not have enough free space.",
            ),
            (
                MsdStorageReadOnly,
                "MSD_STORAGE_READ_ONLY",
                "Virtual media storage is read-only.",
            ),
            (
                MsdStoragePermissionDenied,
                "MSD_STORAGE_PERMISSION_DENIED",
                "Permission to access virtual media storage was denied.",
            ),
            (
                MsdMediumRemovalPrevented,
                "MSD_MEDIUM_REMOVAL_PREVENTED",
                "The controlled computer prevented removal of the virtual medium.",
            ),
            (
                MsdDisconnectFailed,
                "MSD_DISCONNECT_FAILED",
                "The virtual medium could not be disconnected.",
            ),
        ];

        assert_eq!(cases.len(), super::MsdErrorCode::ALL.len());
        for (code, expected_code, expected_message) in cases {
            assert_eq!(code.as_str(), expected_code);
            assert_eq!(code.message(), expected_message);
        }
    }
}
