pub mod config;
pub mod devices;
pub mod extensions;
pub mod terminal;

use axum::{extract::State, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::auth::{Session, SESSION_COOKIE};
use crate::config::{AppConfig, StreamMode};
use crate::error::{AppError, Result};
use crate::events::SystemEvent;
use crate::state::AppState;
use crate::video::encoder::BitratePreset;

// ============================================================================
// Health & Info
// ============================================================================

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// System info response
#[derive(Serialize)]
pub struct SystemInfo {
    pub version: &'static str,
    pub build_date: &'static str,
    pub initialized: bool,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_space: Option<DiskSpaceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_info: Option<DeviceInfo>,
}

/// Device information (hostname, CPU, memory, network)
#[derive(Serialize)]
pub struct DeviceInfo {
    pub hostname: String,
    pub cpu_model: String,
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub network_addresses: Vec<NetworkAddress>,
}

/// Network interface address
#[derive(Serialize)]
pub struct NetworkAddress {
    pub interface: String,
    pub ip: String,
}

/// Disk space information
#[derive(Serialize)]
pub struct DiskSpaceInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
}

#[derive(Serialize)]
pub struct Capabilities {
    pub video: CapabilityInfo,
    pub hid: CapabilityInfo,
    pub msd: CapabilityInfo,
    pub atx: CapabilityInfo,
    pub audio: CapabilityInfo,
}

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub available: bool,
    pub backend: Option<String>,
}

pub async fn system_info(State(state): State<Arc<AppState>>) -> Json<SystemInfo> {
    let config = state.config.get();

    // Get disk space information for MSD base directory
    let disk_space = {
        let msd_dir = config.msd.msd_dir_path();
        if msd_dir.as_os_str().is_empty() {
            None
        } else {
            get_disk_space(&msd_dir).ok()
        }
    };

    // Get device information (hostname, CPU, memory, network)
    let device_info = Some(get_device_info());

    Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION"),
        build_date: env!("BUILD_DATE"),
        initialized: config.initialized,
        capabilities: Capabilities {
            video: CapabilityInfo {
                available: config.video.device.is_some(),
                backend: config.video.device.clone(),
            },
            hid: CapabilityInfo {
                available: config.hid.backend != crate::config::HidBackend::None,
                backend: Some(format!("{:?}", config.hid.backend)),
            },
            msd: CapabilityInfo {
                available: config.msd.enabled,
                backend: None,
            },
            atx: CapabilityInfo {
                available: config.atx.enabled,
                backend: if config.atx.enabled {
                    Some(format!(
                        "power: {:?}, reset: {:?}",
                        config.atx.power.driver, config.atx.reset.driver
                    ))
                } else {
                    None
                },
            },
            audio: CapabilityInfo {
                available: config.audio.enabled,
                backend: Some(config.audio.device.clone()),
            },
        },
        disk_space,
        device_info,
    })
}

/// Get disk space information for a given path
fn get_disk_space(path: &std::path::Path) -> Result<DiskSpaceInfo> {
    let stat = nix::sys::statvfs::statvfs(path)
        .map_err(|e| AppError::Internal(format!("Failed to get disk space: {}", e)))?;

    let block_size = stat.block_size() as u64;
    let total = stat.blocks() as u64 * block_size;
    let available = stat.blocks_available() as u64 * block_size;
    let used = total - available;

    Ok(DiskSpaceInfo {
        total,
        available,
        used,
    })
}

/// Get device information (hostname, CPU, memory, network)
fn get_device_info() -> DeviceInfo {
    // Get memory info in a single read
    let mem_info = get_meminfo();

    DeviceInfo {
        hostname: get_hostname(),
        cpu_model: get_cpu_model(),
        cpu_usage: get_cpu_usage(),
        memory_total: mem_info.total,
        memory_used: mem_info.total.saturating_sub(mem_info.available),
        network_addresses: get_network_addresses(),
    }
}

/// Get system hostname
fn get_hostname() -> String {
    nix::unistd::gethostname()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Get CPU model name from /proc/cpuinfo
fn get_cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            // Try to get model name
            let model = content
                .lines()
                .find(|line| line.starts_with("model name") || line.starts_with("Model"))
                .and_then(|line| line.split(':').nth(1))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            if model.is_some() {
                return model;
            }

            // Fallback: show arch and core count
            let cores = content
                .lines()
                .filter(|line| line.starts_with("processor"))
                .count();
            Some(format!("{} {}C", std::env::consts::ARCH, cores))
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}

/// CPU usage state for calculating usage between samples
static CPU_PREV_STATS: std::sync::OnceLock<std::sync::Mutex<(u64, u64)>> =
    std::sync::OnceLock::new();

/// Get CPU usage percentage (0.0 - 100.0)
fn get_cpu_usage() -> f32 {
    // Parse /proc/stat for CPU times
    let content = match std::fs::read_to_string("/proc/stat") {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    let cpu_line = match content.lines().next() {
        Some(line) if line.starts_with("cpu ") => line,
        _ => return 0.0,
    };

    // Parse CPU times: user, nice, system, idle, iowait, irq, softirq, steal
    let parts: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1) // skip "cpu"
        .take(8)
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() < 4 {
        return 0.0;
    }

    let idle = parts[3] + parts.get(4).unwrap_or(&0); // idle + iowait
    let total: u64 = parts.iter().sum();

    // Get or initialize previous stats
    let prev_mutex = CPU_PREV_STATS.get_or_init(|| std::sync::Mutex::new((0, 0)));
    let mut prev = prev_mutex.lock().unwrap();
    let (prev_idle, prev_total) = *prev;

    // Calculate delta
    let idle_delta = idle.saturating_sub(prev_idle);
    let total_delta = total.saturating_sub(prev_total);

    // Update previous stats
    *prev = (idle, total);

    if total_delta == 0 {
        return 0.0;
    }

    let usage = 100.0 * (1.0 - (idle_delta as f64 / total_delta as f64));
    usage as f32
}

/// Memory info parsed from /proc/meminfo
struct MemInfo {
    total: u64,
    available: u64,
}

/// Parse memory info from /proc/meminfo in a single read
fn get_meminfo() -> MemInfo {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => {
            return MemInfo {
                total: 0,
                available: 0,
            }
        }
    };

    let mut total = 0u64;
    let mut available = 0u64;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(kb) = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
            {
                total = kb * 1024;
            }
        } else if line.starts_with("MemAvailable:") {
            if let Some(kb) = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
            {
                available = kb * 1024;
            }
        }
        // Early exit if both values found
        if total > 0 && available > 0 {
            break;
        }
    }

    MemInfo { total, available }
}

