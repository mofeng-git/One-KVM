//! Stream 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_stream_config;
use super::types::{StreamConfigResponse, StreamConfigUpdate};

/// 获取 Stream 配置
pub async fn get_stream_config(State(state): State<Arc<AppState>>) -> Json<StreamConfigResponse> {
    let config = state.config.get();
    Json(StreamConfigResponse::from(&config.stream))
}

/// 更新 Stream 配置
pub async fn update_stream_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StreamConfigUpdate>,
) -> Result<Json<StreamConfigResponse>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_stream_config = state.config.get().stream.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.stream);
        })
        .await?;

    // 4. 获取新配置
    let new_stream_config = state.config.get().stream.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_stream_config(&state, &old_stream_config, &new_stream_config).await {
        tracing::error!("Failed to apply stream config: {}", e);
    }

    // 6. Enforce codec constraints after any stream config update
    if let Err(e) = super::apply::enforce_stream_codec_constraints(&state).await {
        tracing::error!("Failed to enforce stream codec constraints: {}", e);
    }

    Ok(Json(StreamConfigResponse::from(&new_stream_config)))
}
