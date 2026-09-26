use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::SwitchConfig;
use crate::error::{AppError, Result};
use crate::state::AppState;

use super::apply::{apply_switch_config, try_apply_lock};
use super::types::SwitchConfigUpdate;

pub async fn get_switch_config(State(state): State<Arc<AppState>>) -> Json<SwitchConfig> {
    Json(state.config.get().switch.clone())
}

pub async fn update_switch_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchConfigUpdate>,
) -> Result<Json<SwitchConfig>> {
    let current_config = state.config.get();
    let old_config = current_config.switch.clone();

    let mut merged = old_config.clone();
    req.apply_to(&mut merged);
    merged.normalize();
    validate_effective_config(&merged)?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.switch, "switch")?;

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.switch);
        })
        .await?;

    let new_config = state.config.get().switch.clone();

    apply_switch_config(&state, &old_config, &new_config).await?;

    Ok(Json(new_config))
}

fn validate_effective_config(config: &SwitchConfig) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    if config.device.trim().is_empty() {
        return Err(AppError::BadRequest(
            "KVM switch device cannot be empty when enabled".to_string(),
        ));
    }

    if config.baud_rate == 0 {
        return Err(AppError::BadRequest(
            "KVM switch baud_rate must be greater than 0".to_string(),
        ));
    }

    if !cfg!(windows) && !std::path::Path::new(&config.device).exists() {
        return Err(AppError::BadRequest(format!(
            "KVM switch device '{}' does not exist",
            config.device
        )));
    }

    Ok(())
}
