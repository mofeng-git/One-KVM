use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AuthConfig;
use crate::error::Result;
use crate::state::AppState;

use super::types::AuthConfigUpdate;

/// Get auth configuration (sensitive fields are cleared)
pub async fn get_auth_config(State(state): State<Arc<AppState>>) -> Json<AuthConfig> {
    let mut auth = state.config.get().auth.clone();
    auth.totp_secret = None;
    Json(auth)
}

/// Update auth configuration
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

    let mut auth = state.config.get().auth.clone();
    auth.totp_secret = None;
    Ok(Json(auth))
}
