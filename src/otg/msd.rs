use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use super::configfs::{create_dir, create_symlink, remove_dir, remove_file, write_file};
use super::function::GadgetFunction;
use crate::config::{MsdConfig, DEFAULT_CDROM_INQUIRY_STRING, DEFAULT_FLASH_INQUIRY_STRING};
use crate::error::{AppError, MsdErrorCode, Result};

const MEDIA_TYPE_REBIND_DELAY_MS: u64 = 300;

#[derive(Debug, Clone)]
pub struct MsdLunConfig {
    pub file: PathBuf,
    pub cdrom: bool,
    pub ro: bool,
    pub removable: bool,
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
    pub fn cdrom(file: PathBuf) -> Self {
        Self {
            file,
            cdrom: true,
            ro: true,
            removable: true,
            nofua: true,
        }
    }

    pub fn disk(file: PathBuf, read_only: bool) -> Self {
        Self {
            file,
            cdrom: false,
            ro: read_only,
            removable: true,
            // nofua=false: enforce Force Unit Access so the USB host (e.g. Windows)
            // gets proper write-completion acknowledgements when writing to the
            // virtual .img file. nofua=true can cause write-verify failures
            // that manifest as Windows error 0x80070570 on writable drives.
            nofua: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsdInquiryStrings {
    pub flash: String,
    pub cdrom: String,
}

impl Default for MsdInquiryStrings {
    fn default() -> Self {
        Self {
            flash: DEFAULT_FLASH_INQUIRY_STRING.to_string(),
            cdrom: DEFAULT_CDROM_INQUIRY_STRING.to_string(),
        }
    }
}

impl From<&MsdConfig> for MsdInquiryStrings {
    fn from(config: &MsdConfig) -> Self {
        Self {
            flash: config.flash_inquiry_string.clone(),
            cdrom: config.cdrom_inquiry_string.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MsdFunction {
    name: String,
    lun_capacity: u8,
    inquiry_strings: MsdInquiryStrings,
}

impl MsdFunction {
    pub fn new(instance: u8, lun_capacity: u8, inquiry_strings: MsdInquiryStrings) -> Result<Self> {
        if lun_capacity != 1 && lun_capacity != 8 {
            return Err(AppError::BadRequest(format!(
                "MSD LUN capacity must be 1 or 8, got {lun_capacity}"
            )));
        }

        Ok(Self {
            name: format!("mass_storage.usb{}", instance),
            lun_capacity,
            inquiry_strings,
        })
    }

    fn function_path(&self, gadget_path: &Path) -> PathBuf {
        gadget_path.join("functions").join(self.name())
    }

    fn lun_path(&self, gadget_path: &Path, lun: u8) -> PathBuf {
        self.function_path(gadget_path).join(format!("lun.{}", lun))
    }

    fn existing_lun_paths(&self, gadget_path: &Path) -> Result<Vec<(u16, PathBuf)>> {
        let func_path = self.function_path(gadget_path);
        if !func_path.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&func_path).map_err(|e| {
            AppError::Internal(format!(
                "Failed to read MSD function directory {}: {}",
                func_path.display(),
                e
            ))
        })?;
        let mut luns = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name();
                let name = name.to_str()?;
                let lun = name.strip_prefix("lun.")?.parse::<u16>().ok()?;
                Some((lun, entry.path()))
            })
            .collect::<Vec<_>>();
        luns.sort_by_key(|(lun, _)| *lun);
        Ok(luns)
    }

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

    fn clear_lun_unbound(&self, gadget_path: &Path, lun: u8) -> Result<()> {
        let lun_path = self.lun_path(gadget_path, lun);
        if !lun_path.exists() {
            create_dir(&lun_path)?;
        }
        write_file(&lun_path.join("file"), "")?;
        let _ = write_file(&lun_path.join("cdrom"), "0");
        let _ = write_file(&lun_path.join("ro"), "0");
        let _ = write_file(&lun_path.join("removable"), "1");
        let _ = write_file(&lun_path.join("nofua"), "1");
        Ok(())
    }

    pub fn configure_lun(&self, gadget_path: &Path, lun: u8, config: &MsdLunConfig) -> Result<()> {
        if lun >= self.lun_capacity {
            return Err(AppError::BadRequest(format!(
                "LUN {lun} is outside MSD capacity {}",
                self.lun_capacity
            )));
        }
        let lun_path = self.lun_path(gadget_path, lun);

        if !lun_path.exists() {
            return Err(AppError::Internal(format!(
                "Configured MSD LUN {lun} does not exist"
            )));
        }

        let current_cdrom = fs::read_to_string(lun_path.join("cdrom"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let rebind_required = Self::media_type_rebind_required(&current_cdrom, config);
        let udc_path = gadget_path.join("UDC");
        let bound_udc = if rebind_required && udc_path.exists() {
            fs::read_to_string(&udc_path)
                .map_err(|error| {
                    AppError::Internal(format!(
                        "Failed to read bound UDC before changing LUN {lun} media type: {error}"
                    ))
                })?
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if !bound_udc.is_empty() {
            info!(
                "LUN {} media type is changing; temporarily unbinding UDC {}",
                lun, bound_udc
            );
            write_file(&udc_path, "")?;
            std::thread::sleep(std::time::Duration::from_millis(MEDIA_TYPE_REBIND_DELAY_MS));
        }

        let configure_result = self.configure_lun_attributes(&lun_path, lun, config);
        let rebind_result = if bound_udc.is_empty() {
            Ok(())
        } else {
            let result = write_file(&udc_path, &bound_udc);
            if result.is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(MEDIA_TYPE_REBIND_DELAY_MS));
                info!(
                    "Rebound UDC {} after changing LUN {} media type",
                    bound_udc, lun
                );
            }
            result
        };

        match (configure_result, rebind_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(configure_error), Ok(())) => Err(configure_error),
            (Ok(()), Err(rebind_error)) => Err(AppError::Internal(format!(
                "Configured LUN {lun}, but failed to rebind UDC {bound_udc}: {rebind_error}"
            ))),
            (Err(configure_error), Err(rebind_error)) => Err(AppError::Internal(format!(
                "Failed to configure LUN {lun}: {configure_error}; also failed to rebind UDC {bound_udc}: {rebind_error}"
            ))),
        }
    }

    fn media_type_rebind_required(current_cdrom: &str, config: &MsdLunConfig) -> bool {
        current_cdrom != if config.cdrom { "1" } else { "0" }
    }

    fn inquiry_string_path(lun_path: &Path, cdrom: bool) -> Option<PathBuf> {
        let cdrom_path = lun_path.join("inquiry_string_cdrom");
        if cdrom && cdrom_path.exists() {
            return Some(cdrom_path);
        }
        let generic_path = lun_path.join("inquiry_string");
        generic_path.exists().then_some(generic_path)
    }

    fn inquiry_string(&self, cdrom: bool) -> &str {
        if cdrom {
            &self.inquiry_strings.cdrom
        } else {
            &self.inquiry_strings.flash
        }
    }

    fn configure_lun_attributes(
        &self,
        lun_path: &Path,
        lun: u8,
        config: &MsdLunConfig,
    ) -> Result<()> {
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
        let new_cdrom = if config.cdrom { "1" } else { "0" };
        let new_ro = if config.ro { "1" } else { "0" };
        let new_removable = if config.removable { "1" } else { "0" };
        let new_nofua = if config.nofua { "1" } else { "0" };

        let forced_eject_path = lun_path.join("forced_eject");
        if forced_eject_path.exists() {
            debug!("Using forced_eject to clear LUN {}", lun);
            if let Err(error) = write_file(&forced_eject_path, "1") {
                warn!(
                    "LUN {} forced_eject failed while changing media: {}; clearing file instead",
                    lun, error
                );
                write_file(&lun_path.join("file"), "")?;
            }
        } else {
            write_file(&lun_path.join("file"), "")?;
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        if current_cdrom != new_cdrom {
            debug!(
                "Updating LUN {} cdrom: {} -> {}",
                lun, current_cdrom, new_cdrom
            );
            write_file(&lun_path.join("cdrom"), new_cdrom)?;
            self.write_inquiry_string(lun_path, config.cdrom)?;
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
                    Err(error) => {
                        let is_busy = error.to_string().contains("Device or resource busy")
                            || error.to_string().contains("os error 16");
                        if is_busy && attempt < 4 {
                            warn!(
                                "LUN {} file write busy, retrying (attempt {}/5)",
                                lun,
                                attempt + 1
                            );
                            std::thread::sleep(std::time::Duration::from_millis(50 << attempt));
                            last_error = Some(error);
                            continue;
                        }
                        return Err(error);
                    }
                }
            }

            if let Some(error) = last_error {
                return Err(error);
            }
        } else if !config.file.as_os_str().is_empty() {
            warn!("LUN {} file does not exist: {}", lun, config.file.display());
        }

        Ok(())
    }

    fn write_inquiry_string(&self, lun_path: &Path, cdrom: bool) -> Result<()> {
        if let Some(path) = Self::inquiry_string_path(lun_path, cdrom) {
            write_file(&path, self.inquiry_string(cdrom))?;
        }
        Ok(())
    }

    fn write_inquiry_strings(&self, lun_path: &Path) -> Result<()> {
        let generic_path = lun_path.join("inquiry_string");
        if generic_path.exists() {
            write_file(&generic_path, &self.inquiry_strings.flash)?;
        }

        let cdrom_path = lun_path.join("inquiry_string_cdrom");
        if cdrom_path.exists() {
            write_file(&cdrom_path, &self.inquiry_strings.cdrom)?;
        }
        Ok(())
    }

    pub async fn disconnect_lun_async(&self, gadget_path: &Path, lun: u8) -> Result<()> {
        let gadget_path = gadget_path.to_path_buf();
        let this = self.clone();

        tokio::task::spawn_blocking(move || this.disconnect_lun(&gadget_path, lun))
            .await
            .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?
    }

    pub fn disconnect_lun(&self, gadget_path: &Path, lun: u8) -> Result<()> {
        if lun >= self.lun_capacity {
            return Err(AppError::BadRequest(format!(
                "LUN {lun} is outside MSD capacity {}",
                self.lun_capacity
            )));
        }
        let lun_path = self.lun_path(gadget_path, lun);

        self.disconnect_lun_path(&lun_path, lun as u16)
    }

    fn medium_removal_was_prevented(error: &std::io::Error) -> bool {
        error.raw_os_error() == Some(libc::EBUSY)
    }

    fn clear_lun_file(file_path: &Path, lun: u16) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .open(file_path)
            .map_err(|error| {
                warn!(
                    lun,
                    path = %file_path.display(),
                    %error,
                    "Failed to open MSD LUN backing-file attribute while disconnecting"
                );
                AppError::from(MsdErrorCode::MsdDisconnectFailed)
            })?;

        // An empty configfs value is represented by a newline. Keep this as one
        // write operation so EBUSY can be attributed to fsg_store_file().
        if let Err(error) = file.write_all(b"\n") {
            warn!(
                lun,
                path = %file_path.display(),
                errno = error.raw_os_error(),
                %error,
                "Kernel rejected MSD LUN disconnect"
            );
            return if Self::medium_removal_was_prevented(&error) {
                Err(MsdErrorCode::MsdMediumRemovalPrevented.into())
            } else {
                Err(MsdErrorCode::MsdDisconnectFailed.into())
            };
        }

        file.flush().map_err(|error| {
            warn!(
                lun,
                path = %file_path.display(),
                %error,
                "Failed to flush MSD LUN backing-file attribute while disconnecting"
            );
            MsdErrorCode::MsdDisconnectFailed.into()
        })
    }

    fn disconnect_lun_path(&self, lun_path: &Path, lun: u16) -> Result<()> {
        if lun_path.exists() {
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
                        let file_path = lun_path.join("file");
                        if file_path.exists() {
                            Self::clear_lun_file(&file_path, lun)?;
                        }
                    }
                }
            } else {
                let file_path = lun_path.join("file");
                if file_path.exists() {
                    Self::clear_lun_file(&file_path, lun)?;
                }
            }
            info!("LUN {} disconnected", lun);
        }

        Ok(())
    }

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

    pub fn is_lun_connected(&self, gadget_path: &Path, lun: u8) -> bool {
        self.get_lun_file(gadget_path, lun).is_some()
    }
}

