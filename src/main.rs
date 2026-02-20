use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use clap::{Parser, ValueEnum};
use futures::{stream::FuturesUnordered, StreamExt};
use rustls::crypto::{ring, CryptoProvider};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use one_kvm::atx::AtxController;
use one_kvm::audio::{AudioController, AudioControllerConfig, AudioQuality};
use one_kvm::auth::{SessionStore, UserStore};
use one_kvm::config::{self, AppConfig, ConfigStore};
use one_kvm::events::EventBus;
use one_kvm::extensions::ExtensionManager;
use one_kvm::hid::{HidBackendType, HidController};
use one_kvm::msd::MsdController;
use one_kvm::otg::{configfs, OtgService};
use one_kvm::rtsp::RtspService;
use one_kvm::rustdesk::RustDeskService;
use one_kvm::state::AppState;
use one_kvm::update::UpdateService;
use one_kvm::utils::bind_tcp_listener;
use one_kvm::video::codec_constraints::{
    enforce_constraints_with_stream_manager, StreamCodecConstraints,
};
use one_kvm::video::format::{PixelFormat, Resolution};
use one_kvm::video::{Streamer, VideoStreamManager};
use one_kvm::web;
use one_kvm::webrtc::{WebRtcStreamer, WebRtcStreamerConfig};

/// Log level for the application
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Verbose,
    Debug,
    Trace,
}

/// One-KVM command line arguments
#[derive(Parser, Debug)]
#[command(name = "one-kvm")]
#[command(version, about = "A  open and lightweight IP-KVM solution", long_about = None)]
struct CliArgs {
    /// Listen address (overrides database config)
    #[arg(short = 'a', long, value_name = "ADDRESS")]
    address: Option<String>,

    /// HTTP port (overrides database config)
    #[arg(short = 'p', long, value_name = "PORT")]
    http_port: Option<u16>,

    /// HTTPS port (overrides database config)
    #[arg(long, value_name = "PORT")]
    https_port: Option<u16>,

    /// Enable HTTPS (overrides database config)
    #[arg(long)]
    enable_https: bool,

    /// Path to SSL certificate file (generates self-signed if not provided)
    #[arg(long, value_name = "FILE", requires = "ssl_key")]
    ssl_cert: Option<PathBuf>,

    /// Path to SSL private key file
    #[arg(long, value_name = "FILE", requires = "ssl_cert")]
    ssl_key: Option<PathBuf>,

    /// Data directory path (default: /etc/one-kvm)
    #[arg(short = 'd', long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Log level (error, warn, info, verbose, debug, trace)
    #[arg(short = 'l', long, value_name = "LEVEL", default_value = "info")]
    log_level: LogLevel,

    /// Increase verbosity (-v for verbose, -vv for debug, -vvv for trace)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let args = CliArgs::parse();

    // Initialize logging with CLI arguments
    init_logging(args.log_level, args.verbose);

    // Install default crypto provider (required by rustls 0.23+)
    CryptoProvider::install_default(ring::default_provider())
        .expect("Failed to install rustls crypto provider");

    tracing::info!("Starting One-KVM v{}", env!("CARGO_PKG_VERSION"));

    // Determine data directory (CLI arg takes precedence)
    let data_dir = args.data_dir.unwrap_or_else(get_data_dir);
    tracing::info!("Data directory: {}", data_dir.display());

    // Ensure data directory exists
    tokio::fs::create_dir_all(&data_dir).await?;

    // Initialize configuration store
    let db_path = data_dir.join("one-kvm.db");
    let config_store = ConfigStore::new(&db_path).await?;
    let mut config = (*config_store.get()).clone();

