use super::config::apply::try_apply_lock;
use super::*;

use crate::msd::{
    DiskModeRequest, DownloadProgress, DriveFile, DriveFileAccess, DriveInfo, DriveInitRequest,
    ImageDownloadRequest, ImageInfo, ImageManager, ImageMountRequest, MsdErrorCode, MsdState,
    MsdStateResponse, VentoyDrive, MIN_DRIVE_SIZE_MB,
};
#[cfg(unix)]
use axum::body::Body;
#[cfg(unix)]
use axum::extract::{multipart::MultipartRejection, rejection::JsonRejection};
#[cfg(unix)]
use axum::extract::{Multipart, Path as AxumPath};
#[cfg(unix)]
use axum::http::{header, StatusCode};
#[cfg(unix)]
use axum::response::Response;
#[cfg(unix)]
use std::collections::HashMap;

#[cfg(unix)]
const MIB: u64 = 1024 * 1024;

/// Return an error if the virtual drive is currently connected to the USB host.
/// When connected, the USB host (e.g. Windows) has the filesystem mounted.
/// Any concurrent access from the server side (via VentoyImage::open) would
/// cause double-access corruption, manifesting as Windows error 0x80070570.
#[cfg(unix)]
async fn assert_drive_not_connected(state: &Arc<AppState>) -> Result<()> {
    let msd_guard = state.msd.read().await;
    if let Some(controller) = msd_guard.as_ref() {
        if controller.is_drive_connected().await {
            return Err(MsdErrorCode::MsdDriveConnected.into());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_drive_init_size(size_mb: u32, available_bytes: u64) -> Result<()> {
    let requested_bytes = size_mb as u64 * MIB;
    if size_mb < MIN_DRIVE_SIZE_MB {
        return Err(MsdErrorCode::MsdDriveSizeInvalid.into());
    }
    if requested_bytes > available_bytes {
        return Err(MsdErrorCode::MsdStorageFull.into());
    }
    Ok(())
}

#[cfg(unix)]
fn msd_controller<'a>(
    guard: &'a tokio::sync::RwLockReadGuard<'_, Option<crate::msd::MsdController>>,
) -> Result<&'a crate::msd::MsdController> {
    guard
        .as_ref()
        .ok_or_else(|| MsdErrorCode::MsdUnavailable.into())
}

#[cfg(unix)]
fn classify_storage_error(operation: &'static str, error: std::io::Error) -> AppError {
    tracing::warn!(operation, %error, "MSD storage operation failed");
    match error.raw_os_error() {
        Some(libc::ENOSPC) => MsdErrorCode::MsdStorageFull.into(),
        Some(libc::EROFS) => MsdErrorCode::MsdStorageReadOnly.into(),
        Some(libc::EACCES | libc::EPERM) => MsdErrorCode::MsdStoragePermissionDenied.into(),
        _ => MsdErrorCode::MsdOperationFailed.into(),
    }
}

#[cfg(unix)]
fn operation_failed(operation: &'static str, error: AppError) -> AppError {
    match error {
        AppError::Msd(error) => AppError::Msd(error),
        error => {
            tracing::warn!(operation, %error, "Unclassified MSD operation failed");
            MsdErrorCode::MsdOperationFailed.into()
        }
    }
}

#[cfg(unix)]
fn parse_msd_json<T>(payload: std::result::Result<Json<T>, JsonRejection>) -> Result<T> {
    payload.map(|Json(value)| value).map_err(|error| {
        tracing::warn!(%error, "Failed to parse MSD JSON request");
        MsdErrorCode::MsdInvalidRequest.into()
    })
}

#[cfg(unix)]
fn parse_msd_multipart(
    payload: std::result::Result<Multipart, MultipartRejection>,
) -> Result<Multipart> {
    payload.map_err(|error| {
        tracing::warn!(%error, "Failed to parse MSD multipart request");
        MsdErrorCode::MsdInvalidRequest.into()
    })
}

/// MSD status response
#[cfg(unix)]
#[derive(Serialize)]
pub struct MsdStatus {
    pub available: bool,
    pub state: MsdStateResponse,
}

/// Get MSD status
#[cfg(unix)]
pub async fn msd_status(State(state): State<Arc<AppState>>) -> Result<Json<MsdStatus>> {
    let msd_guard = state.msd.read().await;
    match msd_guard.as_ref() {
        Some(controller) => {
            let msd_state = controller.state().await;
            Ok(Json(MsdStatus {
                available: true,
                state: MsdStateResponse::from(&msd_state),
            }))
        }
        None => Ok(Json(MsdStatus {
            available: false,
            state: MsdStateResponse::from(&MsdState::default()),
        })),
    }
}

/// List all available images
#[cfg(unix)]
pub async fn msd_images_list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<ImageInfo>>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    let images = manager.list()?;
    Ok(Json(images))
}

