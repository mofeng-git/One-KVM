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
    configfs_path().exists()
}

pub fn configfs_path() -> &'static Path {
    Path::new(CONFIGFS_PATH)
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
    list_udcs().into_iter().next()
}

pub fn list_udcs() -> Vec<String> {
    let mut devices = Vec::new();
    collect_dir_names(Path::new("/sys/class/udc"), &mut devices);
    devices.sort();
    devices.dedup();
    devices
}

/// Sorted entry names of a directory, or an empty list when it cannot be read.
pub fn list_dir_names(path: &Path) -> Vec<String> {
    let mut names = Vec::new();
    collect_dir_names(path, &mut names);
    names.sort();
    names.dedup();
    names
}

/// Returns the name of another gadget currently bound to `udc`, if any.
///
/// The gadget called `exclude_gadget` is ignored: callers handle their own
/// gadget (including stale ones) separately.
pub fn udc_owner_in(root: &Path, udc: &str, exclude_gadget: &str) -> Option<String> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().trim().to_string();
        if name.is_empty() || name == exclude_gadget {
            continue;
        }
        let bound = fs::read_to_string(entry.path().join("UDC")).unwrap_or_default();
        if bound.trim() == udc {
            return Some(name);
        }
    }
    None
}

/// Returns the name of another gadget bound to `udc` on the live system.
pub fn udc_claimed_by_other_gadget(udc: &str, exclude_gadget: &str) -> Option<String> {
    udc_owner_in(configfs_path(), udc, exclude_gadget)
}

fn collect_dir_names(path: &Path, devices: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().trim().to_string();
            if !name.is_empty() {
                devices.push(name);
            }
        }
    }
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

/// Write an optional configfs/sysfs attribute when the running kernel exposes it.
/// This keeps newer kernel enhancements compatible with older kernels.
pub fn write_file_if_exists(path: &Path, content: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    write_file(path, content)?;
    Ok(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_udc(root: &Path, gadget: &str, udc: &str) {
        let path = root.join(gadget);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("UDC"), udc).unwrap();
    }

    #[test]
    fn udc_owner_in_reports_other_gadget_bindings() {
        let temp = tempfile::tempdir().unwrap();
        write_udc(temp.path(), "one-kvm", "fe800000.usb\n");
        write_udc(temp.path(), "rockchip", "fe800000.usb\n");
        write_udc(temp.path(), "idle", "");

        assert_eq!(
            udc_owner_in(temp.path(), "fe800000.usb", "one-kvm").as_deref(),
            Some("rockchip")
        );
        assert_eq!(udc_owner_in(temp.path(), "other.usb", "one-kvm"), None);
        assert_eq!(
            udc_owner_in(temp.path(), "fe800000.usb", "rockchip").as_deref(),
            Some("one-kvm")
        );
    }

    #[test]
    fn udc_owner_in_ignores_unreadable_entries() {
        let temp = tempfile::tempdir().unwrap();
        write_udc(temp.path(), "rockchip", "");

        assert_eq!(udc_owner_in(temp.path(), "fe800000.usb", "one-kvm"), None);
        assert_eq!(
            udc_owner_in(&temp.path().join("missing"), "fe800000.usb", "one-kvm"),
            None
        );
    }
}
