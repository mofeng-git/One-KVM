use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::atx::AtxController;
use crate::switch::SwitchController;
use crate::audio::{AudioController, AudioControllerConfig, AudioQuality};
use crate::auth::{SessionStore, TwoFactorService, UserStore};
use crate::computer_use::ComputerUseManager;
use crate::config::{self, AppConfig, ConfigStore};
use crate::db::{open_database_pool, DatabasePool};
use crate::events::EventBus;
use crate::extensions::ExtensionManager;
use crate::hid::{HidBackendType, HidController};
#[cfg(unix)]
use crate::msd::MsdController;
#[cfg(unix)]
use crate::otg::OtgService;
use crate::state::{AppState, ShutdownAction};
use crate::update::UpdateService;
use crate::video::format::{PixelFormat, Resolution};
use crate::video::{Streamer, VideoStreamManager};
use crate::webrtc::{WebRtcStreamer, WebRtcStreamerConfig};

use super::supervisor::RuntimeSupervisor;

#[derive(Debug, Clone, Default)]
pub struct WebConfigOverrides {
    pub address: Option<String>,
    pub http_port: Option<u16>,
    pub https_port: Option<u16>,
    pub enable_https: bool,
    pub ssl_cert: Option<PathBuf>,
    pub ssl_key: Option<PathBuf>,
}

impl WebConfigOverrides {
    fn apply(self, config: &mut AppConfig) {
        if let Some(address) = self.address {
            config.web.bind_address = address.clone();
            config.web.bind_addresses = vec![address];
        }
        if let Some(port) = self.http_port {
            config.web.http_port = port;
        }
        if let Some(port) = self.https_port {
            config.web.https_port = port;
        }
        if self.enable_https {
            config.web.https_enabled = true;
        }
        if let Some(path) = self.ssl_cert {
            config.web.ssl_cert_path = Some(path.to_string_lossy().to_string());
        }
        if let Some(path) = self.ssl_key {
            config.web.ssl_key_path = Some(path.to_string_lossy().to_string());
        }
    }
}

pub struct RuntimeBuilder {
    data_dir: PathBuf,
    web_overrides: WebConfigOverrides,
}

impl RuntimeBuilder {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            web_overrides: WebConfigOverrides::default(),
        }
    }

    pub fn with_web_overrides(mut self, overrides: WebConfigOverrides) -> Self {
        self.web_overrides = overrides;
        self
    }

    pub async fn build(self) -> anyhow::Result<ApplicationRuntime> {
        let Self {
            data_dir,
            web_overrides,
        } = self;
        let (db, config_store, mut config) = load_runtime_config(&data_dir).await?;
        web_overrides.apply(&mut config);

        let sessions = SessionStore::new(config.auth.session_timeout_secs as i64);
        let users = UserStore::new(db.clone_pool());
        let two_factor = TwoFactorService::new(db.clone_pool());
        let (shutdown_tx, _) = broadcast::channel::<ShutdownAction>(1);

        let events = Arc::new(EventBus::new());
        tracing::info!("Event bus initialized");

        let (video_format, video_resolution) = parse_video_config(&config);
        let streamer = build_streamer(&config, &events, video_format, video_resolution).await;
        let webrtc = build_webrtc(&config, video_format, video_resolution);

        #[cfg(unix)]
        let otg_service = build_otg(&config).await;

        let hid_backend = hid_backend_type(&config);
        #[cfg(unix)]
        let hid = Arc::new(HidController::new(hid_backend, Some(otg_service.clone())));
        #[cfg(not(unix))]
        let hid = Arc::new(HidController::new(hid_backend));
        #[cfg(target_os = "linux")]
        hid.set_bond_store(config_store.hid_bonds());
        hid.set_event_bus(events.clone()).await;
        hid.set_screen_resolution(video_resolution.width, video_resolution.height)
            .await;
        if let Err(error) = hid.init().await {
            tracing::warn!("Failed to initialize HID backend: {}", error);
        }

        #[cfg(unix)]
        let msd = build_msd(&config, &data_dir, &otg_service, &events).await;
        let atx = build_atx(&config).await;
        let switch = build_switch(&config).await;
        let audio = build_audio(&config, &events).await;
        let extensions = Arc::new(ExtensionManager::new());
        tracing::info!("Extension manager initialized");

        webrtc.set_hid_controller(hid.clone()).await;
        webrtc.set_audio_controller(audio.clone()).await;
        if config.audio.enabled {
            if let Err(error) = webrtc.set_audio_enabled(true).await {
                tracing::warn!("Failed to enable WebRTC audio: {}", error);
            } else {
                tracing::debug!("WebRTC audio enabled");
            }
        }

        let stream_manager = VideoStreamManager::with_webrtc_streamer(
            streamer.clone(),
            webrtc.clone() as Arc<dyn crate::video::traits::VideoOutput>,
        );
        stream_manager.set_event_bus(events.clone()).await;
        stream_manager.set_config_store(config_store.clone()).await;
        connect_audio_recovery(&audio, &stream_manager).await;

        let initial_mode = config.stream.mode.clone();
        if let Err(error) = stream_manager.init_with_mode(initial_mode.clone()).await {
            tracing::warn!(
                "Failed to initialize stream manager with mode {:?}: {}",
                initial_mode,
                error
            );
        } else {
            tracing::info!(
                "Video stream manager initialized with mode: {:?}",
                initial_mode
            );
        }

        let computer_use = ComputerUseManager::new(config_store.clone(), hid.clone());
        let state = AppState::new(
            db,
            config_store.clone(),
            sessions,
            users,
            two_factor,
            #[cfg(unix)]
            otg_service,
            stream_manager,
            webrtc,
            hid,
            computer_use,
            #[cfg(unix)]
            msd,
            atx,
            switch,
            audio,
            extensions.clone(),
            events.clone(),
            Arc::new(UpdateService::new()),
            shutdown_tx,
            data_dir.clone(),
        );

        start_uac_playback(&state, &config).await;
        start_watchdog(&state, &config).await;
        extensions.set_event_bus(events.clone()).await;
        state.remote_access.start_configured(&config).await;

        let extension_config = config_store.get();
        extensions.start_enabled(&extension_config.extensions).await;

        state.publish_device_info().await;
        let supervisor = RuntimeSupervisor::start(state.clone(), events, extensions, config_store);

        Ok(ApplicationRuntime {
            state,
            config,
            data_dir,
            supervisor,
        })
    }
}

