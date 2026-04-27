//! USB device enumeration and reset via sysfs `authorized`.
//!
//! Provides APIs for the settings page to list and reset USB devices.
//! Requires write access to `/sys/bus/usb/devices/.../authorized` (typically root).

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Walk up from a V4L sysfs `device` link until we find a USB device node
/// (`busnum` + `devnum` present).
fn usb_device_dir_for_v4l_sysfs(device_link: &Path) -> io::Result<PathBuf> {
    let mut p = device_link.canonicalize()?;
    loop {
        if p.join("busnum").is_file() && p.join("devnum").is_file() {
            return Ok(p);
        }
        p = p
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no USB parent in sysfs"))?
            .to_path_buf();
        if p.as_os_str().is_empty() || p == Path::new("/") {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "reached sysfs root without USB device",
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// USB device enumeration & reset-by-bus/dev (for the settings API)
// ---------------------------------------------------------------------------

use serde::Serialize;

/// Information about a single USB device, read from `/sys/bus/usb/devices/`.
#[derive(Debug, Serialize)]
pub struct UsbDeviceInfo {
    /// USB bus number (`busnum` sysfs attribute).
    pub bus_num: u32,
    /// USB device number on the bus (`devnum` sysfs attribute).
    pub dev_num: u32,
    /// Vendor ID hex string, e.g. `"1d6b"`.
    pub id_vendor: String,
    /// Product ID hex string, e.g. `"0002"`.
    pub id_product: String,
    /// Product name from sysfs `product`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// Manufacturer name from sysfs `manufacturer`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// Speed in Mbps from sysfs `speed`, e.g. `"480"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    /// `true` if authorized=1, `false` if authorized=0, `None` if no file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized: Option<bool>,
    /// Kernel driver bound to this device (from driver symlink).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// Associated `/dev/videoN` node, if this USB device has a V4L2 child.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_device: Option<String>,
}

/// Read a sysfs string attribute, trimming trailing newline.
fn read_sysfs_str(dir: &Path, attr: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(attr))
        .ok()
        .map(|s| s.trim_end().to_string())
}

/// Read a sysfs u32 attribute.
fn read_sysfs_u32(dir: &Path, attr: &str) -> Option<u32> {
    read_sysfs_str(dir, attr).and_then(|s| s.parse().ok())
}

/// Build a map from USB sysfs dir → video device name by scanning
/// `/sys/class/video4linux/`.
fn build_usb_to_video_map() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let v4l_class = Path::new("/sys/class/video4linux");
    let entries = match std::fs::read_dir(v4l_class) {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = match name.to_str() {
            Some(s) if s.starts_with("video") => s,
            _ => continue,
        };
        // Resolve the device symlink and walk up to find the USB parent
        let device_link = v4l_class.join(name_str).join("device");
        if let Ok(usb_dir) = usb_device_dir_for_v4l_sysfs(&device_link) {
            if let Some(key) = usb_dir.file_name().and_then(|k| k.to_str()) {
                map.insert(key.to_string(), format!("/dev/{}", name_str));
            }
        }
    }
    map
}

/// List all USB devices visible in `/sys/bus/usb/devices/`.
pub fn list_usb_devices() -> Vec<UsbDeviceInfo> {
    let usb_bus = Path::new("/sys/bus/usb/devices");
    let entries = match std::fs::read_dir(usb_bus) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let video_map = build_usb_to_video_map();

    let mut devices: Vec<UsbDeviceInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            // Only consider entries that have busnum + devnum (actual devices, not interfaces)
            let bus_num = read_sysfs_u32(&dir, "busnum")?;
            let dev_num = read_sysfs_u32(&dir, "devnum")?;

            let id_vendor = read_sysfs_str(&dir, "idVendor").unwrap_or_default();
            let id_product = read_sysfs_str(&dir, "idProduct").unwrap_or_default();

            let product = read_sysfs_str(&dir, "product");
            let manufacturer = read_sysfs_str(&dir, "manufacturer");
            let speed = read_sysfs_str(&dir, "speed");

            let authorized = if dir.join("authorized").exists() {
                read_sysfs_str(&dir, "authorized")
                    .and_then(|s| s.trim().parse::<u8>().ok())
                    .map(|v| v != 0)
            } else {
                None
            };

            let driver = std::fs::read_link(dir.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()));

            let dir_name = dir.file_name()?.to_str()?.to_string();
            let video_device = video_map.get(&dir_name).cloned();

            Some(UsbDeviceInfo {
                bus_num,
                dev_num,
                id_vendor,
                id_product,
                product,
                manufacturer,
                speed,
                authorized,
                driver,
                video_device,
            })
        })
        .collect();

    // Sort by bus, then device number for stable ordering.
    devices.sort_by(|a, b| (a.bus_num, a.dev_num).cmp(&(b.bus_num, b.dev_num)));
    devices
}

/// Reset a USB device identified by bus/dev numbers via the `authorized` sysfs
/// attribute. After re-authorizing, waits for the device to reappear.
pub fn reset_usb_device(bus_num: u32, dev_num: u32) -> io::Result<()> {
    let usb_bus = Path::new("/sys/bus/usb/devices");
    let entries = std::fs::read_dir(usb_bus)?;

    for entry in entries.flatten() {
        let dir = entry.path();
        if read_sysfs_u32(&dir, "busnum") != Some(bus_num)
            || read_sysfs_u32(&dir, "devnum") != Some(dev_num)
        {
            continue;
        }
        let authorized = dir.join("authorized");
        if !authorized.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("device {bus_num}-{dev_num} has no authorized attribute"),
            ));
        }
        std::fs::write(&authorized, b"0")?;
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(&authorized, b"1")?;

        // Wait for device to reappear
        let wait_until = Instant::now() + Duration::from_secs(2);
        while !dir.join("busnum").exists() {
            if Instant::now() >= wait_until {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("USB device {bus_num}-{dev_num} not found in sysfs"),
    ))
}
