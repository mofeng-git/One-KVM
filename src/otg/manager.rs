use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use super::configfs::{
    configfs_path, create_dir, create_symlink, find_udc, is_configfs_available, remove_dir,
    remove_file, write_file, DEFAULT_GADGET_NAME, DEFAULT_USB_BCD_DEVICE, DEFAULT_USB_PRODUCT_ID,
    DEFAULT_USB_VENDOR_ID, USB_BCD_USB,
};
use super::function::GadgetFunction;
use super::hid::HidFunction;
use super::msd::MsdFunction;
use super::network::NetworkFunction;
use crate::config::OtgNetworkConfig;
use crate::error::{AppError, Result};

const REBIND_DELAY_MS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GadgetDescriptor {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
}

impl Default for GadgetDescriptor {
    fn default() -> Self {
        Self {
            vendor_id: DEFAULT_USB_VENDOR_ID,
            product_id: DEFAULT_USB_PRODUCT_ID,
            device_version: DEFAULT_USB_BCD_DEVICE,
            manufacturer: "One-KVM".to_string(),
            product: "One-KVM USB Device".to_string(),
            serial_number: "0123456789".to_string(),
        }
    }
}

pub struct OtgGadgetManager {
    gadget_name: String,
    gadget_path: PathBuf,
    config_path: PathBuf,
    descriptor: GadgetDescriptor,
    hid_instance: u8,
    msd_instance: u8,
    network_instance: u8,
    uac_instance: u8,
    functions: Vec<Box<dyn GadgetFunction>>,
    bound_udc: Option<String>,
    created_by_us: bool,
}

impl OtgGadgetManager {
    pub fn new() -> Self {
        Self::with_config(DEFAULT_GADGET_NAME)
    }

    pub fn with_config(gadget_name: &str) -> Self {
        Self::with_descriptor(gadget_name, GadgetDescriptor::default())
    }

    pub fn with_descriptor(gadget_name: &str, descriptor: GadgetDescriptor) -> Self {
        let gadget_path = configfs_path().join(gadget_name);
        let config_path = gadget_path.join("configs/c.1");

        Self {
            gadget_name: gadget_name.to_string(),
            gadget_path,
            config_path,
            descriptor,
            hid_instance: 0,
            msd_instance: 0,
            network_instance: 0,
            uac_instance: 0,
            functions: Vec::with_capacity(4),
            bound_udc: None,
            created_by_us: false,
        }
    }

    pub fn is_available() -> bool {
        is_configfs_available()
    }

    pub fn find_udc() -> Option<String> {
        find_udc()
    }

    pub fn gadget_exists(&self) -> bool {
        self.gadget_path.exists()
    }

    pub fn is_bound(&self) -> bool {
        let udc_file = self.gadget_path.join("UDC");
        if let Ok(content) = fs::read_to_string(&udc_file) {
            !content.trim().is_empty()
        } else {
            false
        }
    }