/// Get network addresses for all non-loopback interfaces
fn get_network_addresses() -> Vec<NetworkAddress> {
    // Get all interface addresses in a single system call
    let all_addrs = match nix::ifaddrs::getifaddrs() {
        Ok(addrs) => addrs,
        Err(_) => return Vec::new(),
    };

    // Check which interfaces are up
    let mut up_ifaces = std::collections::HashSet::new();
    let net_dir = match std::fs::read_dir("/sys/class/net") {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    for entry in net_dir.flatten() {
        let iface_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        // Skip loopback
        if iface_name == "lo" {
            continue;
        }

        // Check if interface is up by reading operstate
        let operstate_path = entry.path().join("operstate");
        let is_up = std::fs::read_to_string(&operstate_path)
            .map(|s| s.trim() == "up")
            .unwrap_or(false);

        if !is_up {
            continue;
        }

        up_ifaces.insert(iface_name);
    }

    let mut addresses = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ifaddr in all_addrs {
        let iface_name = &ifaddr.interface_name;
        if iface_name == "lo" || !up_ifaces.contains(iface_name) {
            continue;
        }

        if let Some(addr) = ifaddr.address {
            if let Some(sockaddr_in) = addr.as_sockaddr_in() {
                let ip = sockaddr_in.ip();
                if ip.is_loopback() {
                    continue;
                }
                let ip_str = ip.to_string();
                if seen.insert((iface_name.clone(), ip_str.clone())) {
                    addresses.push(NetworkAddress {
                        interface: iface_name.clone(),
                        ip: ip_str,
                    });
                }
            } else if let Some(sockaddr_in6) = addr.as_sockaddr_in6() {
                let ip = sockaddr_in6.ip();
                if ip.is_loopback() || ip.is_unspecified() || ip.is_unicast_link_local() {
                    continue;
                }
                let ip_str = ip.to_string();
                if seen.insert((iface_name.clone(), ip_str.clone())) {
                    addresses.push(NetworkAddress {
                        interface: iface_name.clone(),
                        ip: ip_str,
                    });
                }
            }
        }
    }

    addresses
}

// ============================================================================
// Authentication
// ============================================================================

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: Option<String>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>)> {
    let config = state.config.get();

    // Check if system is initialized
    if !config.initialized {
        return Err(AppError::BadRequest("System not initialized".to_string()));
    }

    // Verify user credentials
    let user = state
        .users
        .verify(&req.username, &req.password)
        .await?
        .ok_or_else(|| AppError::AuthError("Invalid username or password".to_string()))?;

    if !config.auth.single_user_allow_multiple_sessions {
        // Kick existing sessions before creating a new one.
        let revoked_ids = state.sessions.list_ids().await?;
        state.sessions.delete_all().await?;
        state.remember_revoked_sessions(revoked_ids).await;
    }

    // Create session
    let session = state.sessions.create(&user.id).await?;

    // Set session cookie
    let cookie = Cookie::build((SESSION_COOKIE, session.id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(
            config.auth.session_timeout_secs as i64,
        ))
        .build();

    Ok((
        cookies.add(cookie),
        Json(LoginResponse {
            success: true,
            message: None,
        }),
    ))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
) -> Result<(CookieJar, Json<LoginResponse>)> {
    // Get session ID from cookie
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        state.sessions.delete(cookie.value()).await?;
    }

    // Remove cookie
    let cookie = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    Ok((
        cookies.remove(cookie),
        Json(LoginResponse {
            success: true,
            message: Some("Logged out".to_string()),
        }),
    ))
}

#[derive(Serialize)]
pub struct AuthCheckResponse {
    pub authenticated: bool,
    pub user: Option<String>,
}

