//! Tailscale 设置与状态接口。

use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;
use typeshare::typeshare;

use crate::error::{AppError, Result};
use crate::extensions::TailscaleConfig;
use crate::state::AppState;

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct TailscaleConfigUpdate {
    pub enabled: Option<bool>,
    /// 留空表示保留原有密钥
    pub auth_key: Option<String>,
    pub login_server: Option<String>,
    pub hostname: Option<String>,
}

/// GET /api/extensions/tailscale
pub async fn get_tailscale(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let status = crate::tailscale::status().await;
    let config = state.config.get();
    Json(serde_json::json!({
        "status": status,
        "config": config.extensions.tailscale,
    }))
}

/// PATCH /api/extensions/tailscale/config
pub async fn update_tailscale_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TailscaleConfigUpdate>,
) -> Result<Json<TailscaleConfig>> {
    let current = state.config.get();
    let mut next = current.extensions.tailscale.clone();

    if let Some(enabled) = req.enabled {
        next.enabled = enabled;
    }
    if let Some(ref auth_key) = req.auth_key {
        let trimmed = auth_key.trim();
        if !trimmed.is_empty() {
            next.auth_key = trimmed.to_string();
        }
    }
    if let Some(login_server) = req.login_server {
        next.login_server = login_server.trim().to_string();
    }
    if let Some(hostname) = req.hostname {
        next.hostname = hostname.trim().to_string();
    }

    state
        .config
        .update(|config| {
            config.extensions.tailscale = next.clone();
        })
        .await?;

    let new_config = state.config.get();
    let applied = new_config.extensions.tailscale.clone();

    if applied.enabled && !crate::tailscale::installed() {
        return Err(AppError::BadRequest(
            "系统里没有 /usr/bin/tailscale，请先在镜像里安装 tailscale".to_string(),
        ));
    }

    if let Err(error) = crate::tailscale::apply(&applied).await {
        return Err(AppError::BadRequest(error));
    }

    Ok(Json(applied))
}
