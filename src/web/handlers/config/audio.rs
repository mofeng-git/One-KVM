use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::AudioConfig;
use crate::error::Result;
use crate::runtime::{apply_audio_config, try_apply_lock};
use crate::state::AppState;

use super::types::AudioConfigUpdate;

pub async fn get_audio_config(State(state): State<Arc<AppState>>) -> Json<AudioConfig> {
    Json(state.config.get().audio.clone())
}

pub async fn update_audio_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AudioConfigUpdate>,
) -> Result<Json<AudioConfig>> {
    req.validate()?;

    let _apply_guard = try_apply_lock(&state.config_apply_locks.audio, "audio")?;

    state
        .config
        .update(|config| {
            req.apply_to(&mut config.audio);
        })
        .await?;

    let new_audio_config = state.config.get().audio.clone();

    apply_audio_config(&state, &new_audio_config).await?;

    Ok(Json(new_audio_config))
}
