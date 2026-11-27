use super::*;

#[derive(Serialize)]
pub struct SetupStatus {
    pub initialized: bool,
    pub needs_setup: bool,
    pub platform: PlatformCapabilities,
}

#[derive(Serialize)]
pub struct SetupResponse {
    pub success: bool,
    pub message: String,
    pub initialized: bool,
    pub partial: bool,
    pub errors: Vec<String>,
    pub desired: crate::config::VideoConfig,
    pub applied: Vec<serde_json::Value>,
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
    #[serde(alias = "sources")]
    pub video_sources: Option<Vec<crate::config::VideoSourceConfig>>,
    #[serde(alias = "video_primary_source_id")]
    pub primary_source_id: Option<String>,
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
        hid.validate_platform()?;
        hid.validate_composite_selection()?;
        hid.bluetooth.validate()?;
        hid.validate_otg_functions()?;
        Ok(hid)
    }
}

pub async fn setup_init(
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>> {
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

    let _video_guard = crate::runtime::try_apply_lock(&state.config_apply_locks.video, "video")?;
    let _stream_guard = crate::runtime::try_apply_lock(&state.config_apply_locks.stream, "stream")?;
    if let Some(path) = req.video_device.as_deref() {
        let source_following = state
            .stream_manager
            .list_devices()
            .await
            .ok()
            .and_then(|devices| {
                devices.into_iter().find(|device| {
                    crate::video::multiview::canonical_device(&device.path)
                        == crate::video::multiview::canonical_device(path)
                })
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
    let mut video_config = old_config.video.clone();
    if let Some(sources) = req.video_sources.clone() {
        video_config.sources = sources;
        video_config.primary_source_id = req
            .primary_source_id
            .clone()
            .or_else(|| video_config.sources.first().map(|s| s.id.clone()));
    } else {
        // Setup's legacy fields are a primary-only compatibility update.
        let update = serde_json::json!({"device": req.video_device, "format": req.video_format,
            "width": req.video_width, "height": req.video_height, "fps": req.video_fps,
            "primary_source_id": req.primary_source_id});
        let update: super::config::VideoConfigUpdate = serde_json::from_value(update)?;
        update.validate()?;
        update.apply_to(&mut video_config);
    }
    video_config.project_primary();
    state
        .stream_manager
        .validate_video_config(&video_config)
        .await?;
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

            config.video = video_config.clone();

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
    let mut errors = Vec::new();
    if let Err(error) = state
        .stream_manager
        .configure_sources(&new_config.video, false)
        .await
    {
        tracing::warn!(%error, "Setup saved desired video configuration, but runtime apply failed");
        errors.push(format!("video: {error}"));
    }
    if let Err(e) = state.usb.apply_config(&old_config, &new_config).await {
        tracing::warn!("Failed to apply USB config during setup: {}", e);
        errors.push(format!("usb: {e}"));
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
            errors.push(format!("ttyd: {e}"));
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
            errors.push(format!("rustdesk: {e}"));
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
            errors.push(format!("rtsp: {e}"));
        } else {
            tracing::info!("RTSP started during setup");
        }
    }

    // Start audio streaming if audio device was selected during setup
    if new_config.audio.enabled {
        let audio_config = new_config
            .audio
            .quality
            .parse::<crate::audio::AudioQuality>()
            .map(|quality| crate::audio::AudioControllerConfig {
                enabled: true,
                device: new_config.audio.device.clone(),
                quality,
            });
        let audio_result = match audio_config {
            Ok(config) => state.audio.update_config(config).await,
            Err(error) => Err(error),
        };
        if let Err(e) = audio_result {
            tracing::warn!("Failed to start audio during setup: {}", e);
            errors.push(format!("audio: {e}"));
        } else {
            tracing::info!(
                "Audio started during setup: device={}",
                new_config.audio.device
            );
        }
        // Also enable WebRTC audio
        if let Err(e) = state.stream_manager.set_webrtc_audio_enabled(true).await {
            tracing::warn!("Failed to enable WebRTC audio during setup: {}", e);
            errors.push(format!("webrtc_audio: {e}"));
        }
    }

    tracing::info!("System initialized successfully");

    let mut applied = Vec::new();
    for source in state.stream_manager.sources().await {
        let status = state
            .stream_manager
            .source_status(&source.id, None)
            .await
            .ok();
        applied.push(serde_json::json!({"source_id": source.id, "status": status}));
    }
    let partial = !errors.is_empty();
    // Account creation is complete even if hardware needs repair. Returning 2xx
    // preserves the setup client's login/exit path; partial state stays explicit.
    Ok(Json(SetupResponse {
        success: true,
        message: if partial {
            "Setup completed; some runtime settings need attention"
        } else {
            "Setup completed"
        }
        .into(),
        initialized: true,
        partial,
        errors,
        desired: new_config.video.clone(),
        applied,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, HidBackend, HidConfig, OtgHidProfile};
    use serde_json::{json, Value};

    #[tokio::test]
    async fn setup_video_failure_still_attempts_usb_and_extensions_and_exits_setup() {
        use crate::auth::{SessionStore, TwoFactorService, UserStore};
        use crate::config::ConfigStore;
        use crate::hid::{HidBackendType, HidController};
        let root = tempfile::tempdir().unwrap();
        let db = crate::db::open_database_pool(root.path()).await.unwrap();
        let config = ConfigStore::new(db.clone_pool());
        config.load().await.unwrap();
        config
            .update(|c| {
                c.initialized = false;
                c.hid.backend = HidBackend::None;
                c.audio.enabled = false;
                c.rtsp.enabled = false;
                c.rustdesk.enabled = false;
            })
            .await
            .unwrap();
        let streamer = crate::video::Streamer::new();
        *streamer.test_apply_error.lock() = Some("video fixture busy".into());
        let webrtc = crate::webrtc::WebRtcStreamer::new();
        let manager =
            crate::video::VideoStreamManager::with_webrtc_streamer(streamer, webrtc.clone());
        #[cfg(unix)]
        let otg = Arc::new(crate::otg::OtgService::new());
        let hid = Arc::new(HidController::new(
            HidBackendType::None,
            #[cfg(unix)]
            Some(otg.clone()),
        ));
        let extensions = Arc::new(crate::extensions::ExtensionManager::new());
        *extensions.test_start_error.lock() = Some("extension fixture unavailable".into());
        let (shutdown, _) = tokio::sync::broadcast::channel(1);
        let state = AppState::new(
            db.clone(),
            config,
            SessionStore::new(3600),
            UserStore::new(db.clone_pool()),
            TwoFactorService::new(db.clone_pool()),
            #[cfg(unix)]
            otg,
            manager,
            webrtc,
            hid,
            #[cfg(unix)]
            None,
            None,
            Arc::new(crate::audio::AudioController::new(Default::default())),
            extensions,
            Arc::new(crate::events::EventBus::new()),
            Arc::new(crate::update::UpdateService::new()),
            shutdown,
            root.path().to_path_buf(),
        );
        *state.usb.test_apply_error.lock() = Some("usb fixture unavailable".into());
        let Json(response) = setup_init(
            State(state.clone()),
            Json(request(json!({
                "video_device": "/dev/offline-setup-test", "hid_backend": "none",
                "msd_enabled": false, "ttyd_enabled": true
            }))),
        )
        .await
        .unwrap();
        assert!(response.success && response.initialized && response.partial);
        for prefix in ["video:", "usb:", "ttyd:"] {
            assert!(
                response.errors.iter().any(|e| e.starts_with(prefix)),
                "{prefix}"
            );
        }
        assert!(state.usb.test_apply_error.lock().is_none());
        assert!(state.extensions.test_start_error.lock().is_none());
        assert_eq!(
            response.desired.device.as_deref(),
            Some("/dev/offline-setup-test")
        );
        assert_eq!(response.applied[0]["status"]["state"], "error");
        assert!(!setup_status(State(state.clone())).await.0.needs_setup);
        assert!(state
            .users
            .verify("admin", "test-password")
            .await
            .unwrap()
            .is_some());
    }

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
    #[cfg(target_os = "linux")]
    fn setup_applies_composite_selection_to_both_usb_paths() {
        let req = request(json!({
            "hid_backend": "ch9329_otg", "hid_ch9329_port": "/dev/ttyUSB2",
            "hid_ch9329_baudrate": 115200, "hid_otg_udc": "test.udc"
        }));
        let hid = req.hid_config(&HidConfig::default()).unwrap();
        assert_eq!(hid.backend, HidBackend::Ch9329Otg);
        assert_eq!(hid.ch9329_port, "/dev/ttyUSB2");
        assert_eq!(hid.ch9329_baudrate, 115200);
        assert_eq!(hid.otg_udc.as_deref(), Some("test.udc"));
        assert!(hid.constrained_otg_functions().is_empty());
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
