//! Shared device description with platform-specific enumeration backends.

use serde::Serialize;

use crate::error::Result;

#[cfg(unix)]
#[path = "device_linux.rs"]
mod imp;

#[cfg(windows)]
#[path = "device_windows.rs"]
mod imp;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub description: String,
    pub card_index: i32,
    pub device_index: i32,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u32>,
    pub is_capture: bool,
    pub is_hdmi: bool,
    pub usb_bus: Option<String>,
}

pub fn enumerate_audio_devices() -> Result<Vec<AudioDeviceInfo>> {
    imp::enumerate_audio_devices_with_current(None)
}

pub fn enumerate_audio_devices_with_current(
    current_device: Option<&str>,
) -> Result<Vec<AudioDeviceInfo>> {
    imp::enumerate_audio_devices_with_current(current_device)
}

pub(crate) fn find_best_audio_device() -> Result<AudioDeviceInfo> {
    imp::find_best_audio_device()
}

#[cfg(windows)]
pub(crate) use imp::find_wasapi_device;
