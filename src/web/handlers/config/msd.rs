use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::MsdConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_msd_config, try_apply_lock, ConfigApplyOptions};
use super::types::MsdConfigUpdate;

pub async fn get_msd_config(State(state): State<Arc<AppState>>) -> Json<MsdConfig> {
    Json(state.config.get().msd.clone())
}

pub async fn update_msd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MsdConfigUpdate>,
) -> Result<Json<MsdConfig>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.otg, "otg")?;
    let old_msd_config = state.config.get().msd.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.msd);
        })
        .await?;

    let new_msd_config = state.config.get().msd.clone();

    apply_msd_config(
        &state,
        &old_msd_config,
        &new_msd_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    Ok(Json(new_msd_config))
}
