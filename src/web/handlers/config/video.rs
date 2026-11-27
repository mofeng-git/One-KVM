use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::config::VideoConfig;
use crate::error::Result;
use crate::runtime::{apply_video_config, try_apply_lock};
use crate::state::AppState;

use super::types::VideoConfigUpdate;

pub async fn get_video_config(State(state): State<Arc<AppState>>) -> Json<VideoConfig> {
    Json(state.config.get().video.clone())
}

pub async fn update_video_config(
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<VideoConfigUpdate>,
) -> Result<Response> {
    let _apply_guard = try_apply_lock(&state.config_apply_locks.video, "video")?;
    let _stream_guard = try_apply_lock(&state.config_apply_locks.stream, "stream")?;
    if state.stream_manager.is_switching() {
        return Err(crate::error::AppError::ServiceUnavailable(
            "Stream mode is changing".into(),
        ));
    }
    let mut candidate = state.config.get().video.clone();
    req.apply_to(&mut candidate);
    if let Some(path) = candidate.device.as_deref() {
        let resolved = crate::video::multiview::canonical_device(path);
        let source_following = state
            .stream_manager
            .list_devices()
            .await
            .unwrap_or_default()
            .into_iter()
            .find(|device| crate::video::multiview::canonical_device(&device.path) == resolved)
            .is_some_and(|device| {
                device.control_mode == crate::video::device::VideoControlMode::SourceFollowing
            });
        if source_following {
            req.ignore_source_following_parameters();
            candidate = state.config.get().video.clone();
            req.apply_to(&mut candidate);
        }
    }
    req.validate()?;
    state
        .stream_manager
        .validate_video_config(&candidate)
        .await?;
    // Only validated desired state is persisted. Runtime failures explicitly report
    // desired/applied instead of disguising a partial hardware apply as success.
    state
        .config
        .update(|config| config.video = candidate.clone())
        .await?;
    match apply_video_config(&state, &candidate).await {
        Ok(()) => Ok(Json(candidate).into_response()),
        Err(error) => {
            let mut applied = Vec::new();
            for source in state.stream_manager.sources().await {
                let status = state
                    .stream_manager
                    .source_status(&source.id, None)
                    .await
                    .ok();
                applied.push(serde_json::json!({"source_id": source.id, "status": status}));
            }
            Ok((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": error.to_string(), "desired": candidate, "applied": applied
                })),
            )
                .into_response())
        }
    }
}