pub async fn auth_check(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Json<AuthCheckResponse> {
    // Get user info from user_id
    let username = match state.users.get(&session.user_id).await {
        Ok(Some(user)) => Some(user.username),
        _ => Some(session.user_id.clone()), // Fallback to user_id if user not found
    };

    Json(AuthCheckResponse {
        authenticated: true,
        user: username,
    })
}

// ============================================================================
// Setup
// ============================================================================

#[derive(Serialize)]
pub struct SetupStatus {
    pub initialized: bool,
    pub needs_setup: bool,
}

pub async fn setup_status(State(state): State<Arc<AppState>>) -> Json<SetupStatus> {
    let initialized = state.config.is_initialized();
    Json(SetupStatus {
        initialized,
        needs_setup: !initialized,
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
    // Extension settings
    pub ttyd_enabled: Option<bool>,
    pub rustdesk_enabled: Option<bool>,
}

fn normalize_otg_profile_for_low_endpoint(config: &mut AppConfig) {
    if !matches!(config.hid.backend, crate::config::HidBackend::Otg) {
        return;
    }
    let udc = crate::otg::configfs::resolve_udc_name(config.hid.otg_udc.as_deref());
    let Some(udc) = udc else {
        return;
    };
    if !crate::otg::configfs::is_low_endpoint_udc(&udc) {
        return;
    }
    match config.hid.otg_profile {
        crate::config::OtgHidProfile::Full => {
            config.hid.otg_profile = crate::config::OtgHidProfile::FullNoConsumer;
        }
        crate::config::OtgHidProfile::FullNoMsd => {
            config.hid.otg_profile = crate::config::OtgHidProfile::FullNoConsumerNoMsd;
        }
        crate::config::OtgHidProfile::Custom => {
            if config.hid.otg_functions.consumer {
                config.hid.otg_functions.consumer = false;
            }
        }
        _ => {}
    }
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
    state.users.create(&req.username, &req.password).await?;

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
                config.hid.otg_profile = match profile.as_str() {
                    "full" => crate::config::OtgHidProfile::Full,
                    "full_no_msd" => crate::config::OtgHidProfile::FullNoMsd,
                    "full_no_consumer" => crate::config::OtgHidProfile::FullNoConsumer,
                    "full_no_consumer_no_msd" => crate::config::OtgHidProfile::FullNoConsumerNoMsd,
                    "legacy_keyboard" => crate::config::OtgHidProfile::LegacyKeyboard,
                    "legacy_mouse_relative" => crate::config::OtgHidProfile::LegacyMouseRelative,
                    "custom" => crate::config::OtgHidProfile::Custom,
                    _ => config.hid.otg_profile.clone(),
                };
                if matches!(config.hid.backend, crate::config::HidBackend::Otg) {
                    match config.hid.otg_profile {
                        crate::config::OtgHidProfile::Full
                        | crate::config::OtgHidProfile::FullNoConsumer => {
                            config.msd.enabled = true;
                        }
                        crate::config::OtgHidProfile::FullNoMsd
                        | crate::config::OtgHidProfile::FullNoConsumerNoMsd
                        | crate::config::OtgHidProfile::LegacyKeyboard
                        | crate::config::OtgHidProfile::LegacyMouseRelative => {
                            config.msd.enabled = false;
                        }
                        crate::config::OtgHidProfile::Custom => {}
                    }
                }
            }

            // Extension settings
            if let Some(enabled) = req.ttyd_enabled {
                config.extensions.ttyd.enabled = enabled;
            }
            if let Some(enabled) = req.rustdesk_enabled {
                config.rustdesk.enabled = enabled;
            }

            normalize_otg_profile_for_low_endpoint(config);
        })
        .await?;

    // Get updated config for HID reload
    let new_config = state.config.get();

    if matches!(new_config.hid.backend, crate::config::HidBackend::Otg) {
        let mut hid_functions = new_config.hid.effective_otg_functions();
        if let Some(udc) = crate::otg::configfs::resolve_udc_name(new_config.hid.otg_udc.as_deref())
        {
            if crate::otg::configfs::is_low_endpoint_udc(&udc) && hid_functions.consumer {
                tracing::warn!(
                    "UDC {} has low endpoint resources, disabling consumer control",
                    udc
                );
                hid_functions.consumer = false;
            }
        }
        if let Err(e) = state.otg_service.update_hid_functions(hid_functions).await {
            tracing::warn!("Failed to apply HID functions during setup: {}", e);
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
        if let Err(e) =
            config::apply::apply_rustdesk_config(&state, &empty_config, &new_config.rustdesk).await
        {
            tracing::warn!("Failed to start RustDesk during setup: {}", e);
        } else {
            tracing::info!("RustDesk started during setup");
        }
    }

    // Start audio streaming if audio device was selected during setup
    if new_config.audio.enabled {
        let audio_config = crate::audio::AudioControllerConfig {
            enabled: true,
            device: new_config.audio.device.clone(),
            quality: crate::audio::AudioQuality::from_str(&new_config.audio.quality),
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

// ============================================================================
// Configuration
// ============================================================================

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    #[serde(flatten)]
    pub updates: serde_json::Value,
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateConfigRequest>,
) -> Result<Json<LoginResponse>> {
    // Keep old config for rollback
    let old_config = state.config.get();

    tracing::info!("Received config update request");

    // Validate and merge config first (outside the update closure)
    let config_json = serde_json::to_value(&old_config)
        .map_err(|e| AppError::Internal(format!("Failed to serialize config: {}", e)))?;

    let merged = merge_json(config_json, req.updates.clone())
        .map_err(|_| AppError::Internal("Failed to merge config".to_string()))?;

    let new_config: AppConfig = serde_json::from_value(merged)
        .map_err(|e| AppError::BadRequest(format!("Invalid config format: {}", e)))?;

    let mut new_config = new_config;
    normalize_otg_profile_for_low_endpoint(&mut new_config);

    // Apply the validated config
    state.config.set(new_config.clone()).await?;

    tracing::info!("Config updated successfully");

    // Detect which config sections were sent in the request
    let has_video = req.updates.get("video").is_some();
    let has_stream = req.updates.get("stream").is_some();
    let has_hid = req.updates.get("hid").is_some();
    let has_msd = req.updates.get("msd").is_some();
    let has_atx = req.updates.get("atx").is_some();
    let has_audio = req.updates.get("audio").is_some();

    tracing::info!(
        "Config sections sent: video={}, stream={}, hid={}, msd={}, atx={}, audio={}",
        has_video,
        has_stream,
        has_hid,
        has_msd,
        has_atx,
        has_audio
    );

    // Get new config for device reloading
    let new_config = state.config.get();

    // Video config processing - always reload if section was sent
    if has_video {
        tracing::info!("Video config sent, applying settings...");

        let device = new_config
            .video
            .device
            .clone()
            .ok_or_else(|| AppError::BadRequest("video_device is required".to_string()))?;

        // Map to PixelFormat/Resolution
        let format = new_config
            .video
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
            crate::video::format::Resolution::new(new_config.video.width, new_config.video.height);

        // Step 1: Update WebRTC streamer config FIRST
        // This stops the shared pipeline and closes existing sessions BEFORE capturer is recreated
        // This ensures the pipeline won't be subscribed to a stale frame source
        state
            .stream_manager
            .webrtc_streamer()
            .update_video_config(resolution, format, new_config.video.fps)
            .await;
        tracing::info!("WebRTC streamer config updated (pipeline stopped, sessions closed)");

        // Step 2: Apply video config to streamer (recreates capturer)
        if let Err(e) = state
            .stream_manager
            .streamer()
            .apply_video_config(&device, format, resolution, new_config.video.fps)
            .await
        {
            tracing::error!("Failed to apply video config: {}", e);
            // Rollback config on failure
            state.config.set((*old_config).clone()).await?;
            return Ok(Json(LoginResponse {
                success: false,
                message: Some(format!("Video configuration invalid: {}", e)),
            }));
        }
        tracing::info!("Video config applied successfully");

        // Step 3: Start the streamer to begin capturing frames (MJPEG mode only)
        if !state.stream_manager.is_webrtc_enabled().await {
            // This is necessary because apply_video_config only creates the capturer but doesn't start it
            if let Err(e) = state.stream_manager.start().await {
                tracing::error!("Failed to start streamer after config change: {}", e);
                // Don't fail the request - the stream might start later when client connects
            } else {
                tracing::info!("Streamer started after config change");
            }
        }

        // Configure WebRTC direct capture (all modes)
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
    }

    // Stream config processing (encoder backend, bitrate, etc.)
    if has_stream {
        tracing::info!("Stream config sent, applying encoder settings...");

        // Update WebRTC streamer encoder backend
        let encoder_backend = new_config.stream.encoder.to_backend();
        tracing::info!(
            "Updating encoder backend to: {:?} (from config: {:?})",
            encoder_backend,
            new_config.stream.encoder
        );

        state
            .stream_manager
            .webrtc_streamer()
            .update_encoder_backend(encoder_backend)
            .await;

        // Update bitrate if changed
        state
            .stream_manager
            .webrtc_streamer()
            .set_bitrate_preset(new_config.stream.bitrate_preset)
            .await
            .ok(); // Ignore error if no active stream

        tracing::info!(
            "Stream config applied: encoder={:?}, bitrate={}",
            new_config.stream.encoder,
            new_config.stream.bitrate_preset
        );
    }

    // HID config processing - always reload if section was sent
    if has_hid {
        tracing::info!("HID config sent, reloading HID backend...");

        // Determine new backend type
        let new_hid_backend = match new_config.hid.backend {
            crate::config::HidBackend::Otg => crate::hid::HidBackendType::Otg,
            crate::config::HidBackend::Ch9329 => crate::hid::HidBackendType::Ch9329 {
                port: new_config.hid.ch9329_port.clone(),
                baud_rate: new_config.hid.ch9329_baudrate,
            },
            crate::config::HidBackend::None => crate::hid::HidBackendType::None,
        };

        // Reload HID backend - return success=false on error
        if let Err(e) = state.hid.reload(new_hid_backend).await {
            tracing::error!("HID reload failed: {}", e);
            // Rollback config on failure
            state.config.set((*old_config).clone()).await?;
            return Ok(Json(LoginResponse {
                success: false,
                message: Some(format!("HID configuration invalid: {}", e)),
            }));
        }

        tracing::info!(
            "HID backend reloaded successfully: {:?}",
            new_config.hid.backend
        );
    }

    // Audio config processing - always reload if section was sent
    if has_audio {
        tracing::info!("Audio config sent, applying settings...");

        // Create audio controller config from new config
        let audio_config = crate::audio::AudioControllerConfig {
            enabled: new_config.audio.enabled,
            device: new_config.audio.device.clone(),
            quality: crate::audio::AudioQuality::from_str(&new_config.audio.quality),
        };

        // Update audio controller
        if let Err(e) = state.audio.update_config(audio_config).await {
            tracing::error!("Audio config update failed: {}", e);
            // Don't rollback config for audio errors - it's not critical
            // Just log the error
        } else {
            tracing::info!(
                "Audio config applied: enabled={}, device={}",
                new_config.audio.enabled,
                new_config.audio.device
            );
        }

        // Also update WebRTC audio enabled state
        if let Err(e) = state
            .stream_manager
            .set_webrtc_audio_enabled(new_config.audio.enabled)
            .await
        {
            tracing::warn!("Failed to update WebRTC audio state: {}", e);
        } else {
            tracing::info!("WebRTC audio enabled: {}", new_config.audio.enabled);
        }

        // Reconnect audio sources for existing WebRTC sessions
        // This is needed because the audio controller was restarted with new config
        if new_config.audio.enabled {
            state.stream_manager.reconnect_webrtc_audio_sources().await;
        }
    }

    // MSD config processing - reload if enabled state or directory changed
    if has_msd {
        tracing::info!("MSD config sent, checking if reload needed...");
        tracing::debug!("Old MSD config: {:?}", old_config.msd);
        tracing::debug!("New MSD config: {:?}", new_config.msd);

        let old_msd_enabled = old_config.msd.enabled;
        let new_msd_enabled = new_config.msd.enabled;
        let msd_dir_changed = old_config.msd.msd_dir != new_config.msd.msd_dir;

        tracing::info!(
            "MSD enabled: old={}, new={}",
            old_msd_enabled,
            new_msd_enabled
        );
        if msd_dir_changed {
            tracing::info!("MSD directory changed: {}", new_config.msd.msd_dir);
        }

        // Ensure MSD directories exist (msd/images, msd/ventoy)
        let msd_dir = new_config.msd.msd_dir_path();
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
        } else if new_msd_enabled {
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

            let msd = crate::msd::MsdController::new(
                state.otg_service.clone(),
                new_config.msd.msd_dir_path(),
            );
            if let Err(e) = msd.init().await {
                tracing::error!("MSD initialization failed: {}", e);
                // Rollback config on failure
                state.config.set((*old_config).clone()).await?;
                return Ok(Json(LoginResponse {
                    success: false,
                    message: Some(format!("MSD initialization failed: {}", e)),
                }));
            }

            // Set event bus
            let events = state.events.clone();
            msd.set_event_bus(events).await;

            // Store the initialized controller
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
        }
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Configuration updated".to_string()),
    }))
}

fn merge_json(
    base: serde_json::Value,
    updates: serde_json::Value,
) -> std::result::Result<serde_json::Value, ()> {
    match (base, updates) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(updates)) => {
            for (key, value) in updates {
                if let Some(base_value) = base.get(&key).cloned() {
                    base.insert(key, merge_json(base_value, value)?);
                } else {
                    base.insert(key, value);
                }
            }
            Ok(serde_json::Value::Object(base))
        }
        (_, updates) => Ok(updates),
    }
}

// ============================================================================
// Devices
// ============================================================================

#[derive(Serialize)]
pub struct DeviceList {
    pub video: Vec<VideoDevice>,
    pub serial: Vec<SerialDevice>,
    pub audio: Vec<AudioDevice>,
    pub udc: Vec<UdcDevice>,
    pub extensions: ExtensionsAvailability,
}

#[derive(Serialize)]
pub struct ExtensionsAvailability {
    pub ttyd_available: bool,
    pub rustdesk_available: bool,
}

#[derive(Serialize)]
pub struct VideoDevice {
    pub path: String,
    pub name: String,
    pub driver: String,
    pub formats: Vec<VideoFormat>,
    pub usb_bus: Option<String>,
}