pub struct ApplicationRuntime {
    state: Arc<AppState>,
    config: AppConfig,
    data_dir: PathBuf,
    supervisor: RuntimeSupervisor,
}

impl ApplicationRuntime {
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn router(&self) -> axum::Router {
        crate::web::create_router(self.state.clone())
    }

    pub async fn shutdown(&mut self) {
        self.supervisor.shutdown(&self.state).await;
    }
}

async fn load_runtime_config(
    data_dir: &Path,
) -> anyhow::Result<(DatabasePool, ConfigStore, AppConfig)> {
    let db = open_database_pool(data_dir).await?;

    let config_store = ConfigStore::new(db.clone_pool());
    config_store.load().await?;
    let mut config = (*config_store.get()).clone();
    config.apply_platform_defaults();
    normalize_msd_config(data_dir, &config_store, &mut config).await?;

    Ok((db, config_store, config))
}

#[cfg(unix)]
async fn normalize_msd_config(
    data_dir: &Path,
    config_store: &ConfigStore,
    config: &mut AppConfig,
) -> anyhow::Result<()> {
    let mut msd_dir_updated = false;
    if config.msd.msd_dir.trim().is_empty() {
        config.msd.msd_dir = data_dir.join("msd").to_string_lossy().to_string();
        msd_dir_updated = true;
    } else if !PathBuf::from(&config.msd.msd_dir).is_absolute() {
        let msd_dir = data_dir.join(&config.msd.msd_dir);
        tracing::warn!(
            "MSD directory is relative, rebasing to {}",
            msd_dir.display()
        );
        config.msd.msd_dir = msd_dir.to_string_lossy().to_string();
        msd_dir_updated = true;
    }
    if msd_dir_updated {
        config_store.set(config.clone()).await?;
    }
    Ok(())
}

#[cfg(not(unix))]
async fn normalize_msd_config(
    _data_dir: &Path,
    _config_store: &ConfigStore,
    _config: &mut AppConfig,
) -> anyhow::Result<()> {
    Ok(())
}

fn parse_video_config(config: &AppConfig) -> (PixelFormat, Resolution) {
    let format = config
        .video
        .format
        .as_ref()
        .and_then(|format| format.parse::<PixelFormat>().ok())
        .unwrap_or(PixelFormat::Mjpeg);
    (
        format,
        Resolution::new(config.video.width, config.video.height),
    )
}

