//! Android service runtime.
//!
//! Android is treated as a packaged Linux distribution: the APK/Java layer only
//! starts and stops this runtime, while the Rust side builds the same AppState
//! and Axum router used by the desktop service.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustls::crypto::{ring, CryptoProvider};
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::atx::AtxController;
use crate::audio::{AudioController, AudioControllerConfig, AudioQuality};
use crate::auth::{SessionStore, UserStore};
use crate::config::{self, AppConfig, ConfigStore};
use crate::db::DatabasePool;
use crate::events::EventBus;
use crate::extensions::ExtensionManager;
use crate::hid::{HidBackendType, HidController};
use crate::msd::MsdController;
use crate::otg::OtgService;
use crate::rtsp::RtspService;
use crate::rustdesk::RustDeskService;
use crate::state::{AppState, ShutdownAction};
use crate::stream_encoder::encoder_type_to_backend;
use crate::update::UpdateService;
use crate::utils::bind_tcp_listener;
use crate::video::codec_constraints::{
    enforce_constraints_with_stream_manager, validate_third_party_codec_compatibility,
    StreamCodecConstraints,
};
use crate::video::format::{PixelFormat, Resolution};
use crate::video::{Streamer, VideoStreamManager};
use crate::vnc::VncService;
use crate::web;
use crate::webrtc::{config::WebRtcConfig, WebRtcStreamer, WebRtcStreamerConfig};

#[derive(Debug, Clone)]
pub struct AndroidRuntimeConfig {
    pub data_dir: String,
    pub bind_address: String,
    pub port: u16,
}

struct RuntimeHandle {
    stop_tx: oneshot::Sender<()>,
    join: JoinHandle<()>,
}

static HANDLE: OnceLock<Mutex<Option<RuntimeHandle>>> = OnceLock::new();

fn handle_slot() -> &'static Mutex<Option<RuntimeHandle>> {
    HANDLE.get_or_init(|| Mutex::new(None))
}

pub fn start(config: AndroidRuntimeConfig) -> Result<String, String> {
    init_logging();

    let mut slot = handle_slot()
        .lock()
        .map_err(|_| "runtime lock poisoned".to_string())?;
    if slot.is_some() {
        return Ok(status());
    }

    let (stop_tx, stop_rx) = oneshot::channel();
    let config_for_thread = config.clone();
    let join = std::thread::Builder::new()
        .name("one-kvm-android-runtime".to_string())
        .spawn(move || {
            if let Err(err) = run_runtime(config_for_thread, stop_rx) {
                tracing::error!("One-KVM Android runtime exited: {}", err);
            }
        })
        .map_err(|err| format!("failed to spawn runtime: {err}"))?;

    *slot = Some(RuntimeHandle { stop_tx, join });
    Ok(format!(
        "One-KVM Android runtime starting on http://{}:{}",
        config.bind_address, config.port
    ))
}

pub fn run_foreground(config: AndroidRuntimeConfig) -> Result<(), String> {
    init_logging();
    let (_stop_tx, stop_rx) = oneshot::channel();
    run_runtime(config, stop_rx)
}

pub fn init_rustls_provider() {
    ensure_rustls_provider();
}

pub fn stop() -> String {
    let handle = match handle_slot().lock() {
        Ok(mut slot) => slot.take(),
        Err(_) => return "runtime lock poisoned".to_string(),
    };

    let Some(handle) = handle else {
        return "One-KVM Android runtime is not running".to_string();
    };

    let _ = handle.stop_tx.send(());
    match handle.join.join() {
        Ok(()) => "One-KVM Android runtime stopped".to_string(),
        Err(_) => "One-KVM Android runtime stopped after panic".to_string(),
    }
}

pub fn status() -> String {
    match handle_slot().lock() {
        Ok(slot) if slot.is_some() => "One-KVM Android runtime running".to_string(),
        Ok(_) => "One-KVM Android runtime stopped".to_string(),
        Err(_) => "runtime lock poisoned".to_string(),
    }
}

fn run_runtime(config: AndroidRuntimeConfig, stop_rx: oneshot::Receiver<()>) -> Result<(), String> {
    ensure_rustls_provider();
    let runtime = Runtime::new().map_err(|err| format!("failed to create tokio runtime: {err}"))?;
    runtime.block_on(async move { run_async(config, stop_rx).await })
}

