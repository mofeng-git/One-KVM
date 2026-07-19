use std::sync::Arc;

use axum::{extract::State, Json};

use crate::error::{AppError, Result};
use crate::state::AppState;

use super::apply::try_apply_lock;
use super::types::{WatchdogConfigResponse, WatchdogConfigUpdate};

async fn response(state: &AppState) -> WatchdogConfigResponse {
    let runtime = state.watchdog.status().await;
    WatchdogConfigResponse {
        enabled: state.config.get().watchdog.enabled,
        supported: runtime.supported,
        running: runtime.running,
        reason: runtime.reason,
    }
}

pub async fn get_watchdog_config(
    State(state): State<Arc<AppState>>,
) -> Json<WatchdogConfigResponse> {
    Json(response(&state).await)
}

pub async fn update_watchdog_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WatchdogConfigUpdate>,
) -> Result<Json<WatchdogConfigResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.watchdog, "watchdog")?;
    let old_enabled = state.config.get().watchdog.enabled;

    if req.enabled {
        state.watchdog.enable().await.map_err(|error| {
            AppError::Config(format!("Failed to enable hardware watchdog: {error}"))
        })?;

        if let Err(error) = state
            .config
            .update(|config| config.watchdog.enabled = true)
            .await
        {
            if !old_enabled {
                if let Err(disable_error) = state.watchdog.disable().await {
                    tracing::error!(
                        "Failed to roll back watchdog after persistence error: {}",
                        disable_error
                    );
                }
            }
            return Err(error);
        }
    } else {
        state.watchdog.disable().await.map_err(|error| {
            AppError::Config(format!(
                "Hardware watchdog cannot be safely disabled; keepalive continues: {error}"
            ))
        })?;

        if let Err(error) = state
            .config
            .update(|config| config.watchdog.enabled = false)
            .await
        {
            if old_enabled {
                if let Err(enable_error) = state.watchdog.enable().await {
                    tracing::error!(
                        "Failed to restore watchdog after persistence error: {}",
                        enable_error
                    );
                }
            }
            return Err(error);
        }
    }

    Ok(Json(response(&state).await))
}
