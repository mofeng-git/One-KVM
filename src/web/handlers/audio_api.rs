use super::*;

use crate::audio::{AudioQuality, AudioStatus};

/// Audio status response (re-exports AudioStatus from audio module)
pub type AudioStatusResponse = AudioStatus;

/// Get audio status
pub async fn audio_status(State(state): State<Arc<AppState>>) -> Json<AudioStatusResponse> {
    Json(state.audio.status().await)
}

/// Start audio streaming
pub async fn start_audio_streaming(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LoginResponse>> {
    state.audio.start_streaming().await?;

    // Reconnect audio sources for existing WebRTC sessions
    // This ensures sessions created before audio was enabled will receive audio
    state.stream_manager.reconnect_webrtc_audio_sources().await;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Audio streaming started".to_string()),
    }))
}

/// Stop audio streaming
pub async fn stop_audio_streaming(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LoginResponse>> {
    state.audio.stop_streaming().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Audio streaming stopped".to_string()),
    }))
}

/// Set audio quality request
#[derive(Deserialize)]
pub struct SetAudioQualityRequest {
    pub quality: String,
}

/// Set audio quality
pub async fn set_audio_quality(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetAudioQualityRequest>,
) -> Result<Json<LoginResponse>> {
    let quality = req.quality.parse::<AudioQuality>()?;
    state.audio.set_quality(quality).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Audio quality set to {}", quality)),
    }))
}

/// Select audio device request
#[derive(Deserialize)]
pub struct SelectAudioDeviceRequest {
    pub device: String,
}

/// Select audio device
pub async fn select_audio_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SelectAudioDeviceRequest>,
) -> Result<Json<LoginResponse>> {
    state.audio.select_device(&req.device).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Audio device selected: {}", req.device)),
    }))
}

/// List audio devices
pub async fn list_audio_devices(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::audio::AudioDeviceInfo>>> {
    let devices = state.audio.list_devices().await?;
    Ok(Json(devices))
}