async fn run_async(
    config: AndroidRuntimeConfig,
    stop_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let (db, config_store, app_config) =
        load_runtime_config(&PathBuf::from(&config.data_dir), &config).await?;
    let (shutdown_tx, _) = broadcast::channel::<ShutdownAction>(1);
    let state = build_app_state(
        PathBuf::from(&config.data_dir),
        db,
        config_store,
        app_config,
        shutdown_tx.clone(),
    )
    .await?;

    let app = web::create_router(state.clone());
    let listener = bind_android_listener(&config.bind_address, config.port)?;
    let local_addr = listener
        .local_addr()
        .map_err(|err| format!("failed to get listener address: {err}"))?;
    tracing::info!(
        "Starting One-KVM desktop router on Android at http://{}",
        local_addr
    );

    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|err| format!("failed to create tokio listener: {err}"))?;
    let server = axum::serve(listener, app);

    let shutdown_signal = {
        let mut shutdown_rx = shutdown_tx.subscribe();
        async move {
            tokio::select! {
                _ = stop_rx => {
                    tracing::info!("Android stop request received");
                    let _ = shutdown_tx.send(ShutdownAction::Exit);
                }
                request = shutdown_rx.recv() => {
                    match request {
                        Ok(action) => {
                            tracing::info!("Android shutdown request received: {:?}", action);
                        }
                        Err(err) => {
                            tracing::warn!("Android shutdown request channel closed: {}", err);
                        }
                    }
                }
            }
        }
    };

    tokio::select! {
        result = server => {
            if let Err(err) = result {
                tracing::error!("Android HTTP server error: {}", err);
            }
        }
        _ = shutdown_signal => {}
    }

    cleanup(&state).await;
    Ok(())
}

async fn load_runtime_config(
    data_dir: &Path,
    runtime_config: &AndroidRuntimeConfig,
) -> Result<(DatabasePool, ConfigStore, AppConfig), String> {
    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(|err| format!("failed to create data dir {}: {err}", data_dir.display()))?;

    let db_path = data_dir.join("one-kvm.db");
    let db = DatabasePool::new(&db_path)
        .await
        .map_err(|err| format!("failed to open database {}: {err}", db_path.display()))?;
    db.init_schema()
        .await
        .map_err(|err| format!("failed to initialize database schema: {err}"))?;

    let config_store = ConfigStore::new(db.clone_pool())
        .map_err(|err| format!("failed to create config store: {err}"))?;
    config_store
        .load()
        .await
        .map_err(|err| format!("failed to load config: {err}"))?;

    let mut config = (*config_store.get()).clone();
    config.apply_platform_defaults();
    config.web.bind_address = runtime_config.bind_address.clone();
    config.web.bind_addresses = vec![runtime_config.bind_address.clone()];
    config.web.http_port = runtime_config.port;
    config.web.https_enabled = false;
    prepare_android_runtime_dirs(data_dir, &config_store, &mut config).await?;

    if let Some(device) = config.video.device.as_deref() {
        if device == "auto" {
            config.video.device = None;
        }
    }

    config_store
        .set(config.clone())
        .await
        .map_err(|err| format!("failed to persist Android runtime config: {err}"))?;

    Ok((db, config_store, config))
}

