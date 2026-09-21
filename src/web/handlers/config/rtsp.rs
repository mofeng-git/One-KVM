use axum::{extract::State, Json};

use crate::error::Result;
use crate::web::state::RemoteAccessApiState;

use super::types::{RtspConfigResponse, RtspConfigUpdate, RtspStatusResponse};
use crate::runtime::{try_apply_lock, ConfigApplyOptions};

fn validate_candidate(
    state: &RemoteAccessApiState,
    config: &crate::config::RtspConfig,
) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rtsp = config.clone();
    crate::video::codec_constraints::validate_third_party_codec_compatibility(&candidate)
}

async fn persist_and_apply(
    state: &RemoteAccessApiState,
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
    state
        .coordinator
        .apply_rtsp(
            &old_config,
            &stored_config,
            ConfigApplyOptions::preserving_service_state(),
        )
        .await?;
    Ok(stored_config)
}

async fn current_status(state: &RemoteAccessApiState) -> crate::rtsp::RtspServiceStatus {
    state.coordinator.rtsp_status().await
}

pub async fn get_rtsp_config(
    State(state): State<RemoteAccessApiState>,
) -> Json<RtspConfigResponse> {
    let config = state.config.get();
    Json(RtspConfigResponse::from(&config.rtsp))
}

pub async fn get_rtsp_status(
    State(state): State<RemoteAccessApiState>,
) -> Json<RtspStatusResponse> {
    let config = state.config.get().rtsp.clone();
    let status = current_status(&state).await;

    Json(RtspStatusResponse::new(&config, status))
}

pub async fn update_rtsp_config(
    State(state): State<RemoteAccessApiState>,
    Json(req): Json<RtspConfigUpdate>,
) -> Result<Json<RtspConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.rtsp_apply_lock, "rtsp")?;
    let old_config = state.config.get().rtsp.clone();
    let mut merged_config = old_config.clone();
    req.apply_to(&mut merged_config);
    let new_config = persist_and_apply(&state, old_config, merged_config).await?;

    Ok(Json(RtspConfigResponse::from(&new_config)))
}

pub async fn start_rtsp_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RtspStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.rtsp_apply_lock, "rtsp")?;
    let stored_config = state.config.get().rtsp.clone();
    let runtime_config = state.coordinator.runtime_config().await.rtsp;
    let mut start_config = stored_config.clone();
    start_config.enabled = true;
    state
        .coordinator
        .apply_rtsp(
            &runtime_config,
            &start_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;
    let status = current_status(&state).await;

    Ok(Json(RtspStatusResponse::new(&stored_config, status)))
}

pub async fn stop_rtsp_service(
    State(state): State<RemoteAccessApiState>,
) -> Result<Json<RtspStatusResponse>> {
    let _apply_guard = try_apply_lock(&state.rtsp_apply_lock, "rtsp")?;
    let stored_config = state.config.get().rtsp.clone();
    let runtime_config = state.coordinator.runtime_config().await.rtsp;
    let mut stop_config = stored_config.clone();
    stop_config.enabled = false;
    state
        .coordinator
        .apply_rtsp(
            &runtime_config,
            &stop_config,
            ConfigApplyOptions::runtime_only(),
        )
        .await?;
    let status = current_status(&state).await;

    Ok(Json(RtspStatusResponse::new(&stored_config, status)))
}
