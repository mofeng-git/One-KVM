//! MSD Controller
//!
//! Manages the mass storage device lifecycle including:
//! - Image mounting and unmounting
//! - Virtual drive management
//! - State tracking
//! - Image downloads from URL

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::image::ImageManager;
use super::monitor::{MsdHealthMonitor, MsdHealthStatus};
use super::types::{DownloadProgress, DownloadStatus, DriveInfo, ImageInfo, MsdMode, MsdState};
use crate::error::{AppError, Result};
use crate::otg::{MsdFunction, MsdLunConfig, OtgService};

/// USB Gadget path (system constant)
const GADGET_PATH: &str = "/sys/kernel/config/usb_gadget/one-kvm";

/// MSD Controller
pub struct MsdController {
    /// OTG Service reference
    otg_service: Arc<OtgService>,
    /// MSD function manager (provided by OtgService)
    msd_function: RwLock<Option<MsdFunction>>,
    /// Current state
    state: RwLock<MsdState>,
    /// Images storage path
    images_path: PathBuf,
    /// Ventoy directory path
    ventoy_dir: PathBuf,
    /// Virtual drive path
    drive_path: PathBuf,
    /// Event bus for broadcasting state changes (optional)
    events: tokio::sync::RwLock<Option<Arc<crate::events::EventBus>>>,
    /// Active downloads (download_id -> CancellationToken)
    downloads: Arc<RwLock<HashMap<String, CancellationToken>>>,
    /// Operation mutex lock (prevents concurrent operations)
    operation_lock: Arc<RwLock<()>>,
    /// Health monitor for error tracking and recovery
    monitor: Arc<MsdHealthMonitor>,
}

