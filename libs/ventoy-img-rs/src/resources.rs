//! Ventoy resources loader
//!
//! Loads Ventoy boot resources from external files in a resource directory.
//! Resource files (boot.img, core.img, ventoy.disk.img) should be pre-decompressed.

use crate::error::{Result, VentoyError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resource file names
const BOOT_IMG_NAME: &str = "boot.img";
const CORE_IMG_NAME: &str = "core.img";
const VENTOY_DISK_IMG_NAME: &str = "ventoy.disk.img";

/// Ventoy signature (16 bytes at MBR offset 0x190)
pub const VENTOY_SIGNATURE: [u8; 16] = [
    0x56, 0x54, 0x00, 0x47, 0x65, 0x00, 0x48, 0x44, 0x00, 0x52, 0x64, 0x00, 0x20, 0x45, 0x72, 0x0D,
];

/// Cached resources loaded from disk
struct ResourceCache {
    boot_img: Vec<u8>,
    core_img: Vec<u8>,
    ventoy_disk_img: Vec<u8>,
}

/// Global resource cache
static RESOURCE_CACHE: OnceLock<ResourceCache> = OnceLock::new();

/// Initialize resources from a directory
///
/// This function must be called before using `get_boot_img()`, `get_core_img()`,
/// or `get_ventoy_disk_img()`. It loads all resource files into memory.
///
/// # Arguments
/// * `resource_dir` - Path to directory containing boot.img, core.img, ventoy.disk.img
///
/// # Example
/// ```no_run
/// use ventoy_img::resources::init_resources;
/// use std::path::Path;
///
/// init_resources(Path::new("/var/lib/one-kvm/ventoy")).unwrap();
/// ```
pub fn init_resources(resource_dir: &Path) -> Result<()> {
    if RESOURCE_CACHE.get().is_some() {
        // Already initialized
        return Ok(());
    }

    let boot_path = resource_dir.join(BOOT_IMG_NAME);
    let core_path = resource_dir.join(CORE_IMG_NAME);
    let ventoy_disk_path = resource_dir.join(VENTOY_DISK_IMG_NAME);

    // Check all files exist
    if !boot_path.exists() {
        return Err(VentoyError::ResourceNotFound(format!(
            "boot.img not found at {}",
            boot_path.display()
        )));
    }
    if !core_path.exists() {
        return Err(VentoyError::ResourceNotFound(format!(
            "core.img not found at {}",
            core_path.display()
        )));
    }
    if !ventoy_disk_path.exists() {
        return Err(VentoyError::ResourceNotFound(format!(
            "ventoy.disk.img not found at {}",
            ventoy_disk_path.display()
        )));
    }

    // Load files
    let boot_img = fs::read(&boot_path).map_err(|e| {
        VentoyError::ResourceNotFound(format!("Failed to read {}: {}", boot_path.display(), e))
    })?;

    let core_img = fs::read(&core_path).map_err(|e| {
        VentoyError::ResourceNotFound(format!("Failed to read {}: {}", core_path.display(), e))
    })?;

    let ventoy_disk_img = fs::read(&ventoy_disk_path).map_err(|e| {
        VentoyError::ResourceNotFound(format!(
            "Failed to read {}: {}",
            ventoy_disk_path.display(),
            e
        ))
    })?;

    // Validate boot.img size
    if boot_img.len() != 512 {
        return Err(VentoyError::ResourceNotFound(format!(
            "boot.img has invalid size: {} bytes (expected 512)",
            boot_img.len()
        )));
    }

    let cache = ResourceCache {
        boot_img,
        core_img,
        ventoy_disk_img,
    };

    // Try to set the cache (ignore if already set by another thread)
    let _ = RESOURCE_CACHE.set(cache);

    Ok(())
}

/// Check if resources have been initialized
pub fn is_initialized() -> bool {
    RESOURCE_CACHE.get().is_some()
}

/// Get the boot.img data (512 bytes MBR boot code)
pub fn get_boot_img() -> Result<&'static [u8]> {
    RESOURCE_CACHE
        .get()
        .map(|c| c.boot_img.as_slice())
        .ok_or_else(|| {
            VentoyError::ResourceNotFound(
                "Resources not initialized. Call init_resources() first.".to_string(),
            )
        })
}

/// Get the core.img data (GRUB core image, ~1MB)
pub fn get_core_img() -> Result<&'static [u8]> {
    RESOURCE_CACHE
        .get()
        .map(|c| c.core_img.as_slice())
        .ok_or_else(|| {
            VentoyError::ResourceNotFound(
                "Resources not initialized. Call init_resources() first.".to_string(),
            )
        })
}

/// Get the ventoy.disk.img data (EFI partition, ~32MB)
pub fn get_ventoy_disk_img() -> Result<&'static [u8]> {
    RESOURCE_CACHE
        .get()
        .map(|c| c.ventoy_disk_img.as_slice())
        .ok_or_else(|| {
            VentoyError::ResourceNotFound(
                "Resources not initialized. Call init_resources() first.".to_string(),
            )
        })
}

/// Get the resource directory path for a given data directory
///
/// Returns `{data_dir}/ventoy`
pub fn get_resource_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("ventoy")
}

/// List required resource files
pub fn required_files() -> &'static [&'static str] {
    &[BOOT_IMG_NAME, CORE_IMG_NAME, VENTOY_DISK_IMG_NAME]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_resources(dir: &Path) {
        // Create boot.img (512 bytes)
        let mut boot = std::fs::File::create(dir.join(BOOT_IMG_NAME)).unwrap();
        boot.write_all(&[0u8; 512]).unwrap();

        // Create core.img (fake, 1KB)
        let mut core = std::fs::File::create(dir.join(CORE_IMG_NAME)).unwrap();
        core.write_all(&[0u8; 1024]).unwrap();

        // Create ventoy.disk.img (fake, 1KB)
        let mut ventoy = std::fs::File::create(dir.join(VENTOY_DISK_IMG_NAME)).unwrap();
        ventoy.write_all(&[0u8; 1024]).unwrap();
    }

    #[test]
    fn test_required_files() {
        let files = required_files();
        assert_eq!(files.len(), 3);
        assert!(files.contains(&"boot.img"));
        assert!(files.contains(&"core.img"));
        assert!(files.contains(&"ventoy.disk.img"));
    }

    #[test]
    fn test_get_resource_dir() {
        let data_dir = Path::new("/var/lib/one-kvm");
        let resource_dir = get_resource_dir(data_dir);
        assert_eq!(resource_dir, PathBuf::from("/var/lib/one-kvm/ventoy"));
    }
}
