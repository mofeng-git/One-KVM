//! ATX 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AtxConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_atx_config;
use super::types::AtxConfigUpdate;

/// 获取 ATX 配置
pub async fn get_atx_config(State(state): State<Arc<AppState>>) -> Json<AtxConfig> {
    Json(state.config.get().atx.clone())
}

/// 更新 ATX 配置
pub async fn update_atx_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AtxConfigUpdate>,
) -> Result<Json<AtxConfig>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_atx_config = state.config.get().atx.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.atx);
        })
        .await?;

    // 4. 获取新配置
    let new_atx_config = state.config.get().atx.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_atx_config(&state, &old_atx_config, &new_atx_config).await {
        tracing::error!("Failed to apply ATX config: {}", e);
    }

    Ok(Json(new_atx_config))
}
