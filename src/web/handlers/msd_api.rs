use super::config::apply::try_apply_lock;
use super::*;

use crate::msd::{
    DiskModeRequest, DownloadProgress, DriveFile, DriveInfo, DriveInitRequest,
    ImageDownloadRequest, ImageInfo, ImageManager, ImageMountRequest, MsdState, MsdStateResponse,
    VentoyDrive, MIN_DRIVE_SIZE_MB,
};
#[cfg(unix)]
use axum::body::Body;
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
            return Err(AppError::BadRequest(
                "Virtual drive is connected to the USB host; disconnect it before modifying files"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_drive_init_size(size_mb: u32, available_bytes: u64) -> Result<()> {
    let requested_bytes = size_mb as u64 * MIB;
    if size_mb < MIN_DRIVE_SIZE_MB {
        return Err(AppError::BadRequest(format!(
            "Virtual drive size must be at least {} MB",
            MIN_DRIVE_SIZE_MB
        )));
    }
    if requested_bytes > available_bytes {
        return Err(AppError::BadRequest(format!(
            "Virtual drive size cannot exceed available space on the MSD directory filesystem (available {} MB, requested {} MB)",
            available_bytes / MIB,
            size_mb
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn is_unsupported_drive_filesystem(error: &str) -> bool {
    error.contains("Filesystem error")
        || error.contains("Image error")
        || error.contains("Partition error")
}

#[cfg(unix)]
fn unsupported_drive_filesystem_error(error: &str) -> AppError {
    tracing::warn!(
        error = %error,
        "Virtual drive filesystem is not supported"
    );
    AppError::BadRequest("Unsupported drive filesystem".to_string())
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
    mut multipart: Multipart,
) -> Result<Json<ImageInfo>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
                .to_string();

            // Use streaming upload - chunks are written directly to disk
            // This avoids loading the entire file into memory
            let image = manager
                .create_from_multipart_field(&filename, field)
                .await?;
            return Ok(Json(image));
        }
    }

    Err(AppError::BadRequest("No file provided".to_string()))
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
    let controller = msd_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;
    controller.delete_image(&id).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Image deleted".to_string()),
    }))
}

/// Download image from URL
#[cfg(unix)]
pub async fn msd_image_download(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImageDownloadRequest>,
) -> Result<Json<DownloadProgress>> {
    let msd_guard = state.msd.read().await;
    let controller = msd_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    let progress = controller.download_image(req.url, req.filename).await?;

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
    Json(req): Json<CancelDownloadRequest>,
) -> Result<Json<LoginResponse>> {
    let msd_guard = state.msd.read().await;
    let controller = msd_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.cancel_download(&req.download_id).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Download cancelled".to_string()),
    }))
}

/// Change MSD disk mode. This clears all mounted media and re-enumerates USB.
#[cfg(unix)]
pub async fn msd_disk_mode_put(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DiskModeRequest>,
) -> Result<Json<LoginResponse>> {
    let _otg_guard = try_apply_lock(&state.config_apply_locks.otg, "OTG")?;
    let current_mode = {
        let msd_guard = state.msd.read().await;
        let controller = msd_guard
            .as_ref()
            .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;
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
            .map_err(|e| AppError::Config(format!("Failed to prepare OTG HID for rebuild: {e}")))?;
    }

    let switch_result = {
        let mut msd_guard = state.msd.write().await;
        let controller = msd_guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;
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
            return Err(AppError::Internal(format!(
                "MSD disk mode switch failed: {switch_error}; HID recovery failed: {hid_error}"
            )));
        }
        (Err(switch_error), Ok(())) => return Err(switch_error),
        (Ok(_), Err(hid_error)) => return Err(hid_error),
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
    Json(req): Json<ImageMountRequest>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);
    let image = manager.get(&id)?;

    controller
        .mount_image(&image, req.cdrom, req.read_only)
        .await?;

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
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.unmount_image(&id).await?;

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
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.mount_drive().await?;

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
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.unmount_drive().await?;

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

    if !drive.exists() {
        // 404: drive image file does not exist at all — truly not initialized
        return Err(AppError::NotFound("Drive not initialized".to_string()));
    }

    match drive.info().await {
        Ok(info) => Ok(Json(info)),
        Err(e) => {
            let msg = e.to_string();
            // Detect filesystem-level failures (unrecognized format, bad partition table, etc.)
            // These mean the drive FILE exists but was formatted to an unsupported type
            // (e.g. the controlled machine reformatted it as NTFS/exFAT).
            // Return 400 so the frontend can distinguish this from 404 (file missing).
            if is_unsupported_drive_filesystem(&msg) {
                return Err(unsupported_drive_filesystem_error(&msg));
            }
            Err(e)
        }
    }
}

/// Initialize Ventoy drive
#[cfg(unix)]
pub async fn msd_drive_init(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DriveInitRequest>,
) -> Result<Json<DriveInfo>> {
    let config = state.config.get();
    let msd_dir = config.msd.msd_dir_path();

    let disk_space = get_disk_space(&msd_dir).map_err(|e| {
        AppError::BadRequest(format!(
            "Failed to read available space for the MSD directory filesystem: {}",
            e
        ))
    })?;
    validate_drive_init_size(req.size_mb, disk_space.available)?;

    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let info = drive.init(req.size_mb).await?;
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
            return Err(AppError::BadRequest(
                "Cannot delete drive while connected. Disconnect first.".to_string(),
            ));
        }
    }
    drop(msd_guard);

    // Delete the drive file
    let drive_path = config.msd.drive_path();
    if drive_path.exists() {
        std::fs::remove_file(&drive_path)
            .map_err(|e| AppError::Internal(format!("Failed to delete drive file: {}", e)))?;
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
    let files = drive.list_files(dir_path).await.map_err(|e| {
        // Provide a friendly message when the filesystem format is unrecognized
        // (e.g. user formatted it as NTFS/exFAT from the controlled machine)
        let msg = e.to_string();
        if is_unsupported_drive_filesystem(&msg) {
            unsupported_drive_filesystem_error(&msg)
        } else {
            e
        }
    })?;
    Ok(Json(files))
}

/// Upload file to drive (streaming - memory efficient for large files)
#[cfg(unix)]
pub async fn msd_drive_upload(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Result<Json<LoginResponse>> {
    // Block when connected: writing to image while USB host has it mounted
    // causes filesystem corruption (Windows error 0x80070570)
    assert_drive_not_connected(&state).await?;

    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let target_dir = params.get("path").map(|s| s.as_str()).unwrap_or("/");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
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
                .await?;

            return Ok(Json(LoginResponse {
                success: true,
                message: Some(format!("File uploaded: {}", file_path)),
            }));
        }
    }

    Err(AppError::BadRequest("No file provided".to_string()))
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
    let (file_size, mut rx) = drive.read_file_stream(&file_path).await?;

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

    drive.delete(&file_path).await?;

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

    drive.mkdir(&dir_path).await?;

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
        assert!(err.to_string().contains("at least 64 MB"));
    }

    #[test]
    fn validate_drive_init_size_rejects_available_space_overflow() {
        let err = validate_drive_init_size(65, 64 * MIB).unwrap_err();
        assert!(err.to_string().contains("cannot exceed available space"));
    }

    #[test]
    fn detects_unsupported_drive_filesystem_errors() {
        assert!(is_unsupported_drive_filesystem(
            "Internal error: Filesystem error: Invalid exFAT signature"
        ));
        assert!(is_unsupported_drive_filesystem(
            "Internal error: Partition error: invalid partition table"
        ));
        assert!(!is_unsupported_drive_filesystem(
            "IO error: permission denied"
        ));
    }
}