async fn build_streamer(
    config: &AppConfig,
    events: &Arc<EventBus>,
    format: PixelFormat,
    resolution: Resolution,
) -> Arc<Streamer> {
    tracing::debug!(
        "Parsed video config: {} @ {}x{}",
        format,
        resolution.width,
        resolution.height
    );
    let streamer = Streamer::new();
    streamer.set_event_bus(events.clone()).await;
    if let Some(device_path) = config.video.device.as_ref() {
        if let Err(error) = streamer
            .apply_video_config(device_path, format, resolution, config.video.fps)
            .await
        {
            tracing::warn!(
                "Failed to initialize video with config: {}, will auto-detect",
                error
            );
        } else {
            tracing::info!(
                "Video configured: {} @ {}x{} {}",
                device_path,
                resolution.width,
                resolution.height,
                format
            );
        }
    }
    streamer
}

fn build_webrtc(
    config: &AppConfig,
    input_format: PixelFormat,
    resolution: Resolution,
) -> Arc<WebRtcStreamer> {
    let webrtc = WebRtcStreamer::with_config(WebRtcStreamerConfig {
        resolution,
        input_format,
        fps: config.video.fps,
        bitrate_preset: config.stream.bitrate_preset,
        encoder_backend: crate::stream_encoder::encoder_type_to_backend(
            config.stream.encoder.clone(),
        ),
        webrtc: build_ice_config(config),
        ..Default::default()
    });
    tracing::info!("WebRTC streamer created");
    webrtc
}

fn build_ice_config(config: &AppConfig) -> crate::webrtc::config::WebRtcConfig {
    let mut stun_servers = Vec::new();
    let mut turn_servers = Vec::new();
    let has_custom_stun = config
        .stream
        .stun_server
        .as_ref()
        .is_some_and(|server| !server.is_empty());
    let has_custom_turn = config
        .stream
        .turn_server
        .as_ref()
        .is_some_and(|server| !server.is_empty());

    if !has_custom_stun && !has_custom_turn {
        let stun = crate::webrtc::config::public_ice::stun_server().to_string();
        tracing::info!("Using public STUN server: {}", stun);
        stun_servers.push(stun);
    } else {
        if let Some(stun) = config
            .stream
            .stun_server
            .as_ref()
            .filter(|server| !server.is_empty())
        {
            tracing::info!("Using custom STUN server: {}", stun);
            stun_servers.push(stun.clone());
        }
        if let Some(turn) = config
            .stream
            .turn_server
            .as_ref()
            .filter(|server| !server.is_empty())
        {
            let username = config.stream.turn_username.clone().unwrap_or_default();
            let credential = config.stream.turn_password.clone().unwrap_or_default();
            turn_servers.push(crate::webrtc::config::TurnServer::new(
                turn.clone(),
                username.clone(),
                credential,
            ));
            tracing::info!("Using custom TURN server: {} (user: {})", turn, username);
        }
    }

    crate::webrtc::config::WebRtcConfig {
        stun_servers,
        turn_servers,
        ..Default::default()
    }
}

#[cfg(unix)]
async fn build_otg(config: &AppConfig) -> Arc<OtgService> {
    let service = Arc::new(OtgService::new());
    tracing::info!("OTG Service created");
    if let Err(error) = service
        .apply_config(&config.hid, &config.msd, &config.otg_network, &config.uac)
        .await
    {
        tracing::warn!("Failed to apply OTG config: {}", error);
    }
    service
}

fn hid_backend_type(config: &AppConfig) -> HidBackendType {
    match config.hid.backend {
        config::HidBackend::Otg => HidBackendType::Otg {
            macos_drag: config.hid.mouse_macos_drag,
        },
        config::HidBackend::Ch9329 => HidBackendType::Ch9329 {
            port: config.hid.ch9329_port.clone(),
            baud_rate: config.hid.ch9329_baudrate,
            hybrid_mouse: config.hid.ch9329_hybrid_mouse,
            macos_drag: config.hid.mouse_macos_drag,
        },
        config::HidBackend::None => HidBackendType::None,
        config::HidBackend::Bluetooth => HidBackendType::Bluetooth {
            config: config.hid.bluetooth.clone(),
        },
    }
}

#[cfg(unix)]
async fn build_msd(
    config: &AppConfig,
    data_dir: &Path,
    otg: &Arc<OtgService>,
    events: &Arc<EventBus>,
) -> Option<MsdController> {
    if !config.msd.enabled {
        tracing::info!("MSD disabled in configuration");
        return None;
    }

    let controller = MsdController::new(otg.clone(), config.msd.msd_dir_path());
    if let Err(error) = controller.init(&data_dir.join("ventoy")).await {
        tracing::warn!("Failed to initialize MSD controller: {}", error);
        return None;
    }
    controller.set_event_bus(events.clone()).await;
    Some(controller)
}

