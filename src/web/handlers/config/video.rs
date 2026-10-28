use axum::{extract::State, Json};
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
) -> Result<Json<VideoConfig>> {
    let selected_path = req
        .device
        .clone()
        .or_else(|| state.config.get().video.device.clone());
    if let Some(path) = selected_path {
        let source_following = state
            .stream_manager
            .list_devices()
            .await
            .ok()
            .and_then(|devices| {
                devices
                    .into_iter()
                    .find(|device| device.path.to_string_lossy() == path)
            })
            .is_some_and(|device| {
                device.control_mode == crate::video::device::VideoControlMode::SourceFollowing
            });
        if source_following {
            req.ignore_source_following_parameters();
        }
    }
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.video, "video")?;

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.video);
        })
        .await?;

    let new_video_config = state.config.get().video.clone();

    apply_video_config(&state, &new_video_config).await?;

    Ok(Json(new_video_config))
}
