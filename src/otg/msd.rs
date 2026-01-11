//! MSD (Mass Storage Device) Function implementation for USB Gadget

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use super::configfs::{create_dir, create_symlink, remove_dir, remove_file, write_file};
use super::function::{FunctionMeta, GadgetFunction};
use crate::error::{AppError, Result};

/// MSD LUN configuration
#[derive(Debug, Clone)]
pub struct MsdLunConfig {
    /// File/image path to expose
    pub file: PathBuf,
    /// Mount as CD-ROM
    pub cdrom: bool,
    /// Read-only mode
    pub ro: bool,
    /// Removable media
    pub removable: bool,
    /// Disable Force Unit Access
    pub nofua: bool,
}

impl Default for MsdLunConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::new(),
            cdrom: false,
            ro: false,
            removable: true,
            nofua: true,
        }
    }
}

impl MsdLunConfig {
    /// Create CD-ROM configuration
    pub fn cdrom(file: PathBuf) -> Self {
        Self {
            file,
            cdrom: true,
            ro: true,
            removable: true,
            nofua: true,
        }
    }

    /// Create disk configuration
    pub fn disk(file: PathBuf, read_only: bool) -> Self {
        Self {
            file,
            cdrom: false,
            ro: read_only,
            removable: true,
            nofua: true,
        }
    }
}

/// MSD Function for USB Gadget
#[derive(Debug, Clone)]
pub struct MsdFunction {
    /// Instance number (usb0, usb1, ...)
    instance: u8,
    /// Cached function name (avoids repeated allocation)
    name: String,
}

impl MsdFunction {
    /// Create a new MSD function
    pub fn new(instance: u8) -> Self {
        Self {
            instance,
            name: format!("mass_storage.usb{}", instance),
        }
    }

    /// Get function path in gadget
    fn function_path(&self, gadget_path: &Path) -> PathBuf {
        gadget_path.join("functions").join(self.name())
    }

    /// Get LUN path
    fn lun_path(&self, gadget_path: &Path, lun: u8) -> PathBuf {
        self.function_path(gadget_path).join(format!("lun.{}", lun))
    }

