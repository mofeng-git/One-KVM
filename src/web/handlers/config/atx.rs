//! ATX configuration handlers

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::atx::AtxDriverType;
use crate::config::{AtxConfig, HidBackend, HidConfig};
use crate::error::{AppError, Result};
use crate::state::AppState;

use super::apply::apply_atx_config;
use super::types::AtxConfigUpdate;

/// Get ATX configuration
pub async fn get_atx_config(State(state): State<Arc<AppState>>) -> Json<AtxConfig> {
    Json(state.config.get().atx.clone())
}

/// Update ATX configuration
pub async fn update_atx_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AtxConfigUpdate>,
) -> Result<Json<AtxConfig>> {
    // 1. Read current configuration snapshot
    let current_config = state.config.get();
    let old_atx_config = current_config.atx.clone();

    // 2. Validate request, including merged effective serial parameter checks
    req.validate_with_current(&old_atx_config)?;

    // 3. Ensure ATX serial devices do not conflict with HID CH9329 serial device
    let mut merged_atx_config = old_atx_config.clone();
    req.apply_to(&mut merged_atx_config);
    validate_serial_device_conflict(&merged_atx_config, &current_config.hid)?;

    // 4. Persist update into config store
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.atx);
        })
        .await?;

    // 5. Load new config
    let new_atx_config = state.config.get().atx.clone();

    // 6. Apply to subsystem (hot reload)
    if let Err(e) = apply_atx_config(&state, &old_atx_config, &new_atx_config).await {
        tracing::error!("Failed to apply ATX config: {}", e);
    }

    Ok(Json(new_atx_config))
}

fn validate_serial_device_conflict(atx: &AtxConfig, hid: &HidConfig) -> Result<()> {
    if hid.backend != HidBackend::Ch9329 {
        return Ok(());
    }
    let reserved = hid.ch9329_port.trim();
    if reserved.is_empty() {
        return Ok(());
    }

    for (name, key) in [("power", &atx.power), ("reset", &atx.reset)] {
        if key.driver == AtxDriverType::Serial && key.device.trim() == reserved {
            return Err(AppError::BadRequest(format!(
                "ATX {} serial device '{}' conflicts with HID CH9329 serial device",
                name, reserved
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_serial_device_conflict_rejects_ch9329_overlap() {
        let mut atx = AtxConfig::default();
        atx.power.driver = AtxDriverType::Serial;
        atx.power.device = "/dev/ttyUSB0".to_string();

        let mut hid = HidConfig::default();
        hid.backend = HidBackend::Ch9329;
        hid.ch9329_port = "/dev/ttyUSB0".to_string();

        assert!(validate_serial_device_conflict(&atx, &hid).is_err());
    }

    #[test]
    fn test_validate_serial_device_conflict_allows_non_ch9329_backend() {
        let mut atx = AtxConfig::default();
        atx.power.driver = AtxDriverType::Serial;
        atx.power.device = "/dev/ttyUSB0".to_string();

        let mut hid = HidConfig::default();
        hid.backend = HidBackend::None;
        hid.ch9329_port = "/dev/ttyUSB0".to_string();

        assert!(validate_serial_device_conflict(&atx, &hid).is_ok());
    }
}