impl MsdController {
    /// Create new MSD controller
    ///
    /// # Parameters
    /// * `otg_service` - OTG service for gadget management
    /// * `msd_dir` - Base directory for MSD storage
    pub fn new(
        otg_service: Arc<OtgService>,
        msd_dir: impl Into<PathBuf>,
    ) -> Self {
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

    /// Initialize the MSD controller
    pub async fn init(&self) -> Result<()> {
        info!("Initializing MSD controller");

        // 1. Ensure images directory exists
        if let Err(e) = std::fs::create_dir_all(&self.images_path) {
            warn!("Failed to create images directory: {}", e);
        }
        if let Err(e) = std::fs::create_dir_all(&self.ventoy_dir) {
            warn!("Failed to create ventoy directory: {}", e);
        }

        // 2. Request MSD function from OtgService
        info!("Requesting MSD function from OtgService");
        let msd_func = self.otg_service.enable_msd().await?;

        // 3. Store function handle
        *self.msd_function.write().await = Some(msd_func);

        // 4. Update state
        let mut state = self.state.write().await;
        state.available = true;

        // 5. Check for existing virtual drive
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

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> crate::events::SystemEvent {
        let state = self.state.read().await;
        crate::events::SystemEvent::MsdStateChanged {
            mode: state.mode.clone(),
            connected: state.connected,
        }
    }

    /// Get current MSD state
    pub async fn state(&self) -> MsdState {
        self.state.read().await.clone()
    }

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: std::sync::Arc<crate::events::EventBus>) {
        *self.events.write().await = Some(events.clone());
        // Also set event bus on the monitor for health notifications
        self.monitor.set_event_bus(events).await;
    }

    /// Publish an event to the event bus
    async fn publish_event(&self, event: crate::events::SystemEvent) {
        if let Some(ref bus) = *self.events.read().await {
            bus.publish(event);
        }
    }

    /// Check if MSD is available
    pub async fn is_available(&self) -> bool {
        self.state.read().await.available
    }

    /// Connect an image file
    ///
    /// # Parameters
    /// * `image` - Image info to mount
    /// * `cdrom` - Mount as CD-ROM (read-only, removable)
    /// * `read_only` - Mount as read-only
    pub async fn connect_image(
        &self,
        image: &ImageInfo,
        cdrom: bool,
        read_only: bool,
    ) -> Result<()> {
        // Acquire operation lock to prevent concurrent operations
        let _op_guard = self.operation_lock.write().await;

        let mut state = self.state.write().await;

        if !state.available {
            let err = AppError::Internal("MSD not available".to_string());
            self.monitor
                .report_error("MSD not available", "not_available")
                .await;
            return Err(err);
        }

        if state.connected {
            return Err(AppError::Internal(
                "Already connected. Disconnect first.".to_string(),
            ));
        }

        // Verify image exists
        if !image.path.exists() {
            let error_msg = format!("Image file not found: {}", image.path.display());
            self.monitor
                .report_error(&error_msg, "image_not_found")
                .await;
            return Err(AppError::Internal(error_msg));
        }

        // Configure LUN
        let config = if cdrom {
            MsdLunConfig::cdrom(image.path.clone())
        } else {
            MsdLunConfig::disk(image.path.clone(), read_only)
        };

        let gadget_path = PathBuf::from(GADGET_PATH);
        if let Some(ref msd) = *self.msd_function.read().await {
            if let Err(e) = msd.configure_lun_async(&gadget_path, 0, &config).await {
                let error_msg = format!("Failed to configure LUN: {}", e);
                self.monitor
                    .report_error(&error_msg, "configfs_error")
                    .await;
                return Err(e);
            }
        } else {
            let err = AppError::Internal("MSD function not initialized".to_string());
            self.monitor
                .report_error("MSD function not initialized", "not_initialized")
                .await;
            return Err(err);
        }

        state.connected = true;
        state.mode = MsdMode::Image;
        state.current_image = Some(image.clone());

        info!(
            "Connected image: {} (cdrom={}, ro={})",
            image.name, cdrom, read_only
        );

        // Release the lock before publishing events
        drop(state);
        drop(_op_guard);

        // Report recovery if we were in an error state
        if self.monitor.is_error().await {
            self.monitor.report_recovered().await;
        }

        // Publish events
        self.publish_event(crate::events::SystemEvent::MsdImageMounted {
            image_id: image.id.clone(),
            image_name: image.name.clone(),
            size: image.size,
            cdrom,
        })
        .await;

        self.publish_event(crate::events::SystemEvent::MsdStateChanged {
            mode: MsdMode::Image,
            connected: true,
        })
        .await;

        Ok(())
    }

    /// Connect the virtual drive
    pub async fn connect_drive(&self) -> Result<()> {
        // Acquire operation lock to prevent concurrent operations
        let _op_guard = self.operation_lock.write().await;

        let mut state = self.state.write().await;

        if !state.available {
            let err = AppError::Internal("MSD not available".to_string());
            self.monitor
                .report_error("MSD not available", "not_available")
                .await;
            return Err(err);
        }

        if state.connected {
            return Err(AppError::Internal(
                "Already connected. Disconnect first.".to_string(),
            ));
        }

        // Check drive exists
        if !self.drive_path.exists() {
            let err =
                AppError::Internal("Virtual drive not initialized. Call init first.".to_string());
            self.monitor
                .report_error("Virtual drive not initialized", "drive_not_found")
                .await;
            return Err(err);
        }

        // Configure LUN as read-write disk
        let config = MsdLunConfig::disk(self.drive_path.clone(), false);

        let gadget_path = PathBuf::from(GADGET_PATH);
        if let Some(ref msd) = *self.msd_function.read().await {
            if let Err(e) = msd.configure_lun_async(&gadget_path, 0, &config).await {
                let error_msg = format!("Failed to configure LUN: {}", e);
                self.monitor
                    .report_error(&error_msg, "configfs_error")
                    .await;
                return Err(e);
            }
        } else {
            let err = AppError::Internal("MSD function not initialized".to_string());
            self.monitor
                .report_error("MSD function not initialized", "not_initialized")
                .await;
            return Err(err);
        }

        state.connected = true;
        state.mode = MsdMode::Drive;
        state.current_image = None;

        info!("Connected virtual drive: {}", self.drive_path.display());

        // Release the lock before publishing event
        drop(state);
        drop(_op_guard);

        // Report recovery if we were in an error state
        if self.monitor.is_error().await {
            self.monitor.report_recovered().await;
        }

        // Publish event
        self.publish_event(crate::events::SystemEvent::MsdStateChanged {
            mode: MsdMode::Drive,
            connected: true,
        })
        .await;

        Ok(())
    }

    /// Disconnect current storage
    pub async fn disconnect(&self) -> Result<()> {
        // Acquire operation lock to prevent concurrent operations
        let _op_guard = self.operation_lock.write().await;

        let mut state = self.state.write().await;

        if !state.connected {
            debug!("Nothing connected, skipping disconnect");
            return Ok(());
        }

        let gadget_path = PathBuf::from(GADGET_PATH);
        if let Some(ref msd) = *self.msd_function.read().await {
            msd.disconnect_lun_async(&gadget_path, 0).await?;
        }

        state.connected = false;
        state.mode = MsdMode::None;
        state.current_image = None;

        info!("Disconnected storage");

        // Release the lock before publishing events
        drop(state);
        drop(_op_guard);

        // Publish events
        self.publish_event(crate::events::SystemEvent::MsdImageUnmounted)
            .await;

        self.publish_event(crate::events::SystemEvent::MsdStateChanged {
            mode: MsdMode::None,
            connected: false,
        })
        .await;

        Ok(())
    }

    /// Get images storage path
    pub fn images_path(&self) -> &PathBuf {
        &self.images_path
    }

    /// Get ventoy directory path
    pub fn ventoy_dir(&self) -> &PathBuf {
        &self.ventoy_dir
    }

    /// Get virtual drive path
    pub fn drive_path(&self) -> &PathBuf {
        &self.drive_path
    }

    /// Check if currently connected
    pub async fn is_connected(&self) -> bool {
        self.state.read().await.connected
    }

    /// Get current mode
    pub async fn mode(&self) -> MsdMode {
        self.state.read().await.mode.clone()
    }

    /// Update drive info
    pub async fn update_drive_info(&self, info: DriveInfo) {
        let mut state = self.state.write().await;
        state.drive_info = Some(info);
    }

    /// Start downloading an image from URL
    ///
    /// Returns the download_id that can be used to track or cancel the download.
    /// Progress is reported via MsdDownloadProgress events.
    pub async fn download_image(
        &self,
        url: String,
        filename: Option<String>,
    ) -> Result<DownloadProgress> {
        let download_id = uuid::Uuid::new_v4().to_string();
        let cancel_token = CancellationToken::new();

        // Register download
        {
            let mut downloads = self.downloads.write().await;
            downloads.insert(download_id.clone(), cancel_token.clone());
        }

        // Extract filename for initial response
        let display_filename = filename
            .clone()
            .unwrap_or_else(|| url.rsplit('/').next().unwrap_or("download").to_string());

        // Create initial progress
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

        // Publish started event
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

        // Clone what we need for the spawned task
        let images_path = self.images_path.clone();
        let events = self.events.read().await.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.clone();
        let url_clone = url.clone();

        // Spawn download task
        tokio::spawn(async move {
            let manager = ImageManager::new(images_path);

            // Create progress callback
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

            // Run download
            let result = manager
                .download_from_url(&url_clone, filename, progress_callback)
                .await;

            // Remove from active downloads
            {
                let mut downloads_guard = downloads.write().await;
                downloads_guard.remove(&download_id_clone);
            }

            // Publish completion event
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

    /// Cancel an active download
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

    /// Get list of active download IDs
    pub async fn active_downloads(&self) -> Vec<String> {
        let downloads = self.downloads.read().await;
        downloads.keys().cloned().collect()
    }

    /// Shutdown the controller
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MSD controller");

        // 1. Disconnect if connected
        if let Err(e) = self.disconnect().await {
            warn!("Error disconnecting during shutdown: {}", e);
        }

        // 2. Notify OtgService to disable MSD
        info!("Disabling MSD function in OtgService");
        self.otg_service.disable_msd().await?;

        // 3. Clear local state
        *self.msd_function.write().await = None;

        let mut state = self.state.write().await;
        state.available = false;

        info!("MSD controller shutdown complete");
        Ok(())
    }

    /// Get the health monitor reference
    pub fn monitor(&self) -> &Arc<MsdHealthMonitor> {
        &self.monitor
    }

    /// Get current health status
    pub async fn health_status(&self) -> MsdHealthStatus {
        self.monitor.status().await
    }

    /// Check if the MSD is healthy
    pub async fn is_healthy(&self) -> bool {
        self.monitor.is_healthy().await
    }
}

impl Drop for MsdController {
    fn drop(&mut self) {
        // Cleanup is handled by OtgGadgetManager when the gadget is torn down
        // Individual controllers don't need to cleanup the ConfigFS
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

        // Check that MSD is not initialized (msd_function is None)
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
