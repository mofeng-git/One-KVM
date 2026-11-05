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
    pub control_mode: crate::video::device::VideoControlMode,
    pub input_status: crate::video::device::VideoInputStatus,
}

#[derive(Deserialize)]
pub struct VideoInputStatusQuery {
    pub device: String,
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
                    control_mode: d.control_mode,
                    input_status: d.input_status,
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

#[cfg(unix)]
fn validated_video_node(path: &str, sysfs_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(path);
    let name = path.file_name()?.to_str()?;
    if path.parent() != Some(std::path::Path::new("/dev"))
        || !name.starts_with("video")
        || name.len() == "video".len()
        || !name["video".len()..].chars().all(|c| c.is_ascii_digit())
        || !sysfs_root.join(name).exists()
    {
        return None;
    }
    Some(path.to_path_buf())
}

pub async fn video_input_status(
    Query(query): Query<VideoInputStatusQuery>,
) -> Result<Json<crate::video::device::VideoInputStatus>> {
    #[cfg(unix)]
    let path = validated_video_node(
        &query.device,
        std::path::Path::new("/sys/class/video4linux"),
    )
    .ok_or_else(|| AppError::BadRequest("Invalid video device".to_string()))?;

    #[cfg(windows)]
    let path = crate::video::device::enumerate_devices()?
        .into_iter()
        .find(|device| device.path.to_string_lossy() == query.device)
        .map(|device| device.path)
        .ok_or_else(|| AppError::BadRequest("Invalid video device".to_string()))?;

    let probe_path = path.clone();
    let status = tokio::task::spawn_blocking(move || {
        crate::video::device::VideoDevice::open_readonly(&probe_path)
            .and_then(|device| device.input_status())
    })
    .await
    .ok()
    .and_then(|result| result.ok())
    .unwrap_or_else(|| {
        debug!(device = %path.display(), "Unable to read video input status");
        crate::video::device::VideoInputStatus::unavailable()
    });

    Ok(Json(status))
}

#[cfg(all(test, unix))]
mod tests {
    use super::validated_video_node;

    #[test]
    fn only_accepts_dev_video_nodes_present_in_sysfs() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("video7")).unwrap();

        assert_eq!(
            validated_video_node("/dev/video7", root.path()).unwrap(),
            std::path::PathBuf::from("/dev/video7")
        );
        assert!(validated_video_node("/dev/video8", root.path()).is_none());
        assert!(validated_video_node("/tmp/video7", root.path()).is_none());
        assert!(validated_video_node("/dev/video7/../mem", root.path()).is_none());
        assert!(validated_video_node("/dev/video", root.path()).is_none());
    }
}
