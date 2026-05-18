//! Host diagnostics used by the web status API.

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub hostname: String,
    pub cpu_model: String,
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub network_addresses: Vec<NetworkAddress>,
    pub serial_ports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkAddress {
    pub interface: String,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskSpaceInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
}

#[cfg(unix)]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use linux as platform;
#[cfg(windows)]
use windows as platform;

pub fn get_disk_space(path: &std::path::Path) -> Result<DiskSpaceInfo> {
    platform::get_disk_space(path)
}

pub fn get_device_info() -> DeviceInfo {
    platform::get_device_info()
}
