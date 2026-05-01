use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::HidConfig;
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

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.hid);
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

    Ok(Json(new_hid_config))
}
