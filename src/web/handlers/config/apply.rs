//! 配置热重载逻辑
//!
//! 从 handlers.rs 中抽取的配置应用函数，负责将配置变更应用到各个子系统。

use std::sync::Arc;

use crate::config::*;
use crate::error::{AppError, Result};
use crate::events::SystemEvent;
use crate::state::AppState;

/// 应用 Video 配置变更
pub async fn apply_video_config(
    state: &Arc<AppState>,
    old_config: &VideoConfig,
    new_config: &VideoConfig,
) -> Result<()> {
    // 检查配置是否实际变更
    if old_config == new_config {
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

    // Step 1: 更新 WebRTC streamer 配置（停止现有 pipeline 和 sessions）
    state
        .stream_manager
        .webrtc_streamer()
        .update_video_config(resolution, format, new_config.fps)
        .await;
    tracing::info!("WebRTC streamer config updated");

    // Step 2: 应用视频配置到 streamer（重新创建 capturer）
    state
        .stream_manager
        .streamer()
        .apply_video_config(&device, format, resolution, new_config.fps)
        .await
        .map_err(|e| AppError::VideoError(format!("Failed to apply video config: {}", e)))?;
    tracing::info!("Video config applied to streamer");

    // Step 3: 重启 streamer（仅 MJPEG 模式）
    if !state.stream_manager.is_webrtc_enabled().await {
        if let Err(e) = state.stream_manager.start().await {
            tracing::error!("Failed to start streamer after config change: {}", e);
        } else {
            tracing::info!("Streamer started after config change");
        }
    }

    // 配置 WebRTC direct capture（所有模式统一配置）
    let (device_path, _resolution, _format, _fps, jpeg_quality) = state
        .stream_manager
        .streamer()
        .current_capture_config()
        .await;
    if let Some(device_path) = device_path {
        state
            .stream_manager
            .webrtc_streamer()
            .set_capture_device(device_path, jpeg_quality)
            .await;
    } else {
        tracing::warn!("No capture device configured for WebRTC");
    }

    if state.stream_manager.is_webrtc_enabled().await {
        use crate::video::encoder::VideoCodecType;
        let codec = state
            .stream_manager
            .webrtc_streamer()
            .current_video_codec()
            .await;
        let codec_str = match codec {
            VideoCodecType::H264 => "h264",
            VideoCodecType::H265 => "h265",
            VideoCodecType::VP8 => "vp8",
            VideoCodecType::VP9 => "vp9",
        }
        .to_string();
        let is_hardware = state
            .stream_manager
            .webrtc_streamer()
            .is_hardware_encoding()
            .await;
        state.events.publish(SystemEvent::WebRTCReady {
            transition_id: None,
            codec: codec_str,
            hardware: is_hardware,
        });
    }

    tracing::info!("Video config applied successfully");
    Ok(())
}

/// 应用 Stream 配置变更
pub async fn apply_stream_config(
    state: &Arc<AppState>,
    old_config: &StreamConfig,
    new_config: &StreamConfig,
) -> Result<()> {
    tracing::info!("Applying stream config changes...");

    // 更新编码器后端
    if old_config.encoder != new_config.encoder {
        let encoder_backend = new_config.encoder.to_backend();
        tracing::info!(
            "Updating encoder backend to: {:?} (from config: {:?})",
            encoder_backend,
            new_config.encoder
        );
        state
            .stream_manager
            .webrtc_streamer()
            .update_encoder_backend(encoder_backend)
            .await;
    }

    // 更新码率
    if old_config.bitrate_preset != new_config.bitrate_preset {
        state
            .stream_manager
            .webrtc_streamer()
            .set_bitrate_preset(new_config.bitrate_preset)
            .await
            .ok(); // Ignore error if no active stream
    }

    // 更新 ICE 配置 (STUN/TURN)
    let ice_changed = old_config.stun_server != new_config.stun_server
        || old_config.turn_server != new_config.turn_server
        || old_config.turn_username != new_config.turn_username
        || old_config.turn_password != new_config.turn_password;

    if ice_changed {
        tracing::info!(
            "Updating ICE config: STUN={:?}, TURN={:?}",
            new_config.stun_server,
            new_config.turn_server
        );
        state
            .stream_manager
            .webrtc_streamer()
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

/// 应用 HID 配置变更
pub async fn apply_hid_config(
    state: &Arc<AppState>,
    old_config: &HidConfig,
    new_config: &HidConfig,
) -> Result<()> {
    // 检查 OTG 描述符是否变更
    let descriptor_changed = old_config.otg_descriptor != new_config.otg_descriptor;
    let old_hid_functions = old_config.effective_otg_functions();
    let mut new_hid_functions = new_config.effective_otg_functions();

    // Low-endpoint UDCs (e.g., musb) cannot handle consumer control endpoints reliably
    if new_config.backend == HidBackend::Otg {
        if let Some(udc) =
            crate::otg::configfs::resolve_udc_name(new_config.otg_udc.as_deref())
        {
            if crate::otg::configfs::is_low_endpoint_udc(&udc) && new_hid_functions.consumer {
                tracing::warn!(
                    "UDC {} has low endpoint resources, disabling consumer control",
                    udc
                );
                new_hid_functions.consumer = false;
            }
        }
    }

    let hid_functions_changed = old_hid_functions != new_hid_functions;

    if new_config.backend == HidBackend::Otg && new_hid_functions.is_empty() {
        return Err(AppError::BadRequest(
            "OTG HID functions cannot be empty".to_string(),
        ));
    }

    // 如果描述符变更且当前使用 OTG 后端，需要重建 Gadget
    if descriptor_changed && new_config.backend == HidBackend::Otg {
        tracing::info!("OTG descriptor changed, updating gadget...");
        if let Err(e) = state
            .otg_service
            .update_descriptor(&new_config.otg_descriptor)
            .await
        {
            tracing::error!("Failed to update OTG descriptor: {}", e);
            return Err(AppError::Config(format!(
                "OTG descriptor update failed: {}",
                e
            )));
        }
        tracing::info!("OTG descriptor updated successfully");
    }

    // 检查是否需要重载 HID 后端
    if old_config.backend == new_config.backend
        && old_config.ch9329_port == new_config.ch9329_port
        && old_config.ch9329_baudrate == new_config.ch9329_baudrate
        && old_config.otg_udc == new_config.otg_udc
        && !descriptor_changed
        && !hid_functions_changed
    {
        tracing::info!("HID config unchanged, skipping reload");
        return Ok(());
    }

    tracing::info!("Applying HID config changes...");

    if new_config.backend == HidBackend::Otg
        && (hid_functions_changed || old_config.backend != HidBackend::Otg)
    {
        state
            .otg_service
            .update_hid_functions(new_hid_functions.clone())
            .await
            .map_err(|e| AppError::Config(format!("OTG HID function update failed: {}", e)))?;
    }

    let new_hid_backend = match new_config.backend {
        HidBackend::Otg => crate::hid::HidBackendType::Otg,
        HidBackend::Ch9329 => crate::hid::HidBackendType::Ch9329 {
            port: new_config.ch9329_port.clone(),
            baud_rate: new_config.ch9329_baudrate,
        },
        HidBackend::None => crate::hid::HidBackendType::None,
    };

    state
        .hid
        .reload(new_hid_backend)
        .await
        .map_err(|e| AppError::Config(format!("HID reload failed: {}", e)))?;

    tracing::info!(
        "HID backend reloaded successfully: {:?}",
        new_config.backend
    );

    Ok(())
}

/// 应用 MSD 配置变更
pub async fn apply_msd_config(
    state: &Arc<AppState>,
    old_config: &MsdConfig,
    new_config: &MsdConfig,
) -> Result<()> {
    tracing::info!("MSD config sent, checking if reload needed...");
    tracing::debug!("Old MSD config: {:?}", old_config);
    tracing::debug!("New MSD config: {:?}", new_config);

    // Check if MSD enabled state changed
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

    // Ensure MSD directories exist (msd/images, msd/ventoy)
    let msd_dir = new_config.msd_dir_path();
    if let Err(e) = std::fs::create_dir_all(msd_dir.join("images")) {
        tracing::warn!("Failed to create MSD images directory: {}", e);
    }
    if let Err(e) = std::fs::create_dir_all(msd_dir.join("ventoy")) {
        tracing::warn!("Failed to create MSD ventoy directory: {}", e);
    }

    let needs_reload = old_msd_enabled != new_msd_enabled || msd_dir_changed;
    if !needs_reload {
        tracing::info!(
            "MSD enabled state unchanged ({}) and directory unchanged, no reload needed",
            new_msd_enabled
        );
        return Ok(());
    }

    if new_msd_enabled {
        tracing::info!("(Re)initializing MSD...");

        // Shutdown existing controller if present
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

        // Set event bus
        let events = state.events.clone();
        msd.set_event_bus(events).await;

        // Store the initialized controller
        *state.msd.write().await = Some(msd);
        tracing::info!("MSD initialized successfully");
    } else {
        // MSD disabled - shutdown
        tracing::info!("MSD disabled in config, shutting down...");

        let mut msd_guard = state.msd.write().await;
        if let Some(msd) = msd_guard.as_mut() {
            if let Err(e) = msd.shutdown().await {
                tracing::warn!("MSD shutdown failed: {}", e);
            }
        }
        *msd_guard = None;
        tracing::info!("MSD shutdown complete");
    }

    Ok(())
}

/// 应用 ATX 配置变更
pub async fn apply_atx_config(
    state: &Arc<AppState>,
    _old_config: &AtxConfig,
    new_config: &AtxConfig,
) -> Result<()> {
    tracing::info!("Applying ATX config changes...");

    // Convert AtxConfig to AtxControllerConfig
    let controller_config = new_config.to_controller_config();

    // Reload the ATX controller with new configuration
    let atx_guard = state.atx.read().await;
    if let Some(atx) = atx_guard.as_ref() {
        if let Err(e) = atx.reload(controller_config).await {
            tracing::error!("ATX reload failed: {}", e);
            return Err(AppError::Config(format!("ATX reload failed: {}", e)));
        }
        tracing::info!("ATX controller reloaded successfully");
    } else {
        // ATX controller not initialized, create a new one if enabled
        drop(atx_guard);

        if new_config.enabled {
            tracing::info!("ATX enabled in config, initializing...");

            let atx = crate::atx::AtxController::new(controller_config);
            if let Err(e) = atx.init().await {
                tracing::warn!("ATX initialization failed: {}", e);
            } else {
                *state.atx.write().await = Some(atx);
                tracing::info!("ATX controller initialized successfully");
            }
        }
    }

    Ok(())
}

/// 应用 Audio 配置变更
pub async fn apply_audio_config(
    state: &Arc<AppState>,
    _old_config: &AudioConfig,
    new_config: &AudioConfig,
) -> Result<()> {
    tracing::info!("Applying audio config changes...");

    // Create audio controller config from new config
    let audio_config = crate::audio::AudioControllerConfig {
        enabled: new_config.enabled,
        device: new_config.device.clone(),
        quality: crate::audio::AudioQuality::from_str(&new_config.quality),
    };

    // Update audio controller
    if let Err(e) = state.audio.update_config(audio_config).await {
        tracing::error!("Audio config update failed: {}", e);
        // Don't fail - audio errors are not critical
    } else {
        tracing::info!(
            "Audio config applied: enabled={}, device={}",
            new_config.enabled,
            new_config.device
        );
    }

    // Also update WebRTC audio enabled state
    if let Err(e) = state
        .stream_manager
        .set_webrtc_audio_enabled(new_config.enabled)
        .await
    {
        tracing::warn!("Failed to update WebRTC audio state: {}", e);
    } else {
        tracing::info!("WebRTC audio enabled: {}", new_config.enabled);
    }

    // Reconnect audio sources for existing WebRTC sessions
    if new_config.enabled {
        state.stream_manager.reconnect_webrtc_audio_sources().await;
    }

    Ok(())
}

/// 应用 RustDesk 配置变更
pub async fn apply_rustdesk_config(
    state: &Arc<AppState>,
    old_config: &crate::rustdesk::config::RustDeskConfig,
    new_config: &crate::rustdesk::config::RustDeskConfig,
) -> Result<()> {
    tracing::info!("Applying RustDesk config changes...");

    let mut rustdesk_guard = state.rustdesk.write().await;

    // Check if service needs to be stopped
    if old_config.enabled && !new_config.enabled {
        // Disable service
        if let Some(ref service) = *rustdesk_guard {
            if let Err(e) = service.stop().await {
                tracing::error!("Failed to stop RustDesk service: {}", e);
            }
            tracing::info!("RustDesk service stopped");
        }
        *rustdesk_guard = None;
        return Ok(());
    }

    // Check if service needs to be started or restarted
    if new_config.enabled {
        let need_restart = old_config.rendezvous_server != new_config.rendezvous_server
            || old_config.device_id != new_config.device_id
            || old_config.device_password != new_config.device_password;

        let mut credentials_to_save = None;

        if rustdesk_guard.is_none() {
            // Create new service
            tracing::info!("Initializing RustDesk service...");
            let service = crate::rustdesk::RustDeskService::new(
                new_config.clone(),
                state.stream_manager.clone(),
                state.hid.clone(),
                state.audio.clone(),
            );
            if let Err(e) = service.start().await {
                tracing::error!("Failed to start RustDesk service: {}", e);
            } else {
                tracing::info!("RustDesk service started with ID: {}", new_config.device_id);
                // Save generated keypair and UUID to config
                credentials_to_save = service.save_credentials();
            }
            *rustdesk_guard = Some(std::sync::Arc::new(service));
        } else if need_restart {
            // Restart existing service with new config
            if let Some(ref service) = *rustdesk_guard {
                if let Err(e) = service.restart(new_config.clone()).await {
                    tracing::error!("Failed to restart RustDesk service: {}", e);
                } else {
                    tracing::info!(
                        "RustDesk service restarted with ID: {}",
                        new_config.device_id
                    );
                    // Save generated keypair and UUID to config
                    credentials_to_save = service.save_credentials();
                }
            }
        }

        // Save credentials to persistent config store (outside the lock)
        drop(rustdesk_guard);
        if let Some(updated_config) = credentials_to_save {
            tracing::info!("Saving RustDesk credentials to config store...");
            if let Err(e) = state
                .config
                .update(|cfg| {
                    cfg.rustdesk.public_key = updated_config.public_key.clone();
                    cfg.rustdesk.private_key = updated_config.private_key.clone();
                    cfg.rustdesk.signing_public_key = updated_config.signing_public_key.clone();
                    cfg.rustdesk.signing_private_key = updated_config.signing_private_key.clone();
                    cfg.rustdesk.uuid = updated_config.uuid.clone();
                })
                .await
            {
                tracing::warn!("Failed to save RustDesk credentials: {}", e);
            } else {
                tracing::info!("RustDesk credentials saved successfully");
            }
        }
    }

    Ok(())
}
