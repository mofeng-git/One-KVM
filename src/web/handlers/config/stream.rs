use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::runtime::{apply_stream_config, try_apply_lock, ConfigApplyOptions};
use crate::state::AppState;

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
    let _format_guard = state.stream_manager.format_lock.lock().await;
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
        ConfigApplyOptions::default(),
    )
    .await?;

    let constraints = state.stream_manager.codec_constraints().await;
    crate::video::codec_constraints::enforce_constraints_with_stream_manager(
        &state.stream_manager,
        &constraints,
    )
    .await?;

    Ok(Json(StreamConfigResponse::from(&new_stream_config)))
}
