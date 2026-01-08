//! RustDesk 配置 Handler

use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::rustdesk::config::RustDeskConfig;
use crate::state::AppState;

use super::apply::apply_rustdesk_config;
use super::types::RustDeskConfigUpdate;

/// RustDesk 配置响应（隐藏敏感信息）
#[derive(Debug, serde::Serialize)]
pub struct RustDeskConfigResponse {
    pub enabled: bool,
    pub rendezvous_server: String,
    pub relay_server: Option<String>,
    pub device_id: String,
    /// 是否已设置密码
    pub has_password: bool,
    /// 是否已设置密钥对
    pub has_keypair: bool,
    /// 是否已设置 relay key
    pub has_relay_key: bool,
}

impl From<&RustDeskConfig> for RustDeskConfigResponse {
    fn from(config: &RustDeskConfig) -> Self {
        Self {
            enabled: config.enabled,
            rendezvous_server: config.rendezvous_server.clone(),
            relay_server: config.relay_server.clone(),
            device_id: config.device_id.clone(),
            has_password: !config.device_password.is_empty(),
            has_keypair: config.public_key.is_some() && config.private_key.is_some(),
            has_relay_key: config.relay_key.is_some(),
        }
    }
}

/// RustDesk 状态响应
#[derive(Debug, serde::Serialize)]
pub struct RustDeskStatusResponse {
    pub config: RustDeskConfigResponse,
    pub service_status: String,
    pub rendezvous_status: Option<String>,
}

/// 获取 RustDesk 配置
pub async fn get_rustdesk_config(State(state): State<Arc<AppState>>) -> Json<RustDeskConfigResponse> {
    Json(RustDeskConfigResponse::from(&state.config.get().rustdesk))
}

/// 获取 RustDesk 完整状态（配置 + 服务状态）
pub async fn get_rustdesk_status(State(state): State<Arc<AppState>>) -> Json<RustDeskStatusResponse> {
    let config = state.config.get().rustdesk.clone();

    // 获取服务状态
    let (service_status, rendezvous_status) = {
        let guard = state.rustdesk.read().await;
        if let Some(ref service) = *guard {
            let status = format!("{}", service.status());
            let rv_status = service.rendezvous_status().map(|s| format!("{}", s));
            (status, rv_status)
        } else {
            ("not_initialized".to_string(), None)
        }
    };

    Json(RustDeskStatusResponse {
        config: RustDeskConfigResponse::from(&config),
        service_status,
        rendezvous_status,
    })
}

/// 更新 RustDesk 配置
pub async fn update_rustdesk_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RustDeskConfigUpdate>,
) -> Result<Json<RustDeskConfigResponse>> {
    // 1. 验证请求
    req.validate()?;

    // 2. 获取旧配置
    let old_config = state.config.get().rustdesk.clone();

    // 3. 应用更新到配置存储
    state
        .config
        .update(|config| {
            req.apply_to(&mut config.rustdesk);
        })
        .await?;

    // 4. 获取新配置
    let new_config = state.config.get().rustdesk.clone();

    // 5. 应用到子系统（热重载）
    if let Err(e) = apply_rustdesk_config(&state, &old_config, &new_config).await {
        tracing::error!("Failed to apply RustDesk config: {}", e);
    }

    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

/// 重新生成设备 ID
pub async fn regenerate_device_id(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RustDeskConfigResponse>> {
    state
        .config
        .update(|config| {
            config.rustdesk.device_id = RustDeskConfig::generate_device_id();
        })
        .await?;

    let new_config = state.config.get().rustdesk.clone();
    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

/// 重新生成设备密码
pub async fn regenerate_device_password(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RustDeskConfigResponse>> {
    state
        .config
        .update(|config| {
            config.rustdesk.device_password = RustDeskConfig::generate_password();
        })
        .await?;

    let new_config = state.config.get().rustdesk.clone();
    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

/// 获取设备密码（管理员专用）
pub async fn get_device_password(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.config.get().rustdesk.clone();
    Json(serde_json::json!({
        "device_id": config.device_id,
        "device_password": config.device_password
    }))
}
