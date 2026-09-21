use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::image::ImageManager;
use super::monitor::MsdHealthMonitor;
use super::types::{
    DiskMode, DownloadProgress, DownloadStatus, DriveFileAccess, DriveInfo, ImageInfo,
    MountedMedia, MountedMediaKind, MsdState,
};
use super::ventoy_drive::VentoyDrive;
use crate::error::{AppError, MsdErrorCode, Result};
use crate::otg::{MsdFunction, MsdLunConfig, OtgService};

pub struct MsdController {
    otg_service: Arc<OtgService>,
    msd_function: RwLock<Option<MsdFunction>>,
    state: RwLock<MsdState>,
    images_path: PathBuf,
    ventoy_dir: PathBuf,
    drive_path: PathBuf,
    events: tokio::sync::RwLock<Option<Arc<crate::events::EventBus>>>,
    downloads: Arc<RwLock<HashMap<String, CancellationToken>>>,
    operation_lock: Arc<RwLock<()>>,
    monitor: Arc<MsdHealthMonitor>,
}

impl MsdController {
    pub fn new(otg_service: Arc<OtgService>, msd_dir: impl Into<PathBuf>) -> Self {
        let msd_dir = msd_dir.into();
        let images_path = msd_dir.join("images");
        let ventoy_dir = msd_dir.join("ventoy");
        let drive_path = ventoy_dir.join("ventoy.img");
        Self {
            otg_service,
            msd_function: RwLock::new(None),
            state: RwLock::new(MsdState::default()),
            images_path,
            ventoy_dir,
            drive_path,
            events: tokio::sync::RwLock::new(None),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            operation_lock: Arc::new(RwLock::new(())),
            monitor: Arc::new(MsdHealthMonitor::with_defaults()),
        }
    }

    pub async fn init(&self, ventoy_resource_dir: &Path) -> Result<()> {
        info!("Initializing MSD controller");

        match ventoy_img::init_resources(ventoy_resource_dir) {
            Ok(()) => info!(
                "Ventoy resources ready from {}",
                ventoy_resource_dir.display()
            ),
            Err(e) => warn!(
                "Failed to initialize Ventoy resources from {}: {}. Ventoy drive creation will be unavailable, but regular ISO/IMG MSD remains available",
                ventoy_resource_dir.display(),
                e
            ),
        }

        tokio::fs::create_dir_all(&self.images_path).await?;
        tokio::fs::create_dir_all(&self.ventoy_dir).await?;

        info!("Fetching MSD function from OtgService");
        let msd_func = self
            .otg_service
            .msd_function()
            .await
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;

        *self.msd_function.write().await = Some(msd_func);

        let mut state = self.state.write().await;
        state.disk_mode = if self.otg_service.msd_lun_capacity().await == 1 {
            DiskMode::Single
        } else {
            DiskMode::Multi
        };
        state.available = true;

        if self.drive_path.exists() {
            if let Ok(drive_info) =
                VentoyDrive::new(self.drive_path.clone()).raw_info(DriveFileAccess::Unknown)
            {
                state.drive_info = Some(drive_info.clone());
                debug!(
                    "Found existing virtual drive: {}",
                    self.drive_path.display()
                );
            }
        }

        info!("MSD controller initialized");
        Ok(())
    }

    pub async fn state(&self) -> MsdState {
        self.state.read().await.clone()
    }

    pub async fn set_event_bus(&self, events: std::sync::Arc<crate::events::EventBus>) {
        *self.events.write().await = Some(events);
    }

    async fn publish_event(&self, event: crate::events::SystemEvent) {
        if let Some(ref bus) = *self.events.read().await {
            bus.publish(event);
        }
    }

    async fn mark_device_info_dirty(&self) {
        if let Some(ref bus) = *self.events.read().await {
            bus.mark_device_info_dirty();
        }
    }

    pub async fn mount_image(&self, image: &ImageInfo, cdrom: bool, read_only: bool) -> Result<()> {
        self.mount_image_in_slot(image, cdrom, read_only, None)
            .await
    }

    pub async fn mount_image_at_lun(
        &self,
        image: &ImageInfo,
        cdrom: bool,
        read_only: bool,
        lun: u8,
    ) -> Result<()> {
        self.mount_image_in_slot(image, cdrom, read_only, Some(lun))
            .await
    }

