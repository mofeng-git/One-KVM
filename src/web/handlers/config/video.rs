use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::VideoConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_video_config, try_apply_lock, ConfigApplyOptions};
use super::types::VideoConfigUpdate;

pub async fn get_video_config(State(state): State<Arc<AppState>>) -> Json<VideoConfig> {
    Json(state.config.get().video.clone())
}

pub async fn update_video_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VideoConfigUpdate>,
) -> Result<Json<VideoConfig>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.video, "video")?;
    let old_video_config = state.config.get().video.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.video);
        })
        .await?;

    let new_video_config = state.config.get().video.clone();

    apply_video_config(
        &state,
        &old_video_config,
        &new_video_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    Ok(Json(new_video_config))
}
