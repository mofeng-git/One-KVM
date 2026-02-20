//! OTG Gadget Manager - unified management for USB Gadget functions

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use super::configfs::{
    create_dir, create_symlink, find_udc, is_configfs_available, remove_dir, remove_file,
    write_file, CONFIGFS_PATH, DEFAULT_GADGET_NAME, DEFAULT_USB_BCD_DEVICE, DEFAULT_USB_PRODUCT_ID,
    DEFAULT_USB_VENDOR_ID, USB_BCD_USB,
};
use super::endpoint::{EndpointAllocator, DEFAULT_MAX_ENDPOINTS};
use super::function::{FunctionMeta, GadgetFunction};
use super::hid::HidFunction;
use super::msd::MsdFunction;
use crate::error::{AppError, Result};

const REBIND_DELAY_MS: u64 = 300;

/// USB Gadget device descriptor configuration
#[derive(Debug, Clone)]
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

/// OTG Gadget Manager - unified management for HID and MSD
pub struct OtgGadgetManager {
    /// Gadget name
    gadget_name: String,
    /// Gadget path in ConfigFS
    gadget_path: PathBuf,
    /// Configuration path
    config_path: PathBuf,
    /// Device descriptor
    descriptor: GadgetDescriptor,
    /// Endpoint allocator
    endpoint_allocator: EndpointAllocator,
    /// HID instance counter
    hid_instance: u8,
    /// MSD instance counter
    msd_instance: u8,
    /// Registered functions
    functions: Vec<Box<dyn GadgetFunction>>,
    /// Function metadata
    meta: HashMap<String, FunctionMeta>,
    /// Bound UDC name
    bound_udc: Option<String>,
    /// Whether gadget was created by us
    created_by_us: bool,
}

impl OtgGadgetManager {
    /// Create a new gadget manager with default settings
    pub fn new() -> Self {
        Self::with_config(DEFAULT_GADGET_NAME, DEFAULT_MAX_ENDPOINTS)
    }

    /// Create a new gadget manager with custom configuration
    pub fn with_config(gadget_name: &str, max_endpoints: u8) -> Self {
        Self::with_descriptor(gadget_name, max_endpoints, GadgetDescriptor::default())
    }

    /// Create a new gadget manager with custom descriptor
    pub fn with_descriptor(
        gadget_name: &str,
        max_endpoints: u8,
        descriptor: GadgetDescriptor,
    ) -> Self {
        let gadget_path = PathBuf::from(CONFIGFS_PATH).join(gadget_name);
        let config_path = gadget_path.join("configs/c.1");

        Self {
            gadget_name: gadget_name.to_string(),
            gadget_path,
            config_path,
            descriptor,
            endpoint_allocator: EndpointAllocator::new(max_endpoints),
            hid_instance: 0,
            msd_instance: 0,
            // Pre-allocate for typical use: 3 HID (keyboard, rel mouse, abs mouse) + 1 MSD
            functions: Vec::with_capacity(4),
            meta: HashMap::with_capacity(4),
            bound_udc: None,
            created_by_us: false,
        }
    }

    /// Check if ConfigFS is available
    pub fn is_available() -> bool {
        is_configfs_available()
    }

    /// Find available UDC
    pub fn find_udc() -> Option<String> {
        find_udc()
    }

    /// Check if gadget exists
    pub fn gadget_exists(&self) -> bool {
        self.gadget_path.exists()
    }

    /// Check if gadget is bound to UDC
    pub fn is_bound(&self) -> bool {
        let udc_file = self.gadget_path.join("UDC");
        if let Ok(content) = fs::read_to_string(&udc_file) {
            !content.trim().is_empty()
        } else {
            false
        }
    }

