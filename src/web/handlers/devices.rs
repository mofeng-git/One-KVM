//! Device discovery handlers
//!
//! Provides API endpoints for discovering available hardware devices.

use axum::Json;
use serde::Deserialize;

use crate::atx::{discover_devices, AtxDevices};
use crate::error::{AppError, Result};
use crate::video::usb_reset;

/// GET /api/devices/atx - List available ATX devices
///
/// Returns lists of available GPIO chips and USB HID relay devices.
pub async fn list_atx_devices() -> Json<AtxDevices> {
    Json(discover_devices())
}

/// GET /api/devices/usb - List all USB devices
///
/// Enumerates USB devices from `/sys/bus/usb/devices/` with associated
/// video device mappings.
pub async fn list_usb_devices() -> Json<Vec<usb_reset::UsbDeviceInfo>> {
    Json(usb_reset::list_usb_devices())
}

#[derive(Deserialize)]
pub struct UsbResetRequest {
    pub bus_num: u32,
    pub dev_num: u32,
}

/// POST /api/devices/usb/reset - Reset a USB device via authorized cycle
///
/// Writes `0` then `1` to the device's `authorized` sysfs attribute,
/// causing the kernel to deauthorize and re-authorize the device.
/// Requires root or write access to sysfs.
pub async fn reset_usb_device(Json(req): Json<UsbResetRequest>) -> Result<Json<serde_json::Value>> {
    usb_reset::reset_usb_device(req.bus_num, req.dev_num).map_err(|e| {
        AppError::VideoError(format!(
            "USB reset failed for device {}-{}: {}",
            req.bus_num, req.dev_num, e
        ))
    })?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("USB device {}-{} reset successfully", req.bus_num, req.dev_num)
    })))
}
