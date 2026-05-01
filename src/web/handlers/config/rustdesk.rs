use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::rustdesk::config::RustDeskConfig;
use crate::state::AppState;

use super::apply::{apply_rustdesk_config, try_apply_lock, ConfigApplyOptions};
use super::types::RustDeskConfigUpdate;

#[derive(Debug, serde::Serialize)]
pub struct RustDeskConfigResponse {
    pub enabled: bool,
    pub rendezvous_server: String,
    pub relay_server: Option<String>,
    pub device_id: String,
    pub has_password: bool,
    pub has_keypair: bool,
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

#[derive(Debug, serde::Serialize)]
pub struct RustDeskStatusResponse {
    pub config: RustDeskConfigResponse,
    pub service_status: String,
    pub rendezvous_status: Option<String>,
}

pub async fn get_rustdesk_config(
    State(state): State<Arc<AppState>>,
) -> Json<RustDeskConfigResponse> {
    Json(RustDeskConfigResponse::from(&state.config.get().rustdesk))
}

pub async fn get_rustdesk_status(
    State(state): State<Arc<AppState>>,
) -> Json<RustDeskStatusResponse> {
    let config = state.config.get().rustdesk.clone();

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

pub async fn update_rustdesk_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RustDeskConfigUpdate>,
) -> Result<Json<RustDeskConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.rustdesk, "rustdesk")?;
    let old_config = state.config.get().rustdesk.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.rustdesk);
        })
        .await?;

    let new_config = state.config.get().rustdesk.clone();

    apply_rustdesk_config(
        &state,
        &old_config,
        &new_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    let constraints = state.stream_manager.codec_constraints().await;
    if constraints.rustdesk_enabled || constraints.rtsp_enabled {
        tracing::info!(
            "Stream codec constraints active after RustDesk update: {}",
            constraints.reason
        );
    }

    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

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

pub async fn get_device_password(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.config.get().rustdesk.clone();
    Json(serde_json::json!({
        "device_id": config.device_id,
        "device_password": config.device_password
    }))
}
