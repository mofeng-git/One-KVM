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
    state
        .config
        .update(|config| {
            update.apply_to(&mut config.auth);
        })
        .await?;

    let auth = state.config.get().auth.clone();
    state.sessions.set_ttl(auth.session_timeout_secs as i64);
    state.redfish_sessions.set_ttl(auth.session_timeout_secs as i64);
    Ok(Json(auth))
}