    // Normalize MSD directory (absolute path under data dir if empty/relative)
    let mut msd_dir_updated = false;
    if config.msd.msd_dir.trim().is_empty() {
        let msd_dir = data_dir.join("msd");
        config.msd.msd_dir = msd_dir.to_string_lossy().to_string();
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

    // Ensure MSD directories exist (msd/images, msd/ventoy)
    let msd_dir = PathBuf::from(&config.msd.msd_dir);
    if let Err(e) = tokio::fs::create_dir_all(msd_dir.join("images")).await {
        tracing::warn!("Failed to create MSD images directory: {}", e);
    }
    if let Err(e) = tokio::fs::create_dir_all(msd_dir.join("ventoy")).await {
        tracing::warn!("Failed to create MSD ventoy directory: {}", e);
    }

    // Apply CLI argument overrides to config (only if explicitly specified)
    if let Some(addr) = args.address {
        config.web.bind_address = addr.clone();
        config.web.bind_addresses = vec![addr];
    }
    if let Some(port) = args.http_port {
        config.web.http_port = port;
    }
    if let Some(port) = args.https_port {
        config.web.https_port = port;
    }
    if args.enable_https {
        config.web.https_enabled = true;
    }

    if let Some(cert_path) = args.ssl_cert {
        config.web.ssl_cert_path = Some(cert_path.to_string_lossy().to_string());
    }
    if let Some(key_path) = args.ssl_key {
        config.web.ssl_key_path = Some(key_path.to_string_lossy().to_string());
    }

    let bind_ips = resolve_bind_addresses(&config.web)?;
    let scheme = if config.web.https_enabled {
        "https"
    } else {
        "http"
    };
    let bind_port = if config.web.https_enabled {
        config.web.https_port
    } else {
        config.web.http_port
    };

    // Log final configuration
    for ip in &bind_ips {
        let addr = SocketAddr::new(*ip, bind_port);
        tracing::info!("Server will listen on: {}://{}", scheme, addr);
    }

    // Initialize session store
    let session_store = SessionStore::new(
        config_store.pool().clone(),
        config.auth.session_timeout_secs as i64,
    );

    // Initialize user store
    let user_store = UserStore::new(config_store.pool().clone());

    // Create shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Create event bus for real-time notifications
    let events = Arc::new(EventBus::new());
    tracing::info!("Event bus initialized");

    // Parse video configuration once (avoid duplication)
    let (video_format, video_resolution) = parse_video_config(&config);
    tracing::debug!(
        "Parsed video config: {} @ {}x{}",
        video_format,
        video_resolution.width,
        video_resolution.height
    );

    // Create video streamer and initialize with config if device is set
    let streamer = Streamer::new();
    streamer.set_event_bus(events.clone()).await;
    if let Some(ref device_path) = config.video.device {
        if let Err(e) = streamer
            .apply_video_config(
                device_path,
                video_format,
                video_resolution,
                config.video.fps,
            )
            .await
        {
            tracing::warn!(
                "Failed to initialize video with config: {}, will auto-detect",
                e
            );
        } else {
            tracing::info!(
                "Video configured: {} @ {}x{} {}",
                device_path,
                video_resolution.width,
                video_resolution.height,
                video_format
            );
        }
    }

    // Create WebRTC streamer
    let webrtc_streamer = {
        let webrtc_config = WebRtcStreamerConfig {
            resolution: video_resolution,
            input_format: video_format,
            fps: config.video.fps,
            bitrate_preset: config.stream.bitrate_preset,
            encoder_backend: config.stream.encoder.to_backend(),
            webrtc: {
                let mut stun_servers = vec![];
                let mut turn_servers = vec![];

                // Check if user configured custom servers
                let has_custom_stun = config
                    .stream
                    .stun_server
                    .as_ref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                let has_custom_turn = config
                    .stream
                    .turn_server
                    .as_ref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

                // If no custom servers, use public ICE servers (like RustDesk)
                if !has_custom_stun && !has_custom_turn {
                    use one_kvm::webrtc::config::public_ice;
                    if public_ice::is_configured() {
                        if let Some(stun) = public_ice::stun_server() {
                            stun_servers.push(stun.clone());
                            tracing::info!("Using public STUN server: {}", stun);
                        }
                        for turn in public_ice::turn_servers() {
                            tracing::info!("Using public TURN server: {:?}", turn.urls);
                            turn_servers.push(turn);
                        }
                    } else {
                        tracing::info!(
                            "No public ICE servers configured, using host candidates only"
                        );
                    }
                } else {
                    // Use custom servers
                    if let Some(ref stun) = config.stream.stun_server {
                        if !stun.is_empty() {
                            stun_servers.push(stun.clone());
                            tracing::info!("Using custom STUN server: {}", stun);
                        }
                    }
                    if let Some(ref turn) = config.stream.turn_server {
                        if !turn.is_empty() {
                            let username = config.stream.turn_username.clone().unwrap_or_default();
                            let credential =
                                config.stream.turn_password.clone().unwrap_or_default();
                            turn_servers.push(one_kvm::webrtc::config::TurnServer::new(
                                turn.clone(),
                                username.clone(),
                                credential,
                            ));
                            tracing::info!(
                                "Using custom TURN server: {} (user: {})",
                                turn,
                                username
                            );
                        }
                    }
                }

                one_kvm::webrtc::config::WebRtcConfig {
                    stun_servers,
                    turn_servers,
                    ..Default::default()
                }
            },
            ..Default::default()
        };
        WebRtcStreamer::with_config(webrtc_config)
    };
    tracing::info!("WebRTC streamer created (supports H264, extensible to VP8/VP9/H265)");

    // Create OTG Service (single instance for centralized USB gadget management)
    let otg_service = Arc::new(OtgService::new());
    tracing::info!("OTG Service created");

    // Pre-enable OTG functions to avoid gadget recreation (prevents kernel crashes)
    let will_use_otg_hid = matches!(config.hid.backend, config::HidBackend::Otg);
    let will_use_msd = config.msd.enabled;

    if will_use_otg_hid {
        let mut hid_functions = config.hid.effective_otg_functions();
        if let Some(udc) = configfs::resolve_udc_name(config.hid.otg_udc.as_deref()) {
            if configfs::is_low_endpoint_udc(&udc) && hid_functions.consumer {
                tracing::warn!(
                    "UDC {} has low endpoint resources, disabling consumer control",
                    udc
                );
                hid_functions.consumer = false;
            }
        }
        if let Err(e) = otg_service.update_hid_functions(hid_functions).await {
            tracing::warn!("Failed to apply HID functions: {}", e);
        }
        if let Err(e) = otg_service.enable_hid().await {
            tracing::warn!("Failed to pre-enable HID: {}", e);
        }
    }
    if will_use_msd {
        if let Err(e) = otg_service.enable_msd().await {
            tracing::warn!("Failed to pre-enable MSD: {}", e);
        }
    }

    // Create HID controller based on config
    let hid_backend = match config.hid.backend {
        config::HidBackend::Otg => HidBackendType::Otg,
        config::HidBackend::Ch9329 => HidBackendType::Ch9329 {
            port: config.hid.ch9329_port.clone(),
            baud_rate: config.hid.ch9329_baudrate,
        },
        config::HidBackend::None => HidBackendType::None,
    };
    let hid = Arc::new(HidController::new(
        hid_backend,
        Some(otg_service.clone()), // Always pass OtgService to support hot-reload to OTG
    ));
    hid.set_event_bus(events.clone()).await;
    if let Err(e) = hid.init().await {
        tracing::warn!("Failed to initialize HID backend: {}", e);
    }

    // Create MSD controller (optional, based on config)
    let msd = if config.msd.enabled {
        // Initialize Ventoy resources from data directory
        let ventoy_resource_dir = ventoy_img::get_resource_dir(&data_dir);
        if ventoy_resource_dir.exists() {
            if let Err(e) = ventoy_img::init_resources(&ventoy_resource_dir) {
                tracing::warn!("Failed to initialize Ventoy resources: {}", e);
                tracing::info!(
                    "Ventoy resource files should be placed in: {}",
                    ventoy_resource_dir.display()
                );
                tracing::info!("Required files: {:?}", ventoy_img::required_files());
            } else {
                tracing::info!(
                    "Ventoy resources initialized from {}",
                    ventoy_resource_dir.display()
                );
            }
        } else {
            tracing::warn!(
                "Ventoy resource directory not found: {}",
                ventoy_resource_dir.display()
            );
            tracing::info!(
                "Create the directory and place the following files: {:?}",
                ventoy_img::required_files()
            );
        }

        let controller = MsdController::new(otg_service.clone(), config.msd.msd_dir_path());
        if let Err(e) = controller.init().await {
            tracing::warn!("Failed to initialize MSD controller: {}", e);
            None
        } else {
            controller.set_event_bus(events.clone()).await;
            Some(controller)
        }
    } else {
        tracing::info!("MSD disabled in configuration");
        None
    };

    // Create ATX controller (optional, based on config)
    let atx = if config.atx.enabled {
        let controller_config = config.atx.to_controller_config();
        let controller = AtxController::new(controller_config);

        if let Err(e) = controller.init().await {
            tracing::warn!("Failed to initialize ATX controller: {}", e);
            None
        } else {
            Some(controller)
        }
    } else {
        tracing::info!("ATX disabled in configuration");
        None
    };

    // Create Audio controller
    let audio = {
        let audio_config = AudioControllerConfig {
            enabled: config.audio.enabled,
            device: config.audio.device.clone(),
            quality: AudioQuality::from_str(&config.audio.quality),
        };

        let controller = AudioController::new(audio_config);
        controller.set_event_bus(events.clone()).await;

        if config.audio.enabled {
            tracing::info!(
                "Audio enabled: {}, quality={}",
                config.audio.device,
                config.audio.quality
            );
            // Start audio streaming so WebRTC can subscribe to Opus frames
            if let Err(e) = controller.start_streaming().await {
                tracing::warn!("Failed to start audio streaming: {}", e);
            }
        } else {
            tracing::info!("Audio disabled in configuration");
        }

        Arc::new(controller)
    };

    // Create Extension manager (ttyd, gostc, easytier)
    let extensions = Arc::new(ExtensionManager::new());
    tracing::info!("Extension manager initialized");

    // Wire up WebRTC streamer with HID controller
    // This enables WebRTC DataChannel to process HID events
    webrtc_streamer.set_hid_controller(hid.clone()).await;

    // Wire up WebRTC streamer with Audio controller
    // This enables WebRTC audio track to receive Opus frames
    webrtc_streamer.set_audio_controller(audio.clone()).await;
    if config.audio.enabled {
        if let Err(e) = webrtc_streamer.set_audio_enabled(true).await {
            tracing::warn!("Failed to enable WebRTC audio: {}", e);
        } else {
            tracing::info!("WebRTC audio enabled");
        }
    }

    // Configure direct capture for WebRTC encoder pipeline
    let (device_path, actual_resolution, actual_format, actual_fps, jpeg_quality) =
        streamer.current_capture_config().await;
    tracing::info!(
        "Initial video config: {}x{} {:?} @ {}fps",
        actual_resolution.width,
        actual_resolution.height,
        actual_format,
        actual_fps
    );
    webrtc_streamer
        .update_video_config(actual_resolution, actual_format, actual_fps)
        .await;
    if let Some(device_path) = device_path {
        webrtc_streamer
            .set_capture_device(device_path, jpeg_quality)
            .await;
        tracing::info!("WebRTC streamer configured for direct capture");
    } else {
        tracing::warn!("No capture device configured for WebRTC");
    }

    // Create video stream manager (unified MJPEG/WebRTC management)
    // Use with_webrtc_streamer to ensure we use the same WebRtcStreamer instance
    let stream_manager =
        VideoStreamManager::with_webrtc_streamer(streamer.clone(), webrtc_streamer.clone());
    stream_manager.set_event_bus(events.clone()).await;
    stream_manager.set_config_store(config_store.clone()).await;

    // Initialize stream manager with configured mode
    let initial_mode = config.stream.mode.clone();
    if let Err(e) = stream_manager.init_with_mode(initial_mode.clone()).await {
        tracing::warn!(
            "Failed to initialize stream manager with mode {:?}: {}",
            initial_mode,
            e
        );
    } else {
        tracing::info!(
            "Video stream manager initialized with mode: {:?}",
            initial_mode
        );
    }

    // Create RustDesk service (optional, based on config)
    let rustdesk = if config.rustdesk.is_valid() {
        tracing::info!(
            "Initializing RustDesk service: ID={} -> {}",
            config.rustdesk.device_id,
            config.rustdesk.rendezvous_addr()
        );
        let service = RustDeskService::new(
            config.rustdesk.clone(),
            stream_manager.clone(),
            hid.clone(),
            audio.clone(),
        );
        Some(Arc::new(service))
    } else {
        if config.rustdesk.enabled {
            tracing::warn!(
                "RustDesk enabled but configuration is incomplete (missing server or credentials)"
            );
        } else {
            tracing::info!("RustDesk disabled in configuration");
        }
        None
    };

    // Create RTSP service (optional, based on config)
    let rtsp = if config.rtsp.enabled {
        tracing::info!(
            "Initializing RTSP service: rtsp://{}:{}/{}",
            config.rtsp.bind,
            config.rtsp.port,
            config.rtsp.path
        );
        let service = RtspService::new(config.rtsp.clone(), stream_manager.clone());
        Some(Arc::new(service))
    } else {
        tracing::info!("RTSP disabled in configuration");
        None
    };

    // Create application state
    let update_service = Arc::new(UpdateService::new(data_dir.join("updates")));

    let state = AppState::new(
        config_store.clone(),
        session_store,
        user_store,
        otg_service,
        stream_manager,
        hid,
        msd,
        atx,
        audio,
        rustdesk.clone(),
        rtsp.clone(),
        extensions.clone(),
        events.clone(),
        update_service,
        shutdown_tx.clone(),
        data_dir.clone(),
    );

    // Start RustDesk service if enabled
    if let Some(ref service) = rustdesk {
        if let Err(e) = service.start().await {
            tracing::error!("Failed to start RustDesk service: {}", e);
        } else {
            // Save generated keypair and UUID to config
            if let Some(updated_config) = service.save_credentials() {
                if let Err(e) = config_store
                    .update(|cfg| {
                        cfg.rustdesk.public_key = updated_config.public_key.clone();
                        cfg.rustdesk.private_key = updated_config.private_key.clone();
                        cfg.rustdesk.signing_public_key = updated_config.signing_public_key.clone();
                        cfg.rustdesk.signing_private_key =
                            updated_config.signing_private_key.clone();
                        cfg.rustdesk.uuid = updated_config.uuid.clone();
                    })
                    .await
                {
                    tracing::warn!("Failed to save RustDesk credentials: {}", e);
                }
            }
            tracing::info!("RustDesk service started");
        }
    }

    // Start RTSP service if enabled
    if let Some(ref service) = rtsp {
        if let Err(e) = service.start().await {
            tracing::error!("Failed to start RTSP service: {}", e);
        } else {
            tracing::info!("RTSP service started");
        }
    }

    // Enforce startup codec constraints (e.g. RTSP/RustDesk locks)
    {
        let runtime_config = state.config.get();
        let constraints = StreamCodecConstraints::from_config(&runtime_config);
        match enforce_constraints_with_stream_manager(&state.stream_manager, &constraints).await {
            Ok(result) if result.changed => {
                if let Some(message) = result.message {
                    tracing::info!("{}", message);
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("Failed to enforce startup codec constraints: {}", e),
        }
    }

    // Start enabled extensions
    {
        let ext_config = config_store.get();
        extensions.start_enabled(&ext_config.extensions).await;
    }

    // Start extension health check task (every 30 seconds)
    {
        let extensions_clone = extensions.clone();
        let config_store_clone = config_store.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let config = config_store_clone.get();
                extensions_clone.health_check(&config.extensions).await;
            }
        });
        tracing::info!("Extension health check task started");
    }