#[derive(Serialize)]
pub struct VideoFormat {
    pub format: String,
    pub description: String,
    pub resolutions: Vec<VideoResolution>,
}

#[derive(Serialize)]
pub struct VideoResolution {
    pub width: u32,
    pub height: u32,
    pub fps: Vec<u32>,
}

#[derive(Serialize)]
pub struct SerialDevice {
    pub path: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub is_hdmi: bool,
    pub usb_bus: Option<String>,
}

#[derive(Serialize)]
pub struct UdcDevice {
    pub name: String,
}

/// Extract USB bus port from V4L2 bus_info string
/// Examples:
/// - "usb-0000:00:14.0-1" -> Some("1")
/// - "usb-xhci-hcd.0-1.2" -> Some("1.2")
/// - "usb-0000:00:14.0-1.3.2" -> Some("1.3.2")
/// - "platform:..." -> None
fn extract_usb_bus_from_bus_info(bus_info: &str) -> Option<String> {
    if !bus_info.starts_with("usb-") {
        return None;
    }
    // Find the last '-' which separates the USB port
    // e.g., "usb-0000:00:14.0-1" -> "1"
    // e.g., "usb-xhci-hcd.0-1.2" -> "1.2"
    let parts: Vec<&str> = bus_info.rsplitn(2, '-').collect();
    if parts.len() == 2 {
        let port = parts[0];
        // Verify it looks like a USB port (starts with digit)
        if port
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return Some(port.to_string());
        }
    }
    None
}

pub async fn list_devices(State(state): State<Arc<AppState>>) -> Json<DeviceList> {
    // Detect video devices
    let video_devices = match state.stream_manager.list_devices().await {
        Ok(devices) => devices
            .into_iter()
            .map(|d| {
                // Extract USB bus from bus_info (e.g., "usb-0000:00:14.0-1" -> "1")
                // or "usb-xhci-hcd.0-1.2" -> "1.2"
                let usb_bus = extract_usb_bus_from_bus_info(&d.bus_info);
                VideoDevice {
                    path: d.path.to_string_lossy().to_string(),
                    name: d.name,
                    driver: d.driver,
                    formats: d
                        .formats
                        .iter()
                        .map(|f| VideoFormat {
                            format: format!("{}", f.format),
                            description: f.description.clone(),
                            resolutions: f
                                .resolutions
                                .iter()
                                .map(|r| VideoResolution {
                                    width: r.width,
                                    height: r.height,
                                    fps: r.fps.clone(),
                                })
                                .collect(),
                        })
                        .collect(),
                    usb_bus,
                }
            })
            .collect(),
        Err(_) => vec![],
    };

    // Detect serial devices (common USB/ACM ports) - single directory read
    let serial_prefixes = ["ttyUSB", "ttyACM", "ttyS"];
    let mut serial_devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = match file_name.to_str() {
                Some(n) => n,
                None => continue,
            };
            // Check if matches any prefix
            if serial_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
            {
                let path = entry.path();
                if let Some(p) = path.to_str() {
                    serial_devices.push(SerialDevice {
                        path: p.to_string(),
                        name: name.to_string(),
                    });
                }
            }
        }
    }
    serial_devices.sort_by(|a, b| a.path.cmp(&b.path));

    // Detect UDC (USB Device Controller) devices
    let mut udc_devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/udc") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                udc_devices.push(UdcDevice {
                    name: name.to_string(),
                });
            }
        }
    }
    udc_devices.sort_by(|a, b| a.name.cmp(&b.name));

    // Detect audio devices
    let audio_devices = match state.audio.list_devices().await {
        Ok(devices) => devices
            .into_iter()
            .map(|d| AudioDevice {
                name: d.name,
                description: d.description,
                is_hdmi: d.is_hdmi,
                usb_bus: d.usb_bus,
            })
            .collect(),
        Err(_) => vec![],
    };

    // Check extension availability
    let ttyd_available = state
        .extensions
        .check_available(crate::extensions::ExtensionId::Ttyd);

    Json(DeviceList {
        video: video_devices,
        serial: serial_devices,
        audio: audio_devices,
        udc: udc_devices,
        extensions: ExtensionsAvailability {
            ttyd_available,
            rustdesk_available: true, // RustDesk is built-in
        },
    })
}

// ============================================================================
// Stream Control
// ============================================================================

use crate::video::streamer::StreamerStats;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// Get stream state
pub async fn stream_state(State(state): State<Arc<AppState>>) -> Json<StreamerStats> {
    Json(state.stream_manager.stats().await)
}

/// Start streaming
pub async fn stream_start(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.stream_manager.start().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Streaming started".to_string()),
    }))
}

/// Stop streaming
pub async fn stream_stop(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.stream_manager.stop().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Streaming stopped".to_string()),
    }))
}

/// Stream mode request
#[derive(Deserialize)]
pub struct SetStreamModeRequest {
    /// Target mode: "mjpeg" or "webrtc"
    pub mode: String,
}

/// Stream mode response
#[derive(Serialize)]
pub struct StreamModeResponse {
    pub success: bool,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_id: Option<String>,
    pub switching: bool,
    pub message: Option<String>,
}

/// Get current stream mode
pub async fn stream_mode_get(State(state): State<Arc<AppState>>) -> Json<StreamModeResponse> {
    let mode = state.stream_manager.current_mode().await;
    let mode_str = match mode {
        StreamMode::Mjpeg => "mjpeg".to_string(),
        StreamMode::WebRTC => {
            use crate::video::encoder::VideoCodecType;
            let codec = state
                .stream_manager
                .webrtc_streamer()
                .current_video_codec()
                .await;
            match codec {
                VideoCodecType::H264 => "h264".to_string(),
                VideoCodecType::H265 => "h265".to_string(),
                VideoCodecType::VP8 => "vp8".to_string(),
                VideoCodecType::VP9 => "vp9".to_string(),
            }
        }
    };
    Json(StreamModeResponse {
        success: true,
        mode: mode_str,
        transition_id: state.stream_manager.current_transition_id().await,
        switching: state.stream_manager.is_switching(),
        message: None,
    })
}

/// Set stream mode (switch between MJPEG and WebRTC)
pub async fn stream_mode_set(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetStreamModeRequest>,
) -> Result<Json<StreamModeResponse>> {
    use crate::video::encoder::VideoCodecType;

    let mode_lower = req.mode.to_lowercase();
    let (new_mode, video_codec) = match mode_lower.as_str() {
        "mjpeg" => (StreamMode::Mjpeg, None),
        "webrtc" | "h264" => (StreamMode::WebRTC, Some(VideoCodecType::H264)),
        "h265" => (StreamMode::WebRTC, Some(VideoCodecType::H265)),
        "vp8" => (StreamMode::WebRTC, Some(VideoCodecType::VP8)),
        "vp9" => (StreamMode::WebRTC, Some(VideoCodecType::VP9)),
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid mode '{}'. Valid modes: mjpeg, h264, h265, vp8, vp9",
                req.mode
            )));
        }
    };

    // Set video codec if switching to WebRTC mode with specific codec
    if let Some(codec) = video_codec {
        info!("Setting WebRTC video codec to {:?}", codec);
        if let Err(e) = state
            .stream_manager
            .webrtc_streamer()
            .set_video_codec(codec)
            .await
        {
            warn!("Failed to set video codec: {}", e);
        }
    }

    let tx = state
        .stream_manager
        .switch_mode_transaction(new_mode.clone())
        .await?;

    // Return the requested codec identifier (for UI display). The actual active mode
    // may differ if the request was rejected due to an in-progress switch.
    let requested_mode_str = match (&new_mode, &video_codec) {
        (StreamMode::Mjpeg, _) => "mjpeg",
        (StreamMode::WebRTC, Some(VideoCodecType::H264)) => "h264",
        (StreamMode::WebRTC, Some(VideoCodecType::H265)) => "h265",
        (StreamMode::WebRTC, Some(VideoCodecType::VP8)) => "vp8",
        (StreamMode::WebRTC, Some(VideoCodecType::VP9)) => "vp9",
        (StreamMode::WebRTC, None) => "webrtc",
    };

    let active_mode_str = match state.stream_manager.current_mode().await {
        StreamMode::Mjpeg => "mjpeg".to_string(),
        StreamMode::WebRTC => {
            let codec = state
                .stream_manager
                .webrtc_streamer()
                .current_video_codec()
                .await;
            match codec {
                VideoCodecType::H264 => "h264".to_string(),
                VideoCodecType::H265 => "h265".to_string(),
                VideoCodecType::VP8 => "vp8".to_string(),
                VideoCodecType::VP9 => "vp9".to_string(),
            }
        }
    };

    let no_switch_needed = !tx.accepted && !tx.switching && tx.transition_id.is_none();
    Ok(Json(StreamModeResponse {
        success: tx.accepted || no_switch_needed,
        mode: if tx.accepted {
            requested_mode_str.to_string()
        } else {
            active_mode_str
        },
        transition_id: tx.transition_id,
        switching: tx.switching,
        message: Some(if tx.accepted {
            format!("Switching to {} mode", requested_mode_str)
        } else if tx.switching {
            "Mode switch already in progress".to_string()
        } else {
            "No switch needed".to_string()
        }),
    }))
}

