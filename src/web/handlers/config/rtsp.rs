use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_rtsp_config, try_apply_lock, ConfigApplyOptions};
use super::types::{RtspConfigResponse, RtspConfigUpdate, RtspStatusResponse};

pub async fn get_rtsp_config(State(state): State<Arc<AppState>>) -> Json<RtspConfigResponse> {
    let config = state.config.get();
    Json(RtspConfigResponse::from(&config.rtsp))
}

pub async fn get_rtsp_status(State(state): State<Arc<AppState>>) -> Json<RtspStatusResponse> {
    let config = state.config.get().rtsp.clone();
    let status = {
        let guard = state.rtsp.read().await;
        if let Some(ref service) = *guard {
            service.status().await
        } else {
            crate::rtsp::RtspServiceStatus::Stopped
        }
    };

    Json(RtspStatusResponse::new(&config, status))
}

pub async fn update_rtsp_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RtspConfigUpdate>,
) -> Result<Json<RtspConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.rtsp, "rtsp")?;
    let old_config = state.config.get().rtsp.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.rtsp);
        })
        .await?;

    let new_config = state.config.get().rtsp.clone();
    apply_rtsp_config(
        &state,
        &old_config,
        &new_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    Ok(Json(RtspConfigResponse::from(&new_config)))
}
