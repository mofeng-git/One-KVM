use axum::{extract::State, Json};
use serde::Serialize;
use typeshare::typeshare;

use crate::config::{HidConfig, MsdConfig, OtgNetworkConfig};
use crate::error::Result;
use crate::otg::OtgNetworkStatus;
use crate::web::state::UsbApiState;

use super::types::OtgConfigUpdate;
use super::usb_update::{stage_hid_config_update, update_usb_config_with_reset};

#[typeshare]
#[derive(Debug, Serialize)]
pub struct OtgConfigResponse {
    pub hid: HidConfig,
    pub msd: MsdConfig,
    pub network: OtgNetworkConfig,
    pub status: OtgNetworkStatus,
}

pub async fn update_otg_config(
    State(state): State<UsbApiState>,
    Json(request): Json<OtgConfigUpdate>,
) -> Result<Json<OtgConfigResponse>> {
    update_otg_config_inner(&state, request).await.map(Json)
}

pub(super) async fn update_otg_config_inner(
    state: &UsbApiState,
    request: OtgConfigUpdate,
) -> Result<OtgConfigResponse> {
    let reset = request
        .hid
        .as_ref()
        .and_then(|h| h.bluetooth_reset_pairing)
        .unwrap_or(false);
    let staged_config = update_usb_config_with_reset(state, reset, move |staged| {
        let requested_ch9329_descriptor = match request.hid.as_ref() {
            Some(update) => stage_hid_config_update(&mut staged.hid, update)?,
            None => None,
        };

        if let Some(ref update) = request.msd {
            update.validate()?;
            update.apply_to(&mut staged.msd);
        }
        if let Some(ref update) = request.network {
            update.apply_to(&mut staged.otg_network);
        }

        Ok(requested_ch9329_descriptor)
    })
    .await?;

    Ok(OtgConfigResponse {
        hid: staged_config.hid,
        msd: staged_config.msd,
        network: staged_config.otg_network,
        status: state.otg.network_status().await,
    })
}