/// Upload new image (streaming - memory efficient for large files)
#[cfg(unix)]
pub async fn msd_image_upload(
    State(state): State<Arc<AppState>>,
    multipart: std::result::Result<Multipart, MultipartRejection>,
) -> Result<Json<ImageInfo>> {
    let mut multipart = parse_msd_multipart(multipart)?;
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!(%error, "Failed to parse MSD image upload");
        AppError::from(MsdErrorCode::MsdInvalidRequest)
    })? {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::from(MsdErrorCode::MsdInvalidRequest))?
                .to_string();

            // Use streaming upload - chunks are written directly to disk
            // This avoids loading the entire file into memory
            let image = manager
                .create_from_multipart_field(&filename, field)
                .await?;
            return Ok(Json(image));
        }
    }

    Err(MsdErrorCode::MsdInvalidRequest.into())
}

/// Get image by ID
#[cfg(unix)]
pub async fn msd_image_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ImageInfo>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    let image = manager.get(&id)?;
    Ok(Json(image))
}

/// Delete image by ID
#[cfg(unix)]
pub async fn msd_image_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    let msd_guard = state.msd.read().await;
    let controller = msd_controller(&msd_guard)?;
    controller
        .delete_image(&id)
        .await
        .map_err(|error| operation_failed("delete image", error))?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Image deleted".to_string()),
    }))
}

/// Download image from URL
#[cfg(unix)]
pub async fn msd_image_download(
    State(state): State<Arc<AppState>>,
    payload: std::result::Result<Json<ImageDownloadRequest>, JsonRejection>,
) -> Result<Json<DownloadProgress>> {
    let req = parse_msd_json(payload)?;
    let msd_guard = state.msd.read().await;
    let controller = msd_controller(&msd_guard)?;

    let progress = controller
        .download_image(req.url, req.filename)
        .await
        .map_err(|error| operation_failed("start image download", error))?;

    Ok(Json(progress))
}

/// Cancel image download
#[cfg(unix)]
#[derive(serde::Deserialize)]
pub struct CancelDownloadRequest {
    pub download_id: String,
}

#[cfg(unix)]
pub async fn msd_image_download_cancel(
    State(state): State<Arc<AppState>>,
    payload: std::result::Result<Json<CancelDownloadRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>> {
    let req = parse_msd_json(payload)?;
    let msd_guard = state.msd.read().await;
    let controller = msd_controller(&msd_guard)?;

    controller
        .cancel_download(&req.download_id)
        .await
        .map_err(|error| operation_failed("cancel image download", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Download cancelled".to_string()),
    }))
}

/// Change MSD disk mode. This clears all mounted media and re-enumerates USB.
#[cfg(unix)]
pub async fn msd_disk_mode_put(
    State(state): State<Arc<AppState>>,
    payload: std::result::Result<Json<DiskModeRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>> {
    let req = parse_msd_json(payload)?;
    let _otg_guard = try_apply_lock(&state.config_apply_locks.otg, "OTG").map_err(|error| {
        tracing::warn!(%error, "MSD disk mode change is blocked by another OTG operation");
        AppError::from(MsdErrorCode::MsdOperationInProgress)
    })?;
    let current_mode = {
        let msd_guard = state.msd.read().await;
        let controller = msd_controller(&msd_guard)?;
        controller.state().await.disk_mode
    };
    if current_mode == req.disk_mode {
        return Ok(Json(LoginResponse {
            success: true,
            message: Some("MSD disk mode updated".to_string()),
        }));
    }

    let hid_is_otg = matches!(
        state.hid.backend_type().await,
        crate::hid::HidBackendType::Otg
    );

    if hid_is_otg {
        state
            .hid
            .prepare_otg_rebuild()
            .await
            .map_err(|error| operation_failed("prepare HID for disk mode switch", error))?;
    }

    let switch_result = {
        let mut msd_guard = state.msd.write().await;
        let controller = msd_guard
            .as_mut()
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;
        controller.set_disk_mode(req.disk_mode).await
    };

    let hid_reload_result = if hid_is_otg {
        state
            .hid
            .reload(crate::hid::HidBackendType::Otg)
            .await
            .map_err(|e| AppError::Config(format!("OTG HID reload failed: {e}")))
    } else {
        Ok(())
    };

    match (switch_result, hid_reload_result) {
        (Err(switch_error), Err(hid_error)) => {
            tracing::warn!(%switch_error, %hid_error, "MSD mode switch and HID recovery failed");
            return Err(MsdErrorCode::MsdOperationFailed.into());
        }
        (Err(switch_error), Ok(())) => {
            return Err(operation_failed("switch disk mode", switch_error))
        }
        (Ok(_), Err(hid_error)) => {
            return Err(operation_failed(
                "recover HID after disk mode switch",
                hid_error,
            ))
        }
        (Ok(_), Ok(())) => {}
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some("MSD disk mode updated".to_string()),
    }))
}

