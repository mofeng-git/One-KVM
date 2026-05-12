use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::types::{RedfishConfigResponse, RedfishConfigUpdate};

pub async fn get_redfish_config(State(state): State<Arc<AppState>>) -> Json<RedfishConfigResponse> {
    Json(RedfishConfigResponse {
        enabled: state.config.get().redfish.enabled,
    })
}

pub async fn update_redfish_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RedfishConfigUpdate>,
) -> Result<Json<RedfishConfigResponse>> {
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.redfish);
        })
        .await?;

    Ok(Json(RedfishConfigResponse {
        enabled: state.config.get().redfish.enabled,
    }))
}
