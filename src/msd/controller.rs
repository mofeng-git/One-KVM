use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::image::ImageManager;
use super::monitor::MsdHealthMonitor;
use super::types::{DownloadProgress, DownloadStatus, DriveInfo, ImageInfo, MsdMode, MsdState};
use crate::error::{AppError, Result};
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

    pub async fn init(&self) -> Result<()> {
        info!("Initializing MSD controller");

        if let Err(e) = std::fs::create_dir_all(&self.images_path) {
            warn!("Failed to create images directory: {}", e);
        }
        if let Err(e) = std::fs::create_dir_all(&self.ventoy_dir) {
            warn!("Failed to create ventoy directory: {}", e);
        }

        info!("Fetching MSD function from OtgService");
        let msd_func = self.otg_service.msd_function().await.ok_or_else(|| {
            AppError::Internal("MSD function is not active in OtgService".to_string())
        })?;

        *self.msd_function.write().await = Some(msd_func);

        let mut state = self.state.write().await;
        state.available = true;

        if self.drive_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&self.drive_path) {
                state.drive_info = Some(DriveInfo {
                    size: metadata.len(),
                    used: 0,
                    free: metadata.len(),
                    initialized: true,
                    path: self.drive_path.clone(),
                });
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

    pub async fn is_available(&self) -> bool {
        self.state.read().await.available
    }

    pub async fn connect_image(
        &self,
        image: &ImageInfo,
        cdrom: bool,
        read_only: bool,
    ) -> Result<()> {
        let _op_guard = self.operation_lock.write().await;
        let mut state = self.state.write().await;

        self.assert_can_connect(&state).await?;

        if !image.path.exists() {
            let error_msg = format!("Image file not found: {}", image.path.display());
            self.monitor
                .report_error(&error_msg, "image_not_found")
                .await;
            return Err(AppError::Internal(error_msg));
        }

        let config = if cdrom {
            MsdLunConfig::cdrom(image.path.clone())
        } else {
            MsdLunConfig::disk(image.path.clone(), read_only)
        };
        self.configure_lun_now(&config).await?;

        state.connected = true;
        state.mode = MsdMode::Image;
        state.current_image = Some(image.clone());

        info!(
            "Connected image: {} (cdrom={}, ro={})",
            image.name, cdrom, read_only
        );

        drop(state);
        drop(_op_guard);

        self.finish_connect_success().await;
        Ok(())
    }

    pub async fn connect_drive(&self) -> Result<()> {
        let _op_guard = self.operation_lock.write().await;
        let mut state = self.state.write().await;

        self.assert_can_connect(&state).await?;

        if !self.drive_path.exists() {
            let err =
                AppError::Internal("Virtual drive not initialized. Call init first.".to_string());
            self.monitor
                .report_error("Virtual drive not initialized", "drive_not_found")
                .await;
            return Err(err);
        }

        let config = MsdLunConfig::disk(self.drive_path.clone(), false);
        self.configure_lun_now(&config).await?;

        state.connected = true;
        state.mode = MsdMode::Drive;
        state.current_image = None;

        info!("Connected virtual drive: {}", self.drive_path.display());

        drop(state);
        drop(_op_guard);

        self.finish_connect_success().await;
        Ok(())
    }

    async fn assert_can_connect(&self, state: &MsdState) -> Result<()> {
        if !state.available {
            self.monitor
                .report_error("MSD not available", "not_available")
                .await;
            return Err(AppError::Internal("MSD not available".to_string()));
        }
        if state.connected {
            return Err(AppError::Internal(
                "Already connected. Disconnect first.".to_string(),
            ));
        }
        Ok(())
    }

    async fn configure_lun_now(&self, config: &MsdLunConfig) -> Result<()> {
        let gadget_path = self.active_gadget_path().await?;
        let msd_hold = self.msd_function.read().await;
        let Some(ref msd) = *msd_hold else {
            self.monitor
                .report_error("MSD function not initialized", "not_initialized")
                .await;
            return Err(AppError::Internal(
                "MSD function not initialized".to_string(),
            ));
        };
        if let Err(e) = msd.configure_lun_async(&gadget_path, 0, config).await {
            let error_msg = format!("Failed to configure LUN: {}", e);
            self.monitor
                .report_error(&error_msg, "configfs_error")
                .await;
            return Err(e);
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
        let _op_guard = self.operation_lock.write().await;

        let mut state = self.state.write().await;

        if !state.connected {
            debug!("Nothing connected, skipping disconnect");
            return Ok(());
        }

        let gadget_path = self.active_gadget_path().await?;
        if let Some(ref msd) = *self.msd_function.read().await {
            msd.disconnect_lun_async(&gadget_path, 0).await?;
        }

        state.connected = false;
        state.mode = MsdMode::None;
        state.current_image = None;

        info!("Disconnected storage");

        drop(state);
        drop(_op_guard);

        self.mark_device_info_dirty().await;

        Ok(())
    }

    pub fn images_path(&self) -> &PathBuf {
        &self.images_path
    }

    pub fn ventoy_dir(&self) -> &PathBuf {
        &self.ventoy_dir
    }

    pub fn drive_path(&self) -> &PathBuf {
        &self.drive_path
    }

    pub async fn is_connected(&self) -> bool {
        self.state.read().await.connected
    }

    pub async fn mode(&self) -> MsdMode {
        self.state.read().await.mode.clone()
    }

    pub async fn update_drive_info(&self, info: DriveInfo) {
        let mut state = self.state.write().await;
        state.drive_info = Some(info);
    }

    pub async fn download_image(
        &self,
        url: String,
        filename: Option<String>,
    ) -> Result<DownloadProgress> {
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
            error: None,
        };

        self.publish_event(crate::events::SystemEvent::MsdDownloadProgress {
            download_id: download_id.clone(),
            url: url.clone(),
            filename: display_filename.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
            progress_pct: None,
            status: "started".to_string(),
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
                        });
                    }
                }
                Err(e) => {
                    warn!("Download failed: {}", e);
                    if let Some(ref bus) = events {
                        bus.publish(crate::events::SystemEvent::MsdDownloadProgress {
                            download_id: download_id_clone,
                            url: url_clone,
                            filename: display_filename,
                            bytes_downloaded: 0,
                            total_bytes: None,
                            progress_pct: None,
                            status: format!("failed: {}", e),
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
            Err(AppError::NotFound(format!(
                "Download not found: {}",
                download_id
            )))
        }
    }

    async fn active_gadget_path(&self) -> Result<PathBuf> {
        self.otg_service
            .gadget_path()
            .await
            .ok_or_else(|| AppError::Internal("OTG gadget path is not available".to_string()))
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MSD controller");

        if let Err(e) = self.disconnect().await {
            warn!("Error disconnecting during shutdown: {}", e);
        }

        *self.msd_function.write().await = None;

        let mut state = self.state.write().await;
        state.available = false;

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
    async fn test_state_default() {
        let temp_dir = TempDir::new().unwrap();
        let otg_service = Arc::new(OtgService::new());
        let msd_dir = temp_dir.path().join("msd");

        let controller = MsdController::new(otg_service, &msd_dir);

        let state = controller.state().await;
        assert!(!state.available);
        assert!(!state.connected);
        assert_eq!(state.mode, MsdMode::None);
    }
}
