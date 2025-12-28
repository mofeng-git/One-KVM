//! 配置热重载逻辑
//!
//! 从 handlers.rs 中抽取的配置应用函数，负责将配置变更应用到各个子系统。

use std::sync::Arc;

use crate::config::*;
use crate::error::{AppError, Result};
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
            serde_json::from_value::<crate::video::format::PixelFormat>(
                serde_json::Value::String(f.clone()),
            )
            .ok()
        })
        .unwrap_or(crate::video::format::PixelFormat::Mjpeg);

    let resolution =
        crate::video::format::Resolution::new(new_config.width, new_config.height);

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

    // Step 3: 重启 streamer
    if let Err(e) = state.stream_manager.start().await {
        tracing::error!("Failed to start streamer after config change: {}", e);
    } else {
        tracing::info!("Streamer started after config change");
    }

    // Step 4: 更新 WebRTC frame source
    if let Some(frame_tx) = state.stream_manager.frame_sender().await {
        let receiver_count = frame_tx.receiver_count();
        state
            .stream_manager
            .webrtc_streamer()
            .set_video_source(frame_tx)
            .await;
        tracing::info!(
            "WebRTC streamer frame source updated (receiver_count={})",
            receiver_count
        );
    } else {
        tracing::warn!("No frame source available after config change");
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
    if old_config.bitrate_kbps != new_config.bitrate_kbps {
        state
            .stream_manager
            .webrtc_streamer()
            .set_bitrate(new_config.bitrate_kbps)
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
        "Stream config applied: encoder={:?}, bitrate={} kbps",
        new_config.encoder,
        new_config.bitrate_kbps
    );
    Ok(())
}

/// 应用 HID 配置变更
pub async fn apply_hid_config(
    state: &Arc<AppState>,
    old_config: &HidConfig,
    new_config: &HidConfig,
) -> Result<()> {
    // 检查是否需要重载
    if old_config.backend == new_config.backend
        && old_config.ch9329_port == new_config.ch9329_port
        && old_config.ch9329_baudrate == new_config.ch9329_baudrate
        && old_config.otg_udc == new_config.otg_udc
    {
        tracing::info!("HID config unchanged, skipping reload");
        return Ok(());
    }

    tracing::info!("Applying HID config changes...");

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

    tracing::info!("HID backend reloaded successfully: {:?}", new_config.backend);

    // When switching to OTG backend, automatically enable MSD if not already enabled
    // OTG HID and MSD share the same USB gadget, so it makes sense to enable both
    if new_config.backend == HidBackend::Otg && old_config.backend != HidBackend::Otg {
        let msd_guard = state.msd.read().await;
        if msd_guard.is_none() {
            drop(msd_guard); // Release read lock before acquiring write lock

            tracing::info!("OTG HID enabled, automatically initializing MSD...");

            // Get MSD config from store
            let config = state.config.get();

            let msd = crate::msd::MsdController::new(
                state.otg_service.clone(),
                &config.msd.images_path,
                &config.msd.drive_path,
            );

            if let Err(e) = msd.init().await {
                tracing::warn!("Failed to auto-initialize MSD for OTG: {}", e);
            } else {
                let events = state.events.clone();
                msd.set_event_bus(events).await;
                *state.msd.write().await = Some(msd);
                tracing::info!("MSD automatically initialized for OTG mode");
            }
        }
    }

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

    tracing::info!("MSD enabled: old={}, new={}", old_msd_enabled, new_msd_enabled);

    if old_msd_enabled != new_msd_enabled {
        if new_msd_enabled {
            // MSD was disabled, now enabled - need to initialize
            tracing::info!("MSD enabled in config, initializing...");

            let msd = crate::msd::MsdController::new(
                state.otg_service.clone(),
                &new_config.images_path,
                &new_config.drive_path,
            );
            msd.init().await.map_err(|e| {
                AppError::Config(format!("MSD initialization failed: {}", e))
            })?;

            // Set event bus
            let events = state.events.clone();
            msd.set_event_bus(events).await;

            // Store the initialized controller
            *state.msd.write().await = Some(msd);
            tracing::info!("MSD initialized successfully");
        } else {
            // MSD was enabled, now disabled - shutdown
            tracing::info!("MSD disabled in config, shutting down...");

            if let Some(msd) = state.msd.write().await.as_mut() {
                if let Err(e) = msd.shutdown().await {
                    tracing::warn!("MSD shutdown failed: {}", e);
                }
            }
            *state.msd.write().await = None;
            tracing::info!("MSD shutdown complete");
        }
    } else {
        tracing::info!(
            "MSD enabled state unchanged ({}), no reload needed",
            new_msd_enabled
        );
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