/// Mount an image into the next available media slot.
#[cfg(unix)]
pub async fn msd_image_mount(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    payload: std::result::Result<Json<ImageMountRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>> {
    let req = parse_msd_json(payload)?;
    let config = state.config.get();
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;

    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);
    let image = manager.get(&id)?;

    controller
        .mount_image(&image, req.cdrom, req.read_only)
        .await
        .map_err(|error| operation_failed("mount image", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Image mounted".to_string()),
    }))
}

/// Unmount an image from whichever internal LUN currently holds it.
#[cfg(unix)]
pub async fn msd_image_unmount(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;

    controller
        .unmount_image(&id)
        .await
        .map_err(|error| operation_failed("unmount image", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Image unmounted".to_string()),
    }))
}

/// Mount the virtual USB drive into the next available media slot.
#[cfg(unix)]
pub async fn msd_drive_mount(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;

    controller
        .mount_drive()
        .await
        .map_err(|error| operation_failed("mount virtual drive", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Virtual drive mounted".to_string()),
    }))
}

/// Unmount the virtual USB drive.
#[cfg(unix)]
pub async fn msd_drive_unmount(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;

    controller
        .unmount_drive()
        .await
        .map_err(|error| operation_failed("unmount virtual drive", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Virtual drive unmounted".to_string()),
    }))
}

/// Get drive info
#[cfg(unix)]
pub async fn msd_drive_info(State(state): State<Arc<AppState>>) -> Result<Json<DriveInfo>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let msd_guard = state.msd.read().await;
    let connected = match msd_guard.as_ref() {
        Some(controller) => controller.is_drive_connected().await,
        None => false,
    };

    // Never parse the filesystem while the USB host owns it. Metadata is safe
    // to read and still lets the UI show the backing image capacity.
    let info = if connected {
        drive.raw_info(DriveFileAccess::BlockedWhileConnected)
    } else {
        drive.info().await
    }
    .map_err(|error| operation_failed("read virtual drive info", error))?;

    if let Some(controller) = msd_guard.as_ref() {
        controller.set_drive_info(Some(info.clone())).await;
    }

    Ok(Json(info))
}

/// Initialize Ventoy drive
#[cfg(unix)]
pub async fn msd_drive_init(
    State(state): State<Arc<AppState>>,
    payload: std::result::Result<Json<DriveInitRequest>, JsonRejection>,
) -> Result<Json<DriveInfo>> {
    let req = parse_msd_json(payload)?;
    let config = state.config.get();
    let msd_dir = config.msd.msd_dir_path();

    let disk_space = get_disk_space(&msd_dir).map_err(|error| {
        tracing::warn!(%error, "Failed to read MSD storage space");
        AppError::from(MsdErrorCode::MsdStorageSpaceUnavailable)
    })?;
    validate_drive_init_size(req.size_mb, disk_space.available)?;

    // Mount/unmount handlers also take this outer write lock. Holding it
    // across image creation prevents a mount from racing the destructive
    // reinitialization after the connected-state check.
    let msd_guard = state.msd.write().await;
    if let Some(controller) = msd_guard.as_ref() {
        if controller.is_drive_connected().await {
            return Err(MsdErrorCode::MsdDriveConnected.into());
        }
    }

    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let info = drive
        .init(req.size_mb)
        .await
        .map_err(|error| operation_failed("initialize virtual drive", error))?;
    if let Some(controller) = msd_guard.as_ref() {
        controller.set_drive_info(Some(info.clone())).await;
    }
    Ok(Json(info))
}

/// Delete virtual drive
#[cfg(unix)]
pub async fn msd_drive_delete(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    let config = state.config.get();

    // Check if drive is currently connected
    let msd_guard = state.msd.write().await;
    if let Some(controller) = msd_guard.as_ref() {
        if controller.is_drive_connected().await {
            return Err(MsdErrorCode::MsdDriveConnected.into());
        }
    }
    // Delete the drive file
    let drive_path = config.msd.drive_path();
    if drive_path.exists() {
        std::fs::remove_file(&drive_path)
            .map_err(|error| classify_storage_error("delete virtual drive", error))?;
    }
    if let Some(controller) = msd_guard.as_ref() {
        controller.set_drive_info(None).await;
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Virtual drive deleted".to_string()),
    }))
}

