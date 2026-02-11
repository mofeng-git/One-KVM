use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::state::AppState;

use super::apply::apply_rtsp_config;
use super::types::{RtspConfigResponse, RtspConfigUpdate, RtspStatusResponse};

/// Get RTSP config
pub async fn get_rtsp_config(State(state): State<Arc<AppState>>) -> Json<RtspConfigResponse> {
    let config = state.config.get();
    Json(RtspConfigResponse::from(&config.rtsp))
}

/// Get RTSP status (config + service status)
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

/// Update RTSP config
pub async fn update_rtsp_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RtspConfigUpdate>,
) -> Result<Json<RtspConfigResponse>> {
    req.validate()?;

    let old_config = state.config.get().rtsp.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.rtsp);
        })
        .await?;

    let new_config = state.config.get().rtsp.clone();
    if let Err(err) = apply_rtsp_config(&state, &old_config, &new_config).await {
        tracing::error!("Failed to apply RTSP config: {}", err);
        if let Err(rollback_err) = state
            .config
            .update(|config| {
                config.rtsp = old_config.clone();
            })
            .await
        {
            tracing::error!("Failed to rollback RTSP config after apply failure: {}", rollback_err);
            return Err(AppError::ServiceUnavailable(format!(
                "RTSP apply failed: {}; rollback failed: {}",
                err, rollback_err
            )));
        }
        return Err(err);
    }

    Ok(Json(RtspConfigResponse::from(&new_config)))
}
