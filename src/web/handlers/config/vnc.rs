use axum::{extract::State, Json};

use crate::error::Result;
use crate::web::state::RemoteAccessApiState;

use super::types::{VncConfigResponse, VncConfigUpdate, VncStatusResponse};
use crate::runtime::{try_apply_lock, ConfigApplyOptions};

fn validate_candidate(
    state: &RemoteAccessApiState,
    config: &crate::config::VncConfig,
) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.vnc = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &RemoteAccessApiState,
    old_config: crate::config::VncConfig,
    new_config: crate::config::VncConfig,
) -> Result<crate::config::VncConfig> {
    validate_candidate(state, &new_config)?;
    state
        .config
        .update(|config| {
            config.vnc = new_config.clone();
        })
        .await?;
    let stored_config = state.config.get().vnc.clone();
    state
        .coordinator
        .apply_vnc(
            &old_config,
            &stored_config,
            ConfigApplyOptions::preserving_service_state(),
        )
        .await?;
    Ok(stored_config)
}

async fn current_status(state: &RemoteAccessApiState) -> (crate::vnc::VncServiceStatus, usize) {
    state.coordinator.vnc_status().await
}

pub async fn get_vnc_config(State(state): State<RemoteAccessApiState>) -> Json<VncConfigResponse> {
    Json(VncConfigResponse::from(&state.config.get().vnc))
}

pub async fn get_vnc_status(State(state): State<RemoteAccessApiState>) -> Json<VncStatusResponse> {
    let config = state.config.get().vnc.clone();
    let (status, connection_count) = current_status(&state).await;

    Json(VncStatusResponse::new(&config, status, connection_count))
}

pub async fn update_vnc_config(
    State(state): State<RemoteAccessApiState>,
    Json(req): Json<VncConfigUpdate>,
) -> Result<Json<VncConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.vnc_apply_lock, "vnc")?;
    let old_config = state.config.get().vnc.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    req.validate_merged(&merged_config)?;
    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

    Ok(Json(VncConfigResponse::from(&new_config)))
}

pub async fn start_vnc_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<VncStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.vnc_apply_lock, "vnc")?;
    let stored_config = state.config.get().vnc.clone();
    let runtime_config = state.coordinator.runtime_config().await.vnc;
    let mut start_config = stored_config.clone();
    start_config.enabled = true;
    if start_config.password.as_deref().unwrap_or("").is_empty() {
        start_config.password = stored_config.password.clone();
    }
    state
        .coordinator
        .apply_vnc(
            &runtime_config,
            &start_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;
    let (status, connection_count) = current_status(&state).await;

    Ok(Json(VncStatusResponse::new(
        &stored_config,
        status,
        connection_count,
    )))
}

pub async fn stop_vnc_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<VncStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.vnc_apply_lock, "vnc")?;
    let stored_config = state.config.get().vnc.clone();
    let runtime_config = state.coordinator.runtime_config().await.vnc;
    let mut stop_config = stored_config.clone();
    stop_config.enabled = false;
    state
        .coordinator
        .apply_vnc(
            &runtime_config,
            &stop_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;

    Ok(Json(VncStatusResponse::new(
        &stored_config,
        crate::vnc::VncServiceStatus::Stopped,
        0,
    )))
}