/// List drive files
#[cfg(unix)]
pub async fn msd_drive_files(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<DriveFile>>> {
    // Block when connected: concurrent access corrupts the filesystem
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let dir_path = params.get("path").map(|s| s.as_str()).unwrap_or("/");
    let files = drive
        .list_files(dir_path)
        .await
        .map_err(|error| operation_failed("list virtual drive files", error))?;
    Ok(Json(files))
}

/// Upload file to drive (streaming - memory efficient for large files)
#[cfg(unix)]
pub async fn msd_drive_upload(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    multipart: std::result::Result<Multipart, MultipartRejection>,
) -> Result<Json<LoginResponse>> {
    let mut multipart = parse_msd_multipart(multipart)?;
    // Block when connected: writing to image while USB host has it mounted
    // causes filesystem corruption (Windows error 0x80070570)
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let target_dir = params.get("path").map(|s| s.as_str()).unwrap_or("/");

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!(%error, "Failed to parse virtual drive file upload");
        AppError::from(MsdErrorCode::MsdInvalidRequest)
    })? {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::from(MsdErrorCode::MsdInvalidRequest))?
                .to_string();

            let file_path = if target_dir == "/" {
                format!("/{}", filename)
            } else {
                format!("{}/{}", target_dir.trim_end_matches('/'), filename)
            };

            // Use streaming upload - chunks are written directly to disk
            // This avoids loading the entire file into memory
            drive
                .write_file_from_multipart_field(&file_path, field)
                .await
                .map_err(|error| operation_failed("upload virtual drive file", error))?;

            return Ok(Json(LoginResponse {
                success: true,
                message: Some(format!("File uploaded: {}", file_path)),
            }));
        }
    }

    Err(MsdErrorCode::MsdInvalidRequest.into())
}

/// Download file from drive (streaming for large files)
#[cfg(unix)]
pub async fn msd_drive_download(
    State(state): State<Arc<AppState>>,
    AxumPath(file_path): AxumPath<String>,
) -> Result<Response> {
    // Block when connected: concurrent read from server side can cause
    // filesystem inconsistency while USB host has the image mounted
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    // Get file stream (returns file size and channel receiver)
    let (file_size, mut rx) = drive
        .read_file_stream(&file_path)
        .await
        .map_err(|error| operation_failed("download virtual drive file", error))?;

    // Extract filename for Content-Disposition
    let filename = file_path.split('/').next_back().unwrap_or("download");

    // Create a stream from the channel receiver
    let body_stream = async_stream::stream! {
        while let Some(chunk) = rx.recv().await {
            yield chunk;
        }
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file_size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from_stream(body_stream))
        .unwrap())
}

/// Delete file from drive
#[cfg(unix)]
pub async fn msd_drive_file_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(file_path): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    // Block when connected: deleting from image while USB host has it mounted
    // causes filesystem corruption
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    drive
        .delete(&file_path)
        .await
        .map_err(|error| operation_failed("delete virtual drive file", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Deleted: {}", file_path)),
    }))
}

/// Create directory in drive
#[cfg(unix)]
pub async fn msd_drive_mkdir(
    State(state): State<Arc<AppState>>,
    AxumPath(dir_path): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    // Block when connected: modifying image while USB host has it mounted
    // causes filesystem corruption
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    drive
        .mkdir(&dir_path)
        .await
        .map_err(|error| operation_failed("create virtual drive directory", error))?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Directory created: {}", dir_path)),
    }))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn validate_drive_init_size_accepts_64mb() {
        validate_drive_init_size(MIN_DRIVE_SIZE_MB, MIN_DRIVE_SIZE_MB as u64 * MIB).unwrap();
    }

    #[test]
    fn validate_drive_init_size_rejects_below_64mb() {
        let err = validate_drive_init_size(MIN_DRIVE_SIZE_MB - 1, 1024 * MIB).unwrap_err();
        assert!(
            matches!(err, AppError::Msd(error) if error.code() == MsdErrorCode::MsdDriveSizeInvalid)
        );
    }

    #[test]
    fn validate_drive_init_size_rejects_available_space_overflow() {
        let err = validate_drive_init_size(65, 64 * MIB).unwrap_err();
        assert!(
            matches!(err, AppError::Msd(error) if error.code() == MsdErrorCode::MsdStorageFull)
        );
    }

    #[test]
    fn classifies_storage_permissions_without_exposing_the_io_error() {
        let error = classify_storage_error("test", std::io::Error::from_raw_os_error(libc::EACCES));
        assert!(
            matches!(error, AppError::Msd(error) if error.code() == MsdErrorCode::MsdStoragePermissionDenied)
        );
    }
}