async fn prepare_android_runtime_dirs(
    data_dir: &Path,
    config_store: &ConfigStore,
    config: &mut AppConfig,
) -> Result<(), String> {
    let mut updated = false;
    if config.msd.msd_dir.trim().is_empty() {
        config.msd.msd_dir = data_dir.join("msd").to_string_lossy().to_string();
        updated = true;
    } else if !PathBuf::from(&config.msd.msd_dir).is_absolute() {
        config.msd.msd_dir = data_dir
            .join(&config.msd.msd_dir)
            .to_string_lossy()
            .to_string();
        updated = true;
    }

    let msd_dir = config.msd.msd_dir_path();
    tokio::fs::create_dir_all(msd_dir.join("images"))
        .await
        .map_err(|err| format!("failed to create Android MSD images dir: {err}"))?;
    tokio::fs::create_dir_all(msd_dir.join("ventoy"))
        .await
        .map_err(|err| format!("failed to create Android MSD ventoy dir: {err}"))?;

    if updated {
        config_store
            .set(config.clone())
            .await
            .map_err(|err| format!("failed to persist Android MSD dir: {err}"))?;
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn build_app_state(
    data_dir: PathBuf,
    db: DatabasePool,
    config_store: ConfigStore,
    config: AppConfig,
    shutdown_tx: broadcast::Sender<ShutdownAction>,
) -> Result<Arc<AppState>, String> {
    let session_store = SessionStore::new(config.auth.session_timeout_secs as i64);
    let user_store = UserStore::new(db.clone_pool());
    let events = Arc::new(EventBus::new());

    let (video_format, video_resolution) = parse_video_config(&config);
    let streamer = Streamer::new();
    streamer.set_event_bus(events.clone()).await;
    if let Some(ref device_path) = config.video.device {
        if let Err(err) = streamer
            .apply_video_config(
                device_path,
                video_format,
                video_resolution,
                config.video.fps,
            )
            .await
        {
            tracing::warn!("Android video config failed, falling back to auto: {}", err);
        }
    }

    let webrtc_streamer = WebRtcStreamer::with_config(WebRtcStreamerConfig {
        resolution: video_resolution,
        input_format: video_format,
        fps: config.video.fps,
        bitrate_preset: config.stream.bitrate_preset,
        encoder_backend: encoder_type_to_backend(config.stream.encoder.clone()),
        webrtc: build_webrtc_config(&config),
        ..Default::default()
    });

    let hid_backend = match config.hid.backend {
        config::HidBackend::Otg => HidBackendType::Otg,
        config::HidBackend::Ch9329 => HidBackendType::Ch9329 {
            port: config.hid.ch9329_port.clone(),
            baud_rate: config.hid.ch9329_baudrate,
            hybrid_mouse: config.hid.ch9329_hybrid_mouse,
        },
        config::HidBackend::None => HidBackendType::None,
    };
    let otg_service = Arc::new(OtgService::new());
    if let Err(err) = otg_service.apply_config(&config.hid, &config.msd).await {
        tracing::warn!("Failed to apply Android OTG config: {}", err);
    }

    let hid = Arc::new(HidController::new(hid_backend, Some(otg_service.clone())));
    hid.set_event_bus(events.clone()).await;
    if let Err(err) = hid.init().await {
        tracing::warn!("Failed to initialize Android HID backend: {}", err);
    }

    let msd = if config.msd.enabled {
        let ventoy_resource_dir = data_dir.join("ventoy");
        if ventoy_resource_dir.exists() {
            if let Err(err) = ventoy_img::init_resources(&ventoy_resource_dir) {
                tracing::warn!("Failed to initialize Android Ventoy resources: {}", err);
            }
        }

        let controller = MsdController::new(otg_service.clone(), config.msd.msd_dir_path());
        if let Err(err) = controller.init().await {
            tracing::warn!("Failed to initialize Android MSD controller: {}", err);
            None
        } else {
            controller.set_event_bus(events.clone()).await;
            Some(controller)
        }
    } else {
        None
    };

    let atx = if config.atx.enabled {
        let controller = AtxController::new(config.atx.to_controller_config());
        if let Err(err) = controller.init().await {
            tracing::warn!("Failed to initialize Android ATX controller: {}", err);
            None
        } else {
            Some(controller)
        }
    } else {
        None
    };

    let audio = {
        let audio_config = AudioControllerConfig {
            enabled: config.audio.enabled,
            device: config.audio.device.clone(),
            quality: config
                .audio
                .quality
                .parse::<AudioQuality>()
                .unwrap_or(AudioQuality::Balanced),
        };
        let controller = AudioController::new(audio_config);
        controller.set_event_bus(events.clone()).await;
        if config.audio.enabled {
            if let Err(err) = controller.start_streaming().await {
                tracing::warn!("Failed to start Android audio: {}", err);
            }
        }
        Arc::new(controller)
    };

    let extensions = Arc::new(ExtensionManager::new());
    webrtc_streamer.set_hid_controller(hid.clone()).await;
    webrtc_streamer.set_audio_controller(audio.clone()).await;

    let (device_path, actual_resolution, actual_format, actual_fps, jpeg_quality) =
        streamer.current_capture_config().await;
    webrtc_streamer
        .update_video_config(actual_resolution, actual_format, actual_fps)
        .await;
    if let Some(device_path) = device_path {
        let (subdev_path, bridge_kind, v4l2_driver) = streamer
            .current_device()
            .await
            .map(|device| {
                (
                    device.subdev_path.clone(),
                    device.bridge_kind.clone(),
                    Some(device.driver.clone()),
                )
            })
            .unwrap_or((None, None, None));
        webrtc_streamer
            .set_capture_device(
                device_path,
                jpeg_quality,
                subdev_path,
                bridge_kind,
                v4l2_driver,
            )
            .await;
    }

    let stream_manager = VideoStreamManager::with_webrtc_streamer(
        streamer.clone(),
        webrtc_streamer.clone() as Arc<dyn crate::video::traits::VideoOutput>,
    );
    stream_manager.set_event_bus(events.clone()).await;
    stream_manager.set_config_store(config_store.clone()).await;
    {
        let stream_manager_weak = Arc::downgrade(&stream_manager);
        audio
            .set_recovered_callback(Arc::new(move || {
                if let Some(stream_manager) = stream_manager_weak.upgrade() {
                    tokio::spawn(async move {
                        stream_manager.reconnect_webrtc_audio_sources().await;
                    });
                }
            }))
            .await;
    }

    if let Err(err) = stream_manager
        .init_with_mode(config.stream.mode.clone())
        .await
    {
        tracing::warn!("Failed to initialize Android stream manager: {}", err);
    }

    let third_party_codec_config_valid = match validate_third_party_codec_compatibility(&config) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(
                    "Android third-party access codec configuration is invalid; RustDesk/VNC/RTSP will not start: {}",
                    e
                );
            false
        }
    };

    let rustdesk = if third_party_codec_config_valid && config.rustdesk.is_valid() {
        Some(Arc::new(RustDeskService::new(
            config.rustdesk.clone(),
            stream_manager.clone(),
            hid.clone(),
            audio.clone(),
        )))
    } else {
        None
    };

    let rtsp = if third_party_codec_config_valid && config.rtsp.enabled {
        Some(Arc::new(RtspService::new(
            config.rtsp.clone(),
            stream_manager.clone(),
        )))
    } else {
        None
    };
    let vnc = if third_party_codec_config_valid && config.vnc.enabled {
        Some(Arc::new(VncService::new(
            config.vnc.clone(),
            stream_manager.clone(),
            hid.clone(),
        )))
    } else {
        None
    };

    let update_service = Arc::new(UpdateService::new(data_dir.join("updates")));
    let state = AppState::new(
        db,
        config_store.clone(),
        session_store,
        user_store,
        otg_service,
        stream_manager,
        webrtc_streamer,
        hid,
        msd,
        atx,
        audio,
        rustdesk.clone(),
        vnc.clone(),
        rtsp.clone(),
        extensions.clone(),
        events.clone(),
        update_service,
        shutdown_tx,
        data_dir,
    );

    extensions.set_event_bus(events.clone()).await;

    if let Some(service) = rustdesk {
        if let Err(err) = service.start().await {
            tracing::warn!("Failed to start Android RustDesk service: {}", err);
        }
    }
    if let Some(service) = vnc {
        if let Err(err) = service.start().await {
            tracing::warn!("Failed to start Android VNC service: {}", err);
        }
    }
    if let Some(service) = rtsp {
        if let Err(err) = service.start().await {
            tracing::warn!("Failed to start Android RTSP service: {}", err);
        }
    }

    let constraints = StreamCodecConstraints::from_config(&state.config.get());
    if let Err(err) =
        enforce_constraints_with_stream_manager(&state.stream_manager, &constraints).await
    {
        tracing::warn!("Failed to enforce Android stream constraints: {}", err);
    }

    state.publish_device_info().await;
    spawn_device_info_broadcaster(state.clone(), events);

    Ok(state)
}

