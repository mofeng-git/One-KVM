use std::sync::Arc;

use crate::config::*;
use crate::error::{AppError, Result};
use crate::rtsp::RtspService;
use crate::state::AppState;
use crate::stream_encoder::encoder_type_to_backend;
use crate::video::codec_constraints::{
    enforce_constraints_with_stream_manager, validate_third_party_codec_compatibility,
    StreamCodecConstraints,
};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigApplyOptions {
    pub force: bool,
    pub preserve_service_state: bool,
    pub runtime_only: bool,
}

impl ConfigApplyOptions {
    pub const fn forced() -> Self {
        Self {
            force: true,
            preserve_service_state: false,
            runtime_only: false,
        }
    }

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

fn hid_backend_type(config: &HidConfig) -> crate::hid::HidBackendType {
    match config.backend {
        HidBackend::Otg => crate::hid::HidBackendType::Otg,
        HidBackend::Ch9329 => crate::hid::HidBackendType::Ch9329 {
            port: config.ch9329_port.clone(),
            baud_rate: config.ch9329_baudrate,
            hybrid_mouse: config.ch9329_hybrid_mouse,
        },
        HidBackend::None => crate::hid::HidBackendType::None,
    }
}

fn hid_otg_config_changed(old_config: &HidConfig, new_config: &HidConfig) -> bool {
    old_config.backend == HidBackend::Otg
        || new_config.backend == HidBackend::Otg
        || old_config.otg_udc != new_config.otg_udc
        || old_config.otg_descriptor != new_config.otg_descriptor
        || old_config.constrained_otg_functions() != new_config.constrained_otg_functions()
        || old_config.effective_otg_keyboard_leds() != new_config.effective_otg_keyboard_leds()
}

async fn reconcile_otg_config(
    state: &Arc<AppState>,
    hid: &HidConfig,
    msd: &MsdConfig,
    network: &OtgNetworkConfig,
) -> Result<()> {
    #[cfg(not(unix))]
    {
        let _ = (state, hid, msd, network);
        Ok(())
    }
    #[cfg(unix)]
    {
        state
            .otg_service
            .apply_config(hid, msd, network)
            .await
            .map_err(|e| AppError::Config(format!("OTG reconcile failed: {}", e)))
    }
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
    msd_config: &MsdConfig,
    network_config: &OtgNetworkConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    new_config.validate_otg_functions()?;

    let descriptor_changed = old_config.otg_descriptor != new_config.otg_descriptor;
    let old_hid_functions = old_config.constrained_otg_functions();
    let new_hid_functions = new_config.constrained_otg_functions();
    let hid_functions_changed = old_hid_functions != new_hid_functions;
    let keyboard_leds_changed =
        old_config.effective_otg_keyboard_leds() != new_config.effective_otg_keyboard_leds();
    let ch9329_runtime_changed = old_config.ch9329_hybrid_mouse != new_config.ch9329_hybrid_mouse;

    if old_config.backend == new_config.backend
        && old_config.ch9329_port == new_config.ch9329_port
        && old_config.ch9329_baudrate == new_config.ch9329_baudrate
        && !ch9329_runtime_changed
        && old_config.otg_udc == new_config.otg_udc
        && !descriptor_changed
        && !hid_functions_changed
        && !keyboard_leds_changed
        && !options.force
    {
        tracing::info!("HID config unchanged, skipping reload");
        return Ok(());
    }

    tracing::info!("Applying HID config changes...");

    let new_hid_backend = hid_backend_type(new_config);
    let transitioning_away_from_otg =
        old_config.backend == HidBackend::Otg && new_config.backend != HidBackend::Otg;
    let otg_config_changed = hid_otg_config_changed(old_config, new_config);

    if transitioning_away_from_otg {
        state
            .hid
            .reload(new_hid_backend.clone())
            .await
            .map_err(|e| AppError::Config(format!("HID reload failed: {}", e)))?;
    }

    if otg_config_changed {
        reconcile_otg_config(state, new_config, msd_config, network_config).await?;
    }

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

#[cfg(unix)]
pub async fn apply_msd_config(
    state: &Arc<AppState>,
    old_config: &MsdConfig,
    new_config: &MsdConfig,
    hid_config: &HidConfig,
    network_config: &OtgNetworkConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    let hid_backend_is_otg = hid_config.backend == HidBackend::Otg;
    let effective_new_msd_enabled = new_config.enabled && hid_backend_is_otg;

    tracing::info!("MSD config sent, checking if reload needed...");
    tracing::debug!("Old MSD config: {:?}", old_config);
    tracing::debug!("New MSD config: {:?}", new_config);

    let old_msd_enabled = old_config.enabled;
    let new_msd_enabled = effective_new_msd_enabled;
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

        reconcile_otg_config(state, hid_config, new_config, network_config).await?;

        let mut msd_guard = state.msd.write().await;
        if let Some(msd) = msd_guard.as_mut() {
            msd.shutdown()
                .await
                .map_err(|e| AppError::Config(format!("MSD shutdown failed: {e}")))?;
        }
        *msd_guard = None;
        drop(msd_guard);

        let msd =
            crate::msd::MsdController::new(state.otg_service.clone(), new_config.msd_dir_path());
        let ventoy_resource_dir = state.data_dir().join("ventoy");
        msd.init(&ventoy_resource_dir)
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
            msd.shutdown()
                .await
                .map_err(|e| AppError::Config(format!("MSD shutdown failed: {e}")))?;
        }
        *msd_guard = None;
        tracing::info!("MSD shutdown complete");

        reconcile_otg_config(state, hid_config, new_config, network_config).await?;
    }

