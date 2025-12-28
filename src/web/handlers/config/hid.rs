//! HID 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::HidConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_hid_config;
use super::types::HidConfigUpdate;

/// 获取 HID 配置
pub async fn get_hid_config(State(state): State<Arc<AppState>>) -> Json<HidConfig> {
    Json(state.config.get().hid.clone())
}

/// 更新 HID 配置
pub async fn update_hid_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<HidConfigUpdate>,
) -> Result<Json<HidConfig>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_hid_config = state.config.get().hid.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.hid);
        })
        .await?;

    // 4. 获取新配置
    let new_hid_config = state.config.get().hid.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_hid_config(&state, &old_hid_config, &new_hid_config).await {
        tracing::error!("Failed to apply HID config: {}", e);
    }

    Ok(Json(new_hid_config))
}
