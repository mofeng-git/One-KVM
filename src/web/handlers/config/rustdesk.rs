use axum::{extract::State, Json};

use crate::error::Result;
use crate::rustdesk::config::RustDeskConfig;
use crate::web::state::RemoteAccessApiState;

use super::types::RustDeskConfigUpdate;
use crate::runtime::{try_apply_lock, ConfigApplyOptions};

fn validate_candidate(state: &RemoteAccessApiState, config: &RustDeskConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rustdesk = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &RemoteAccessApiState,
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
    state
        .coordinator
        .apply_rustdesk(
            &old_config,
            &stored_config,
            ConfigApplyOptions::preserving_service_state(),
        )
        .await?;
    Ok(stored_config)
}

async fn current_status(
    state: &RemoteAccessApiState,
    config: RustDeskConfig,
) -> RustDeskStatusResponse {
    let runtime = state.coordinator.rustdesk_status().await;

    RustDeskStatusResponse {
        config: RustDeskConfigResponse::from(&config),
        service_status: runtime.service_status,
        rendezvous_status: runtime.rendezvous_status,
        connection_count: runtime.connection_count,
        listening: runtime.listening,
        listen_port: runtime.listen_port,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RustDeskConfigResponse {
    pub enabled: bool,
    pub mode: crate::rustdesk::config::RustDeskMode,
    pub codec: crate::rustdesk::config::RustDeskCodec,
    pub direct_access_port: u16,
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
            mode: config.mode,
            codec: config.codec,
            direct_access_port: config.direct_access_port,
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
    pub connection_count: usize,
    pub listening: bool,
    pub listen_port: Option<u16>,
}

pub async fn get_rustdesk_config(
    State(state): State<RemoteAccessApiState>,
) -> Json<RustDeskConfigResponse> {
    Json(RustDeskConfigResponse::from(&state.config.get().rustdesk))
}

pub async fn get_rustdesk_status(
    State(state): State<RemoteAccessApiState>,
) -> Json<RustDeskStatusResponse> {
    let config = state.config.get().rustdesk.clone();
    Json(current_status(&state, config).await)
}

pub async fn update_rustdesk_config(
    State(state): State<RemoteAccessApiState>,
    Json(req): Json<RustDeskConfigUpdate>,
) -> Result<Json<RustDeskConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.rustdesk_apply_lock, "rustdesk")?;
    let old_config = state.config.get().rustdesk.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    req.validate_merged(&merged_config)?;

    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

pub async fn regenerate_device_id(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RustDeskConfigResponse>> {
    let _apply_guard = try_apply_lock(&state.rustdesk_apply_lock, "rustdesk")?;
    let old_config = state.config.get().rustdesk.clone();
    let mut regenerated = old_config.clone();
    regenerated.device_id = RustDeskConfig::generate_device_id();
    let new_config = persist_and_apply(&state, old_config, regenerated).await?;
    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

pub async fn regenerate_device_password(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RustDeskConfigResponse>> {
    let _apply_guard = try_apply_lock(&state.rustdesk_apply_lock, "rustdesk")?;
    let old_config = state.config.get().rustdesk.clone();
    let mut regenerated = old_config.clone();
    regenerated.device_password = RustDeskConfig::generate_password();
    let new_config = persist_and_apply(&state, old_config, regenerated).await?;
    Ok(Json(RustDeskConfigResponse::from(&new_config)))
}

pub async fn get_device_password(
    State(state): State<RemoteAccessApiState>,
) -> Json<serde_json::Value> {
    let config = state.config.get().rustdesk.clone();
    Json(serde_json::json!({
        "device_id": config.device_id,
        "device_password": config.device_password
    }))
}

pub async fn start_rustdesk_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RustDeskStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.rustdesk_apply_lock, "rustdesk")?;
    let stored_config = state.config.get().rustdesk.clone();
    let runtime_config = state.coordinator.runtime_config().await.rustdesk;
    let mut start_config = stored_config.clone();
    start_config.enabled = true;
    state
        .coordinator
        .apply_rustdesk(
            &runtime_config,
            &start_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;
    let stored_config = state.config.get().rustdesk.clone();
    Ok(Json(current_status(&state, stored_config).await))
}

pub async fn stop_rustdesk_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RustDeskStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.rustdesk_apply_lock, "rustdesk")?;
    let stored_config = state.config.get().rustdesk.clone();
    let runtime_config = state.coordinator.runtime_config().await.rustdesk;
    let mut stop_config = stored_config.clone();
    stop_config.enabled = false;
    state
        .coordinator
        .apply_rustdesk(
            &runtime_config,
            &stop_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;
    Ok(Json(current_status(&state, stored_config).await))
}
