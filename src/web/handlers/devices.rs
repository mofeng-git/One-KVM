//! Device discovery handlers
//!
//! Provides API endpoints for discovering available hardware devices.

use axum::Json;

use crate::atx::{discover_devices, AtxDevices};

/// GET /api/devices/atx - List available ATX devices
///
/// Returns lists of available GPIO chips and USB HID relay devices.
pub async fn list_atx_devices() -> Json<AtxDevices> {
    Json(discover_devices())
}
