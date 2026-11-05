use axum::{extract::State, Json};

use crate::config::OtgNetworkConfig;
use crate::error::Result;
use crate::otg::OtgNetworkStatus;
use crate::web::state::UsbApiState;

use super::otg::update_otg_config_inner;
use super::types::{OtgConfigUpdate, OtgNetworkConfigUpdate};

pub async fn get_otg_network_config(State(state): State<UsbApiState>) -> Json<OtgNetworkConfig> {
    Json(state.config.get().otg_network.clone())
}

pub async fn update_otg_network_config(
    State(state): State<UsbApiState>,
    Json(request): Json<OtgNetworkConfigUpdate>,
) -> Result<Json<OtgNetworkConfig>> {
    let response = update_otg_config_inner(
        &state,
        OtgConfigUpdate {
            network: Some(request),
            ..Default::default()
        },
    )
    .await?;
    Ok(Json(response.network))
}

pub async fn get_otg_network_status(State(state): State<UsbApiState>) -> Json<OtgNetworkStatus> {
    Json(state.otg.network_status().await)
}