/// Available video codec info
#[derive(Serialize)]
pub struct VideoCodecInfo {
    /// Codec identifier (mjpeg, h264, h265, vp8, vp9)
    pub id: String,
    /// Display name
    pub name: String,
    /// Protocol (http or webrtc)
    pub protocol: String,
    /// Whether hardware accelerated
    pub hardware: bool,
    /// Encoder backend name (e.g., "vaapi", "nvenc", "software")
    pub backend: Option<String>,
    /// Whether this codec is available
    pub available: bool,
}

/// Encoder backend info
#[derive(Serialize)]
pub struct EncoderBackendInfo {
    /// Backend identifier (vaapi, nvenc, qsv, amf, rkmpp, v4l2m2m, software)
    pub id: String,
    /// Display name
    pub name: String,
    /// Whether this is a hardware backend
    pub is_hardware: bool,
    /// Supported video formats (h264, h265, vp8, vp9)
    pub supported_formats: Vec<String>,
}

/// Available codecs response
#[derive(Serialize)]
pub struct AvailableCodecsResponse {
    pub success: bool,
    /// Available encoder backends
    pub backends: Vec<EncoderBackendInfo>,
    /// Available codecs (for backward compatibility)
    pub codecs: Vec<VideoCodecInfo>,
}

/// Set bitrate request
#[derive(Deserialize)]
pub struct SetBitrateRequest {
    pub bitrate_preset: BitratePreset,
}

/// Set stream bitrate (real-time adjustment)
pub async fn stream_set_bitrate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetBitrateRequest>,
) -> Result<Json<LoginResponse>> {
    // Update config
    state
        .config
        .update(|config| {
            config.stream.bitrate_preset = req.bitrate_preset;
        })
        .await?;

    // Apply to WebRTC streamer (real-time adjustment)
    if let Err(e) = state
        .stream_manager
        .webrtc_streamer()
        .set_bitrate_preset(req.bitrate_preset)
        .await
    {
        warn!("Failed to set bitrate dynamically: {}", e);
        // Don't fail the request - config is saved, will apply on next connection
    } else {
        info!("Bitrate updated to {}", req.bitrate_preset);
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Bitrate set to {}", req.bitrate_preset)),
    }))
}

/// Get available video codecs
pub async fn stream_codecs_list() -> Json<AvailableCodecsResponse> {
    use crate::video::encoder::registry::{EncoderRegistry, VideoEncoderType};

    let registry = EncoderRegistry::global();

    // Build backends list
    let mut backends = Vec::new();
    for backend in registry.available_backends() {
        let formats = registry.formats_for_backend(backend);
        let format_ids: Vec<String> = formats
            .iter()
            .map(|f| match f {
                VideoEncoderType::H264 => "h264",
                VideoEncoderType::H265 => "h265",
                VideoEncoderType::VP8 => "vp8",
                VideoEncoderType::VP9 => "vp9",
            })
            .map(String::from)
            .collect();

        backends.push(EncoderBackendInfo {
            id: format!("{:?}", backend).to_lowercase(),
            name: backend.display_name().to_string(),
            is_hardware: backend.is_hardware(),
            supported_formats: format_ids,
        });
    }

    // Build codecs list (for backward compatibility)
    let mut codecs = Vec::new();

    // MJPEG is always available (HTTP streaming)
    codecs.push(VideoCodecInfo {
        id: "mjpeg".to_string(),
        name: "MJPEG / HTTP".to_string(),
        protocol: "http".to_string(),
        hardware: false,
        backend: Some("software".to_string()),
        available: true,
    });

    // Check H264 availability (supports software fallback)
    let h264_encoder = registry.best_encoder(VideoEncoderType::H264, false);
    codecs.push(VideoCodecInfo {
        id: "h264".to_string(),
        name: "H.264 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: h264_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: h264_encoder.map(|e| e.backend.to_string()),
        available: h264_encoder.is_some(),
    });

    // Check H265 availability (now supports software too)
    let h265_encoder = registry.best_encoder(VideoEncoderType::H265, false);
    codecs.push(VideoCodecInfo {
        id: "h265".to_string(),
        name: "H.265 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: h265_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: h265_encoder.map(|e| e.backend.to_string()),
        available: h265_encoder.is_some(),
    });

    // Check VP8 availability (now supports software too)
    let vp8_encoder = registry.best_encoder(VideoEncoderType::VP8, false);
    codecs.push(VideoCodecInfo {
        id: "vp8".to_string(),
        name: "VP8 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: vp8_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: vp8_encoder.map(|e| e.backend.to_string()),
        available: vp8_encoder.is_some(),
    });

    // Check VP9 availability (now supports software too)
    let vp9_encoder = registry.best_encoder(VideoEncoderType::VP9, false);
    codecs.push(VideoCodecInfo {
        id: "vp9".to_string(),
        name: "VP9 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: vp9_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: vp9_encoder.map(|e| e.backend.to_string()),
        available: vp9_encoder.is_some(),
    });

    Json(AvailableCodecsResponse {
        success: true,
        backends,
        codecs,
    })
}

/// Query parameters for MJPEG stream
#[derive(Deserialize, Default)]
pub struct MjpegStreamQuery {
    /// Optional client ID (if not provided, a random UUID will be generated)
    pub client_id: Option<String>,
}

