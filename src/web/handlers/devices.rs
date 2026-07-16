use axum::Json;
#[cfg(unix)]
use serde::Deserialize;

use crate::atx::{discover_devices, AtxDevices};
#[cfg(unix)]
use crate::error::{AppError, Result};
#[cfg(unix)]
use crate::platform::usb_reset;

pub async fn list_atx_devices() -> Json<AtxDevices> {
    Json(discover_devices())
}

#[cfg(unix)]
pub async fn list_usb_devices() -> Json<Vec<usb_reset::UsbDeviceInfo>> {
    Json(usb_reset::list_usb_devices())
}

#[cfg(unix)]
pub async fn list_network_interfaces() -> Result<Json<Vec<crate::otg::bridge::NetworkInterfaceInfo>>>
{
    crate::otg::bridge::list_network_interfaces().map(Json)
}

#[cfg(unix)]
#[derive(Deserialize)]
pub struct UsbResetRequest {
    pub bus_num: u32,
    pub dev_num: u32,
}

#[cfg(unix)]
pub async fn reset_usb_device(Json(req): Json<UsbResetRequest>) -> Result<Json<serde_json::Value>> {
    usb_reset::reset_usb_device(req.bus_num, req.dev_num).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "USB reset failed for device {}-{}: {}",
                req.bus_num, req.dev_num, e
            ),
        ))
    })?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("USB device {}-{} reset successfully", req.bus_num, req.dev_num)
    })))
}
