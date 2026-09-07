use axum::{extract::State, Json};

use crate::config::UacConfig;
use crate::error::Result;
use crate::web::state::UsbApiState;

use super::usb_update::update_usb_config;

pub async fn get_uac_config(State(state): State<UsbApiState>) -> Json<UacConfig> {
    Json(state.config.get().uac.clone())
}

pub async fn update_uac_config(
    State(state): State<UsbApiState>,
    Json(request): Json<UacConfig>,
) -> Result<Json<UacConfig>> {
    request.validate()?;
    let config = update_usb_config(&state, move |staged| {
        staged.uac = request;
        Ok(None)
    })
    .await?;
    Ok(Json(config.uac))
}