/// MJPEG stream endpoint
pub async fn mjpeg_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MjpegStreamQuery>,
) -> impl IntoResponse {
    // Check if MJPEG mode is active
    if !state.stream_manager.is_mjpeg_enabled().await {
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                r#"{"error":"MJPEG mode not active. Current mode is WebRTC."}"#,
            ))
            .unwrap();
    }

    // Check if config is being changed - reject new connections during config change
    if state.stream_manager.is_config_changing() {
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                r#"{"error":"Video configuration is being changed. Please retry shortly."}"#,
            ))
            .unwrap();
    }

    // Ensure stream is started (but not during config change)
    if !state.stream_manager.is_streaming().await && !state.stream_manager.is_config_changing() {
        if let Err(e) = state.stream_manager.start().await {
            tracing::error!("Failed to auto-start stream: {}", e);
        }
    }

    let handler = state.stream_manager.mjpeg_handler();

    // Use provided client ID or generate a new one
    let client_id = query
        .client_id
        .filter(|id| !id.is_empty() && id.len() <= 64) // Validate: non-empty, max 64 chars
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Create RAII guard - this will automatically register and unregister the client
    let guard = Arc::new(crate::stream::mjpeg::ClientGuard::new(
        client_id.clone(),
        handler.clone(),
    ));

    // Use bounded channel (capacity=1) to implement backpressure
    // This ensures record_frame_sent() is only called when the previous frame
    // has been successfully consumed by the HTTP client
    let (tx, mut rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);

    // Spawn background task to send frames to channel
    let guard_clone = guard.clone();
    let handler_clone = handler.clone();
    tokio::spawn(async move {
        let _guard = guard_clone; // Keep guard alive
        let mut notify_rx = handler_clone.subscribe();
        let mut last_seq = 0u64;
        let mut timeout_count = 0;

        // Send initial frame if available
        if let Some(frame) = handler_clone.current_frame() {
            if frame.is_valid_jpeg() {
                let data = create_mjpeg_part(frame.data());
                // send() blocks until receiver is ready (backpressure)
                if tx.send(data).await.is_ok() {
                    // FPS recording moved to async_stream after yield
                    last_seq = frame.sequence;
                } else {
                    return; // Receiver closed
                }
            }
        }

        loop {
            // Check if stream went offline (e.g., during config change)
            if !handler_clone.is_online() {
                break;
            }

            // Wait for new frame notification with timeout
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(5), notify_rx.recv()).await;

            match result {
                Ok(Ok(())) => {
                    // Check online status after receiving notification
                    // set_offline() sends a notification, so we need to check here
                    if !handler_clone.is_online() {
                        break;
                    }
                    timeout_count = 0;
                    if let Some(frame) = handler_clone.current_frame() {
                        // Use != instead of > to handle sequence reset when capturer restarts
                        // (e.g., after video config change, new capturer starts from seq=0)
                        if frame.sequence != last_seq && frame.is_valid_jpeg() {
                            let data = create_mjpeg_part(frame.data());
                            if tx.send(data).await.is_ok() {
                                last_seq = frame.sequence;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    break;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    // Receiver was too slow - skip missed frames and jump to latest
                    if !handler_clone.is_online() {
                        break;
                    }
                    timeout_count = 0;

                    if let Some(frame) = handler_clone.current_frame() {
                        if frame.is_valid_jpeg() {
                            // Send current frame immediately and reset sequence tracking
                            if tx.send(create_mjpeg_part(frame.data())).await.is_ok() {
                                last_seq = frame.sequence;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Timeout - check if still online
                    timeout_count += 1;
                    if timeout_count > 6 || !handler_clone.is_online() {
                        break;
                    }
                    // Send last frame again to keep connection alive
                    let Some(frame) = handler_clone.current_frame() else {
                        continue;
                    };

                    if frame.is_valid_jpeg() && tx.send(create_mjpeg_part(frame.data())).await.is_err() {
                        break;
                    }
                }
            }
        }

        // Guard is automatically dropped here
    });

    // Create stream that receives from channel
    // Record FPS after yield - this is closer to actual TCP send than tx.send()
    let handler_for_stream = handler.clone();
    let guard_for_stream = guard.clone();
    let body_stream = async_stream::stream! {
        // Consume from channel - this drives the backpressure
        while let Some(data) = rx.recv().await {
            yield Ok::<bytes::Bytes, std::io::Error>(data);
            // Record FPS after yield - data has been handed to Axum/hyper
            // This is closer to actual TCP send than recording at tx.send()
            handler_for_stream.record_frame_sent(guard_for_stream.id());
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

/// Single JPEG snapshot
pub async fn snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let handler = state.stream_manager.mjpeg_handler();

    match handler.current_frame() {
        Some(frame) if frame.is_valid_jpeg() => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(frame.data_bytes()))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("No frame available"))
            .unwrap(),
    }
}

/// Create MJPEG multipart frame bytes
fn create_mjpeg_part(jpeg_data: &[u8]) -> bytes::Bytes {
    use bytes::{BufMut, BytesMut};

    let mut buf = BytesMut::with_capacity(128 + jpeg_data.len());

    // Write boundary and headers
    buf.put_slice(b"--frame\r\n");
    buf.put_slice(b"Content-Type: image/jpeg\r\n");
    buf.put_slice(format!("Content-Length: {}\r\n", jpeg_data.len()).as_bytes());
    buf.put_slice(b"\r\n");

    // Write JPEG data
    buf.put_slice(jpeg_data);
    buf.put_slice(b"\r\n");

    buf.freeze()
}

// ============================================================================
// WebRTC
// ============================================================================

use crate::webrtc::signaling::{AnswerResponse, IceCandidateRequest, OfferRequest};

/// Create WebRTC session
#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
}

pub async fn webrtc_create_session(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CreateSessionResponse>> {
    // Check if WebRTC mode is active
    if !state.stream_manager.is_webrtc_enabled().await {
        return Err(AppError::ServiceUnavailable(
            "WebRTC mode not active. Current mode is MJPEG.".to_string(),
        ));
    }

    let session_id = state
        .stream_manager
        .webrtc_streamer()
        .create_session()
        .await?;
    Ok(Json(CreateSessionResponse { session_id }))
}

/// Handle WebRTC offer
pub async fn webrtc_offer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OfferRequest>,
) -> Result<Json<AnswerResponse>> {
    // Check if WebRTC mode is active
    if !state.stream_manager.is_webrtc_enabled().await {
        return Err(AppError::ServiceUnavailable(
            "WebRTC mode not active. Current mode is MJPEG.".to_string(),
        ));
    }

    // Create session if client_id not provided
    let webrtc = state.stream_manager.webrtc_streamer();
    let session_id = if let Some(client_id) = &req.client_id {
        // Check if session exists
        if webrtc.get_session(client_id).await.is_some() {
            client_id.clone()
        } else {
            webrtc.create_session().await?
        }
    } else {
        webrtc.create_session().await?
    };

    // Handle offer
    let offer = crate::webrtc::SdpOffer::new(req.sdp);
    let answer = webrtc.handle_offer(&session_id, offer).await?;

    Ok(Json(AnswerResponse::new(
        answer.sdp,
        session_id,
        answer.ice_candidates.unwrap_or_default(),
    )))
}

/// Add ICE candidate
pub async fn webrtc_ice_candidate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<Json<LoginResponse>> {
    state
        .stream_manager
        .webrtc_streamer()
        .add_ice_candidate(&req.session_id, req.candidate)
        .await?;

    Ok(Json(LoginResponse {
        success: true,
        message: None,
    }))
}

/// Get WebRTC session info
#[derive(Serialize)]
pub struct WebRtcSessionInfo {
    pub session_id: String,
    pub state: String,
}

#[derive(Serialize)]
pub struct WebRtcStatus {
    pub session_count: usize,
    pub sessions: Vec<WebRtcSessionInfo>,
}

pub async fn webrtc_status(State(state): State<Arc<AppState>>) -> Json<WebRtcStatus> {
    let sessions = state.stream_manager.webrtc_streamer().list_sessions().await;
    Json(WebRtcStatus {
        session_count: sessions.len(),
        sessions: sessions
            .into_iter()
            .map(|s| WebRtcSessionInfo {
                session_id: s.session_id,
                state: s.state,
            })
            .collect(),
    })
}

/// Close WebRTC session
#[derive(Deserialize)]
pub struct CloseSessionRequest {
    pub session_id: String,
}

pub async fn webrtc_close_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CloseSessionRequest>,
) -> Result<Json<LoginResponse>> {
    state
        .stream_manager
        .webrtc_streamer()
        .close_session(&req.session_id)
        .await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Session closed".to_string()),
    }))
}

/// ICE servers configuration for WebRTC
#[derive(Serialize)]
pub struct IceServersResponse {
    pub ice_servers: Vec<IceServerInfo>,
    pub mdns_mode: String,
}

#[derive(Serialize)]
pub struct IceServerInfo {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// Get ICE servers configuration for client-side WebRTC
/// Returns user-configured servers, or Google STUN as fallback if none configured
pub async fn webrtc_ice_servers(State(state): State<Arc<AppState>>) -> Json<IceServersResponse> {
    use crate::webrtc::config::public_ice;
    use crate::webrtc::mdns::{mdns_mode, mdns_mode_label};

    let config = state.config.get();
    let mut ice_servers = Vec::new();

    // Check if user has configured custom ICE servers
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

    if has_custom_stun || has_custom_turn {
        // Use user-configured ICE servers
        if let Some(ref stun) = config.stream.stun_server {
            if !stun.is_empty() {
                ice_servers.push(IceServerInfo {
                    urls: vec![stun.clone()],
                    username: None,
                    credential: None,
                });
            }
        }

        if let Some(ref turn) = config.stream.turn_server {
            if !turn.is_empty() {
                let username = config.stream.turn_username.clone();
                let credential = config.stream.turn_password.clone();
                if username.is_some() && credential.is_some() {
                    ice_servers.push(IceServerInfo {
                        urls: vec![turn.clone()],
                        username,
                        credential,
                    });
                }
            }
        }
    } else {
        // No custom servers configured - use Google STUN as default
        if let Some(stun) = public_ice::stun_server() {
            ice_servers.push(IceServerInfo {
                urls: vec![stun],
                username: None,
                credential: None,
            });
        }
        // Note: TURN servers are not provided - users must configure their own
    }

    let mdns_mode = mdns_mode();
    let mdns_mode = mdns_mode_label(mdns_mode).to_string();

    Json(IceServersResponse {
        ice_servers,
        mdns_mode,
    })
}

// ============================================================================
// HID Control
// ============================================================================

/// HID status response
#[derive(Serialize)]
pub struct HidStatus {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub supports_absolute_mouse: bool,
    pub screen_resolution: Option<(u32, u32)>,
}

/// Get HID status
pub async fn hid_status(State(state): State<Arc<AppState>>) -> Json<HidStatus> {
    let info = state.hid.info().await;
    Json(HidStatus {
        available: info.is_some(),
        backend: info
            .as_ref()
            .map(|i| i.name.to_string())
            .unwrap_or_else(|| "none".to_string()),
        initialized: info.as_ref().map(|i| i.initialized).unwrap_or(false),
        supports_absolute_mouse: info
            .as_ref()
            .map(|i| i.supports_absolute_mouse)
            .unwrap_or(false),
        screen_resolution: info.and_then(|i| i.screen_resolution),
    })
}

/// Reset HID state
pub async fn hid_reset(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.hid.reset().await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("HID state reset".to_string()),
    }))
}