fn build_webrtc_config(config: &AppConfig) -> WebRtcConfig {
    let mut webrtc = WebRtcConfig::default();
    if let Some(stun) = config
        .stream
        .stun_server
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        webrtc.stun_servers.push(stun.clone());
    }
    if let Some(turn) = config
        .stream
        .turn_server
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        webrtc
            .turn_servers
            .push(crate::webrtc::config::TurnServer::new(
                turn.clone(),
                config.stream.turn_username.clone().unwrap_or_default(),
                config.stream.turn_password.clone().unwrap_or_default(),
            ));
    }
    webrtc
}

fn parse_video_config(config: &AppConfig) -> (PixelFormat, Resolution) {
    let format = config
        .video
        .format
        .as_ref()
        .and_then(|value| value.parse::<PixelFormat>().ok())
        .unwrap_or(PixelFormat::Mjpeg);
    (
        format,
        Resolution::new(config.video.width, config.video.height),
    )
}

fn bind_android_listener(bind_address: &str, port: u16) -> Result<std::net::TcpListener, String> {
    let ip = bind_address
        .parse::<IpAddr>()
        .map_err(|err| format!("invalid Android bind address {bind_address}: {err}"))?;
    bind_tcp_listener(SocketAddr::new(ip, port))
        .map_err(|err| format!("failed to bind Android listener {bind_address}:{port}: {err}"))
}

