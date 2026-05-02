use std::sync::Arc;

use crate::config::*;
use crate::error::{AppError, Result};
use crate::rtsp::RtspService;
use crate::state::AppState;
use crate::stream_encoder::encoder_type_to_backend;
use crate::video::codec_constraints::{
    enforce_constraints_with_stream_manager, StreamCodecConstraints,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigApplyOptions {
    pub force: bool,
}

impl ConfigApplyOptions {
    pub const fn forced() -> Self {
        Self { force: true }
    }
}

pub fn try_apply_lock(lock: &Arc<Mutex<()>>, domain: &str) -> Result<OwnedMutexGuard<()>> {
    lock.clone().try_lock_owned().map_err(|_| {
        AppError::ServiceUnavailable(format!("{domain} configuration is already applying"))
    })
}

fn hid_backend_type(config: &HidConfig) -> crate::hid::HidBackendType {
    match config.backend {
        HidBackend::Otg => crate::hid::HidBackendType::Otg,
        HidBackend::Ch9329 => crate::hid::HidBackendType::Ch9329 {
            port: config.ch9329_port.clone(),
            baud_rate: config.ch9329_baudrate,
        },
        HidBackend::None => crate::hid::HidBackendType::None,
    }
}

async fn reconcile_otg_from_store(state: &Arc<AppState>) -> Result<()> {
    let config = state.config.get();
    state
        .otg_service
        .apply_config(&config.hid, &config.msd)
        .await
        .map_err(|e| AppError::Config(format!("OTG reconcile failed: {}", e)))
}

pub async fn apply_video_config(
    state: &Arc<AppState>,
    old_config: &VideoConfig,
    new_config: &VideoConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    if old_config == new_config && !options.force {
        tracing::info!("Video config unchanged, skipping reload");
        return Ok(());
    }

    tracing::info!("Applying video config changes...");

    let device = new_config
        .device
        .clone()
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
        .apply_video_config(&device, format, resolution, new_config.fps)
        .await
        .map_err(|e| AppError::VideoError(format!("Failed to apply video config: {}", e)))?;

    tracing::info!("Video config applied successfully");
    Ok(())
}

pub async fn apply_stream_config(
    state: &Arc<AppState>,
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

pub async fn apply_hid_config(
    state: &Arc<AppState>,
    old_config: &HidConfig,
    new_config: &HidConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    let current_msd_enabled = state.config.get().msd.enabled;
    new_config.validate_otg_endpoint_budget(current_msd_enabled)?;

    let descriptor_changed = old_config.otg_descriptor != new_config.otg_descriptor;
    let old_hid_functions = old_config.constrained_otg_functions();
    let new_hid_functions = new_config.constrained_otg_functions();
    let hid_functions_changed = old_hid_functions != new_hid_functions;
    let keyboard_leds_changed =
        old_config.effective_otg_keyboard_leds() != new_config.effective_otg_keyboard_leds();
    let endpoint_budget_changed =
        old_config.resolved_otg_endpoint_limit() != new_config.resolved_otg_endpoint_limit();

    if old_config.backend == new_config.backend
        && old_config.ch9329_port == new_config.ch9329_port
        && old_config.ch9329_baudrate == new_config.ch9329_baudrate
        && old_config.otg_udc == new_config.otg_udc
        && !descriptor_changed
        && !hid_functions_changed
        && !keyboard_leds_changed
        && !endpoint_budget_changed
        && !options.force
    {
        tracing::info!("HID config unchanged, skipping reload");
        return Ok(());
    }

    tracing::info!("Applying HID config changes...");

    let new_hid_backend = hid_backend_type(new_config);
    let transitioning_away_from_otg =
        old_config.backend == HidBackend::Otg && new_config.backend != HidBackend::Otg;

    if transitioning_away_from_otg {
        state
            .hid
            .reload(new_hid_backend.clone())
            .await
            .map_err(|e| AppError::Config(format!("HID reload failed: {}", e)))?;
    }

    reconcile_otg_from_store(state).await?;

    if !transitioning_away_from_otg {
        state
            .hid
            .reload(new_hid_backend)
            .await
            .map_err(|e| AppError::Config(format!("HID reload failed: {}", e)))?;
    }

    tracing::info!(
        "HID backend reloaded successfully: {:?}",
        new_config.backend
    );

    Ok(())
}

pub async fn apply_msd_config(
    state: &Arc<AppState>,
    old_config: &MsdConfig,
    new_config: &MsdConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    state
        .config
        .get()
        .hid
        .validate_otg_endpoint_budget(new_config.enabled)?;

    tracing::info!("MSD config sent, checking if reload needed...");
    tracing::debug!("Old MSD config: {:?}", old_config);
    tracing::debug!("New MSD config: {:?}", new_config);

    let old_msd_enabled = old_config.enabled;
    let new_msd_enabled = new_config.enabled;
    let msd_dir_changed = old_config.msd_dir != new_config.msd_dir;

    tracing::info!(
        "MSD enabled: old={}, new={}",
        old_msd_enabled,
        new_msd_enabled
    );
    if msd_dir_changed {
        tracing::info!("MSD directory changed: {}", new_config.msd_dir);
    }

    let msd_dir = new_config.msd_dir_path();
    if let Err(e) = std::fs::create_dir_all(msd_dir.join("images")) {
        tracing::warn!("Failed to create MSD images directory: {}", e);
    }
    if let Err(e) = std::fs::create_dir_all(msd_dir.join("ventoy")) {
        tracing::warn!("Failed to create MSD ventoy directory: {}", e);
    }

    let needs_reload = options.force || old_msd_enabled != new_msd_enabled || msd_dir_changed;
    if !needs_reload {
        tracing::info!(
            "MSD enabled state unchanged ({}) and directory unchanged, no reload needed",
            new_msd_enabled
        );
        return Ok(());
    }

    if new_msd_enabled {
        tracing::info!("(Re)initializing MSD...");

        reconcile_otg_from_store(state).await?;

        let mut msd_guard = state.msd.write().await;
        if let Some(msd) = msd_guard.as_mut() {
            if let Err(e) = msd.shutdown().await {
                tracing::warn!("MSD shutdown failed: {}", e);
            }
        }
        *msd_guard = None;
        drop(msd_guard);

        let msd =
            crate::msd::MsdController::new(state.otg_service.clone(), new_config.msd_dir_path());
        msd.init()
            .await
            .map_err(|e| AppError::Config(format!("MSD initialization failed: {}", e)))?;

        let events = state.events.clone();
        msd.set_event_bus(events).await;

        *state.msd.write().await = Some(msd);
        tracing::info!("MSD initialized successfully");
    } else {
        tracing::info!("MSD disabled in config, shutting down...");

        let mut msd_guard = state.msd.write().await;
        if let Some(msd) = msd_guard.as_mut() {
            if let Err(e) = msd.shutdown().await {
                tracing::warn!("MSD shutdown failed: {}", e);
            }
        }
        *msd_guard = None;
        tracing::info!("MSD shutdown complete");

        reconcile_otg_from_store(state).await?;
    }

    let current_config = state.config.get();
    if current_config.hid.backend == HidBackend::Otg
        && (options.force || old_msd_enabled != new_msd_enabled)
    {
        state
            .hid
            .reload(crate::hid::HidBackendType::Otg)
            .await
            .map_err(|e| AppError::Config(format!("OTG HID reload failed: {}", e)))?;
    }

    Ok(())
}

pub async fn apply_atx_config(
    state: &Arc<AppState>,
    _old_config: &AtxConfig,
    new_config: &AtxConfig,
) -> Result<()> {
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

pub async fn apply_audio_config(
    state: &Arc<AppState>,
    _old_config: &AudioConfig,
    new_config: &AudioConfig,
) -> Result<()> {
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

pub async fn enforce_stream_codec_constraints(state: &Arc<AppState>) -> Result<Option<String>> {
    let config = state.config.get();
    let constraints = StreamCodecConstraints::from_config(&config);
    let enforcement =
        enforce_constraints_with_stream_manager(&state.stream_manager, &constraints).await?;
    Ok(enforcement.message)
}

pub async fn apply_rustdesk_config(
    state: &Arc<AppState>,
    old_config: &crate::rustdesk::config::RustDeskConfig,
    new_config: &crate::rustdesk::config::RustDeskConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    tracing::info!("Applying RustDesk config changes...");

    let mut rustdesk_guard = state.rustdesk.write().await;
    let mut credentials_to_save = None;

    if old_config.enabled && !new_config.enabled {
        if let Some(ref service) = *rustdesk_guard {
            service
                .stop()
                .await
                .map_err(|e| AppError::Config(format!("Failed to stop RustDesk service: {}", e)))?;
            tracing::info!("RustDesk service stopped");
        }
        *rustdesk_guard = None;
    }

    if new_config.enabled {
        let need_restart = options.force
            || old_config.rendezvous_server != new_config.rendezvous_server
            || old_config.device_id != new_config.device_id
            || old_config.device_password != new_config.device_password;

        if rustdesk_guard.is_none() {
            tracing::info!("Initializing RustDesk service...");
            let service = crate::rustdesk::RustDeskService::new(
                new_config.clone(),
                state.stream_manager.clone(),
                state.hid.clone(),
                state.audio.clone(),
            );
            service.start().await.map_err(|e| {
                AppError::Config(format!("Failed to start RustDesk service: {}", e))
            })?;
            tracing::info!("RustDesk service started with ID: {}", new_config.device_id);
            credentials_to_save = service.save_credentials();
            *rustdesk_guard = Some(std::sync::Arc::new(service));
        } else if need_restart {
            if let Some(ref service) = *rustdesk_guard {
                service.restart(new_config.clone()).await.map_err(|e| {
                    AppError::Config(format!("Failed to restart RustDesk service: {}", e))
                })?;
                tracing::info!(
                    "RustDesk service restarted with ID: {}",
                    new_config.device_id
                );
                credentials_to_save = service.save_credentials();
            }
        }
    }

    drop(rustdesk_guard);
    if let Some(updated_config) = credentials_to_save {
        tracing::info!("Saving RustDesk credentials to config store...");
        state
            .config
            .update(|cfg| {
                cfg.rustdesk.public_key = updated_config.public_key.clone();
                cfg.rustdesk.private_key = updated_config.private_key.clone();
                cfg.rustdesk.signing_public_key = updated_config.signing_public_key.clone();
                cfg.rustdesk.signing_private_key = updated_config.signing_private_key.clone();
                cfg.rustdesk.uuid = updated_config.uuid.clone();
            })
            .await?;
        tracing::info!("RustDesk credentials saved successfully");
    }

    if let Some(message) = enforce_stream_codec_constraints(state).await? {
        tracing::info!("{}", message);
    }

    Ok(())
}

pub async fn apply_rtsp_config(
    state: &Arc<AppState>,
    old_config: &RtspConfig,
    new_config: &RtspConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    tracing::info!("Applying RTSP config changes...");

    let mut rtsp_guard = state.rtsp.write().await;

    if old_config.enabled && !new_config.enabled {
        if let Some(ref service) = *rtsp_guard {
            service
                .stop()
                .await
                .map_err(|e| AppError::Config(format!("Failed to stop RTSP service: {}", e)))?;
        }
        *rtsp_guard = None;
    }

    if new_config.enabled {
        let need_restart = options.force
            || old_config.bind != new_config.bind
            || old_config.port != new_config.port
            || old_config.path != new_config.path
            || old_config.codec != new_config.codec
            || old_config.username != new_config.username
            || old_config.password != new_config.password
            || old_config.allow_one_client != new_config.allow_one_client;

        if rtsp_guard.is_none() {
            let service = RtspService::new(new_config.clone(), state.stream_manager.clone());
            service.start().await?;
            tracing::info!("RTSP service started");
            *rtsp_guard = Some(Arc::new(service));
        } else if need_restart {
            if let Some(ref service) = *rtsp_guard {
                service.restart(new_config.clone()).await?;
                tracing::info!("RTSP service restarted");
            }
        }
    }

    drop(rtsp_guard);

    if let Some(message) = enforce_stream_codec_constraints(state).await? {
        tracing::info!("{}", message);
    }

    Ok(())
}