// ============================================================================
// MSD (Mass Storage Device)
// ============================================================================

use crate::msd::{
    DownloadProgress, DriveFile, DriveInfo, DriveInitRequest, ImageDownloadRequest, ImageInfo,
    ImageManager, MsdConnectRequest, MsdMode, MsdState, VentoyDrive,
};
use axum::extract::{Multipart, Path as AxumPath, Query};
use std::collections::HashMap;

/// MSD status response
#[derive(Serialize)]
pub struct MsdStatus {
    pub available: bool,
    pub state: MsdState,
}

/// Get MSD status
pub async fn msd_status(State(state): State<Arc<AppState>>) -> Result<Json<MsdStatus>> {
    let msd_guard = state.msd.read().await;
    match msd_guard.as_ref() {
        Some(controller) => {
            let msd_state = controller.state().await;
            Ok(Json(MsdStatus {
                available: true,
                state: msd_state,
            }))
        }
        None => Ok(Json(MsdStatus {
            available: false,
            state: MsdState::default(),
        })),
    }
}

/// List all available images
pub async fn msd_images_list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<ImageInfo>>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    let images = manager.list()?;
    Ok(Json(images))
}

/// Upload new image (streaming - memory efficient for large files)
pub async fn msd_image_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ImageInfo>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
                .to_string();

            // Use streaming upload - chunks are written directly to disk
            // This avoids loading the entire file into memory
            let image = manager
                .create_from_multipart_field(&filename, field)
                .await?;
            return Ok(Json(image));
        }
    }

    Err(AppError::BadRequest("No file provided".to_string()))
}

/// Get image by ID
pub async fn msd_image_get(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ImageInfo>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    let image = manager.get(&id)?;
    Ok(Json(image))
}

/// Delete image by ID
pub async fn msd_image_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let images_path = config.msd.images_dir();
    let manager = ImageManager::new(images_path);

    manager.delete(&id)?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Image deleted".to_string()),
    }))
}

/// Download image from URL
pub async fn msd_image_download(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImageDownloadRequest>,
) -> Result<Json<DownloadProgress>> {
    let msd_guard = state.msd.read().await;
    let controller = msd_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    let progress = controller.download_image(req.url, req.filename).await?;

    Ok(Json(progress))
}

/// Cancel image download
#[derive(serde::Deserialize)]
pub struct CancelDownloadRequest {
    pub download_id: String,
}

pub async fn msd_image_download_cancel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CancelDownloadRequest>,
) -> Result<Json<LoginResponse>> {
    let msd_guard = state.msd.read().await;
    let controller = msd_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.cancel_download(&req.download_id).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Download cancelled".to_string()),
    }))
}

/// Connect MSD (image or drive)
pub async fn msd_connect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MsdConnectRequest>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    match req.mode {
        MsdMode::Image => {
            let image_id = req.image_id.ok_or_else(|| {
                AppError::BadRequest("image_id required for image mode".to_string())
            })?;

            // Get image info from ImageManager
            let images_path = config.msd.images_dir();
            let manager = ImageManager::new(images_path);
            let image = manager.get(&image_id)?;

            // Get mount options from request (defaults: cdrom=false, read_only=false)
            let cdrom = req.cdrom.unwrap_or(false);
            let read_only = req.read_only.unwrap_or(false);

            controller.connect_image(&image, cdrom, read_only).await?;
        }
        MsdMode::Drive => {
            controller.connect_drive().await?;
        }
        MsdMode::None => {
            return Err(AppError::BadRequest("Invalid mode: none".to_string()));
        }
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some("MSD connected".to_string()),
    }))
}

/// Disconnect MSD
pub async fn msd_disconnect(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    let mut msd_guard = state.msd.write().await;
    let controller = msd_guard
        .as_mut()
        .ok_or_else(|| AppError::Internal("MSD not initialized".to_string()))?;

    controller.disconnect().await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("MSD disconnected".to_string()),
    }))
}

// ============================================================================
// MSD Virtual Drive
// ============================================================================

/// Get drive info
pub async fn msd_drive_info(State(state): State<Arc<AppState>>) -> Result<Json<DriveInfo>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    if !drive.exists() {
        return Err(AppError::NotFound("Drive not initialized".to_string()));
    }

    let info = drive.info().await?;
    Ok(Json(info))
}

/// Initialize Ventoy drive
pub async fn msd_drive_init(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DriveInitRequest>,
) -> Result<Json<DriveInfo>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let info = drive.init(req.size_mb).await?;
    Ok(Json(info))
}

/// Delete virtual drive
pub async fn msd_drive_delete(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    let config = state.config.get();

    // Check if drive is currently connected
    let msd_guard = state.msd.write().await;
    if let Some(controller) = msd_guard.as_ref() {
        let msd_state = controller.state().await;
        if msd_state.connected && msd_state.mode == crate::msd::types::MsdMode::Drive {
            return Err(AppError::BadRequest(
                "Cannot delete drive while connected. Disconnect first.".to_string(),
            ));
        }
    }
    drop(msd_guard);

    // Delete the drive file
    let drive_path = config.msd.drive_path();
    if drive_path.exists() {
        std::fs::remove_file(&drive_path)
            .map_err(|e| AppError::Internal(format!("Failed to delete drive file: {}", e)))?;
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Virtual drive deleted".to_string()),
    }))
}

/// List drive files
pub async fn msd_drive_files(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<DriveFile>>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let dir_path = params.get("path").map(|s| s.as_str()).unwrap_or("/");
    let files = drive.list_files(dir_path).await?;
    Ok(Json(files))
}

/// Upload file to drive (streaming - memory efficient for large files)
pub async fn msd_drive_upload(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    let target_dir = params.get("path").map(|s| s.as_str()).unwrap_or("/");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| AppError::BadRequest("Missing filename".to_string()))?
                .to_string();

            let file_path = if target_dir == "/" {
                format!("/{}", filename)
            } else {
                format!("{}/{}", target_dir.trim_end_matches('/'), filename)
            };

            // Use streaming upload - chunks are written directly to disk
            // This avoids loading the entire file into memory
            drive
                .write_file_from_multipart_field(&file_path, field)
                .await?;

            return Ok(Json(LoginResponse {
                success: true,
                message: Some(format!("File uploaded: {}", file_path)),
            }));
        }
    }

    Err(AppError::BadRequest("No file provided".to_string()))
}

/// Download file from drive (streaming for large files)
pub async fn msd_drive_download(
    State(state): State<Arc<AppState>>,
    AxumPath(file_path): AxumPath<String>,
) -> Result<Response> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    // Get file stream (returns file size and channel receiver)
    let (file_size, mut rx) = drive.read_file_stream(&file_path).await?;

    // Extract filename for Content-Disposition
    let filename = file_path.split('/').next_back().unwrap_or("download");

    // Create a stream from the channel receiver
    let body_stream = async_stream::stream! {
        while let Some(chunk) = rx.recv().await {
            yield chunk;
        }
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file_size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from_stream(body_stream))
        .unwrap())
}

