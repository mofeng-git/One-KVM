use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::HidConfig;
use crate::error::Result;
use crate::state::AppState;

use super::otg::update_otg_config_inner;
use super::types::{HidConfigUpdate, OtgConfigUpdate};

pub async fn get_hid_config(State(state): State<Arc<AppState>>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

pub async fn update_hid_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    let response = update_otg_config_inner(
        &state,
        OtgConfigUpdate {
            hid: Some(req),
            ..Default::default()
        },
    )
    .await?;
    Ok(Json(response.hid))
}