    if hid_config.backend == HidBackend::Otg
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

pub async fn apply_usb_config(
    state: &Arc<AppState>,
    old_config: &AppConfig,
    new_config: &AppConfig,
) -> Result<()> {
    #[cfg(unix)]
    {
        let transitioning_away_from_otg =
            old_config.hid.backend == HidBackend::Otg && new_config.hid.backend != HidBackend::Otg;

        if transitioning_away_from_otg {
            apply_hid_config(
                state,
                &old_config.hid,
                &new_config.hid,
                &new_config.msd,
                &new_config.otg_network,
                ConfigApplyOptions::default(),
            )
            .await?;
        } else {
            reconcile_otg_config(
                state,
                &new_config.hid,
                &new_config.msd,
                &new_config.otg_network,
            )
            .await?;
            apply_hid_config(
                state,
                &old_config.hid,
                &new_config.hid,
                &new_config.msd,
                &new_config.otg_network,
                ConfigApplyOptions::default(),
            )
            .await?;
        }

        apply_msd_config(
            state,
            &old_config.msd,
            &new_config.msd,
            &new_config.hid,
            &new_config.otg_network,
            ConfigApplyOptions::default(),
        )
        .await
    }

    #[cfg(not(unix))]
    {
        apply_hid_config(
            state,
            &old_config.hid,
            &new_config.hid,
            &new_config.msd,
            &new_config.otg_network,
            ConfigApplyOptions::default(),
        )
        .await
    }
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
    let config = state.runtime_third_party_config().await;
    let constraints = StreamCodecConstraints::from_config(&config);
    state
        .stream_manager
        .set_runtime_codec_constraints(constraints.clone())
        .await;
    let enforcement =
        enforce_constraints_with_stream_manager(&state.stream_manager, &constraints).await?;
    Ok(enforcement.message)
}

async fn validate_runtime_candidate<T>(
    state: &Arc<AppState>,
    apply: impl FnOnce(&mut crate::config::AppConfig, T),
    config: T,
) -> Result<()> {
    let mut candidate = state.runtime_third_party_config().await;
    apply(&mut candidate, config);
    validate_third_party_codec_compatibility(&candidate)
}

fn validate_rustdesk_candidate(
    state: &Arc<AppState>,
    new_config: &crate::rustdesk::config::RustDeskConfig,
) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rustdesk = new_config.clone();
    validate_third_party_codec_compatibility(&candidate)
}

