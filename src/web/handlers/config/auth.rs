use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AuthConfig;
use crate::error::Result;
use crate::state::AppState;

use super::types::AuthConfigUpdate;

pub async fn get_auth_config(State(state): State<Arc<AppState>>) -> Json<AuthConfig> {
    Json(state.config.get().auth.clone())
}

pub async fn update_auth_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<AuthConfigUpdate>,
) -> Result<Json<AuthConfig>> {
    update.validate()?;
    state
        .config
        .update(|config| {
            update.apply_to(&mut config.auth);
        })
        .await?;

    Ok(Json(state.config.get().auth.clone()))
}
