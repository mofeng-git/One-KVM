use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;
use typeshare::typeshare;

use crate::config::validate_shell;
use crate::error::{AppError, Result};
use crate::state::AppState;

#[typeshare]
#[derive(Debug, Clone, serde::Serialize)]
pub struct TerminalConfigResponse {
    pub enabled: bool,
    pub shell: String,
    /// Whether this platform has a built-in PTY terminal implementation.
    pub supported: bool,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct TerminalConfigUpdate {
    pub enabled: Option<bool>,
    pub shell: Option<String>,
}

fn response(state: &AppState) -> TerminalConfigResponse {
    let config = state.config.get();
    TerminalConfigResponse {
        enabled: config.terminal.enabled,
        shell: config.terminal.shell.clone(),
        supported: cfg!(unix),
    }
}

pub async fn get_terminal_config(
    State(state): State<Arc<AppState>>,
) -> Json<TerminalConfigResponse> {
    Json(response(&state))
}

pub async fn update_terminal_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TerminalConfigUpdate>,
) -> Result<Json<TerminalConfigResponse>> {
    let new_shell = match req.shell.as_deref() {
        Some(raw) => {
            let shell = raw.trim();
            validate_shell(shell).map_err(AppError::BadRequest)?;
            Some(shell.to_string())
        }
        None => None,
    };

    state
        .config
        .update(|config| {
            if let Some(enabled) = req.enabled {
                config.terminal.enabled = enabled;
            }
            if let Some(shell) = &new_shell {
                config.terminal.shell = shell.clone();
            }
        })
        .await?;

    // Let connected consoles update their web-terminal availability.
    state.events.mark_device_info_dirty();

    Ok(Json(response(&state)))
}