    async fn mount_image_in_slot(
        &self,
        image: &ImageInfo,
        cdrom: bool,
        read_only: bool,
        requested_lun: Option<u8>,
    ) -> Result<()> {
        let _op_guard = self.try_operation()?;
        let mut state = self.state.write().await;
        let previous_state = state.clone();

        self.assert_available(&state).await?;

        if !image.path.exists() {
            let error_msg = format!("Image file not found: {}", image.path.display());
            self.monitor
                .report_error(&error_msg, "image_not_found")
                .await;
            return Err(MsdErrorCode::MsdResourceNotFound.into());
        }

        if state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Image && media.id == image.id)
        {
            return Err(MsdErrorCode::MsdMediaAlreadyMounted.into());
        }

        let lun = Self::select_lun(&state, requested_lun)?;

        let media = MountedMedia::image(lun, image, cdrom, read_only);
        if let Err(e) = self.configure_media(&media).await {
            *state = previous_state;
            return Err(e);
        }
        state.mounted_media.push(media);

        info!(
            "Mounted image: {} on LUN {} (cdrom={}, ro={})",
            image.name,
            lun,
            cdrom,
            cdrom || read_only
        );

        drop(state);
        drop(_op_guard);

        self.finish_connect_success().await;
        Ok(())
    }

    pub async fn mount_drive(&self) -> Result<()> {
        let _op_guard = self.try_operation()?;
        let mut state = self.state.write().await;
        let previous_state = state.clone();

        self.assert_available(&state).await?;

        if state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive)
        {
            return Err(MsdErrorCode::MsdMediaAlreadyMounted.into());
        }

        let drive_info = match self.drive_mount_info() {
            Ok(info) => info,
            Err(error) => {
                if matches!(
                    &error,
                    AppError::Msd(msd) if msd.code() == MsdErrorCode::MsdDriveNotInitialized
                ) {
                    self.monitor
                        .report_error("Virtual drive not initialized", "drive_not_found")
                        .await;
                }
                return Err(error);
            }
        };
        state.drive_info = Some(drive_info.clone());

        let lun = Self::lowest_free_lun(&state)
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdMediaSlotsFull))?;

        let media = MountedMedia::drive(lun, &drive_info);
        if let Err(e) = self.configure_media(&media).await {
            *state = previous_state;
            return Err(e);
        }
        state.mounted_media.push(media);
        state.drive_info =
            Some(drive_info.with_file_access(DriveFileAccess::BlockedWhileConnected));

        info!(
            "Mounted virtual drive on LUN {}: {}",
            lun,
            self.drive_path.display()
        );

        drop(state);
        drop(_op_guard);

        self.finish_connect_success().await;
        Ok(())
    }

    fn drive_mount_info(&self) -> Result<DriveInfo> {
        VentoyDrive::new(self.drive_path.clone()).raw_info(DriveFileAccess::Unknown)
    }

    pub async fn set_drive_info(&self, drive_info: Option<DriveInfo>) {
        self.state.write().await.drive_info = drive_info;
        self.mark_device_info_dirty().await;
    }

    async fn assert_available(&self, state: &MsdState) -> Result<()> {
        if !state.available {
            self.monitor
                .report_error("MSD not available", "not_available")
                .await;
            return Err(MsdErrorCode::MsdUnavailable.into());
        }
        Ok(())
    }

    fn media_config(media: &MountedMedia) -> MsdLunConfig {
        if media.cdrom {
            MsdLunConfig::cdrom(media.path.clone())
        } else {
            MsdLunConfig::disk(media.path.clone(), media.read_only)
        }
    }

    fn lowest_free_lun(state: &MsdState) -> Option<u8> {
        (0..state.disk_mode.capacity())
            .find(|lun| !state.mounted_media.iter().any(|media| media.lun == *lun))
    }

    fn select_lun(state: &MsdState, requested_lun: Option<u8>) -> Result<u8> {
        let Some(lun) = requested_lun else {
            return Self::lowest_free_lun(state)
                .ok_or_else(|| AppError::from(MsdErrorCode::MsdMediaSlotsFull));
        };

        if lun >= state.disk_mode.capacity() {
            return Err(MsdErrorCode::MsdInvalidRequest.into());
        }
        if state.mounted_media.iter().any(|media| media.lun == lun) {
            return Err(MsdErrorCode::MsdMediaSlotsFull.into());
        }
        Ok(lun)
    }

    fn reset_mounts_for_mode(state: &mut MsdState, disk_mode: DiskMode) {
        if state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive)
        {
            state.drive_info = state
                .drive_info
                .take()
                .map(|info| info.with_file_access(DriveFileAccess::Unknown));
        }
        state.disk_mode = disk_mode;
        state.mounted_media.clear();
    }

    pub async fn set_disk_mode(&self, disk_mode: DiskMode) -> Result<bool> {
        let _op_guard = self.try_operation()?;
        let previous_state = {
            let mut state = self.state.write().await;
            self.assert_available(&state).await?;
            if state.disk_mode == disk_mode {
                return Ok(false);
            }
            let previous_state = state.clone();
            state.usb_reenumerating = true;
            previous_state
        };
        self.mark_device_info_dirty().await;

        let switch_result = async {
            self.otg_service
                .set_msd_lun_capacity(disk_mode.capacity())
                .await?;
            self.otg_service
                .msd_function()
                .await
                .ok_or_else(|| AppError::from(MsdErrorCode::MsdOperationFailed))
        }
        .await;

        let msd_function = match switch_result {
            Ok(msd_function) => msd_function,
            Err(switch_error) => {
                if let Err(rollback_error) = self.rollback_mode_switch(&previous_state).await {
                    let mut state = self.state.write().await;
                    state.available = false;
                    state.mounted_media.clear();
                    state.usb_reenumerating = false;
                    *self.msd_function.write().await = None;
                    let error_msg = format!(
                        "Failed to switch MSD disk mode: {switch_error}; rollback failed: {rollback_error}"
                    );
                    self.monitor
                        .report_error(&error_msg, "disk_mode_rollback_failed")
                        .await;
                    self.mark_device_info_dirty().await;
                    return Err(MsdErrorCode::MsdOperationFailed.into());
                }

                let mut state = self.state.write().await;
                *state = previous_state;
                state.usb_reenumerating = false;
                let error_msg = format!("Failed to switch MSD disk mode: {switch_error}");
                self.monitor
                    .report_error(&error_msg, "disk_mode_switch_failed")
                    .await;
                self.mark_device_info_dirty().await;
                return Err(MsdErrorCode::MsdOperationFailed.into());
            }
        };
        *self.msd_function.write().await = Some(msd_function);

        let mut state = self.state.write().await;
        Self::reset_mounts_for_mode(&mut state, disk_mode);
        state.usb_reenumerating = false;
        info!("Switched MSD disk mode to {:?}", disk_mode);

        drop(state);
        drop(_op_guard);

        self.mark_device_info_dirty().await;
        Ok(true)
    }

    pub async fn unmount_image(&self, image_id: &str) -> Result<()> {
        self.unmount_media(|media| media.kind == MountedMediaKind::Image && media.id == image_id)
            .await
            .map(|_| ())
    }

    pub async fn unmount_drive(&self) -> Result<()> {
        self.unmount_media(|media| media.kind == MountedMediaKind::Drive)
            .await
            .map(|_| ())
    }

    pub async fn unmount_lun(&self, lun: u8) -> Result<bool> {
        self.unmount_media(|media| media.lun == lun).await
    }

    async fn unmount_media<F>(&self, predicate: F) -> Result<bool>
    where
        F: Fn(&MountedMedia) -> bool,
    {
        let _op_guard = self.try_operation()?;

        let mut state = self.state.write().await;
        let Some(index) = state.mounted_media.iter().position(predicate) else {
            debug!("Requested media was not mounted, skipping unmount");
            return Ok(false);
        };
        let media = state.mounted_media[index].clone();

        self.disconnect_lun(media.lun).await?;
        state.mounted_media.remove(index);
        if media.kind == MountedMediaKind::Drive {
            state.drive_info = state
                .drive_info
                .take()
                .map(|info| info.with_file_access(DriveFileAccess::Unknown));
        }
        info!("Unmounted media");

        drop(state);
        drop(_op_guard);

        self.mark_device_info_dirty().await;

        Ok(true)
    }

    async fn configure_media(&self, media: &MountedMedia) -> Result<()> {
        if let Err(e) = self
            .otg_service
            .configure_msd_lun(media.lun, &Self::media_config(media))
            .await
        {
            let error_msg = format!("Failed to configure LUN {}: {}", media.lun, e);
            self.monitor
                .report_error(&error_msg, "configfs_error")
                .await;
            return Err(match e {
                AppError::Msd(error) => AppError::Msd(error),
                error => {
                    warn!(%error, "Unclassified MSD media configuration failure");
                    MsdErrorCode::MsdOperationFailed.into()
                }
            });
        }
        Ok(())
    }

    async fn disconnect_lun(&self, lun: u8) -> Result<()> {
        let gadget_path = self.active_gadget_path().await?;
        let msd_hold = self.msd_function.read().await;
        let msd = msd_hold
            .as_ref()
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;
        msd.disconnect_lun_async(&gadget_path, lun).await
    }

    async fn rollback_mode_switch(&self, previous_state: &MsdState) -> Result<()> {
        self.otg_service
            .set_msd_lun_capacity(previous_state.disk_mode.capacity())
            .await?;
        let msd_function = self
            .otg_service
            .msd_function()
            .await
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdOperationFailed))?;
        *self.msd_function.write().await = Some(msd_function);
        for media in &previous_state.mounted_media {
            self.configure_media(media).await?;
        }
        Ok(())
    }

    async fn finish_connect_success(&self) {
        if self.monitor.is_error().await {
            self.monitor.report_recovered().await;
        }
        self.mark_device_info_dirty().await;
    }

    pub async fn disconnect(&self) -> Result<()> {
        let _op_guard = self.try_operation()?;

        let mut state = self.state.write().await;
        if state.mounted_media.is_empty() {
            debug!("Nothing mounted, skipping disconnect");
            return Ok(());
        }

        let mounted_media = state.mounted_media.clone();
        let mut disconnected = Vec::new();
        for media in &mounted_media {
            if let Err(error) = self.disconnect_lun(media.lun).await {
                for prior in &disconnected {
                    if let Err(restore_error) = self.configure_media(prior).await {
                        state.available = false;
                        warn!(
                            lun = media.lun,
                            disconnect_error = %error,
                            %restore_error,
                            "Failed to restore MSD media after disconnect failure"
                        );
                        return Err(MsdErrorCode::MsdDisconnectFailed.into());
                    }
                }
                return Err(error);
            }
            disconnected.push(media.clone());
        }

        if state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive)
        {
            state.drive_info = state
                .drive_info
                .take()
                .map(|info| info.with_file_access(DriveFileAccess::Unknown));
        }
        state.mounted_media.clear();
        info!("Disconnected all mounted media");

        drop(state);
        drop(_op_guard);

        self.mark_device_info_dirty().await;

        Ok(())
    }

    pub async fn is_drive_connected(&self) -> bool {
        self.state
            .read()
            .await
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive)
    }

    pub async fn delete_image(&self, image_id: &str) -> Result<()> {
        let _op_guard = self.try_operation()?;
        let state = self.state.read().await;
        if state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Image && media.id == image_id)
        {
            return Err(MsdErrorCode::MsdMediaInUse.into());
        }

        ImageManager::new(self.images_path.clone()).delete(image_id)
    }

    pub async fn download_image(
        &self,
        url: String,
        filename: Option<String>,
    ) -> Result<DownloadProgress> {
        let parsed_url =
            reqwest::Url::parse(&url).map_err(|_| AppError::from(MsdErrorCode::MsdInvalidUrl))?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(MsdErrorCode::MsdInvalidUrl.into());
        }

        let download_id = uuid::Uuid::new_v4().to_string();
        let cancel_token = CancellationToken::new();

        {
            let mut downloads = self.downloads.write().await;
            downloads.insert(download_id.clone(), cancel_token.clone());
        }

        let display_filename = filename
            .clone()
            .unwrap_or_else(|| url.rsplit('/').next().unwrap_or("download").to_string());

        let initial_progress = DownloadProgress {
            download_id: download_id.clone(),
            url: url.clone(),
            filename: display_filename.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
            progress_pct: None,
            status: DownloadStatus::Started,
            error_code: None,
        };

        self.publish_event(crate::events::SystemEvent::MsdDownloadProgress {
            download_id: download_id.clone(),
            url: url.clone(),
            filename: display_filename.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
            progress_pct: None,
            status: "started".to_string(),
            error_code: None,
        })
        .await;

        let images_path = self.images_path.clone();
        let events = self.events.read().await.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.clone();
        let url_clone = url.clone();

        tokio::spawn(async move {
            let manager = ImageManager::new(images_path);

            let events_for_callback = events.clone();
            let download_id_for_callback = download_id_clone.clone();
            let url_for_callback = url_clone.clone();
            let filename_for_callback = display_filename.clone();

            let progress_callback = move |downloaded: u64, total: Option<u64>| {
                let progress_pct = total.map(|t| (downloaded as f32 / t as f32) * 100.0);

                if let Some(ref bus) = events_for_callback {
                    bus.publish(crate::events::SystemEvent::MsdDownloadProgress {
                        download_id: download_id_for_callback.clone(),
                        url: url_for_callback.clone(),
                        filename: filename_for_callback.clone(),
                        bytes_downloaded: downloaded,
                        total_bytes: total,
                        progress_pct,
                        status: "in_progress".to_string(),
                        error_code: None,
                    });
                }
            };

            let result = manager
                .download_from_url(&url_clone, filename, progress_callback)
                .await;

            {
                let mut downloads_guard = downloads.write().await;
                downloads_guard.remove(&download_id_clone);
            }

            match result {
                Ok(image_info) => {
                    if let Some(ref bus) = events {
                        bus.publish(crate::events::SystemEvent::MsdDownloadProgress {
                            download_id: download_id_clone,
                            url: url_clone,
                            filename: image_info.name,
                            bytes_downloaded: image_info.size,
                            total_bytes: Some(image_info.size),
                            progress_pct: Some(100.0),
                            status: "completed".to_string(),
                            error_code: None,
                        });
                    }
                }
                Err(e) => {
                    warn!(error = %e, "MSD image download failed");
                    let code = match e {
                        AppError::Msd(error) => error.code(),
                        _ => MsdErrorCode::MsdOperationFailed,
                    };
                    if let Some(ref bus) = events {
                        bus.publish(crate::events::SystemEvent::MsdDownloadProgress {
                            download_id: download_id_clone,
                            url: url_clone,
                            filename: display_filename,
                            bytes_downloaded: 0,
                            total_bytes: None,
                            progress_pct: None,
                            status: "failed".to_string(),
                            error_code: Some(code.as_str().to_string()),
                        });
                    }
                }
            }
        });

        Ok(initial_progress)
    }

    pub async fn cancel_download(&self, download_id: &str) -> Result<()> {
        let mut downloads = self.downloads.write().await;

        if let Some(token) = downloads.remove(download_id) {
            token.cancel();
            info!("Download cancelled: {}", download_id);
            Ok(())
        } else {
            Err(MsdErrorCode::MsdResourceNotFound.into())
        }
    }

    async fn active_gadget_path(&self) -> Result<PathBuf> {
        self.otg_service
            .gadget_path()
            .await
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))
    }

    fn try_operation(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, ()>> {
        self.operation_lock
            .try_write()
            .map_err(|_| MsdErrorCode::MsdOperationInProgress.into())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MSD controller");

        if let Err(e) = self.disconnect().await {
            warn!("Error disconnecting during shutdown: {}", e);
        }

        *self.msd_function.write().await = None;

        let mut state = self.state.write().await;
        state.available = false;
        state.mounted_media.clear();
        state.usb_reenumerating = false;

        info!("MSD controller shutdown complete");
        Ok(())
    }

    pub fn monitor(&self) -> &Arc<MsdHealthMonitor> {
        &self.monitor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msd::MULTI_DISK_MSD_LUNS;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_controller_creation() {
        let temp_dir = TempDir::new().unwrap();
        let otg_service = Arc::new(OtgService::new());
        let msd_dir = temp_dir.path().join("msd");

        let controller = MsdController::new(otg_service, &msd_dir);

        let state = controller.state().await;
        assert!(!state.available);
        assert!(controller.images_path.ends_with("images"));
        assert!(controller.drive_path.ends_with("ventoy.img"));
    }

    #[tokio::test]
    async fn concurrent_operations_have_a_stable_error_code() {
        let temp_dir = TempDir::new().unwrap();
        let controller = MsdController::new(Arc::new(OtgService::new()), temp_dir.path());
        let _guard = controller.operation_lock.write().await;

        assert!(matches!(
            controller.try_operation().unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdOperationInProgress
        ));
    }

    #[tokio::test]
    async fn test_state_default() {
        let temp_dir = TempDir::new().unwrap();
        let otg_service = Arc::new(OtgService::new());
        let msd_dir = temp_dir.path().join("msd");

        let controller = MsdController::new(otg_service, &msd_dir);

        let state = controller.state().await;
        assert!(!state.available);
        assert_eq!(state.disk_mode, DiskMode::Single);
        assert!(state.mounted_media.is_empty());
    }

    #[tokio::test]
    async fn drive_mount_metadata_ignores_cached_drive_info() {
        let temp_dir = TempDir::new().unwrap();
        let controller = MsdController::new(Arc::new(OtgService::new()), temp_dir.path());
        std::fs::create_dir_all(&controller.ventoy_dir).unwrap();
        std::fs::write(&controller.drive_path, vec![0u8; 128]).unwrap();
        controller.state.write().await.drive_info = Some(DriveInfo::from_raw(
            controller.drive_path.clone(),
            64,
            DriveFileAccess::Available,
        ));

        std::fs::write(&controller.drive_path, vec![0u8; 256]).unwrap();
        let info = controller.drive_mount_info().unwrap();

        assert_eq!(info.size, 256);
        assert_eq!(info.used, None);
        assert_eq!(info.file_access, DriveFileAccess::Unknown);
        let media = MountedMedia::drive(0, &info);
        let config = MsdController::media_config(&media);
        assert_eq!(config.file, controller.drive_path);
    }

    #[test]
    fn single_disk_mode_only_exposes_lun_zero() {
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Single);
        assert_eq!(state.disk_mode.capacity(), 1);
        assert_eq!(MsdController::lowest_free_lun(&state), Some(0));

        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.iso");
        std::fs::write(&image_path, b"iso").unwrap();
        let image = ImageInfo::new("test".into(), "test.iso".into(), image_path, 3);
        state
            .mounted_media
            .push(MountedMedia::image(0, &image, true, false));

        assert_eq!(MsdController::lowest_free_lun(&state), None);
        let config = MsdController::media_config(&state.mounted_media[0]);
        assert!(config.cdrom);
        assert!(config.ro);
    }

    #[test]
    fn multi_disk_mode_allocates_lowest_free_lun() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Multi);

        for lun in [0, 1, 3] {
            let image_path = temp_dir.path().join(format!("test{lun}.img"));
            std::fs::write(&image_path, b"img").unwrap();
            let image = ImageInfo::new(
                format!("test{lun}"),
                format!("test{lun}.img"),
                image_path,
                3,
            );
            state
                .mounted_media
                .push(MountedMedia::image(lun, &image, false, false));
        }

        assert_eq!(MsdController::lowest_free_lun(&state), Some(2));
    }

    #[test]
    fn explicit_lun_selection_rejects_occupied_and_out_of_range_slots() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.img");
        std::fs::write(&image_path, b"img").unwrap();
        let image = ImageInfo::new("test".into(), "test.img".into(), image_path, 3);
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Multi);
        state
            .mounted_media
            .push(MountedMedia::image(3, &image, false, true));

        assert_eq!(MsdController::select_lun(&state, Some(5)).unwrap(), 5);
        assert!(matches!(
            MsdController::select_lun(&state, Some(3)).unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdMediaSlotsFull
        ));
        assert!(matches!(
            MsdController::select_lun(&state, Some(8)).unwrap_err(),
            AppError::Msd(error) if error.code() == MsdErrorCode::MsdInvalidRequest
        ));
    }

    #[test]
    fn multi_disk_mode_supports_eight_images_and_rejects_ninth_slot() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Multi);

        for lun in 0..MULTI_DISK_MSD_LUNS {
            let image_path = temp_dir.path().join(format!("test{lun}.img"));
            std::fs::write(&image_path, b"img").unwrap();
            let image = ImageInfo::new(
                format!("test{lun}"),
                format!("test{lun}.img"),
                image_path,
                3,
            );
            let next_lun = MsdController::lowest_free_lun(&state).unwrap();
            assert_eq!(next_lun, lun);
            state
                .mounted_media
                .push(MountedMedia::image(next_lun, &image, false, false));
        }

        assert_eq!(state.mounted_media.len(), 8);
        assert_eq!(MsdController::lowest_free_lun(&state), None);
    }

    #[test]
    fn multi_disk_mode_supports_drive_plus_seven_images() {
        let temp_dir = TempDir::new().unwrap();
        let drive_path = temp_dir.path().join("ventoy.img");
        std::fs::write(&drive_path, b"drive").unwrap();
        let drive = DriveInfo::from_raw(drive_path, 5, DriveFileAccess::Unknown);
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Multi);
        state.mounted_media.push(MountedMedia::drive(0, &drive));

        for lun in 1..MULTI_DISK_MSD_LUNS {
            let image_path = temp_dir.path().join(format!("test{lun}.img"));
            std::fs::write(&image_path, b"img").unwrap();
            let image = ImageInfo::new(
                format!("test{lun}"),
                format!("test{lun}.img"),
                image_path,
                3,
            );
            state
                .mounted_media
                .push(MountedMedia::image(lun, &image, false, false));
        }

        assert_eq!(state.mounted_media.len(), 8);
        assert_eq!(MsdController::lowest_free_lun(&state), None);
        assert!(state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive));
    }

    #[test]
    fn mode_switch_clears_mount_state() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.img");
        std::fs::write(&image_path, b"img").unwrap();
        let image = ImageInfo::new("test".into(), "test.img".into(), image_path, 3);
        let mut state = MsdState::default();
        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Multi);
        state
            .mounted_media
            .push(MountedMedia::image(0, &image, false, false));

        MsdController::reset_mounts_for_mode(&mut state, DiskMode::Single);

        assert_eq!(state.disk_mode, DiskMode::Single);
        assert_eq!(state.disk_mode.capacity(), 1);
        assert!(state.mounted_media.is_empty());
    }

    #[test]
    fn duplicate_image_and_drive_detection_use_media_identity() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.img");
        std::fs::write(&image_path, b"img").unwrap();
        let image = ImageInfo::new("test".into(), "test.img".into(), image_path, 3);
        let drive = DriveInfo::from_raw(
            temp_dir.path().join("ventoy.img"),
            5,
            DriveFileAccess::Unknown,
        );
        let mut state = MsdState::default();
        state
            .mounted_media
            .push(MountedMedia::image(0, &image, false, false));
        state.mounted_media.push(MountedMedia::drive(1, &drive));

        assert!(state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Image && media.id == "test"));
        assert!(state
            .mounted_media
            .iter()
            .any(|media| media.kind == MountedMediaKind::Drive));
    }

    #[tokio::test]
    async fn delete_image_is_serialized_with_mount_operations() {
        let temp_dir = TempDir::new().unwrap();
        let otg_service = Arc::new(OtgService::new());
        let controller = MsdController::new(otg_service, temp_dir.path());
        std::fs::create_dir_all(&controller.images_path).unwrap();
        let image_path = controller.images_path.join("test.img");
        std::fs::write(&image_path, b"img").unwrap();
        let image = ImageManager::new(controller.images_path.clone())
            .get_by_name("test.img")
            .unwrap();

        controller
            .state
            .write()
            .await
            .mounted_media
            .push(MountedMedia::image(0, &image, false, false));
        assert!(controller.delete_image(&image.id).await.is_err());
        assert!(image_path.exists());

        controller.state.write().await.mounted_media.clear();
        controller.delete_image(&image.id).await.unwrap();
        assert!(!image_path.exists());
    }

    #[test]
    fn slot_configs_force_cdrom_read_only() {
        let temp_dir = TempDir::new().unwrap();
        let image_path = temp_dir.path().join("test.iso");
        std::fs::write(&image_path, b"iso").unwrap();
        let image = ImageInfo::new("test".into(), "test.iso".into(), image_path, 3);
        let mut state = MsdState::default();
        state
            .mounted_media
            .push(MountedMedia::image(0, &image, true, false));

        let config = MsdController::media_config(&state.mounted_media[0]);
        assert!(config.cdrom);
        assert!(config.ro);
    }
}
