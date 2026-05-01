use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AudioConfig;
use crate::error::Result;
use crate::state::AppState;

use super::apply::apply_audio_config;
use super::types::AudioConfigUpdate;

pub async fn get_audio_config(State(state): State<Arc<AppState>>) -> Json<AudioConfig> {
    Json(state.config.get().audio.clone())
}

pub async fn update_audio_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AudioConfigUpdate>,
) -> Result<Json<AudioConfig>> {
    req.validate()?;

    let old_audio_config = state.config.get().audio.clone();

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.audio);
        })
        .await?;

    let new_audio_config = state.config.get().audio.clone();

    if let Err(e) = apply_audio_config(&state, &old_audio_config, &new_audio_config).await {
        tracing::error!("Failed to apply audio config: {}", e);
    }

    Ok(Json(new_audio_config))
}
