use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::MsdConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_msd_config;
use super::types::MsdConfigUpdate;

pub async fn get_msd_config(State(state): State<Arc<AppState>>) -> Json<MsdConfig> {
    Json(state.config.get().msd.clone())
}

pub async fn update_msd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MsdConfigUpdate>,
) -> Result<Json<MsdConfig>> {
    req.validate()?;

    let old_msd_config = state.config.get().msd.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.msd);
        })
        .await?;

    let new_msd_config = state.config.get().msd.clone();

    if let Err(e) = apply_msd_config(&state, &old_msd_config, &new_msd_config).await {
        tracing::error!("Failed to apply MSD config: {}", e);
    }

    Ok(Json(new_msd_config))
}
