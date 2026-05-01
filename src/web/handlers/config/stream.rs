use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::{apply_stream_config, try_apply_lock, ConfigApplyOptions};
use super::types::{StreamConfigResponse, StreamConfigUpdate};

pub async fn get_stream_config(State(state): State<Arc<AppState>>) -> Json<StreamConfigResponse> {
    let config = state.config.get();
    Json(StreamConfigResponse::from(&config.stream))
}

pub async fn update_stream_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StreamConfigUpdate>,
) -> Result<Json<StreamConfigResponse>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.stream, "stream")?;
    let old_stream_config = state.config.get().stream.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.stream);
        })
        .await?;

    let new_stream_config = state.config.get().stream.clone();

    apply_stream_config(
        &state,
        &old_stream_config,
        &new_stream_config,
        ConfigApplyOptions::forced(),
    )
    .await?;

    super::apply::enforce_stream_codec_constraints(&state).await?;

    Ok(Json(StreamConfigResponse::from(&new_stream_config)))
}
