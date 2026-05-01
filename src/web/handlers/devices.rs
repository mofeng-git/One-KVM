use axum::Json;
use serde::Deserialize;

use crate::atx::{discover_devices, AtxDevices};
use crate::error::{AppError, Result};
use crate::video::usb_reset;

pub async fn list_atx_devices() -> Json<AtxDevices> {
    Json(discover_devices())
}

pub async fn list_usb_devices() -> Json<Vec<usb_reset::UsbDeviceInfo>> {
    Json(usb_reset::list_usb_devices())
}

#[derive(Deserialize)]
pub struct UsbResetRequest {
    pub bus_num: u32,
    pub dev_num: u32,
}

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
