use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::HidConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_hid_config;
use super::types::HidConfigUpdate;

pub async fn get_hid_config(State(state): State<Arc<AppState>>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

pub async fn update_hid_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    req.validate()?;

    let old_hid_config = state.config.get().hid.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.hid);
        })
        .await?;

    let new_hid_config = state.config.get().hid.clone();

    if let Err(e) = apply_hid_config(&state, &old_hid_config, &new_hid_config).await {
        tracing::error!("Failed to apply HID config: {}", e);
    }

    Ok(Json(new_hid_config))
}
