use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};

pub const CONFIGFS_PATH: &str = "/sys/kernel/config/usb_gadget";
pub const DEFAULT_GADGET_NAME: &str = "one-kvm";
pub const DEFAULT_USB_VENDOR_ID: u16 = 0x1d6b;
pub const DEFAULT_USB_PRODUCT_ID: u16 = 0x0104;
pub const DEFAULT_USB_BCD_DEVICE: u16 = 0x0100;
pub const USB_BCD_USB: u16 = 0x0200;

pub fn is_configfs_available() -> bool {
    Path::new(CONFIGFS_PATH).exists()
}

/// Loads `libcomposite` if needed; does not mount configfs.
pub fn ensure_libcomposite_loaded() -> Result<()> {
    if is_configfs_available() {
        return Ok(());
    }

    if !Path::new("/sys/module/libcomposite").exists() {
        let status = Command::new("modprobe")
            .arg("libcomposite")
            .status()
            .map_err(|e| {
                AppError::Internal(format!("Failed to run modprobe libcomposite: {}", e))
            })?;

        if !status.success() {
            return Err(AppError::Internal(format!(
                "modprobe libcomposite failed with status {}",
                status
            )));
        }
    }

    if is_configfs_available() {
        Ok(())
    } else {
        Err(AppError::Internal(
            "libcomposite is not available after modprobe; check configfs mount and kernel support"
                .to_string(),
        ))
    }
}

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

pub fn is_low_endpoint_udc(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("musb") || name.contains("musb-hdrc")
}

/// Sysfs/configfs: one write syscall with final buffer (incl. newline when needed).
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .or_else(|e| {
            if path.exists() {
                Err(e)
            } else {
                File::create(path)
            }
        })
        .map_err(|e| AppError::Internal(format!("Failed to open {}: {}", path.display(), e)))?;

    let data: std::borrow::Cow<[u8]> = if content.ends_with('\n') {
        content.as_bytes().into()
    } else {
        let mut buf = content.as_bytes().to_vec();
        buf.push(b'\n');
        buf.into()
    };

    file.write_all(&data)
        .map_err(|e| AppError::Internal(format!("Failed to write to {}: {}", path.display(), e)))?;

    file.flush()
        .map_err(|e| AppError::Internal(format!("Failed to flush {}: {}", path.display(), e)))?;

    Ok(())
}

pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = File::create(path)
        .map_err(|e| AppError::Internal(format!("Failed to create {}: {}", path.display(), e)))?;

    file.write_all(data)
        .map_err(|e| AppError::Internal(format!("Failed to write to {}: {}", path.display(), e)))?;

    Ok(())
}

pub fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| {
        AppError::Internal(format!(
            "Failed to create directory {}: {}",
            path.display(),
            e
        ))
    })
}

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

pub fn remove_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| {
            AppError::Internal(format!("Failed to remove file {}: {}", path.display(), e))
        })?;
    }
    Ok(())
}

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
