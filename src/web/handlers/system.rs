use super::*;

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
    pub platform: PlatformCapabilities,
    pub capabilities: Capabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_space: Option<DiskSpaceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_info: Option<DeviceInfo>,
}

#[derive(Serialize)]
pub struct Capabilities {
    pub video: CapabilityInfo,
    pub hid: CapabilityInfo,
    pub msd: CapabilityInfo,
    pub atx: CapabilityInfo,
    pub audio: CapabilityInfo,
    pub rustdesk: CapabilityInfo,
    pub vnc: CapabilityInfo,
}

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub available: bool,
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub async fn system_info(State(state): State<Arc<AppState>>) -> Json<SystemInfo> {
    let config = state.config.get();
    let platform = PlatformCapabilities::current();

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
        platform: platform.clone(),
        capabilities: Capabilities {
            video: CapabilityInfo {
                available: config.video.device.is_some(),
                backend: config.video.device.clone(),
                reason: None,
            },
            hid: CapabilityInfo {
                available: config.hid.backend != crate::config::HidBackend::None,
                backend: Some(format!("{:?}", config.hid.backend)),
                reason: None,
            },
            msd: CapabilityInfo {
                available: config.msd.enabled && platform.msd.available,
                backend: None,
                reason: platform.msd.reason.clone(),
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
                reason: None,
            },
            audio: CapabilityInfo {
                available: config.audio.enabled && platform.audio.available,
                backend: Some(config.audio.device.clone()),
                reason: platform.audio.reason.clone(),
            },
            rustdesk: CapabilityInfo {
                available: config.rustdesk.enabled && platform.rustdesk.available,
                backend: platform.rustdesk.selected_backend.clone(),
                reason: platform.rustdesk.reason.clone(),
            },
            vnc: CapabilityInfo {
                available: config.vnc.enabled && platform.vnc.available,
                backend: platform.vnc.selected_backend.clone(),
                reason: platform.vnc.reason.clone(),
            },
        },
        disk_space,
        device_info,
    })
}
