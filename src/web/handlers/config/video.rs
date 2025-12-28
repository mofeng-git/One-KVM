//! Video 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::VideoConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_video_config;
use super::types::VideoConfigUpdate;

/// 获取 Video 配置
pub async fn get_video_config(State(state): State<Arc<AppState>>) -> Json<VideoConfig> {
    Json(state.config.get().video.clone())
}

/// 更新 Video 配置
pub async fn update_video_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VideoConfigUpdate>,
) -> Result<Json<VideoConfig>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_video_config = state.config.get().video.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.video);
        })
        .await?;

    // 4. 获取新配置
    let new_video_config = state.config.get().video.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_video_config(&state, &old_video_config, &new_video_config).await {
        tracing::error!("Failed to apply video config: {}", e);
        // 根据用户选择，仅记录错误，不回滚
    }

    Ok(Json(new_video_config))
}
