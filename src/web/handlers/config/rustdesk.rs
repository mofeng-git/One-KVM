use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::rustdesk::config::RustDeskConfig;
use crate::state::AppState;

use super::apply::{apply_rustdesk_config, try_apply_lock, ConfigApplyOptions};
use super::types::RustDeskConfigUpdate;

fn validate_candidate(state: &Arc<AppState>, config: &RustDeskConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rustdesk = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &Arc<AppState>,
    old_config: RustDeskConfig,
    new_config: RustDeskConfig,
) -> Result<RustDeskConfig> {
    validate_candidate(state, &new_config)?;
    state
        .config
        .update(|config| {
            config.rustdesk = new_config.clone();
        })
        .await?;
    let stored_config = state.config.get().rustdesk.clone();
    apply_rustdesk_config(
        state,
        &old_config,
        &stored_config,
        ConfigApplyOptions::forced(),
    )
    .await?;
    Ok(stored_config)
}

async fn current_status(state: &Arc<AppState>, config: RustDeskConfig) -> RustDeskStatusResponse {
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

    RustDeskStatusResponse {
        config: RustDeskConfigResponse::from(&config),
        service_status,
        rendezvous_status,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RustDeskConfigResponse {
    pub enabled: bool,
    pub codec: crate::rustdesk::config::RustDeskCodec,
    pub rendezvous_server: String,
    pub relay_server: Option<String>,
    pub device_id: String,
    pub has_password: bool,
    pub has_keypair: bool,
    pub relay_key: Option<String>,
}

impl From<&RustDeskConfig> for RustDeskConfigResponse {
    fn from(config: &RustDeskConfig) -> Self {
        Self {
            enabled: config.enabled,
            codec: config.codec,
            rendezvous_server: config.rendezvous_server.clone(),
            relay_server: config.relay_server.clone(),
            device_id: config.device_id.clone(),
            has_password: !config.device_password.is_empty(),
            has_keypair: config.public_key.is_some() && config.private_key.is_some(),
            relay_key: config.relay_key.clone(),
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
    Json(current_status(&state, config).await)
}

pub async fn update_rustdesk_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RustDeskConfigUpdate>,
) -> Result<Json<RustDeskConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.rustdesk, "rustdesk")?;
    let old_config = state.config.get().rustdesk.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    req.validate_merged(&merged_config)?;

    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

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

pub async fn start_rustdesk_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RustDeskStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.rustdesk, "rustdesk")?;
    let current_config = state.config.get().rustdesk.clone();
    let mut start_config = current_config.clone();
    start_config.enabled = true;
    let stored_config = persist_and_apply(&state, current_config, start_config).await?;
    Ok(Json(current_status(&state, stored_config).await))
}

pub async fn stop_rustdesk_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RustDeskStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.rustdesk, "rustdesk")?;
    let current_config = state.config.get().rustdesk.clone();
    let mut stop_config = current_config.clone();
    stop_config.enabled = false;

    let stored_config = persist_and_apply(&state, current_config, stop_config).await?;
    Ok(Json(current_status(&state, stored_config).await))
}
