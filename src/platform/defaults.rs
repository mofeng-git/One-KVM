use crate::config::{AppConfig, AtxDriverType, HidBackend};

pub fn apply(config: &mut AppConfig) {
    if cfg!(windows) {
        apply_windows(config);
    }
}

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
        config.atx.power.driver,
        AtxDriverType::Gpio | AtxDriverType::UsbRelay
    ) {
        config.atx.power.driver = AtxDriverType::None;
    }
    if matches!(
        config.atx.reset.driver,
        AtxDriverType::Gpio | AtxDriverType::UsbRelay
    ) {
        config.atx.reset.driver = AtxDriverType::None;
    }
    if !config.initialized
        && config.atx.power.driver == AtxDriverType::None
        && config.atx.power.device.is_empty()
    {
        config.atx.power.driver = AtxDriverType::Serial;
        config.atx.power.device = "COM4".to_string();
        config.atx.power.pin = 1;
        config.atx.power.baud_rate = 9600;
    }
    if !config.initialized
        && config.atx.reset.driver == AtxDriverType::None
        && config.atx.reset.device.is_empty()
    {
        config.atx.reset.driver = AtxDriverType::Serial;
        config.atx.reset.device = "COM4".to_string();
        config.atx.reset.pin = 2;
        config.atx.reset.baud_rate = 9600;
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
