use crate::config::AppConfig;
#[cfg(windows)]
use crate::config::AtxDriverType;
#[cfg(windows)]
use crate::config::HidBackend;

pub fn apply(config: &mut AppConfig) {
    #[cfg(not(windows))]
    {
        let _ = config;
    }

    #[cfg(windows)]
    {
        apply_windows(config);
    }
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
