use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::MsdConfig;
use crate::error::Result;
use crate::state::AppState;

use super::otg::update_otg_config_inner;
use super::types::{MsdConfigUpdate, OtgConfigUpdate};

pub async fn get_msd_config(State(state): State<Arc<AppState>>) -> Json<MsdConfig> {
    Json(state.config.get().msd.clone())
}

pub async fn update_msd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MsdConfigUpdate>,
) -> Result<Json<MsdConfig>> {
    let response = update_otg_config_inner(
        &state,
        OtgConfigUpdate {
            msd: Some(req),
            ..Default::default()
        },
    )
    .await?;
    Ok(Json(response.msd))
}
