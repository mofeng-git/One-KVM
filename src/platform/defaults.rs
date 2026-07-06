use crate::config::AppConfig;
#[cfg(windows)]
use crate::config::AtxDriverType;
#[cfg(any(windows, all(unix, feature = "android")))]
use crate::config::HidBackend;

pub fn apply(config: &mut AppConfig) {
    #[cfg(not(any(windows, all(unix, feature = "android"))))]
    {
        let _ = config;
    }

    #[cfg(all(unix, feature = "android"))]
    {
        apply_android(config);
    }

    #[cfg(windows)]
    {
        apply_windows(config);
    }
}

#[cfg(all(unix, feature = "android"))]
fn apply_android(config: &mut AppConfig) {
    let detected_udc = crate::otg::configfs::find_udc();
    if config
        .hid
        .otg_udc
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.hid.otg_udc = detected_udc;
    }

    let otg_available = config.hid.otg_udc.is_some();
    if !config.initialized && otg_available {
        config.hid.backend = HidBackend::Otg;
    } else if config.hid.backend == HidBackend::Ch9329
        && config.hid.ch9329_port == "/dev/ttyUSB0"
        && !std::path::Path::new(&config.hid.ch9329_port).exists()
        && otg_available
    {
        config.hid.backend = HidBackend::Otg;
    }

    if !config.initialized {
        config.audio.enabled = false;
        config.audio.device.clear();
        config.atx.enabled = false;
        config.rustdesk.enabled = false;
        config.rtsp.enabled = false;
        config.redfish.enabled = false;
    }

    config
        .video
        .device
        .get_or_insert_with(|| "auto".to_string());
    config
        .video
        .format
        .get_or_insert_with(|| "MJPEG".to_string());
    config.web.bind_address = "0.0.0.0".to_string();
    config.web.bind_addresses = vec!["0.0.0.0".to_string()];
}

#[cfg(windows)]
fn apply_windows(config: &mut AppConfig) {
    config.msd.enabled = false;
    config.hid.otg_udc = None;
    if config.hid.backend == HidBackend::Otg {
        config.hid.backend = HidBackend::None;
    }
    if config.hid.ch9329_port == "/dev/ttyUSB0" {
        config.hid.ch9329_port = "COM3".to_string();
    }
    if !config.initialized {
        config.audio.enabled = false;
        config.audio.device.clear();
    }

    if matches!(
        config.atx.driver,
        AtxDriverType::Gpio | AtxDriverType::UsbRelay
    ) {
        config.atx.driver = AtxDriverType::None;
        config.atx.enabled = false;
    }
    if !config.initialized
        && config.atx.driver == AtxDriverType::None
        && config.atx.device.is_empty()
    {
        config.atx.driver = AtxDriverType::Serial;
        config.atx.device = "COM4".to_string();
        config.atx.baud_rate = 9600;
        config.atx.power.enabled = true;
        config.atx.power.pin = 1;
        config.atx.reset.enabled = true;
        config.atx.reset.pin = 2;
    }

    config
        .video
        .device
        .get_or_insert_with(|| "auto".to_string());
    config
        .video
        .format
        .get_or_insert_with(|| "MJPEG".to_string());
}