fn spawn_device_info_broadcaster(state: Arc<AppState>, events: Arc<EventBus>) {
    enum DeviceInfoTrigger {
        Event,
        Lagged { topic: &'static str, count: u64 },
    }

    const DEVICE_INFO_TOPICS: &[&str] = &[
        "stream.state_changed",
        "stream.config_applied",
        "stream.mode_ready",
    ];
    const DEBOUNCE_MS: u64 = 100;

    let (trigger_tx, mut trigger_rx) = mpsc::unbounded_channel();
    for topic in DEVICE_INFO_TOPICS {
        let Some(mut rx) = events.subscribe_topic(topic) else {
            continue;
        };
        let trigger_tx = trigger_tx.clone();
        let topic_name = *topic;
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(_) => {
                        if trigger_tx.send(DeviceInfoTrigger::Event).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        if trigger_tx
                            .send(DeviceInfoTrigger::Lagged {
                                topic: topic_name,
                                count,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    {
        let mut dirty_rx = events.subscribe_device_info_dirty();
        let trigger_tx = trigger_tx.clone();
        tokio::spawn(async move {
            loop {
                match dirty_rx.recv().await {
                    Ok(()) => {
                        if trigger_tx.send(DeviceInfoTrigger::Event).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        if trigger_tx
                            .send(DeviceInfoTrigger::Lagged {
                                topic: "device_info_dirty",
                                count,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    tokio::spawn(async move {
        let mut last_broadcast = Instant::now() - Duration::from_millis(DEBOUNCE_MS);
        let mut pending_broadcast = false;

        loop {
            let recv_result = if pending_broadcast {
                let remaining =
                    DEBOUNCE_MS.saturating_sub(last_broadcast.elapsed().as_millis() as u64);
                tokio::time::timeout(Duration::from_millis(remaining), trigger_rx.recv()).await
            } else {
                Ok(trigger_rx.recv().await)
            };

            match recv_result {
                Ok(Some(DeviceInfoTrigger::Event)) => pending_broadcast = true,
                Ok(Some(DeviceInfoTrigger::Lagged { topic, count })) => {
                    tracing::warn!(
                        "Android device info broadcaster lagged by {} events on {}",
                        count,
                        topic
                    );
                    pending_broadcast = true;
                }
                Ok(None) => break,
                Err(_) => {}
            }

            if pending_broadcast && last_broadcast.elapsed() >= Duration::from_millis(DEBOUNCE_MS) {
                state.publish_device_info().await;
                last_broadcast = Instant::now();
                pending_broadcast = false;
            }
        }
    });
}

async fn cleanup(state: &Arc<AppState>) {
    state.extensions.stop_all().await;

    if let Some(service) = state.rustdesk.read().await.as_ref() {
        if let Err(err) = service.stop().await {
            tracing::warn!("Failed to stop Android RustDesk service: {}", err);
        }
    }

    if let Some(service) = state.vnc.read().await.as_ref() {
        if let Err(err) = service.stop().await {
            tracing::warn!("Failed to stop Android VNC service: {}", err);
        }
    }

    if let Some(service) = state.rtsp.read().await.as_ref() {
        if let Err(err) = service.stop().await {
            tracing::warn!("Failed to stop Android RTSP service: {}", err);
        }
    }

    if let Err(err) = state.stream_manager.stop().await {
        tracing::warn!("Failed to stop Android stream manager: {}", err);
    }
    if let Err(err) = state.hid.shutdown().await {
        tracing::warn!("Failed to stop Android HID: {}", err);
    }
    if let Some(msd) = state.msd.write().await.as_mut() {
        if let Err(err) = msd.shutdown().await {
            tracing::warn!("Failed to stop Android MSD: {}", err);
        }
    }
    if let Err(err) = state.otg_service.shutdown().await {
        tracing::warn!("Failed to stop Android OTG: {}", err);
    }
    if let Some(atx) = state.atx.write().await.as_mut() {
        if let Err(err) = atx.shutdown().await {
            tracing::warn!("Failed to stop Android ATX: {}", err);
        }
    }
    if let Err(err) = state.audio.shutdown().await {
        tracing::warn!("Failed to stop Android audio: {}", err);
    }
}

fn init_logging() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = tracing_log::LogTracer::init();
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "one_kvm=info,tower_http=info,webrtc_sctp=warn".into());
        let fmt_layer = tracing_subscriber::fmt::layer();
        if let Ok(path) = std::env::var("ONE_KVM_ANDROID_LOG_FILE") {
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                Ok(file) => {
                    let file_layer = tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(Arc::new(file));
                    let _ = tracing_subscriber::registry()
                        .with(filter)
                        .with(fmt_layer)
                        .with(file_layer)
                        .try_init();
                }
                Err(err) => {
                    eprintln!("failed to open Android Rust log file {path}: {err}");
                    let _ = tracing_subscriber::registry()
                        .with(filter)
                        .with(fmt_layer)
                        .try_init();
                }
            }
        } else {
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .try_init();
        }
    });
}

fn ensure_rustls_provider() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = CryptoProvider::install_default(ring::default_provider());
    });
}
