//! MSD 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::MsdConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_msd_config;
use super::types::MsdConfigUpdate;

/// 获取 MSD 配置
pub async fn get_msd_config(State(state): State<Arc<AppState>>) -> Json<MsdConfig> {
    Json(state.config.get().msd.clone())
}

/// 更新 MSD 配置
pub async fn update_msd_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MsdConfigUpdate>,
) -> Result<Json<MsdConfig>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_msd_config = state.config.get().msd.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.msd);
        })
        .await?;

    // 4. 获取新配置
    let new_msd_config = state.config.get().msd.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_msd_config(&state, &old_msd_config, &new_msd_config).await {
        tracing::error!("Failed to apply MSD config: {}", e);
    }

    Ok(Json(new_msd_config))
}