async fn build_atx(config: &AppConfig) -> Option<AtxController> {
    if !config.atx.enabled {
        tracing::info!("ATX disabled in configuration");
        return None;
    }

    let controller = AtxController::new(config.atx.to_controller_config());
    if let Err(error) = controller.init().await {
        tracing::warn!("Failed to initialize ATX controller: {}", error);
        return None;
    }
    Some(controller)
}

async fn build_switch(config: &AppConfig) -> Option<SwitchController> {
    if !config.switch.enabled {
        tracing::info!("KVM switch disabled in configuration");
        return None;
    }

    let controller = SwitchController::new(config.switch.to_controller_config());
    if let Err(error) = controller.init().await {
        tracing::warn!("Failed to initialize KVM switch controller: {}", error);
        return None;
    }
    Some(controller)
}

async fn build_audio(config: &AppConfig, events: &Arc<EventBus>) -> Arc<AudioController> {
    let quality = config
        .audio
        .quality
        .parse::<AudioQuality>()
        .unwrap_or_else(|error| {
            tracing::warn!(
                "Invalid audio quality in config (value={:?}): {}, using balanced",
                config.audio.quality,
                error
            );
            AudioQuality::Balanced
        });
    let controller = Arc::new(AudioController::new(AudioControllerConfig {
        enabled: config.audio.enabled,
        device: config.audio.device.clone(),
        quality,
    }));
    controller.set_event_bus(events.clone()).await;

    if config.audio.enabled {
        tracing::info!(
            "Audio enabled: {}, quality={}",
            config.audio.device,
            config.audio.quality
        );
        if let Err(error) = controller.start_streaming().await {
            tracing::warn!("Failed to start audio streaming: {}", error);
        }
    } else {
        tracing::info!("Audio disabled in configuration");
    }
    controller
}

async fn connect_audio_recovery(
    audio: &Arc<AudioController>,
    stream_manager: &Arc<VideoStreamManager>,
) {
    let stream_manager = Arc::downgrade(stream_manager);
    audio
        .set_recovered_callback(Arc::new(move || {
            if let Some(stream_manager) = stream_manager.upgrade() {
                tokio::spawn(async move {
                    stream_manager.reconnect_webrtc_audio_sources().await;
                });
            }
        }))
        .await;
}

#[cfg(unix)]
async fn start_uac_playback(state: &Arc<AppState>, config: &AppConfig) {
    if !config.uac.enabled {
        return;
    }
    let playback_config = crate::audio::uac::UacPlaybackConfig {
        sample_rate: config.uac.sample_rate,
        channels: config.uac.channels as u16,
        ..Default::default()
    };
    match crate::audio::uac::UacPlayback::start(playback_config) {
        Ok(writer) => {
            *state.uac_playback.write().await = Some(writer);
            tracing::info!("UAC playback writer started");
        }
        Err(error) => tracing::warn!("Failed to start UAC playback writer: {}", error),
    }
}

#[cfg(not(unix))]
async fn start_uac_playback(_state: &Arc<AppState>, _config: &AppConfig) {}

async fn start_watchdog(state: &Arc<AppState>, config: &AppConfig) {
    if !config.watchdog.enabled {
        return;
    }
    if let Err(error) = state.watchdog.enable().await {
        tracing::error!(
            "Configured hardware watchdog failed to start; web service will continue: {}",
            error
        );
    } else {
        tracing::info!("Hardware watchdog started");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_overrides_only_replace_explicit_values() {
        let mut config = AppConfig::default();
        let original_https_port = config.web.https_port;

        WebConfigOverrides {
            address: Some("127.0.0.1".to_string()),
            http_port: Some(9000),
            enable_https: true,
            ..Default::default()
        }
        .apply(&mut config);

        assert_eq!(config.web.bind_address, "127.0.0.1");
        assert_eq!(config.web.bind_addresses, ["127.0.0.1"]);
        assert_eq!(config.web.http_port, 9000);
        assert_eq!(config.web.https_port, original_https_port);
        assert!(config.web.https_enabled);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn normalizing_disabled_msd_does_not_create_module_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let msd_dir = temp_dir.path().join("disabled-msd");
        let db = open_database_pool(&data_dir).await.unwrap();
        let config_store = ConfigStore::new(db.clone_pool());
        config_store.load().await.unwrap();
        let mut config = (*config_store.get()).clone();
        config.msd.enabled = false;
        config.msd.msd_dir = msd_dir.to_string_lossy().into_owned();

        normalize_msd_config(&data_dir, &config_store, &mut config)
            .await
            .unwrap();

        assert!(!msd_dir.join("images").exists());
        assert!(!msd_dir.join("ventoy").exists());
    }
}
