use axum::{extract::State, Json};

use crate::config::HidConfig;
use crate::error::Result;
use crate::web::state::UsbApiState;

use super::types::HidConfigUpdate;
use super::usb_update::{stage_hid_config_update, update_usb_config_with_reset};

pub async fn get_hid_config(State(state): State<UsbApiState>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

pub async fn update_hid_config(
    State(state): State<UsbApiState>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    let reset = req.bluetooth_reset_pairing.unwrap_or(false);
    let config = update_usb_config_with_reset(&state, reset, move |staged| {
        stage_hid_config_update(&mut staged.hid, &req)
    })
    .await?;
    Ok(Json(config.hid))
}
