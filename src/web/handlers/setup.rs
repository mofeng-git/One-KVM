use super::*;

#[derive(Serialize)]
pub struct SetupStatus {
    pub initialized: bool,
    pub needs_setup: bool,
    pub platform: PlatformCapabilities,
}

pub async fn setup_status(State(state): State<Arc<AppState>>) -> Json<SetupStatus> {
    let initialized = state.config.is_initialized();
    Json(SetupStatus {
        initialized,
        needs_setup: !initialized,
        platform: PlatformCapabilities::current(),
    })
}

#[derive(Deserialize)]
pub struct SetupRequest {
    // Account settings
    pub username: String,
    pub password: String,
    // Video settings
    pub video_device: Option<String>,
    pub video_format: Option<String>,
    pub video_width: Option<u32>,
    pub video_height: Option<u32>,
    pub video_fps: Option<u32>,
    // Audio settings
    pub audio_device: Option<String>,
    // HID settings
    pub hid_backend: Option<String>,
    pub hid_ch9329_port: Option<String>,
    pub hid_ch9329_baudrate: Option<u32>,
    pub hid_otg_udc: Option<String>,
    pub hid_otg_profile: Option<String>,
    pub hid_otg_keyboard_leds: Option<bool>,
    pub msd_enabled: Option<bool>,
    // Extension settings
    pub ttyd_enabled: Option<bool>,
    pub rustdesk_enabled: Option<bool>,
}

pub async fn setup_init(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<LoginResponse>> {
    // Check if already initialized
    if state.config.is_initialized() {
        return Err(AppError::BadRequest("Already initialized".to_string()));
    }

    // Validate username
    if req.username.len() < 2 {
        return Err(AppError::BadRequest(
            "Username must be at least 2 characters".to_string(),
        ));
    }

    // Validate password
    if req.password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".to_string(),
        ));
    }

    // Create single system user
    state
        .users
        .create_first_user(&req.username, &req.password)
        .await?;

    // Update config
    state
        .config
        .update(|config| {
            config.initialized = true;

            // Video settings
            if let Some(device) = req.video_device.clone() {
                config.video.device = Some(device);
            }
            if let Some(format) = req.video_format.clone() {
                config.video.format = Some(format);
            }
            if let Some(width) = req.video_width {
                config.video.width = width;
            }
            if let Some(height) = req.video_height {
                config.video.height = height;
            }
            if let Some(fps) = req.video_fps {
                config.video.fps = fps;
            }

            // Audio settings
            if let Some(device) = req.audio_device.clone() {
                config.audio.device = device;
                config.audio.enabled = true;
            }

            // HID settings
            if let Some(backend) = req.hid_backend.clone() {
                config.hid.backend = match backend.as_str() {
                    "otg" => crate::config::HidBackend::Otg,
                    "ch9329" => crate::config::HidBackend::Ch9329,
                    _ => crate::config::HidBackend::None,
                };
            }
            if let Some(port) = req.hid_ch9329_port.clone() {
                config.hid.ch9329_port = port;
            }
            if let Some(baudrate) = req.hid_ch9329_baudrate {
                config.hid.ch9329_baudrate = baudrate;
            }
            if let Some(udc) = req.hid_otg_udc.clone() {
                config.hid.otg_udc = Some(udc);
            }
            if let Some(profile) = req.hid_otg_profile.clone() {
                if let Some(parsed) = crate::config::OtgHidProfile::from_legacy_str(&profile) {
                    config.hid.otg_profile = parsed;
                }
            }
            if let Some(enabled) = req.hid_otg_keyboard_leds {
                config.hid.otg_keyboard_leds = enabled;
            }
            if let Some(enabled) = req.msd_enabled {
                config.msd.enabled = enabled;
            }
            config.enforce_invariants();

            // Extension settings
            if let Some(enabled) = req.ttyd_enabled {
                config.extensions.ttyd.enabled = enabled;
            }
            if let Some(enabled) = req.rustdesk_enabled {
                config.rustdesk.enabled = enabled;
            }
        })
        .await?;

    // Get updated config for HID reload
    let new_config = state.config.get();

    #[cfg(unix)]
    {
        if let Err(e) = state
            .otg_service
            .apply_config(&new_config.hid, &new_config.msd, &new_config.otg_network)
            .await
        {
            tracing::warn!("Failed to apply OTG config during setup: {}", e);
        }
    }

    tracing::info!(
        "Extension config after save: ttyd.enabled={}, rustdesk.enabled={}",
        new_config.extensions.ttyd.enabled,
        new_config.rustdesk.enabled
    );

    // Initialize HID backend with new config
    let new_hid_backend = match new_config.hid.backend {
        crate::config::HidBackend::Otg => crate::hid::HidBackendType::Otg,
        crate::config::HidBackend::Ch9329 => crate::hid::HidBackendType::Ch9329 {
            port: new_config.hid.ch9329_port.clone(),
            baud_rate: new_config.hid.ch9329_baudrate,
            hybrid_mouse: new_config.hid.ch9329_hybrid_mouse,
        },
        crate::config::HidBackend::None => crate::hid::HidBackendType::None,
    };

    // Reload HID backend
    if let Err(e) = state.hid.reload(new_hid_backend).await {
        tracing::warn!("Failed to initialize HID backend during setup: {}", e);
        // Don't fail setup, just warn
    } else {
        tracing::info!("HID backend initialized: {:?}", new_config.hid.backend);
    }

    // Start extensions if enabled
    if new_config.extensions.ttyd.enabled {
        if let Err(e) = state
            .extensions
            .start(crate::extensions::ExtensionId::Ttyd, &new_config.extensions)
            .await
        {
            tracing::warn!("Failed to start ttyd during setup: {}", e);
        } else {
            tracing::info!("ttyd started during setup");
        }
    }

    // Start RustDesk if enabled
    if new_config.rustdesk.enabled {
        let empty_config = crate::rustdesk::config::RustDeskConfig::default();
        if let Err(e) = config::apply::apply_rustdesk_config(
            &state,
            &empty_config,
            &new_config.rustdesk,
            ConfigApplyOptions::default(),
        )
        .await
        {
            tracing::warn!("Failed to start RustDesk during setup: {}", e);
        } else {
            tracing::info!("RustDesk started during setup");
        }
    }

    // Start RTSP if enabled
    if new_config.rtsp.enabled {
        let empty_config = crate::config::RtspConfig::default();
        if let Err(e) = config::apply::apply_rtsp_config(
            &state,
            &empty_config,
            &new_config.rtsp,
            ConfigApplyOptions::default(),
        )
        .await
        {
            tracing::warn!("Failed to start RTSP during setup: {}", e);
        } else {
            tracing::info!("RTSP started during setup");
        }
    }

    // Start audio streaming if audio device was selected during setup
    if new_config.audio.enabled {
        let audio_config = crate::audio::AudioControllerConfig {
            enabled: true,
            device: new_config.audio.device.clone(),
            quality: new_config
                .audio
                .quality
                .parse::<crate::audio::AudioQuality>()?,
        };
        if let Err(e) = state.audio.update_config(audio_config).await {
            tracing::warn!("Failed to start audio during setup: {}", e);
        } else {
            tracing::info!(
                "Audio started during setup: device={}",
                new_config.audio.device
            );
        }
        // Also enable WebRTC audio
        if let Err(e) = state.stream_manager.set_webrtc_audio_enabled(true).await {
            tracing::warn!("Failed to enable WebRTC audio during setup: {}", e);
        }
    }

    tracing::info!("System initialized successfully");

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Setup completed".to_string()),
    }))
}
