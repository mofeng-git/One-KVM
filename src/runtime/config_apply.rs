use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::config::{AtxConfig, AudioConfig, StreamConfig, VideoConfig};
use crate::error::{AppError, Result};
use crate::state::AppState;
use crate::stream_encoder::encoder_type_to_backend;

#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigApplyOptions {
    pub force: bool,
    pub preserve_service_state: bool,
    pub runtime_only: bool,
}

impl ConfigApplyOptions {
    pub const fn preserving_service_state() -> Self {
        Self {
            force: false,
            preserve_service_state: true,
            runtime_only: false,
        }
    }

    pub const fn runtime_only() -> Self {
        Self {
            force: false,
            preserve_service_state: false,
            runtime_only: true,
        }
    }
}

pub fn try_apply_lock(lock: &Arc<Mutex<()>>, domain: &str) -> Result<OwnedMutexGuard<()>> {
    lock.clone().try_lock_owned().map_err(|_| {
        AppError::ServiceUnavailable(format!("{domain} configuration is already applying"))
    })
}

pub(crate) async fn apply_video_config(state: &AppState, new_config: &VideoConfig) -> Result<()> {
    tracing::info!("Applying video config changes...");

    let device = new_config
        .device
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("video_device is required".to_string()))?;

    let format = new_config
        .format
        .as_ref()
        .and_then(|f| {
            serde_json::from_value::<crate::video::format::PixelFormat>(serde_json::Value::String(
                f.clone(),
            ))
            .ok()
        })
        .unwrap_or(crate::video::format::PixelFormat::Mjpeg);

    let resolution = crate::video::format::Resolution::new(new_config.width, new_config.height);

    state
        .stream_manager
        .apply_video_config(device, format, resolution, new_config.fps)
        .await
        .map_err(|e| AppError::VideoError(format!("Failed to apply video config: {}", e)))?;

    tracing::info!("Video config applied successfully");
    Ok(())
}

pub(crate) async fn apply_stream_config(
    state: &AppState,
    old_config: &StreamConfig,
    new_config: &StreamConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    tracing::info!("Applying stream config changes...");

    if options.force || old_config.encoder != new_config.encoder {
        let encoder_backend = encoder_type_to_backend(new_config.encoder.clone());
        tracing::info!(
            "Updating encoder backend to: {:?} (from config: {:?})",
            encoder_backend,
            new_config.encoder
        );
        state.webrtc.update_encoder_backend(encoder_backend).await;
    }

    if options.force || old_config.bitrate_preset != new_config.bitrate_preset {
        state
            .stream_manager
            .set_bitrate_preset(new_config.bitrate_preset)
            .await?;
    }

    let ice_changed = old_config.stun_server != new_config.stun_server
        || old_config.turn_server != new_config.turn_server
        || old_config.turn_username != new_config.turn_username
        || old_config.turn_password != new_config.turn_password;

    if options.force || ice_changed {
        tracing::info!(
            "Updating ICE config: STUN={:?}, TURN={:?}",
            new_config.stun_server,
            new_config.turn_server
        );
        state
            .webrtc
            .update_ice_config(
                new_config.stun_server.clone(),
                new_config.turn_server.clone(),
                new_config.turn_username.clone(),
                new_config.turn_password.clone(),
            )
            .await;
    }

    tracing::info!(
        "Stream config applied: encoder={:?}, bitrate={}",
        new_config.encoder,
        new_config.bitrate_preset
    );
    Ok(())
}

pub(crate) async fn apply_atx_config(state: &AppState, new_config: &AtxConfig) -> Result<()> {
    tracing::info!("Applying ATX config changes...");

    let controller_config = new_config.to_controller_config();

    let atx_guard = state.atx.read().await;
    if let Some(atx) = atx_guard.as_ref() {
        if let Err(e) = atx.reload(controller_config).await {
            tracing::error!("ATX reload failed: {}", e);
            return Err(AppError::Config(format!("ATX reload failed: {}", e)));
        }
        tracing::info!("ATX controller reloaded successfully");
    } else {
        drop(atx_guard);

        if new_config.enabled {
            tracing::info!("ATX enabled in config, initializing...");

            let atx = crate::atx::AtxController::new(controller_config);
            atx.init()
                .await
                .map_err(|e| AppError::Config(format!("ATX initialization failed: {}", e)))?;
            *state.atx.write().await = Some(atx);
            tracing::info!("ATX controller initialized successfully");
        }
    }

    Ok(())
}

pub(crate) async fn apply_audio_config(state: &AppState, new_config: &AudioConfig) -> Result<()> {
    tracing::info!("Applying audio config changes...");

    let audio_config = crate::audio::AudioControllerConfig {
        enabled: new_config.enabled,
        device: new_config.device.clone(),
        quality: new_config.quality.parse::<crate::audio::AudioQuality>()?,
    };

    state.audio.update_config(audio_config).await?;
    tracing::info!(
        "Audio config applied: enabled={}, device={}",
        new_config.enabled,
        new_config.device
    );

    state
        .stream_manager
        .set_webrtc_audio_enabled(new_config.enabled)
        .await?;
    tracing::debug!("WebRTC audio enabled: {}", new_config.enabled);

    if new_config.enabled {
        state.stream_manager.reconnect_webrtc_audio_sources().await;
    }

    Ok(())
}