    /// Add keyboard function
    /// Returns the expected device path (e.g., /dev/hidg0)
    pub fn add_keyboard(&mut self) -> Result<PathBuf> {
        let func = HidFunction::keyboard(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    /// Add relative mouse function
    pub fn add_mouse_relative(&mut self) -> Result<PathBuf> {
        let func = HidFunction::mouse_relative(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    /// Add absolute mouse function
    pub fn add_mouse_absolute(&mut self) -> Result<PathBuf> {
        let func = HidFunction::mouse_absolute(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    /// Add consumer control function (multimedia keys)
    pub fn add_consumer_control(&mut self) -> Result<PathBuf> {
        let func = HidFunction::consumer_control(self.hid_instance);
        let device_path = func.device_path();
        self.add_function(Box::new(func))?;
        self.hid_instance += 1;
        Ok(device_path)
    }

    /// Add MSD function (returns MsdFunction handle for LUN configuration)
    pub fn add_msd(&mut self) -> Result<MsdFunction> {
        let func = MsdFunction::new(self.msd_instance);
        let func_clone = func.clone();
        self.add_function(Box::new(func))?;
        self.msd_instance += 1;
        Ok(func_clone)
    }

    /// Add a generic function
    fn add_function(&mut self, func: Box<dyn GadgetFunction>) -> Result<()> {
        let endpoints = func.endpoints_required();

        // Check endpoint availability
        if !self.endpoint_allocator.can_allocate(endpoints) {
            return Err(AppError::Internal(format!(
                "Not enough endpoints for function {}: need {}, available {}",
                func.name(),
                endpoints,
                self.endpoint_allocator.available()
            )));
        }

        // Allocate endpoints
        self.endpoint_allocator.allocate(endpoints)?;

        // Store metadata
        self.meta.insert(func.name().to_string(), func.meta());

        // Store function
        self.functions.push(func);

        Ok(())
    }

    /// Setup the gadget (create directories and configure)
    pub fn setup(&mut self) -> Result<()> {
        info!("Setting up OTG USB Gadget: {}", self.gadget_name);

        // Check ConfigFS availability
        if !Self::is_available() {
            return Err(AppError::Internal(
                "ConfigFS not available. Is it mounted at /sys/kernel/config?".to_string(),
            ));
        }

        // Check if gadget already exists and is bound
        if self.gadget_exists() {
            if self.is_bound() {
                info!("Gadget already exists and is bound, skipping setup");
                return Ok(());
            }
            warn!("Gadget exists but not bound, will reconfigure");
            self.cleanup()?;
        }

        // Create gadget directory
        create_dir(&self.gadget_path)?;
        self.created_by_us = true;

        // Set device descriptors
        self.set_device_descriptors()?;

        // Create strings
        self.create_strings()?;

        // Create configuration
        self.create_configuration()?;

        // Create and link all functions
        for func in &self.functions {
            func.create(&self.gadget_path)?;
            func.link(&self.config_path, &self.gadget_path)?;
        }

        info!("OTG USB Gadget setup complete");
        Ok(())
    }

    /// Bind gadget to UDC
    pub fn bind(&mut self) -> Result<()> {
        let udc = Self::find_udc().ok_or_else(|| {
            AppError::Internal("No USB Device Controller (UDC) found".to_string())
        })?;

        // Recreate config symlinks before binding to avoid kernel gadget issues after rebind
        if let Err(e) = self.recreate_config_links() {
            warn!("Failed to recreate gadget config links before bind: {}", e);
        }

        info!("Binding gadget to UDC: {}", udc);
        write_file(&self.gadget_path.join("UDC"), &udc)?;
        self.bound_udc = Some(udc);
        std::thread::sleep(std::time::Duration::from_millis(REBIND_DELAY_MS));

        Ok(())
    }

    /// Unbind gadget from UDC
    pub fn unbind(&mut self) -> Result<()> {
        if self.is_bound() {
            write_file(&self.gadget_path.join("UDC"), "")?;
            self.bound_udc = None;
            info!("Unbound gadget from UDC");
            std::thread::sleep(std::time::Duration::from_millis(REBIND_DELAY_MS));
        }
        Ok(())
    }

    /// Cleanup all resources
    pub fn cleanup(&mut self) -> Result<()> {
        if !self.gadget_exists() {
            return Ok(());
        }

        info!("Cleaning up OTG USB Gadget: {}", self.gadget_name);

        // Unbind from UDC first
        let _ = self.unbind();

        // Unlink and cleanup functions
        for func in self.functions.iter().rev() {
            let _ = func.unlink(&self.config_path);
        }

        // Remove config strings
        let config_strings = self.config_path.join("strings/0x409");
        let _ = remove_dir(&config_strings);
        let _ = remove_dir(&self.config_path);

        // Cleanup functions
        for func in self.functions.iter().rev() {
            let _ = func.cleanup(&self.gadget_path);
        }

        // Remove gadget strings
        let gadget_strings = self.gadget_path.join("strings/0x409");
        let _ = remove_dir(&gadget_strings);

        // Remove gadget directory
        if let Err(e) = remove_dir(&self.gadget_path) {
            warn!("Could not remove gadget directory: {}", e);
        }

        self.created_by_us = false;
        info!("OTG USB Gadget cleanup complete");
        Ok(())
    }

    /// Set USB device descriptors
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
        write_file(&self.gadget_path.join("bDeviceClass"), "0x00")?; // Composite device
        write_file(&self.gadget_path.join("bDeviceSubClass"), "0x00")?;
        write_file(&self.gadget_path.join("bDeviceProtocol"), "0x00")?;
        debug!("Set device descriptors");
        Ok(())
    }

    /// Create USB strings
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

    /// Create configuration
    fn create_configuration(&self) -> Result<()> {
        create_dir(&self.config_path)?;

        // Create config strings
        let strings_path = self.config_path.join("strings/0x409");
        create_dir(&strings_path)?;
        write_file(&strings_path.join("configuration"), "Config 1: HID + MSD")?;

        // Set max power (500mA)
        write_file(&self.config_path.join("MaxPower"), "500")?;

        debug!("Created configuration c.1");
        Ok(())
    }

    /// Get function metadata
    pub fn get_meta(&self) -> &HashMap<String, FunctionMeta> {
        &self.meta
    }

    /// Get endpoint usage info
    pub fn endpoint_info(&self) -> (u8, u8) {
        (
            self.endpoint_allocator.used(),
            self.endpoint_allocator.max(),
        )
    }

    /// Get gadget path
    pub fn gadget_path(&self) -> &PathBuf {
        &self.gadget_path
    }

    /// Recreate config symlinks from functions directory
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

/// Wait for HID devices to become available
///
/// Uses exponential backoff starting from 10ms, capped at 100ms,
/// to reduce CPU usage while still providing fast response.
pub async fn wait_for_hid_devices(device_paths: &[PathBuf], timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);

    // Exponential backoff: start at 10ms, double each time, cap at 100ms
    let mut delay_ms = 10u64;
    const MAX_DELAY_MS: u64 = 100;

    while start.elapsed() < timeout {
        if device_paths.iter().all(|p| p.exists()) {
            return true;
        }

        // Calculate remaining time to avoid overshooting timeout
        let remaining = timeout.saturating_sub(start.elapsed());
        let sleep_duration = std::time::Duration::from_millis(delay_ms).min(remaining);

        if sleep_duration.is_zero() {
            break;
        }

        tokio::time::sleep(sleep_duration).await;

        // Exponential backoff with cap
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
        assert!(!manager.gadget_exists()); // Won't exist in test environment
    }

    #[test]
    fn test_endpoint_tracking() {
        let mut manager = OtgGadgetManager::with_config("test", 8);

        // Keyboard uses 1 endpoint
        let _ = manager.add_keyboard();
        assert_eq!(manager.endpoint_allocator.used(), 1);

        // Mouse uses 1 endpoint each
        let _ = manager.add_mouse_relative();
        let _ = manager.add_mouse_absolute();
        assert_eq!(manager.endpoint_allocator.used(), 3);
    }
}