    /// Configure a LUN with specified settings (async version)
    ///
    /// This is the preferred method for async contexts. It runs the blocking
    /// file I/O and USB timing operations in a separate thread pool.
    pub async fn configure_lun_async(
        &self,
        gadget_path: &Path,
        lun: u8,
        config: &MsdLunConfig,
    ) -> Result<()> {
        let gadget_path = gadget_path.to_path_buf();
        let config = config.clone();
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.configure_lun(&gadget_path, lun, &config))
            .await
            .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// Configure a LUN with specified settings
    /// Note: This should be called after the gadget is set up
    ///
    /// This implementation is based on PiKVM's MSD drive configuration.
    /// Key improvements:
    /// - Uses forced_eject when available (safer than clearing file directly)
    /// - Reduced sleep times to minimize HID interference
    /// - Better retry logic for EBUSY errors
    ///
    /// **Note**: This is a blocking function. In async contexts, prefer
    /// `configure_lun_async` to avoid blocking the runtime.
    pub fn configure_lun(&self, gadget_path: &Path, lun: u8, config: &MsdLunConfig) -> Result<()> {
        let lun_path = self.lun_path(gadget_path, lun);

        if !lun_path.exists() {
            create_dir(&lun_path)?;
        }

        // Batch read all current values to minimize syscalls
        let read_attr = |attr: &str| -> String {
            fs::read_to_string(lun_path.join(attr))
                .unwrap_or_default()
                .trim()
                .to_string()
        };

        let current_cdrom = read_attr("cdrom");
        let current_ro = read_attr("ro");
        let current_removable = read_attr("removable");
        let current_nofua = read_attr("nofua");

        // Prepare new values
        let new_cdrom = if config.cdrom { "1" } else { "0" };
        let new_ro = if config.ro { "1" } else { "0" };
        let new_removable = if config.removable { "1" } else { "0" };
        let new_nofua = if config.nofua { "1" } else { "0" };

        // Disconnect current file first using forced_eject if available (PiKVM approach)
        let forced_eject_path = lun_path.join("forced_eject");
        if forced_eject_path.exists() {
            // forced_eject is safer - it forcibly detaches regardless of host state
            debug!("Using forced_eject to clear LUN {}", lun);
            let _ = write_file(&forced_eject_path, "1");
        } else {
            // Fallback to clearing file directly
            let _ = write_file(&lun_path.join("file"), "");
        }

        // Brief yield to allow USB stack to process the disconnect
        // Reduced from 200ms to 50ms - let USB protocol handle timing
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Write only changed attributes
        let cdrom_changed = current_cdrom != new_cdrom;
        if cdrom_changed {
            debug!(
                "Updating LUN {} cdrom: {} -> {}",
                lun, current_cdrom, new_cdrom
            );
            write_file(&lun_path.join("cdrom"), new_cdrom)?;
        }
        if current_ro != new_ro {
            debug!("Updating LUN {} ro: {} -> {}", lun, current_ro, new_ro);
            write_file(&lun_path.join("ro"), new_ro)?;
        }
        if current_removable != new_removable {
            debug!(
                "Updating LUN {} removable: {} -> {}",
                lun, current_removable, new_removable
            );
            write_file(&lun_path.join("removable"), new_removable)?;
        }
        if current_nofua != new_nofua {
            debug!(
                "Updating LUN {} nofua: {} -> {}",
                lun, current_nofua, new_nofua
            );
            write_file(&lun_path.join("nofua"), new_nofua)?;
        }

        // If cdrom mode changed, brief yield for USB host
        if cdrom_changed {
            debug!("CDROM mode changed, brief yield for USB host");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Set file path (this triggers the actual mount) - with retry on EBUSY
        if config.file.exists() {
            let file_path = config.file.to_string_lossy();
            let mut last_error = None;

            for attempt in 0..5 {
                match write_file(&lun_path.join("file"), file_path.as_ref()) {
                    Ok(_) => {
                        info!(
                            "LUN {} configured with file: {} (cdrom={}, ro={})",
                            lun,
                            config.file.display(),
                            config.cdrom,
                            config.ro
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        // Check if it's EBUSY (error code 16)
                        let is_busy = e.to_string().contains("Device or resource busy")
                            || e.to_string().contains("os error 16");

                        if is_busy && attempt < 4 {
                            warn!(
                                "LUN {} file write busy, retrying (attempt {}/5)",
                                lun,
                                attempt + 1
                            );
                            // Exponential backoff: 50, 100, 200, 400ms
                            std::thread::sleep(std::time::Duration::from_millis(50 << attempt));
                            last_error = Some(e);
                            continue;
                        }

                        return Err(e);
                    }
                }
            }

            // If we get here, all retries failed
            if let Some(e) = last_error {
                return Err(e);
            }
        } else if !config.file.as_os_str().is_empty() {
            warn!("LUN {} file does not exist: {}", lun, config.file.display());
        }

        Ok(())
    }

    /// Disconnect LUN (async version)
    ///
    /// Preferred for async contexts.
    pub async fn disconnect_lun_async(&self, gadget_path: &Path, lun: u8) -> Result<()> {
        let gadget_path = gadget_path.to_path_buf();
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.disconnect_lun(&gadget_path, lun))
            .await
            .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    /// Disconnect LUN (clear file)
    ///
    /// This method uses forced_eject when available, which is safer than
    /// directly clearing the file path. Based on PiKVM's implementation.
    /// See: https://docs.kernel.org/usb/mass-storage.html
    pub fn disconnect_lun(&self, gadget_path: &Path, lun: u8) -> Result<()> {
        let lun_path = self.lun_path(gadget_path, lun);

        if lun_path.exists() {
            // Prefer forced_eject if available (PiKVM approach)
            // forced_eject forcibly detaches the backing file regardless of host state
            let forced_eject_path = lun_path.join("forced_eject");
            if forced_eject_path.exists() {
                debug!(
                    "Using forced_eject to disconnect LUN {} at {:?}",
                    lun, forced_eject_path
                );
                match write_file(&forced_eject_path, "1") {
                    Ok(_) => debug!("forced_eject write succeeded"),
                    Err(e) => {
                        warn!(
                            "forced_eject write failed: {}, falling back to clearing file",
                            e
                        );
                        write_file(&lun_path.join("file"), "")?;
                    }
                }
            } else {
                // Fallback to clearing file directly
                write_file(&lun_path.join("file"), "")?;
            }
            info!("LUN {} disconnected", lun);
        }

        Ok(())
    }

    /// Get current LUN file path
    pub fn get_lun_file(&self, gadget_path: &Path, lun: u8) -> Option<PathBuf> {
        let lun_path = self.lun_path(gadget_path, lun);
        let file_path = lun_path.join("file");

        if let Ok(content) = fs::read_to_string(&file_path) {
            let content = content.trim();
            if !content.is_empty() {
                return Some(PathBuf::from(content));
            }
        }

        None
    }

    /// Check if LUN is connected
    pub fn is_lun_connected(&self, gadget_path: &Path, lun: u8) -> bool {
        self.get_lun_file(gadget_path, lun).is_some()
    }
}

impl GadgetFunction for MsdFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn endpoints_required(&self) -> u8 {
        2 // IN + OUT for bulk transfers
    }

    fn meta(&self) -> FunctionMeta {
        FunctionMeta {
            name: self.name().to_string(),
            description: if self.instance == 0 {
                "Mass Storage Drive".to_string()
            } else {
                format!("Extra Drive #{}", self.instance)
            },
            endpoints: self.endpoints_required(),
            enabled: true,
        }
    }

    fn create(&self, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        create_dir(&func_path)?;

        // Set stall to 0 (workaround for some hosts)
        let stall_path = func_path.join("stall");
        if stall_path.exists() {
            let _ = write_file(&stall_path, "0");
        }

        // LUN 0 is created automatically, but ensure it exists
        let lun0_path = func_path.join("lun.0");
        if !lun0_path.exists() {
            create_dir(&lun0_path)?;
        }

        // Set default LUN 0 parameters
        let _ = write_file(&lun0_path.join("cdrom"), "0");
        let _ = write_file(&lun0_path.join("ro"), "0");
        let _ = write_file(&lun0_path.join("removable"), "1");
        let _ = write_file(&lun0_path.join("nofua"), "1");

        debug!("Created MSD function: {}", self.name());
        Ok(())
    }

    fn link(&self, config_path: &Path, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        let link_path = config_path.join(self.name());

        if !link_path.exists() {
            create_symlink(&func_path, &link_path)?;
            debug!("Linked MSD function {} to config", self.name());
        }

        Ok(())
    }

    fn unlink(&self, config_path: &Path) -> Result<()> {
        let link_path = config_path.join(self.name());
        remove_file(&link_path)?;
        debug!("Unlinked MSD function {}", self.name());
        Ok(())
    }

    fn cleanup(&self, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);

        // Disconnect all LUNs first
        for lun in 0..8 {
            let _ = self.disconnect_lun(gadget_path, lun);
        }

        // Remove function directory
        if let Err(e) = remove_dir(&func_path) {
            warn!("Could not remove MSD function directory: {}", e);
        }

        debug!("Cleaned up MSD function {}", self.name());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lun_config_cdrom() {
        let config = MsdLunConfig::cdrom(PathBuf::from("/tmp/test.iso"));
        assert!(config.cdrom);
        assert!(config.ro);
        assert!(config.removable);
    }

    #[test]
    fn test_lun_config_disk() {
        let config = MsdLunConfig::disk(PathBuf::from("/tmp/test.img"), false);
        assert!(!config.cdrom);
        assert!(!config.ro);
        assert!(config.removable);
    }

    #[test]
    fn test_msd_function_name() {
        let msd = MsdFunction::new(0);
        assert_eq!(msd.name(), "mass_storage.usb0");
        assert_eq!(msd.endpoints_required(), 2);
    }
}
