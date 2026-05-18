use super::*;

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
    pub has_signal: bool,
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
    pub fps: Vec<f64>,
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
    let platform = PlatformCapabilities::current();

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
                    has_signal: d.has_signal,
                }
            })
            .collect(),
        Err(e) => {
            warn!(error = %e, "Video device enumeration failed; returning empty video list for /api/devices");
            vec![]
        }
    };

    let serial_devices = list_serial_ports()
        .into_iter()
        .map(|path| SerialDevice {
            name: std::path::Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&path)
                .to_string(),
            path,
        })
        .collect();

    #[cfg(unix)]
    let udc_devices = crate::otg::list_udc_devices()
        .into_iter()
        .map(|name| UdcDevice { name })
        .collect();
    #[cfg(not(unix))]
    let udc_devices = Vec::new();

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
            rustdesk_available: platform.rustdesk.available,
        },
    })
}