impl GadgetFunction for MsdFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn create(&self, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        create_dir(&func_path)?;

        let stall_path = func_path.join("stall");
        if stall_path.exists() {
            let _ = write_file(&stall_path, "0");
        }

        for lun in 0..self.lun_capacity {
            self.clear_lun_unbound(gadget_path, lun)?;
            self.write_inquiry_strings(&self.lun_path(gadget_path, lun))?;
        }

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
        let mut errors = Vec::new();

        let lun_paths = match self.existing_lun_paths(gadget_path) {
            Ok(luns) => luns,
            Err(e) => {
                errors.push(format!("could not enumerate MSD LUN directories: {e}"));
                Vec::new()
            }
        };
        for (lun, lun_path) in lun_paths {
            if let Err(e) = self.disconnect_lun_path(&lun_path, lun) {
                errors.push(format!("could not disconnect LUN {lun}: {e}"));
            }
            // lun.0 is the mass-storage function's configfs default group. It
            // cannot be removed directly and is released with the function.
            if lun == 0 {
                continue;
            }
            if let Err(e) = remove_dir(&lun_path) {
                errors.push(format!("could not remove LUN {lun} directory: {e}"));
            }
        }

        if let Err(e) = remove_dir(&func_path) {
            errors.push(format!("could not remove MSD function directory: {e}"));
        }

        if !errors.is_empty() {
            return Err(AppError::Config(format!(
                "MSD cleanup incomplete: {}",
                errors.join("; ")
            )));
        }

        debug!("Cleaned up MSD function {}", self.name());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_msd(capacity: u8) -> MsdFunction {
        MsdFunction::new(0, capacity, MsdInquiryStrings::default()).unwrap()
    }

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
    fn inquiry_string_uses_cdrom_attribute_with_stock_fallback() {
        let msd = MsdFunction::new(
            0,
            1,
            MsdInquiryStrings {
                flash: "Custom Flash".into(),
                cdrom: "Custom Optical".into(),
            },
        )
        .unwrap();
        let patched = TempDir::new().unwrap();
        std::fs::write(patched.path().join("inquiry_string"), b"generic\n").unwrap();
        std::fs::write(patched.path().join("inquiry_string_cdrom"), b"cdrom\n").unwrap();

        msd.write_inquiry_strings(patched.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(patched.path().join("inquiry_string_cdrom"))
                .unwrap()
                .trim(),
            "Custom Optical"
        );
        assert_eq!(
            std::fs::read_to_string(patched.path().join("inquiry_string"))
                .unwrap()
                .trim(),
            "Custom Flash"
        );

        let stock = TempDir::new().unwrap();
        std::fs::write(stock.path().join("inquiry_string"), b"generic\n").unwrap();
        msd.write_inquiry_string(stock.path(), true).unwrap();
        assert_eq!(
            std::fs::read_to_string(stock.path().join("inquiry_string"))
                .unwrap()
                .trim(),
            "Custom Optical"
        );
    }

    #[test]
    fn test_msd_function_name() {
        let msd = test_msd(1);
        assert_eq!(msd.name(), "mass_storage.usb0");
        assert_eq!(msd.lun_capacity, 1);

        let multi = test_msd(8);
        assert_eq!(multi.lun_capacity, 8);
    }

    #[test]
    fn test_msd_function_rejects_invalid_capacity() {
        assert!(MsdFunction::new(0, 0, MsdInquiryStrings::default()).is_err());
        assert!(MsdFunction::new(0, 2, MsdInquiryStrings::default()).is_err());
        assert!(MsdFunction::new(0, 9, MsdInquiryStrings::default()).is_err());
    }

    #[test]
    fn only_ebusy_means_the_host_prevented_medium_removal() {
        let busy = std::io::Error::from_raw_os_error(libc::EBUSY);
        let io = std::io::Error::from_raw_os_error(libc::EIO);

        assert!(MsdFunction::medium_removal_was_prevented(&busy));
        assert!(!MsdFunction::medium_removal_was_prevented(&io));
    }

    #[test]
    fn disconnect_lun_prefers_forced_eject() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(&lun_path).unwrap();
        std::fs::write(lun_path.join("file"), b"backing.img\n").unwrap();
        std::fs::write(lun_path.join("forced_eject"), b"0\n").unwrap();
        let msd = test_msd(1);

        msd.disconnect_lun(temp_dir.path(), 0).unwrap();

        assert_eq!(
            std::fs::read(lun_path.join("forced_eject")).unwrap(),
            b"1\n"
        );
        assert_eq!(
            std::fs::read(lun_path.join("file")).unwrap(),
            b"backing.img\n"
        );
    }

    #[test]
    fn disconnect_lun_without_forced_eject_clears_file() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(&lun_path).unwrap();
        std::fs::write(lun_path.join("file"), b"backing.img\n").unwrap();
        let msd = test_msd(1);

        msd.disconnect_lun(temp_dir.path(), 0).unwrap();

        assert!(std::fs::read(lun_path.join("file"))
            .unwrap()
            .starts_with(b"\n"));
    }

    #[test]
    fn disconnect_lun_falls_back_when_forced_eject_write_fails() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(lun_path.join("forced_eject")).unwrap();
        std::fs::write(lun_path.join("file"), b"backing.img\n").unwrap();
        let msd = test_msd(1);

        msd.disconnect_lun(temp_dir.path(), 0).unwrap();

        assert!(std::fs::read(lun_path.join("file"))
            .unwrap()
            .starts_with(b"\n"));
    }

    #[test]
    fn disconnect_lun_only_changes_the_selected_lun() {
        let temp_dir = TempDir::new().unwrap();
        let function_path = temp_dir.path().join("functions/mass_storage.usb0");
        for lun in 0..2 {
            let lun_path = function_path.join(format!("lun.{lun}"));
            std::fs::create_dir_all(&lun_path).unwrap();
            std::fs::write(lun_path.join("file"), format!("backing-{lun}.img\n")).unwrap();
            std::fs::write(lun_path.join("forced_eject"), b"0\n").unwrap();
        }
        let msd = test_msd(8);

        msd.disconnect_lun(temp_dir.path(), 1).unwrap();

        assert_eq!(
            std::fs::read(function_path.join("lun.0/forced_eject")).unwrap(),
            b"0\n"
        );
        assert_eq!(
            std::fs::read(function_path.join("lun.1/forced_eject")).unwrap(),
            b"1\n"
        );
        assert_eq!(
            std::fs::read(function_path.join("lun.0/file")).unwrap(),
            b"backing-0.img\n"
        );
        assert_eq!(
            std::fs::read(function_path.join("lun.1/file")).unwrap(),
            b"backing-1.img\n"
        );
    }

    #[test]
    fn create_uses_configured_lun_capacity() {
        for capacity in [1, 8] {
            let temp_dir = TempDir::new().unwrap();
            std::fs::create_dir_all(temp_dir.path().join("functions")).unwrap();
            let msd = test_msd(capacity);

            msd.create(temp_dir.path()).unwrap();

            for lun in 0..capacity {
                assert!(msd.lun_path(temp_dir.path(), lun).exists());
            }
            assert!(!msd.lun_path(temp_dir.path(), capacity).exists());
        }
    }

    #[test]
    fn configure_lun_does_not_rebind_udc() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(&lun_path).unwrap();
        for attr in ["file", "cdrom", "ro", "removable", "nofua"] {
            std::fs::write(lun_path.join(attr), b"0\n").unwrap();
        }
        std::fs::write(temp_dir.path().join("UDC"), b"test.udc\n").unwrap();
        let image_path = temp_dir.path().join("test.img");
        std::fs::write(&image_path, b"image").unwrap();
        let msd = test_msd(1);

        msd.configure_lun(temp_dir.path(), 0, &MsdLunConfig::disk(image_path, false))
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("UDC")).unwrap(),
            "test.udc\n"
        );
    }

    #[test]
    fn media_type_changes_require_udc_rebind() {
        let iso = MsdLunConfig::cdrom(PathBuf::from("/tmp/test.iso"));
        let disk = MsdLunConfig::disk(PathBuf::from("/tmp/test.img"), false);

        assert!(MsdFunction::media_type_rebind_required("0", &iso));
        assert!(!MsdFunction::media_type_rebind_required("1", &iso));
        assert!(MsdFunction::media_type_rebind_required("1", &disk));
        assert!(!MsdFunction::media_type_rebind_required("0", &disk));
    }

    #[test]
    fn configure_cdrom_restores_bound_udc() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(&lun_path).unwrap();
        for attr in ["file", "cdrom", "ro", "removable", "nofua"] {
            std::fs::write(lun_path.join(attr), b"0\n").unwrap();
        }
        std::fs::write(temp_dir.path().join("UDC"), b"test.udc\n").unwrap();
        let image_path = temp_dir.path().join("test.iso");
        std::fs::write(&image_path, b"iso").unwrap();
        let msd = test_msd(1);

        msd.configure_lun(temp_dir.path(), 0, &MsdLunConfig::cdrom(image_path.clone()))
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("UDC"))
                .unwrap()
                .trim(),
            "test.udc"
        );
        assert_eq!(
            std::fs::read_to_string(lun_path.join("cdrom"))
                .unwrap()
                .trim(),
            "1"
        );
        assert_eq!(
            std::fs::read_to_string(lun_path.join("ro")).unwrap().trim(),
            "1"
        );
        assert_eq!(
            std::fs::read_to_string(lun_path.join("file"))
                .unwrap()
                .trim(),
            image_path.to_string_lossy()
        );
    }

    #[test]
    fn configure_failure_still_restores_bound_udc() {
        let temp_dir = TempDir::new().unwrap();
        let lun_path = temp_dir.path().join("functions/mass_storage.usb0/lun.0");
        std::fs::create_dir_all(lun_path.join("file")).unwrap();
        for attr in ["cdrom", "ro", "removable", "nofua"] {
            std::fs::write(lun_path.join(attr), b"0\n").unwrap();
        }
        std::fs::write(temp_dir.path().join("UDC"), b"test.udc\n").unwrap();
        let image_path = temp_dir.path().join("test.iso");
        std::fs::write(&image_path, b"iso").unwrap();
        let msd = test_msd(1);

        assert!(msd
            .configure_lun(temp_dir.path(), 0, &MsdLunConfig::cdrom(image_path),)
            .is_err());
        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("UDC"))
                .unwrap()
                .trim(),
            "test.udc"
        );
    }

    #[test]
    fn cleanup_removes_all_dynamic_luns_including_stale_capacity() {
        let temp_dir = TempDir::new().unwrap();
        let func_path = temp_dir.path().join("functions/mass_storage.usb0");
        for lun in 1..8 {
            std::fs::create_dir_all(func_path.join(format!("lun.{lun}"))).unwrap();
        }
        let msd = test_msd(1);

        msd.cleanup(temp_dir.path()).unwrap();

        assert!(!func_path.exists());
    }

    #[test]
    fn cleanup_forced_ejects_every_existing_lun() {
        let temp_dir = TempDir::new().unwrap();
        let func_path = temp_dir.path().join("functions/mass_storage.usb0");
        for lun in 0..3 {
            let lun_path = func_path.join(format!("lun.{lun}"));
            std::fs::create_dir_all(&lun_path).unwrap();
            std::fs::write(lun_path.join("file"), format!("backing-{lun}.img\n")).unwrap();
            std::fs::write(lun_path.join("forced_eject"), b"0\n").unwrap();
        }
        let msd = test_msd(1);

        // Ordinary files do not disappear with configfs groups, so cleanup is
        // expected to report directory-removal failures in this test fixture.
        assert!(msd.cleanup(temp_dir.path()).is_err());

        for lun in 0..3 {
            assert_eq!(
                std::fs::read(func_path.join(format!("lun.{lun}/forced_eject"))).unwrap(),
                b"1\n"
            );
        }
    }

    #[test]
    fn cleanup_reports_when_non_configfs_cannot_release_default_lun() {
        let temp_dir = TempDir::new().unwrap();
        let func_path = temp_dir.path().join("functions/mass_storage.usb0");
        for lun in 0..2 {
            std::fs::create_dir_all(func_path.join(format!("lun.{lun}"))).unwrap();
        }
        let msd = test_msd(1);

        let error = msd.cleanup(temp_dir.path()).unwrap_err();

        assert!(error.to_string().contains("MSD cleanup incomplete"));
        assert!(func_path.join("lun.0").exists());
        assert!(!func_path.join("lun.1").exists());
    }
}
