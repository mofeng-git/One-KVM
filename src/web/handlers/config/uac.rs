use std::sync::Arc;

use axum::{extract::State, Json};

use crate::error::Result;
use crate::otg::service::UacConfig;
use crate::state::AppState;

use super::apply::try_apply_lock;

pub async fn get_uac_config(State(state): State<Arc<AppState>>) -> Json<UacConfig> {
    Json(state.config.get().uac.clone())
}

pub async fn update_uac_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UacConfig>,
) -> Result<Json<UacConfig>> {
    req.validate()?;
    let _guard = try_apply_lock(&state.config_apply_locks.otg, "uac")?;

    let old_config = (*state.config.get()).clone();
    let mut new_config = old_config.clone();
    new_config.uac = req;

    state
        .config
        .update(|config| {
            config.uac = new_config.uac.clone();
        })
        .await?;

    super::apply::apply_usb_config(&state, &old_config, &new_config).await?;

    Ok(Json(state.config.get().uac.clone()))
}