    // Start device info broadcast task
    // This monitors state change events and broadcasts DeviceInfo to all clients
    spawn_device_info_broadcaster(state.clone(), events);

    // Create router
    let app = web::create_router(state.clone());

    // Bind sockets for configured addresses
    let listeners = bind_tcp_listeners(&bind_ips, bind_port)?;

    // Setup graceful shutdown
    let shutdown_signal = async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
        tracing::info!("Shutdown signal received");
        let _ = shutdown_tx.send(());
    };

    // Start server
    if config.web.https_enabled {
        // Generate self-signed certificate if no custom cert provided
        let tls_config = if let (Some(cert_path), Some(key_path)) =
            (&config.web.ssl_cert_path, &config.web.ssl_key_path)
        {
            RustlsConfig::from_pem_file(cert_path, key_path).await?
        } else {
            let cert_dir = data_dir.join("certs");
            let cert_path = cert_dir.join("server.crt");
            let key_path = cert_dir.join("server.key");

            // Check if certificate already exists, only generate if missing
            if !cert_path.exists() || !key_path.exists() {
                tracing::info!("Generating new self-signed TLS certificate");
                let cert = generate_self_signed_cert()?;
                tokio::fs::create_dir_all(&cert_dir).await?;
                tokio::fs::write(&cert_path, cert.cert.pem()).await?;
                tokio::fs::write(&key_path, cert.signing_key.serialize_pem()).await?;
            } else {
                tracing::info!("Using existing TLS certificate from {}", cert_dir.display());
            }

            RustlsConfig::from_pem_file(&cert_path, &key_path).await?
        };

        let mut servers = FuturesUnordered::new();
        for listener in listeners {
            let local_addr = listener.local_addr()?;
            tracing::info!("Starting HTTPS server on {}", local_addr);

            let server = axum_server::from_tcp_rustls(listener, tls_config.clone())?
                .serve(app.clone().into_make_service());
            servers.push(server);
        }

        tokio::select! {
            _ = shutdown_signal => {
                cleanup(&state).await;
            }
            result = servers.next() => {
                if let Some(Err(e)) = result {
                    tracing::error!("HTTPS server error: {}", e);
                }
                cleanup(&state).await;
            }
        }
    } else {
        let mut servers = FuturesUnordered::new();
        for listener in listeners {
            let local_addr = listener.local_addr()?;
            tracing::info!("Starting HTTP server on {}", local_addr);

            let listener = tokio::net::TcpListener::from_std(listener)?;
            let server = axum::serve(listener, app.clone());
            servers.push(async move { server.await });
        }

        tokio::select! {
            _ = shutdown_signal => {
                cleanup(&state).await;
            }
            result = servers.next() => {
                if let Some(Err(e)) = result {
                    tracing::error!("HTTP server error: {}", e);
                }
                cleanup(&state).await;
            }
        }
    }

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Initialize logging with tracing
fn init_logging(level: LogLevel, verbose_count: u8) {
    // Verbose count overrides log level
    let effective_level = match verbose_count {
        0 => level,
        1 => LogLevel::Verbose,
        2 => LogLevel::Debug,
        _ => LogLevel::Trace,
    };

    // Build filter string based on effective level
    let filter = match effective_level {
        LogLevel::Error => "one_kvm=error,tower_http=error",
        LogLevel::Warn => "one_kvm=warn,tower_http=warn",
        LogLevel::Info => "one_kvm=info,tower_http=info",
        LogLevel::Verbose => "one_kvm=debug,tower_http=info",
        LogLevel::Debug => "one_kvm=debug,tower_http=debug",
        LogLevel::Trace => "one_kvm=trace,tower_http=debug",
    };

    // Environment variable takes highest priority
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into());

    if let Err(err) = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
    {
        eprintln!("failed to initialize tracing: {}", err);
    }
}

