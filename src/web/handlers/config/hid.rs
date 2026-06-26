use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::{HidBackend, HidConfig};
use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_hid_config, try_apply_lock, ConfigApplyOptions};
use super::types::HidConfigUpdate;

pub async fn get_hid_config(State(state): State<Arc<AppState>>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

pub async fn update_hid_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.otg, "otg")?;
    let old_hid_config = state.config.get().hid.clone();

    let mut staged_hid_config = old_hid_config.clone();
    req.apply_to(&mut staged_hid_config);
    let descriptor_update = req
        .ch9329_descriptor
        .as_ref()
        .map(|_| staged_hid_config.ch9329_descriptor.clone());
    if descriptor_update.is_some() {
        staged_hid_config.ch9329_descriptor = old_hid_config.ch9329_descriptor.clone();
    }

    state
        .config
        .update(|config| {
            config.hid = staged_hid_config.clone();
            config.enforce_invariants();
        })
        .await?;

    let new_hid_config = state.config.get().hid.clone();

    apply_hid_config(
        &state,
        &old_hid_config,
        &new_hid_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    if let Some(descriptor) = descriptor_update {
        if new_hid_config.backend != HidBackend::Ch9329 {
            return Ok(Json(new_hid_config));
        }

        let actual = state.hid.apply_ch9329_descriptor(&descriptor).await?;
        state
            .config
            .update(|config| {
                config.hid.ch9329_descriptor = actual.descriptor.clone();
                config.enforce_invariants();
            })
            .await?;
        return Ok(Json(state.config.get().hid.clone()));
    }

    Ok(Json(new_hid_config))
}
