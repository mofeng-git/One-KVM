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
    pub hid_backend: Option<crate::config::HidBackend>,
    pub hid_bluetooth: Option<crate::config::BluetoothHidConfig>,
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

impl SetupRequest {
    fn hid_config(&self, current: &crate::config::HidConfig) -> Result<crate::config::HidConfig> {
        let mut hid = current.clone();
        if let Some(backend) = &self.hid_backend {
            hid.backend = backend.clone();
        }
        if let Some(bluetooth) = &self.hid_bluetooth {
            hid.bluetooth = bluetooth.clone();
        }
        if let Some(port) = &self.hid_ch9329_port {
            hid.ch9329_port = port.clone();
        }
        if let Some(baudrate) = self.hid_ch9329_baudrate {
            hid.ch9329_baudrate = baudrate;
        }
        if let Some(udc) = &self.hid_otg_udc {
            hid.otg_udc = Some(udc.clone());
        }
        if let Some(profile) = &self.hid_otg_profile {
            if let Some(parsed) = crate::config::OtgHidProfile::from_legacy_str(profile) {
                hid.otg_profile = parsed;
            }
        }
        if let Some(enabled) = self.hid_otg_keyboard_leds {
            hid.otg_keyboard_leds = enabled;
        }
        hid.bluetooth.validate()?;
        hid.validate_otg_functions()?;
        Ok(hid)
    }
}

pub async fn setup_init(
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<SetupRequest>,
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

    if let Some(path) = req.video_device.as_deref() {
        let source_following = state
            .stream_manager
            .list_devices()
            .await
            .ok()
            .and_then(|devices| {
                devices
                    .into_iter()
                    .find(|device| device.path.to_string_lossy() == path)
            })
            .is_some_and(|device| {
                device.control_mode == crate::video::device::VideoControlMode::SourceFollowing
            });
        if source_following {
            if req.video_format.is_some()
                || req.video_width.is_some()
                || req.video_height.is_some()
                || req.video_fps.is_some()
            {
                tracing::debug!(
                    "Ignoring setup-supplied format, resolution, and FPS for source-following video input"
                );
            }
            req.video_format = None;
            req.video_width = None;
            req.video_height = None;
            req.video_fps = None;
        }
    }

    let old_config = state.config.get();
    // Validate the selected HID configuration before creating the one-time account.
    let hid_config = req.hid_config(&old_config.hid)?;

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
            config.hid = hid_config.clone();
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

    // Apply the complete USB runtime configuration, including the MSD controller.
    let new_config = state.config.get();
    if let Err(e) = state.usb.apply_config(&old_config, &new_config).await {
        tracing::warn!("Failed to apply USB config during setup: {}", e);
    }

    tracing::info!(
        "Extension config after save: ttyd.enabled={}, rustdesk.enabled={}",
        new_config.extensions.ttyd.enabled,
        new_config.rustdesk.enabled
    );

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
        if let Err(e) = state
            .remote_access
            .apply_rustdesk(
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
        if let Err(e) = state
            .remote_access
            .apply_rtsp(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, HidBackend, HidConfig, OtgHidProfile};
    use serde_json::{json, Value};

    fn request(mut hid: Value) -> SetupRequest {
        hid["username"] = json!("admin");
        hid["password"] = json!("test-password");
        serde_json::from_value(hid).unwrap()
    }

    #[test]
    fn setup_applies_selected_otg_device_and_legacy_options() {
        let req = request(json!({
            "hid_backend": "otg",
            "hid_otg_udc": "fe800000.usb",
            "hid_otg_profile": "full_no_consumer_no_msd",
            "hid_otg_keyboard_leds": true
        }));
        let hid = req.hid_config(&HidConfig::default()).unwrap();
        assert_eq!(hid.backend, HidBackend::Otg);
        assert_eq!(hid.otg_udc.as_deref(), Some("fe800000.usb"));
        assert_eq!(hid.otg_profile, OtgHidProfile::FullNoConsumer);
        assert!(hid.effective_otg_keyboard_leds());
    }

    #[test]
    fn setup_applies_selected_ch9329_port_and_baudrate() {
        let req = request(json!({
            "hid_backend": "ch9329",
            "hid_ch9329_port": "/dev/ttyUSB2",
            "hid_ch9329_baudrate": 115200
        }));
        let hid = req.hid_config(&HidConfig::default()).unwrap();
        assert_eq!(hid.backend, HidBackend::Ch9329);
        assert_eq!(hid.ch9329_port, "/dev/ttyUSB2");
        assert_eq!(hid.ch9329_baudrate, 115200);
    }

    #[test]
    fn setup_applies_bluetooth_selection_and_usb_invariants() {
        let req = request(json!({
            "hid_backend": "bluetooth",
            "hid_bluetooth": { "adapter": "hci1", "name": "My KVM" }
        }));
        let mut config = AppConfig::default();
        config.hid = req.hid_config(&config.hid).unwrap();
        config.hid.mouse_absolute = true;
        config.msd.enabled = true;
        config.otg_network.enabled = true;
        config.uac.enabled = true;
        config.enforce_invariants();

        assert_eq!(config.hid.backend, HidBackend::Bluetooth);
        assert_eq!(config.hid.bluetooth.adapter, "hci1");
        assert_eq!(config.hid.bluetooth.name, "My KVM");
        assert_eq!(config.hid.bluetooth.peer, None);
        assert!(!config.hid.mouse_absolute);
        assert!(!config.msd.enabled && !config.otg_network.enabled && !config.uac.enabled);
    }

    #[test]
    fn setup_preserves_omitted_settings_and_allows_explicit_disable() {
        let current = HidConfig {
            backend: HidBackend::Ch9329,
            ch9329_port: "COM7".into(),
            ch9329_baudrate: 57600,
            ..Default::default()
        };
        assert_eq!(request(json!({})).hid_config(&current).unwrap(), current);
        let disabled = request(json!({ "hid_backend": "none" }))
            .hid_config(&current)
            .unwrap();
        assert_eq!(disabled.backend, HidBackend::None);
        assert_eq!(disabled.ch9329_port, current.ch9329_port);
        assert_eq!(disabled.ch9329_baudrate, current.ch9329_baudrate);
    }

    #[test]
    fn setup_rejects_unknown_backend_instead_of_disabling_hid() {
        assert!(serde_json::from_value::<SetupRequest>(json!({
            "username": "admin", "password": "test-password",
            "hid_backend": "unsupported"
        }))
        .is_err());
    }

    #[test]
    fn setup_rejects_invalid_bluetooth_configuration() {
        for bluetooth in [
            json!({ "adapter": "/dev/hci0", "name": "My KVM" }),
            json!({ "adapter": "hci0", "name": "" }),
            json!({ "adapter": "hci0", "name": "蓝".repeat(24) }),
        ] {
            let req = request(json!({
                "hid_backend": "bluetooth", "hid_bluetooth": bluetooth
            }));
            assert!(req.hid_config(&HidConfig::default()).is_err());
        }
    }
}
