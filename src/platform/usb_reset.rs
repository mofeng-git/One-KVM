//! USB device enumeration and reset via sysfs `authorized`.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct UsbDeviceInfo {
    pub bus_num: u32,
    pub dev_num: u32,
    pub id_vendor: String,
    pub id_product: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_device: Option<String>,
}

fn read_sysfs_str(dir: &Path, attr: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(attr))
        .ok()
        .map(|s| s.trim_end().to_string())
}

fn read_sysfs_u32(dir: &Path, attr: &str) -> Option<u32> {
    read_sysfs_str(dir, attr).and_then(|s| s.parse().ok())
}

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
        let device_link = v4l_class.join(name_str).join("device");
        if let Ok(usb_dir) = usb_device_dir_for_v4l_sysfs(&device_link) {
            if let Some(key) = usb_dir.file_name().and_then(|k| k.to_str()) {
                map.insert(key.to_string(), format!("/dev/{}", name_str));
            }
        }
    }
    map
}

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

    devices.sort_by(|a, b| (a.bus_num, a.dev_num).cmp(&(b.bus_num, b.dev_num)));
    devices
}

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
