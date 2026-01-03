//! Web 服务器配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::WebConfig;
use crate::error::Result;
use crate::state::AppState;

use super::types::WebConfigUpdate;

/// 获取 Web 配置
pub async fn get_web_config(State(state): State<Arc<AppState>>) -> Json<WebConfig> {
    Json(state.config.get().web.clone())
}

/// 更新 Web 配置
pub async fn update_web_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WebConfigUpdate>,
) -> Result<Json<WebConfig>> {
    req.validate()?;

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.web);
        })
        .await?;

    Ok(Json(state.config.get().web.clone()))
}