/// Get the application data directory
fn get_data_dir() -> PathBuf {
    // Check environment variable first
    if let Ok(path) = std::env::var("ONE_KVM_DATA_DIR") {
        return PathBuf::from(path);
    }

    // Default to system configuration directory
    PathBuf::from("/etc/one-kvm")
}

/// Resolve bind IPs from config, preferring bind_addresses when set.
fn resolve_bind_addresses(web: &config::WebConfig) -> anyhow::Result<Vec<IpAddr>> {
    let raw_addrs = if !web.bind_addresses.is_empty() {
        web.bind_addresses.as_slice()
    } else {
        std::slice::from_ref(&web.bind_address)
    };

    let mut seen = HashSet::new();
    let mut addrs = Vec::new();
    for addr in raw_addrs {
        let ip: IpAddr = addr
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid bind address: {}", addr))?;
        if seen.insert(ip) {
            addrs.push(ip);
        }
    }

    Ok(addrs)
}

fn bind_tcp_listeners(addrs: &[IpAddr], port: u16) -> anyhow::Result<Vec<std::net::TcpListener>> {
    let mut listeners = Vec::new();
    for ip in addrs {
        let addr = SocketAddr::new(*ip, port);
        match bind_tcp_listener(addr) {
            Ok(listener) => listeners.push(listener),
            Err(err) => {
                tracing::warn!("Failed to bind {}: {}", addr, err);
            }
        }
    }

    if listeners.is_empty() {
        anyhow::bail!("Failed to bind any addresses on port {}", port);
    }

    Ok(listeners)
}

