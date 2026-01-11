//! ConfigFS file operations for USB Gadget

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::{AppError, Result};

/// ConfigFS base path for USB gadgets
pub const CONFIGFS_PATH: &str = "/sys/kernel/config/usb_gadget";

/// Default gadget name
pub const DEFAULT_GADGET_NAME: &str = "one-kvm";

/// USB Vendor ID (Linux Foundation) - default value
pub const DEFAULT_USB_VENDOR_ID: u16 = 0x1d6b;

/// USB Product ID (Multifunction Composite Gadget) - default value
pub const DEFAULT_USB_PRODUCT_ID: u16 = 0x0104;

/// USB device version - default value
pub const DEFAULT_USB_BCD_DEVICE: u16 = 0x0100;

/// USB spec version (USB 2.0)
pub const USB_BCD_USB: u16 = 0x0200;

/// Check if ConfigFS is available
pub fn is_configfs_available() -> bool {
    Path::new(CONFIGFS_PATH).exists()
}

/// Find available UDC (USB Device Controller)
pub fn find_udc() -> Option<String> {
    let udc_path = Path::new("/sys/class/udc");
    if !udc_path.exists() {
        return None;
    }

    fs::read_dir(udc_path)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .next()
}

/// Write string content to a file
///
/// For sysfs files, this function appends a newline and flushes
/// to ensure the kernel processes the write immediately.
///
/// IMPORTANT: sysfs attributes require a single atomic write() syscall.
/// The kernel processes the value on the first write(), so we must
/// build the complete buffer (including newline) before writing.
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    // For sysfs files (especially write-only ones like forced_eject),
    // we need to use simple O_WRONLY without O_TRUNC
    // O_TRUNC may fail on special files or require read permission
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .or_else(|e| {
            // If open fails, try create (for regular files)
            if path.exists() {
                Err(e)
            } else {
                File::create(path)
            }
        })
        .map_err(|e| AppError::Internal(format!("Failed to open {}: {}", path.display(), e)))?;

    // Build complete buffer with newline, then write in single syscall.
    // This is critical for sysfs - multiple write() calls may cause
    // the kernel to only process partial data or return EINVAL.
    let data: std::borrow::Cow<[u8]> = if content.ends_with('\n') {
        content.as_bytes().into()
    } else {
        let mut buf = content.as_bytes().to_vec();
        buf.push(b'\n');
        buf.into()
    };

    file.write_all(&data)
        .map_err(|e| AppError::Internal(format!("Failed to write to {}: {}", path.display(), e)))?;

    // Explicitly flush to ensure sysfs processes the write
    file.flush()
        .map_err(|e| AppError::Internal(format!("Failed to flush {}: {}", path.display(), e)))?;

    Ok(())
}

/// Write binary content to a file
pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = File::create(path)
        .map_err(|e| AppError::Internal(format!("Failed to create {}: {}", path.display(), e)))?;

    file.write_all(data)
        .map_err(|e| AppError::Internal(format!("Failed to write to {}: {}", path.display(), e)))?;

    Ok(())
}

/// Read string content from a file
pub fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .map_err(|e| AppError::Internal(format!("Failed to read {}: {}", path.display(), e)))
}

/// Create directory if not exists
pub fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| {
        AppError::Internal(format!(
            "Failed to create directory {}: {}",
            path.display(),
            e
        ))
    })
}

/// Remove directory
pub fn remove_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir(path).map_err(|e| {
            AppError::Internal(format!(
                "Failed to remove directory {}: {}",
                path.display(),
                e
            ))
        })?;
    }
    Ok(())
}

/// Remove file
pub fn remove_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| {
            AppError::Internal(format!("Failed to remove file {}: {}", path.display(), e))
        })?;
    }
    Ok(())
}

/// Create symlink
pub fn create_symlink(src: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dest).map_err(|e| {
        AppError::Internal(format!(
            "Failed to create symlink {} -> {}: {}",
            dest.display(),
            src.display(),
            e
        ))
    })
}
