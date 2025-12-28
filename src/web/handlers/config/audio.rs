//! Audio 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AudioConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_audio_config;
use super::types::AudioConfigUpdate;

/// 获取 Audio 配置
pub async fn get_audio_config(State(state): State<Arc<AppState>>) -> Json<AudioConfig> {
    Json(state.config.get().audio.clone())
}

/// 更新 Audio 配置
pub async fn update_audio_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AudioConfigUpdate>,
) -> Result<Json<AudioConfig>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_audio_config = state.config.get().audio.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.audio);
        })
        .await?;

    // 4. 获取新配置
    let new_audio_config = state.config.get().audio.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_audio_config(&state, &old_audio_config, &new_audio_config).await {
        tracing::error!("Failed to apply audio config: {}", e);
    }

    Ok(Json(new_audio_config))
}