/// Parse video format and resolution from config (avoids code duplication)
fn parse_video_config(config: &AppConfig) -> (PixelFormat, Resolution) {
    let format = config
        .video
        .format
        .as_ref()
        .and_then(|f: &String| f.parse::<PixelFormat>().ok())
        .unwrap_or(PixelFormat::Mjpeg);
    let resolution = Resolution::new(config.video.width, config.video.height);
    (format, resolution)
}

/// Generate a self-signed TLS certificate
fn generate_self_signed_cert() -> anyhow::Result<rcgen::CertifiedKey<rcgen::KeyPair>> {
    use rcgen::generate_simple_self_signed;

    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];

    let certified_key = generate_simple_self_signed(subject_alt_names)?;
    Ok(certified_key)
}

/// Spawn a background task that monitors state change events
/// and broadcasts DeviceInfo to all WebSocket clients with debouncing
fn spawn_device_info_broadcaster(state: Arc<AppState>, events: Arc<EventBus>) {
    use one_kvm::events::SystemEvent;
    use std::time::{Duration, Instant};

    let mut rx = events.subscribe();
    const DEBOUNCE_MS: u64 = 100;

    tokio::spawn(async move {
        let mut last_broadcast = Instant::now() - Duration::from_millis(DEBOUNCE_MS);
        let mut pending_broadcast = false;

        loop {
            // Use timeout to handle pending broadcasts
            let recv_result = if pending_broadcast {
                let remaining =
                    DEBOUNCE_MS.saturating_sub(last_broadcast.elapsed().as_millis() as u64);
                tokio::time::timeout(Duration::from_millis(remaining), rx.recv()).await
            } else {
                Ok(rx.recv().await)
            };

            match recv_result {
                Ok(Ok(event)) => {
                    let should_broadcast = matches!(
                        event,
                        SystemEvent::StreamStateChanged { .. }
                            | SystemEvent::StreamConfigApplied { .. }
                            | SystemEvent::StreamModeReady { .. }
                            | SystemEvent::HidStateChanged { .. }
                            | SystemEvent::MsdStateChanged { .. }
                            | SystemEvent::AtxStateChanged { .. }
                            | SystemEvent::AudioStateChanged { .. }
                    );
                    if should_broadcast {
                        pending_broadcast = true;
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    tracing::warn!("DeviceInfo broadcaster lagged by {} events", n);
                    pending_broadcast = true;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    tracing::info!("Event bus closed, stopping DeviceInfo broadcaster");
                    break;
                }
                Err(_timeout) => {
                    // Debounce timeout reached, broadcast now
                }
            }

            // Broadcast if pending and debounce time has passed
            if pending_broadcast && last_broadcast.elapsed() >= Duration::from_millis(DEBOUNCE_MS) {
                state.publish_device_info().await;
                tracing::trace!("Broadcasted DeviceInfo (debounced)");
                last_broadcast = Instant::now();
                pending_broadcast = false;
            }
        }
    });

    tracing::info!(
        "DeviceInfo broadcaster task started (debounce: {}ms)",
        DEBOUNCE_MS
    );
}

/// Clean up subsystems on shutdown
async fn cleanup(state: &Arc<AppState>) {
    // Stop all extensions
    state.extensions.stop_all().await;
    tracing::info!("Extensions stopped");

    // Stop RustDesk service
    if let Some(ref service) = *state.rustdesk.read().await {
        if let Err(e) = service.stop().await {
            tracing::warn!("Failed to stop RustDesk service: {}", e);
        } else {
            tracing::info!("RustDesk service stopped");
        }
    }

    // Stop RTSP service
    if let Some(ref service) = *state.rtsp.read().await {
        if let Err(e) = service.stop().await {
            tracing::warn!("Failed to stop RTSP service: {}", e);
        } else {
            tracing::info!("RTSP service stopped");
        }
    }

    // Stop video
    if let Err(e) = state.stream_manager.stop().await {
        tracing::warn!("Failed to stop streamer: {}", e);
    }

    // Shutdown HID
    if let Err(e) = state.hid.shutdown().await {
        tracing::warn!("Failed to shutdown HID: {}", e);
    }

    // Shutdown MSD
    if let Some(msd) = state.msd.write().await.as_mut() {
        if let Err(e) = msd.shutdown().await {
            tracing::warn!("Failed to shutdown MSD: {}", e);
        }
    }

    // Shutdown ATX
    if let Some(atx) = state.atx.write().await.as_mut() {
        if let Err(e) = atx.shutdown().await {
            tracing::warn!("Failed to shutdown ATX: {}", e);
        }
    }

    // Shutdown Audio
    if let Err(e) = state.audio.shutdown().await {
        tracing::warn!("Failed to shutdown audio: {}", e);
    }
}