/// Delete file from drive
pub async fn msd_drive_file_delete(
    State(state): State<Arc<AppState>>,
    AxumPath(file_path): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    drive.delete(&file_path).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Deleted: {}", file_path)),
    }))
}

/// Create directory in drive
pub async fn msd_drive_mkdir(
    State(state): State<Arc<AppState>>,
    AxumPath(dir_path): AxumPath<String>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();
    let drive_path = config.msd.drive_path();
    let drive = VentoyDrive::new(drive_path);

    drive.mkdir(&dir_path).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Directory created: {}", dir_path)),
    }))
}

// ============================================================================
// ATX (Power Control)
// ============================================================================

use crate::atx::{AtxState, PowerStatus};

/// ATX state response
#[derive(Serialize)]
pub struct AtxStateResponse {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub power_status: String,
    pub led_supported: bool,
}

impl From<AtxState> for AtxStateResponse {
    fn from(state: AtxState) -> Self {
        Self {
            available: state.available,
            backend: if state.power_configured || state.reset_configured {
                format!(
                    "power: {}, reset: {}",
                    if state.power_configured { "yes" } else { "no" },
                    if state.reset_configured { "yes" } else { "no" }
                )
            } else {
                "none".to_string()
            },
            initialized: state.power_configured || state.reset_configured,
            power_status: match state.power_status {
                PowerStatus::On => "on".to_string(),
                PowerStatus::Off => "off".to_string(),
                PowerStatus::Unknown => "unknown".to_string(),
            },
            led_supported: state.led_supported,
        }
    }
}

/// Get ATX status
pub async fn atx_status(State(state): State<Arc<AppState>>) -> Result<Json<AtxStateResponse>> {
    let atx_guard = state.atx.read().await;

    match atx_guard.as_ref() {
        Some(atx) => {
            let atx_state = atx.state().await;
            Ok(Json(AtxStateResponse::from(atx_state)))
        }
        None => Ok(Json(AtxStateResponse {
            available: false,
            backend: "none".to_string(),
            initialized: false,
            power_status: "unknown".to_string(),
            led_supported: false,
        })),
    }
}

/// ATX power control request
#[derive(Deserialize)]
pub struct AtxPowerControlRequest {
    pub action: String, // "short", "long", "reset"
}

/// Control ATX power
pub async fn atx_power(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AtxPowerControlRequest>,
) -> Result<Json<LoginResponse>> {
    let atx_guard = state.atx.read().await;
    let atx = atx_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("ATX controller not initialized".to_string()))?;

    match req.action.as_str() {
        "short" => {
            atx.power_short().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power short press executed".to_string()),
            }))
        }
        "long" => {
            atx.power_long().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power long press (force off) executed".to_string()),
            }))
        }
        "reset" => {
            atx.reset().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Reset button pressed".to_string()),
            }))
        }
        _ => Err(AppError::BadRequest(format!(
            "Unknown ATX action: {}. Valid actions: short, long, reset",
            req.action
        ))),
    }
}

/// WOL request body
#[derive(Debug, Deserialize)]
pub struct WolRequest {
    /// Target MAC address (e.g., "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF")
    pub mac_address: String,
}

/// Send Wake-on-LAN magic packet
pub async fn atx_wol(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WolRequest>,
) -> Result<Json<LoginResponse>> {
    // Get WOL interface from config
    let config = state.config.get();
    let interface = if config.atx.wol_interface.is_empty() {
        None
    } else {
        Some(config.atx.wol_interface.as_str())
    };

    // Send WOL packet
    crate::atx::send_wol(&req.mac_address, interface)?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("WOL packet sent to {}", req.mac_address)),
    }))
}

// ============================================================================
// Audio Control
// ============================================================================

use crate::audio::{AudioQuality, AudioStatus};

/// Audio status response (re-exports AudioStatus from audio module)
pub type AudioStatusResponse = AudioStatus;

/// Get audio status
pub async fn audio_status(State(state): State<Arc<AppState>>) -> Json<AudioStatusResponse> {
    Json(state.audio.status().await)
}

/// Start audio streaming
pub async fn start_audio_streaming(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LoginResponse>> {
    state.audio.start_streaming().await?;

    // Reconnect audio sources for existing WebRTC sessions
    // This ensures sessions created before audio was enabled will receive audio
    state.stream_manager.reconnect_webrtc_audio_sources().await;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Audio streaming started".to_string()),
    }))
}

/// Stop audio streaming
pub async fn stop_audio_streaming(
    State(state): State<Arc<AppState>>,
) -> Result<Json<LoginResponse>> {
    state.audio.stop_streaming().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Audio streaming stopped".to_string()),
    }))
}

/// Set audio quality request
#[derive(Deserialize)]
pub struct SetAudioQualityRequest {
    pub quality: String,
}

/// Set audio quality
pub async fn set_audio_quality(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetAudioQualityRequest>,
) -> Result<Json<LoginResponse>> {
    let quality = AudioQuality::from_str(&req.quality);
    state.audio.set_quality(quality).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Audio quality set to {}", quality)),
    }))
}

/// Select audio device request
#[derive(Deserialize)]
pub struct SelectAudioDeviceRequest {
    pub device: String,
}

/// Select audio device
pub async fn select_audio_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SelectAudioDeviceRequest>,
) -> Result<Json<LoginResponse>> {
    state.audio.select_device(&req.device).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Audio device selected: {}", req.device)),
    }))
}

/// List audio devices
pub async fn list_audio_devices(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::audio::AudioDeviceInfo>>> {
    let devices = state.audio.list_devices().await?;
    Ok(Json(devices))
}

// ============================================================================
// Password Management
// ============================================================================

/// Change password request
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Change current user's password
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<LoginResponse>> {
    let current_user = state
        .users
        .get(&session.user_id)
        .await?
        .ok_or_else(|| AppError::AuthError("User not found".to_string()))?;

    if req.new_password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".to_string(),
        ));
    }

    let verified = state
        .users
        .verify(&current_user.username, &req.current_password)
        .await?;
    if verified.is_none() {
        return Err(AppError::AuthError(
            "Current password is incorrect".to_string(),
        ));
    }

    state
        .users
        .update_password(&session.user_id, &req.new_password)
        .await?;
    info!("Password changed for user ID: {}", session.user_id);

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Password changed successfully".to_string()),
    }))
}

/// Change username request
#[derive(Deserialize)]
pub struct ChangeUsernameRequest {
    pub username: String,
    pub current_password: String,
}

/// Change current user's username
pub async fn change_username(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ChangeUsernameRequest>,
) -> Result<Json<LoginResponse>> {
    let current_user = state
        .users
        .get(&session.user_id)
        .await?
        .ok_or_else(|| AppError::AuthError("User not found".to_string()))?;

    if req.username.len() < 2 {
        return Err(AppError::BadRequest(
            "Username must be at least 2 characters".to_string(),
        ));
    }

    let verified = state
        .users
        .verify(&current_user.username, &req.current_password)
        .await?;
    if verified.is_none() {
        return Err(AppError::AuthError(
            "Current password is incorrect".to_string(),
        ));
    }

    if current_user.username != req.username {
        state
            .users
            .update_username(&session.user_id, &req.username)
            .await?;
    }
    info!("Username changed for user ID: {}", session.user_id);

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Username changed successfully".to_string()),
    }))
}

// ============================================================================
// System Control
// ============================================================================

/// Restart the application
pub async fn system_restart(State(state): State<Arc<AppState>>) -> Json<LoginResponse> {
    info!("System restart requested via API");

    // Send shutdown signal
    let _ = state.shutdown_tx.send(());

    // Spawn restart task in background
    tokio::spawn(async {
        // Wait for resources to be released (OTG, video, etc.)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Get current executable and args
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to get current exe: {}", e);
                std::process::exit(1);
            }
        };
        let args: Vec<String> = std::env::args().skip(1).collect();

        info!("Restarting: {:?} {:?}", exe, args);

        // Use exec to replace current process (Unix)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let err = std::process::Command::new(&exe).args(&args).exec();
            tracing::error!("Failed to restart: {}", err);
            std::process::exit(1);
        }

        #[cfg(not(unix))]
        {
            let _ = std::process::Command::new(&exe).args(&args).spawn();
            std::process::exit(0);
        }
    });

    Json(LoginResponse {
        success: true,
        message: Some("Restarting...".to_string()),
    })
}