fn validate_vnc_candidate(state: &Arc<AppState>, new_config: &VncConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.vnc = new_config.clone();
    validate_third_party_codec_compatibility(&candidate)
}

fn validate_rtsp_candidate(state: &Arc<AppState>, new_config: &RtspConfig) -> Result<()> {
    let mut candidate = state.config.get().as_ref().clone();
    candidate.rtsp = new_config.clone();
    validate_third_party_codec_compatibility(&candidate)
}

pub async fn apply_rustdesk_config(
    state: &Arc<AppState>,
    old_config: &crate::rustdesk::config::RustDeskConfig,
    new_config: &crate::rustdesk::config::RustDeskConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    tracing::info!("Applying RustDesk config changes...");

    if options.runtime_only {
        validate_runtime_candidate(
            state,
            |candidate, config| candidate.rustdesk = config,
            new_config.clone(),
        )
        .await?;
    } else {
        validate_rustdesk_candidate(state, new_config)?;
    }

    let mut rustdesk_guard = state.rustdesk.write().await;
    let mut credentials_to_save = None;
    let need_restart = options.force
        || old_config.codec != new_config.codec
        || old_config.rendezvous_server != new_config.rendezvous_server
        || old_config.device_id != new_config.device_id
        || old_config.device_password != new_config.device_password;

    if !options.preserve_service_state && !new_config.enabled {
        if let Some(ref service) = *rustdesk_guard {
            service
                .stop()
                .await
                .map_err(|e| AppError::Config(format!("Failed to stop RustDesk service: {}", e)))?;
            tracing::info!("RustDesk service stopped");
        }
        *rustdesk_guard = None;
    }

    if !options.preserve_service_state && new_config.enabled {
        if rustdesk_guard.is_none() {
            tracing::info!("Initializing RustDesk service...");
            let service = std::sync::Arc::new(crate::rustdesk::RustDeskService::new(
                new_config.clone(),
                state.stream_manager.clone(),
                state.hid.clone(),
                state.audio.clone(),
            ));
            *rustdesk_guard = Some(service.clone());
            service.start().await.map_err(|e| {
                AppError::Config(format!("Failed to start RustDesk service: {}", e))
            })?;
            tracing::info!("RustDesk service started with ID: {}", new_config.device_id);
            credentials_to_save = service.save_credentials();
        } else {
            if let Some(ref service) = *rustdesk_guard {
                if service.is_listening() {
                    if need_restart {
                        service.restart(new_config.clone()).await.map_err(|e| {
                            AppError::Config(format!("Failed to restart RustDesk service: {}", e))
                        })?;
                        tracing::info!(
                            "RustDesk service restarted with ID: {}",
                            new_config.device_id
                        );
                    }
                } else {
                    service.update_config(new_config.clone());
                    service.start().await.map_err(|e| {
                        AppError::Config(format!("Failed to start RustDesk service: {}", e))
                    })?;
                }
                credentials_to_save = service.save_credentials();
            }
        }
    } else if options.preserve_service_state && need_restart {
        if let Some(ref service) = *rustdesk_guard {
            let mut runtime_config = new_config.clone();
            runtime_config.enabled = true;
            service.restart(runtime_config).await.map_err(|e| {
                AppError::Config(format!("Failed to restart RustDesk service: {}", e))
            })?;
            credentials_to_save = service.save_credentials();
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

pub async fn apply_vnc_config(
    state: &Arc<AppState>,
    old_config: &VncConfig,
    new_config: &VncConfig,
    options: ConfigApplyOptions,
) -> Result<()> {
    tracing::info!("Applying VNC config changes...");

    if options.runtime_only {
        validate_runtime_candidate(
            state,
            |candidate, config| candidate.vnc = config,
            new_config.clone(),
        )
        .await?;
    } else {
        validate_vnc_candidate(state, new_config)?;
    }

    let runtime_config = state.runtime_third_party_config().await;
    let will_run = if options.preserve_service_state {
        runtime_config.vnc.enabled
    } else {
        new_config.enabled
    };
    if will_run {
        let mut candidate = runtime_config;
        candidate.vnc = new_config.clone();
        candidate.vnc.enabled = true;
        let constraints = StreamCodecConstraints::from_config(&candidate);
        match enforce_constraints_with_stream_manager(&state.stream_manager, &constraints).await {
            Ok(result) if result.changed => {
                if let Some(message) = result.message {
                    tracing::info!("{}", message);
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(
                "Failed to enforce VNC stream constraints before start: {}",
                e
            ),
        }
    }

    let mut vnc_guard = state.vnc.write().await;
    let need_restart = options.force
        || old_config.bind != new_config.bind
        || old_config.port != new_config.port
        || old_config.encoding != new_config.encoding
        || old_config.password != new_config.password
        || old_config.allow_one_client != new_config.allow_one_client;

    if !options.preserve_service_state && !new_config.enabled {
        if let Some(ref service) = *vnc_guard {
            service.stop().await?;
        }
        *vnc_guard = None;
    }

    if !options.preserve_service_state && new_config.enabled {
        if vnc_guard.is_none() {
            let service = Arc::new(crate::vnc::VncService::new(
                new_config.clone(),
                state.stream_manager.clone(),
                state.hid.clone(),
            ));
            *vnc_guard = Some(service.clone());
            service.start().await?;
            tracing::info!("VNC service started");
        } else {
            if let Some(ref service) = *vnc_guard {
                if matches!(
                    service.status().await,
                    crate::vnc::VncServiceStatus::Running
                ) {
                    if need_restart {
                        service.restart(new_config.clone()).await?;
                        tracing::info!("VNC service restarted");
                    }
                } else {
                    service.update_config(new_config.clone()).await;
                    service.start().await?;
                }
            }
        }
    } else if options.preserve_service_state && need_restart {
        if let Some(ref service) = *vnc_guard {
            let mut runtime_config = new_config.clone();
            runtime_config.enabled = true;
            service.restart(runtime_config).await?;
        }
    }

    drop(vnc_guard);
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

    if options.runtime_only {
        validate_runtime_candidate(
            state,
            |candidate, config| candidate.rtsp = config,
            new_config.clone(),
        )
        .await?;
    } else {
        validate_rtsp_candidate(state, new_config)?;
    }

    let mut rtsp_guard = state.rtsp.write().await;
    let need_restart = options.force
        || old_config.bind != new_config.bind
        || old_config.port != new_config.port
        || old_config.path != new_config.path
        || old_config.codec != new_config.codec
        || old_config.username != new_config.username
        || old_config.password != new_config.password
        || old_config.allow_one_client != new_config.allow_one_client;

    if !options.preserve_service_state && !new_config.enabled {
        if let Some(ref service) = *rtsp_guard {
            service
                .stop()
                .await
                .map_err(|e| AppError::Config(format!("Failed to stop RTSP service: {}", e)))?;
        }
        *rtsp_guard = None;
    }

    if !options.preserve_service_state && new_config.enabled {
        if rtsp_guard.is_none() {
            let service = Arc::new(RtspService::new(
                new_config.clone(),
                state.stream_manager.clone(),
            ));
            *rtsp_guard = Some(service.clone());
            service.start().await?;
            tracing::info!("RTSP service started");
        } else {
            if let Some(ref service) = *rtsp_guard {
                if matches!(
                    service.status().await,
                    crate::rtsp::RtspServiceStatus::Running
                ) {
                    if need_restart {
                        service.restart(new_config.clone()).await?;
                        tracing::info!("RTSP service restarted");
                    }
                } else {
                    service.update_config(new_config.clone()).await;
                    service.start().await?;
                }
            }
        }
    } else if options.preserve_service_state && need_restart {
        if let Some(ref service) = *rtsp_guard {
            let mut runtime_config = new_config.clone();
            runtime_config.enabled = true;
            service.restart(runtime_config).await?;
        }
    }

    drop(rtsp_guard);

    if let Some(message) = enforce_stream_codec_constraints(state).await? {
        tracing::info!("{}", message);
    }

    Ok(())
}
