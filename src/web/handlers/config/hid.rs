use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::HidConfig;
use crate::error::Result;
use crate::state::AppState;

use super::types::HidConfigUpdate;
use super::usb_update::{stage_hid_config_update, update_usb_config};

pub async fn get_hid_config(State(state): State<Arc<AppState>>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

pub async fn update_hid_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    let config = update_usb_config(&state, move |staged| {
        stage_hid_config_update(&mut staged.hid, &req)
    })
    .await?;
    Ok(Json(config.hid))
}