    pub fn add_keyboard(&mut self, keyboard_leds: bool) -> Result<PathBuf> {
        let func = HidFunction::keyboard(self.hid_instance, keyboard_leds);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    pub fn add_mouse_relative(&mut self) -> Result<PathBuf> {
        let func = HidFunction::mouse_relative(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    pub fn add_mouse_absolute(&mut self) -> Result<PathBuf> {
        let func = HidFunction::mouse_absolute(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    pub fn add_consumer_control(&mut self) -> Result<PathBuf> {
        let func = HidFunction::consumer_control(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    pub fn add_msd(&mut self, lun_capacity: u8) -> Result<MsdFunction> {
        let func = MsdFunction::new(self.msd_instance, lun_capacity)?;
        let func_clone = func.clone();
        self.add_function(Box::new(func))?;
        self.msd_instance += 1;
        Ok(func_clone)
    }

    pub fn add_network(&mut self, config: &OtgNetworkConfig) -> Result<NetworkFunction> {
        let func = NetworkFunction::new(self.network_instance, config)?;
        let func_clone = func.clone();
        self.add_function(Box::new(func))?;
        self.network_instance += 1;
        Ok(func_clone)
    }

    pub fn add_uac(&mut self, sample_rate: u32, channels: u8) -> Result<super::uac::UacFunction> {
        let func = super::uac::UacFunction::new(self.uac_instance, sample_rate, channels)?;
        let func_clone = func.clone();
        self.add_function(Box::new(func))?;
        self.uac_instance += 1;
        Ok(func_clone)
    }

    fn add_function(&mut self, func: Box<dyn GadgetFunction>) -> Result<()> {
        self.functions.push(func);
        Ok(())
    }

    pub fn setup(&mut self) -> Result<()> {
        debug!("Setting up OTG USB Gadget: {}", self.gadget_name);

        if !Self::is_available() {
            return Err(AppError::Internal(format!(
                "ConfigFS not available at {}",
                configfs_path().display()
            )));
        }

        if self.gadget_exists() {
            if self.is_bound() {
                debug!("Gadget already exists and is bound, skipping setup");
                return Ok(());
            }
            warn!("Gadget exists but not bound, will reconfigure");
            self.cleanup()?;
        }

        create_dir(&self.gadget_path)?;
        self.created_by_us = true;

        self.set_device_descriptors()?;

        self.create_strings()?;

        self.create_configuration()?;

        for func in &self.functions {
            func.create(&self.gadget_path)?;
            func.link(&self.config_path, &self.gadget_path)?;
        }

        debug!("OTG USB Gadget setup complete");
        Ok(())
    }

    pub fn bind(&mut self, udc: &str) -> Result<()> {
        if let Err(e) = self.recreate_config_links() {
            warn!("Failed to recreate gadget config links before bind: {}", e);
        }

        debug!("Binding gadget to UDC: {}", udc);
        write_file(&self.gadget_path.join("UDC"), &udc)?;
        self.bound_udc = Some(udc.to_string());
        std::thread::sleep(std::time::Duration::from_millis(REBIND_DELAY_MS));

        Ok(())
    }

    pub fn unbind(&mut self) -> Result<()> {
        if self.is_bound() {
            write_file(&self.gadget_path.join("UDC"), "")?;
            self.bound_udc = None;
            info!("Unbound gadget from UDC");
            std::thread::sleep(std::time::Duration::from_millis(REBIND_DELAY_MS));
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if !self.gadget_exists() {
            self.created_by_us = false;
            return Ok(());
        }

        info!("Cleaning up OTG USB Gadget: {}", self.gadget_name);
        let mut errors = Vec::new();

        if let Err(error) = self.unbind() {
            errors.push(format!("unbind failed: {error}"));
        }

        for func in self.functions.iter().rev() {
            if let Err(error) = func.unlink(&self.config_path) {
                errors.push(format!("unlink {} failed: {error}", func.name()));
            }
        }

        let config_strings = self.config_path.join("strings/0x409");
        if let Err(error) = remove_dir(&config_strings) {
            errors.push(error.to_string());
        }
        if let Err(error) = remove_dir(&self.config_path) {
            errors.push(error.to_string());
        }

        for func in self.functions.iter().rev() {
            if let Err(error) = func.cleanup(&self.gadget_path) {
                errors.push(format!("cleanup {} failed: {error}", func.name()));
            }
        }

        let gadget_strings = self.gadget_path.join("strings/0x409");
        if let Err(error) = remove_dir(&gadget_strings) {
            errors.push(error.to_string());
        }

        if let Err(error) = remove_dir(&self.gadget_path) {
            errors.push(error.to_string());
        }

        if !errors.is_empty() {
            return Err(AppError::Config(format!(
                "OTG gadget cleanup incomplete: {}",
                errors.join("; ")
            )));
        }

        self.created_by_us = false;
        info!("OTG USB Gadget cleanup complete");
        Ok(())
    }

    fn set_device_descriptors(&self) -> Result<()> {
        write_file(
            &self.gadget_path.join("idVendor"),
            &format!("0x{:04x}", self.descriptor.vendor_id),
        )?;
        write_file(
            &self.gadget_path.join("idProduct"),
            &format!("0x{:04x}", self.descriptor.product_id),
        )?;
        write_file(
            &self.gadget_path.join("bcdDevice"),
            &format!("0x{:04x}", self.descriptor.device_version),
        )?;
        write_file(
            &self.gadget_path.join("bcdUSB"),
            &format!("0x{:04x}", USB_BCD_USB),
        )?;
        write_file(&self.gadget_path.join("bDeviceClass"), "0x00")?;
        write_file(&self.gadget_path.join("bDeviceSubClass"), "0x00")?;
        write_file(&self.gadget_path.join("bDeviceProtocol"), "0x00")?;
        debug!("Set device descriptors");
        Ok(())
    }

    fn create_strings(&self) -> Result<()> {
        let strings_path = self.gadget_path.join("strings/0x409");
        create_dir(&strings_path)?;

        write_file(
            &strings_path.join("serialnumber"),
            &self.descriptor.serial_number,
        )?;
        write_file(
            &strings_path.join("manufacturer"),
            &self.descriptor.manufacturer,
        )?;
        write_file(&strings_path.join("product"), &self.descriptor.product)?;
        debug!("Created USB strings");
        Ok(())
    }

    fn create_configuration(&self) -> Result<()> {
        create_dir(&self.config_path)?;

        let strings_path = self.config_path.join("strings/0x409");
        create_dir(&strings_path)?;
        write_file(
            &strings_path.join("configuration"),
            self.configuration_label(),
        )?;

        write_file(&self.config_path.join("MaxPower"), "500")?;

        debug!("Created configuration c.1");
        Ok(())
    }

    fn configuration_label(&self) -> &'static str {
        let has_msd = self
            .functions
            .iter()
            .any(|func| func.name().starts_with("mass_storage."));
        let has_network = self.functions.iter().any(|func| {
            ["ncm.", "ecm.", "rndis."]
                .iter()
                .any(|prefix| func.name().starts_with(prefix))
        });
        if has_msd && has_network {
            "Config 1: HID + MSD + NET"
        } else if has_msd {
            "Config 1: HID + MSD"
        } else if has_network {
            "Config 1: HID + NET"
        } else {
            "Config 1: HID"
        }
    }

    pub fn gadget_path(&self) -> &PathBuf {
        &self.gadget_path
    }

    fn recreate_config_links(&self) -> Result<()> {
        let functions_path = self.gadget_path.join("functions");
        if !functions_path.exists() || !self.config_path.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&functions_path).map_err(|e| {
            AppError::Internal(format!(
                "Failed to read functions directory {}: {}",
                functions_path.display(),
                e
            ))
        })?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = match name.to_str() {
                Some(n) => n,
                None => continue,
            };
            if !name.contains(".usb") {
                continue;
            }

            let src = functions_path.join(name);
            let dest = self.config_path.join(name);

            if dest.exists() {
                if let Err(e) = remove_file(&dest) {
                    warn!(
                        "Failed to remove existing config link {}: {}",
                        dest.display(),
                        e
                    );
                    continue;
                }
            }

            create_symlink(&src, &dest)?;
        }

        Ok(())
    }
}

impl Default for OtgGadgetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OtgGadgetManager {
    fn drop(&mut self) {
        if self.created_by_us {
            if let Err(e) = self.cleanup() {
                error!("Failed to cleanup OTG gadget on drop: {}", e);
            }
        }
    }
}

pub async fn wait_for_hid_devices(device_paths: &[PathBuf], timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);

    let mut delay_ms = 10u64;
    const MAX_DELAY_MS: u64 = 100;

    while start.elapsed() < timeout {
        if device_paths.iter().all(|p| p.exists()) {
            return true;
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        let sleep_duration = std::time::Duration::from_millis(delay_ms).min(remaining);

        if sleep_duration.is_zero() {
            break;
        }

        tokio::time::sleep(sleep_duration).await;

        delay_ms = (delay_ms * 2).min(MAX_DELAY_MS);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = OtgGadgetManager::new();
        assert_eq!(manager.gadget_name, DEFAULT_GADGET_NAME);
        assert!(!manager.gadget_exists());
    }

    #[test]
    fn test_function_selection_is_not_prevalidated() {
        let mut manager = OtgGadgetManager::with_config("test");

        assert!(manager.add_keyboard(false).is_ok());
        assert!(manager.add_mouse_relative().is_ok());
        assert!(manager.add_mouse_absolute().is_ok());
        assert_eq!(manager.functions.len(), 3);
    }
}
