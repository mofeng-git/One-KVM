use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_vnc_config, try_apply_lock, ConfigApplyOptions};
use super::types::{VncConfigResponse, VncConfigUpdate, VncStatusResponse};

fn validate_candidate(state: &Arc<AppState>, config: &crate::config::VncConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.vnc = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &Arc<AppState>,
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
    apply_vnc_config(
        state,
        &old_config,
        &stored_config,
        ConfigApplyOptions::forced(),
    )
    .await?;
    Ok(stored_config)
}

async fn current_status(state: &Arc<AppState>) -> (crate::vnc::VncServiceStatus, usize) {
    let guard = state.vnc.read().await;
    if let Some(ref service) = *guard {
        (service.status().await, service.connection_count())
    } else {
        (crate::vnc::VncServiceStatus::Stopped, 0)
    }
}

pub async fn get_vnc_config(State(state): State<Arc<AppState>>) -> Json<VncConfigResponse> {
    Json(VncConfigResponse::from(&state.config.get().vnc))
}

pub async fn get_vnc_status(State(state): State<Arc<AppState>>) -> Json<VncStatusResponse> {
    let config = state.config.get().vnc.clone();
    let (status, connection_count) = current_status(&state).await;

    Json(VncStatusResponse::new(&config, status, connection_count))
}

pub async fn update_vnc_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VncConfigUpdate>,
) -> Result<Json<VncConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.vnc, "vnc")?;
    let old_config = state.config.get().vnc.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    req.validate_merged(&merged_config)?;
    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

    Ok(Json(VncConfigResponse::from(&new_config)))
}

pub async fn start_vnc_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<VncStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.vnc, "vnc")?;
    let current_config = state.config.get().vnc.clone();
    let mut start_config = current_config.clone();
    start_config.enabled = true;
    if start_config.password.as_deref().unwrap_or("").is_empty() {
        start_config.password = current_config.password.clone();
    }
    let stored_config = persist_and_apply(&state, current_config, start_config).await?;
    let (status, connection_count) = current_status(&state).await;

    Ok(Json(VncStatusResponse::new(
        &stored_config,
        status,
        connection_count,
    )))
}

pub async fn stop_vnc_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<VncStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.vnc, "vnc")?;
    let current_config = state.config.get().vnc.clone();
    let mut stop_config = current_config.clone();
    stop_config.enabled = false;

    let stored_config = persist_and_apply(&state, current_config, stop_config).await?;

    Ok(Json(VncStatusResponse::new(
        &stored_config,
        crate::vnc::VncServiceStatus::Stopped,
        0,
    )))
}
