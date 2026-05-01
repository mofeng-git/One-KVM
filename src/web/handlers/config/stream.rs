use axum::{extract::State, Json};
use std::sync::Arc;

use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_stream_config;
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

    let old_stream_config = state.config.get().stream.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.stream);
        })
        .await?;

    let new_stream_config = state.config.get().stream.clone();

    if let Err(e) = apply_stream_config(&state, &old_stream_config, &new_stream_config).await {
        tracing::error!("Failed to apply stream config: {}", e);
    }

    if let Err(e) = super::apply::enforce_stream_codec_constraints(&state).await {
        tracing::error!("Failed to enforce stream codec constraints: {}", e);
    }

    Ok(Json(StreamConfigResponse::from(&new_stream_config)))
}
