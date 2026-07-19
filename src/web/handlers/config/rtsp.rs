use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_rtsp_config, try_apply_lock, ConfigApplyOptions};
use super::types::{RtspConfigResponse, RtspConfigUpdate, RtspStatusResponse};

fn validate_candidate(state: &Arc<AppState>, config: &crate::config::RtspConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rtsp = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &Arc<AppState>,
    old_config: crate::config::RtspConfig,
    new_config: crate::config::RtspConfig,
) -> Result<crate::config::RtspConfig> {
    validate_candidate(state, &new_config)?;
    state
        .config
        .update(|config| {
            config.rtsp = new_config.clone();
        })
        .await?;
    let stored_config = state.config.get().rtsp.clone();
    apply_rtsp_config(
        state,
        &old_config,
        &stored_config,
        ConfigApplyOptions::preserving_service_state(),
    )
    .await?;
    Ok(stored_config)
}

async fn current_status(state: &Arc<AppState>) -> crate::rtsp::RtspServiceStatus {
    let guard = state.rtsp.read().await;
    if let Some(ref service) = *guard {
        service.status().await
    } else {
        crate::rtsp::RtspServiceStatus::Stopped
    }
}

pub async fn get_rtsp_config(State(state): State<Arc<AppState>>) -> Json<RtspConfigResponse> {
    let config = state.config.get();
    Json(RtspConfigResponse::from(&config.rtsp))
}

pub async fn get_rtsp_status(State(state): State<Arc<AppState>>) -> Json<RtspStatusResponse> {
    let config = state.config.get().rtsp.clone();
    let status = current_status(&state).await;

    Json(RtspStatusResponse::new(&config, status))
}

pub async fn update_rtsp_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RtspConfigUpdate>,
) -> Result<Json<RtspConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.rtsp, "rtsp")?;
    let old_config = state.config.get().rtsp.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

    Ok(Json(RtspConfigResponse::from(&new_config)))
}

pub async fn start_rtsp_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RtspStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.rtsp, "rtsp")?;
    let stored_config = state.config.get().rtsp.clone();
    let runtime_config = state.runtime_third_party_config().await.rtsp;
    let mut start_config = stored_config.clone();
    start_config.enabled = true;
    apply_rtsp_config(
        &state,
        &runtime_config,
        &start_config,
        ConfigApplyOptions::runtime_only(),
    )
    .await?;
    let status = current_status(&state).await;

    Ok(Json(RtspStatusResponse::new(&stored_config, status)))
}

pub async fn stop_rtsp_service(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RtspStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.rtsp, "rtsp")?;
    let stored_config = state.config.get().rtsp.clone();
    let runtime_config = state.runtime_third_party_config().await.rtsp;
    let mut stop_config = stored_config.clone();
    stop_config.enabled = false;
    apply_rtsp_config(
        &state,
        &runtime_config,
        &stop_config,
        ConfigApplyOptions::runtime_only(),
    )
    .await?;
    let status = current_status(&state).await;

    Ok(Json(RtspStatusResponse::new(&stored_config, status)))
}
