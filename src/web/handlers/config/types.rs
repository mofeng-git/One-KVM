use crate::config::*;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::path::Path;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct WatchdogConfigUpdate {
    pub enabled: bool,
}

#[typeshare]
#[derive(Debug, Serialize)]
pub struct WatchdogConfigResponse {
    pub enabled: bool,
    pub supported: bool,
    pub running: bool,
    pub reason: Option<String>,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AuthConfigUpdate {
    pub single_user_allow_multiple_sessions: Option<bool>,
}

impl AuthConfigUpdate {
    pub fn apply_to(&self, config: &mut AuthConfig) {
        if let Some(allow_multiple) = self.single_user_allow_multiple_sessions {
            config.single_user_allow_multiple_sessions = allow_multiple;
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct VideoConfigUpdate {
    pub device: Option<String>,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Option<u32>,
}

impl VideoConfigUpdate {
    pub fn ignore_source_following_parameters(&mut self) {
        if self.format.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.fps.is_some()
        {
            tracing::debug!(
                "Ignoring client-supplied format, resolution, and FPS for source-following video input"
            );
        }
        self.format = None;
        self.width = None;
        self.height = None;
        self.fps = None;
    }

    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(width) = self.width {
            if !(320..=7680).contains(&width) {
                return Err(AppError::BadRequest(
                    "Invalid width: must be 320-7680".into(),
                ));
            }
        }
        if let Some(height) = self.height {
            if !(240..=4320).contains(&height) {
                return Err(AppError::BadRequest(
                    "Invalid height: must be 240-4320".into(),
                ));
            }
        }
        if let Some(fps) = self.fps {
            if !(1..=120).contains(&fps) {
                return Err(AppError::BadRequest("Invalid fps: must be 1-120".into()));
            }
        }
        if let Some(quality) = self.quality {
            if !(1..=100).contains(&quality) {
                return Err(AppError::BadRequest(
                    "Invalid quality: must be 1-100".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut VideoConfig) {
        if let Some(ref device) = self.device {
            config.device = Some(device.clone());
        }
        if let Some(ref format) = self.format {
            config.format = Some(format.clone());
        }
        if let Some(width) = self.width {
            config.width = width;
        }
        if let Some(height) = self.height {
            config.height = height;
        }
        if let Some(fps) = self.fps {
            config.fps = fps;
        }
        if let Some(quality) = self.quality {
            config.quality = quality;
        }
    }
}

#[cfg(test)]
mod video_config_update_tests {
    use super::VideoConfigUpdate;

    #[test]
    fn source_following_parameters_are_silently_discarded() {
        let mut update = VideoConfigUpdate {
            device: Some("/dev/video0".to_string()),
            format: Some("MJPEG".to_string()),
            width: Some(7680),
            height: Some(4320),
            fps: Some(120),
            quality: Some(90),
        };
        update.ignore_source_following_parameters();

        assert_eq!(update.device.as_deref(), Some("/dev/video0"));
        assert!(update.format.is_none());
        assert!(update.width.is_none());
        assert!(update.height.is_none());
        assert!(update.fps.is_none());
        assert_eq!(update.quality, Some(90));
        assert!(update.validate().is_ok());
    }
}

/// Stream configuration response
#[typeshare]
#[derive(Debug, serde::Serialize)]
pub struct StreamConfigResponse {
    pub mode: StreamMode,
    pub keep_capture_alive: bool,
    pub encoder: EncoderType,
    pub bitrate_preset: BitratePreset,
    /// Whether public ICE servers are available (compile-time decision)
    pub has_public_ice_servers: bool,
    /// Whether public ICE servers are currently in use (when STUN/TURN are unset)
    pub using_public_ice_servers: bool,
    pub stun_server: Option<String>,
    pub turn_server: Option<String>,
    pub turn_username: Option<String>,
    pub turn_password: Option<String>,
}

impl From<&StreamConfig> for StreamConfigResponse {
    fn from(config: &StreamConfig) -> Self {
        use crate::webrtc::config::public_ice;
        Self {
            mode: config.mode.clone(),
            keep_capture_alive: config.keep_capture_alive,
            encoder: config.encoder.clone(),
            bitrate_preset: config.bitrate_preset,
            has_public_ice_servers: public_ice::is_configured(),
            using_public_ice_servers: config.is_using_public_ice_servers(),
            stun_server: config.stun_server.clone(),
            turn_server: config.turn_server.clone(),
            turn_username: config.turn_username.clone(),
            turn_password: config.turn_password.clone(),
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct StreamConfigUpdate {
    pub mode: Option<StreamMode>,
    pub keep_capture_alive: Option<bool>,
    pub encoder: Option<EncoderType>,
    pub bitrate_preset: Option<BitratePreset>,
    /// STUN server URL (e.g., "stun:stun.l.google.com:19302")
    /// Leave empty to use public ICE servers
    pub stun_server: Option<String>,
    /// TURN server URL (e.g., "turn:turn.example.com:3478")
    /// Leave empty to use public ICE servers
    pub turn_server: Option<String>,
    /// TURN username
    pub turn_username: Option<String>,
    /// TURN password
    pub turn_password: Option<String>,
}

impl StreamConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        // BitratePreset is always valid (enum)
        // Validate STUN server format
        if let Some(ref stun) = self.stun_server {
            if !stun.is_empty() && !stun.starts_with("stun:") {
                return Err(AppError::BadRequest(
                    "STUN server must start with 'stun:' (e.g., stun:stun.l.google.com:19302)"
                        .into(),
                ));
            }
        }
        // Validate TURN server format
        if let Some(ref turn) = self.turn_server {
            if !turn.is_empty() && !turn.starts_with("turn:") && !turn.starts_with("turns:") {
                return Err(AppError::BadRequest(
                    "TURN server must start with 'turn:' or 'turns:' (e.g., turn:turn.example.com:3478)".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut StreamConfig) {
        if let Some(enabled) = self.keep_capture_alive {
            config.keep_capture_alive = enabled;
        }
        if let Some(mode) = self.mode.clone() {
            config.mode = mode;
        }
        if let Some(encoder) = self.encoder.clone() {
            config.encoder = encoder;
        }
        if let Some(preset) = self.bitrate_preset {
            config.bitrate_preset = preset;
        }
        // STUN/TURN settings - empty string means clear (use public servers), Some("value") means set custom
        if let Some(ref stun) = self.stun_server {
            config.stun_server = if stun.is_empty() {
                None
            } else {
                Some(stun.clone())
            };
        }
        if let Some(ref turn) = self.turn_server {
            config.turn_server = if turn.is_empty() {
                None
            } else {
                Some(turn.clone())
            };
        }
        if let Some(ref username) = self.turn_username {
            config.turn_username = if username.is_empty() {
                None
            } else {
                Some(username.clone())
            };
        }
        if let Some(ref password) = self.turn_password {
            config.turn_password = if password.is_empty() {
                None
            } else {
                Some(password.clone())
            };
        }
    }
}

/// OTG USB device descriptor configuration update
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct OtgDescriptorConfigUpdate {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

impl OtgDescriptorConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate manufacturer string length
        if let Some(ref s) = self.manufacturer {
            if s.len() > 126 {
                return Err(AppError::BadRequest(
                    "Manufacturer string too long (max 126 chars)".into(),
                ));
            }
        }
        // Validate product string length
        if let Some(ref s) = self.product {
            if s.len() > 126 {
                return Err(AppError::BadRequest(
                    "Product string too long (max 126 chars)".into(),
                ));
            }
        }
        // Validate serial number string length
        if let Some(ref s) = self.serial_number {
            if s.len() > 126 {
                return Err(AppError::BadRequest(
                    "Serial number string too long (max 126 chars)".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut crate::config::OtgDescriptorConfig) {
        if let Some(v) = self.vendor_id {
            config.vendor_id = v;
        }
        if let Some(v) = self.product_id {
            config.product_id = v;
        }
        if let Some(ref v) = self.manufacturer {
            config.manufacturer = v.clone();
        }
        if let Some(ref v) = self.product {
            config.product = v.clone();
        }
        if let Some(ref v) = self.serial_number {
            config.serial_number = Some(v.clone());
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct OtgHidFunctionsUpdate {
    pub keyboard: Option<bool>,
    pub mouse_relative: Option<bool>,
    pub mouse_absolute: Option<bool>,
    pub consumer: Option<bool>,
}

impl OtgHidFunctionsUpdate {
    pub fn apply_to(&self, config: &mut OtgHidFunctions) {
        if let Some(enabled) = self.keyboard {
            config.keyboard = enabled;
        }
        if let Some(enabled) = self.mouse_relative {
            config.mouse_relative = enabled;
        }
        if let Some(enabled) = self.mouse_absolute {
            config.mouse_absolute = enabled;
        }
        if let Some(enabled) = self.consumer {
            config.consumer = enabled;
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct Ch9329DescriptorConfigUpdate {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

impl Ch9329DescriptorConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        Self::validate_optional_string("Manufacturer", self.manufacturer.as_deref())?;
        Self::validate_optional_string("Product", self.product.as_deref())?;
        Self::validate_optional_string("Serial number", self.serial_number.as_deref())?;
        Ok(())
    }

    fn validate_optional_string(label: &str, value: Option<&str>) -> crate::error::Result<()> {
        if let Some(value) = value {
            if value.as_bytes().len() > 23 {
                return Err(AppError::BadRequest(format!(
                    "{} string too long (max 23 bytes for CH9329)",
                    label
                )));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut Ch9329DescriptorConfig) {
        if let Some(v) = self.vendor_id {
            config.vendor_id = v;
        }
        if let Some(v) = self.product_id {
            config.product_id = v;
        }
        if let Some(ref v) = self.manufacturer {
            config.manufacturer = v.clone();
        }
        if let Some(ref v) = self.product {
            config.product = v.clone();
        }
        if let Some(ref v) = self.serial_number {
            config.serial_number = if v.is_empty() { None } else { Some(v.clone()) };
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct MouseJigglerConfigUpdate {
    pub enabled: Option<bool>,
    pub interval_secs: Option<u32>,
}

impl MouseJigglerConfigUpdate {
    fn apply_to(&self, config: &mut MouseJigglerConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(interval) = self.interval_secs {
            config.interval_secs = interval;
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct HidConfigUpdate {
    pub jiggler: Option<MouseJigglerConfigUpdate>,
    /// Request-only; never persisted or replayed during startup.
    pub bluetooth_reset_pairing: Option<bool>,
    pub bluetooth: Option<crate::config::BluetoothHidConfig>,
    pub backend: Option<HidBackend>,
    pub ch9329_port: Option<String>,
    pub ch9329_baudrate: Option<u32>,
    pub ch9329_hybrid_mouse: Option<bool>,
    #[serde(alias = "ch9329_macos_drag")]
    pub mouse_macos_drag: Option<bool>,
    pub ch9329_descriptor: Option<Ch9329DescriptorConfigUpdate>,
    pub otg_udc: Option<String>,
    pub otg_descriptor: Option<OtgDescriptorConfigUpdate>,
    pub otg_profile: Option<OtgHidProfile>,
    pub otg_functions: Option<OtgHidFunctionsUpdate>,
    pub otg_keyboard_leds: Option<bool>,
    pub mouse_absolute: Option<bool>,
}

#[typeshare]
#[cfg(unix)]
#[derive(Debug, Deserialize, Default)]
pub struct OtgConfigUpdate {
    pub hid: Option<HidConfigUpdate>,
    pub msd: Option<MsdConfigUpdate>,
    pub network: Option<OtgNetworkConfigUpdate>,
    pub uac: Option<crate::config::UacConfig>,
}

impl HidConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(update) = &self.jiggler {
            let mut config = MouseJigglerConfig::default();
            update.apply_to(&mut config);
            config.validate()?;
        }
        if let Some(config) = &self.bluetooth {
            config.validate()?;
        }
        if let Some(baudrate) = self.ch9329_baudrate {
            let valid_rates = [9600, 19200, 38400, 57600, 115200];
            if !valid_rates.contains(&baudrate) {
                return Err(AppError::BadRequest(
                    "Invalid baudrate: must be 9600, 19200, 38400, 57600, or 115200".into(),
                ));
            }
        }
        if let Some(ref desc) = self.otg_descriptor {
            desc.validate()?;
        }
        if let Some(ref desc) = self.ch9329_descriptor {
            desc.validate()?;
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut HidConfig) {
        if let Some(update) = &self.jiggler {
            update.apply_to(&mut config.jiggler);
        }
        if let Some(bluetooth) = &self.bluetooth {
            config.bluetooth = bluetooth.clone();
        }
        if let Some(backend) = self.backend.clone() {
            config.backend = backend;
        }
        if let Some(ref port) = self.ch9329_port {
            config.ch9329_port = port.clone();
        }
        if let Some(baudrate) = self.ch9329_baudrate {
            config.ch9329_baudrate = baudrate;
        }
        if let Some(enabled) = self.ch9329_hybrid_mouse {
            config.ch9329_hybrid_mouse = enabled;
        }
        if let Some(enabled) = self.mouse_macos_drag {
            config.mouse_macos_drag = enabled;
        }
        if let Some(ref desc) = self.ch9329_descriptor {
            desc.apply_to(&mut config.ch9329_descriptor);
        }
        if let Some(ref udc) = self.otg_udc {
            config.otg_udc = Some(udc.clone());
        }
        if let Some(ref desc) = self.otg_descriptor {
            desc.apply_to(&mut config.otg_descriptor);
        }
        if let Some(profile) = self.otg_profile.clone() {
            config.otg_profile = profile;
        }
        if let Some(ref functions) = self.otg_functions {
            functions.apply_to(&mut config.otg_functions);
        }
        if let Some(enabled) = self.otg_keyboard_leds {
            config.otg_keyboard_leds = enabled;
        }
        if let Some(absolute) = self.mouse_absolute {
            config.mouse_absolute = absolute;
        }
    }
}

#[typeshare]
#[cfg(unix)]
#[derive(Debug, Deserialize)]
pub struct OtgNetworkConfigUpdate {
    pub enabled: Option<bool>,
    pub driver_mode: Option<OtgNetworkDriverMode>,
    pub bridge_interface: Option<String>,
    pub host_mac: Option<String>,
    pub device_mac: Option<String>,
}

#[cfg(unix)]
impl OtgNetworkConfigUpdate {
    pub fn apply_to(&self, config: &mut OtgNetworkConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(driver_mode) = self.driver_mode {
            config.driver_mode = driver_mode;
        }
        if let Some(ref interface) = self.bridge_interface {
            config.bridge_interface = interface.trim().to_string();
        }
        if let Some(ref mac) = self.host_mac {
            config.host_mac = mac.trim().to_ascii_lowercase();
        }
        if let Some(ref mac) = self.device_mac {
            config.device_mac = mac.trim().to_ascii_lowercase();
        }
    }
}

#[typeshare]
#[cfg(unix)]
#[derive(Debug, Deserialize)]
pub struct MsdConfigUpdate {
    pub enabled: Option<bool>,
    pub msd_dir: Option<String>,
    pub flash_inquiry_string: Option<String>,
    pub cdrom_inquiry_string: Option<String>,
}

#[cfg(unix)]
impl MsdConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(ref dir) = self.msd_dir {
            let trimmed = dir.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest("MSD directory cannot be empty".into()));
            }
            if !Path::new(trimmed).is_absolute() {
                return Err(AppError::BadRequest(
                    "MSD directory must be an absolute path".into(),
                ));
            }
        }
        if let Some(ref value) = self.flash_inquiry_string {
            MsdConfig::validate_inquiry_string("Flash", value)?;
        }
        if let Some(ref value) = self.cdrom_inquiry_string {
            MsdConfig::validate_inquiry_string("CD-ROM", value)?;
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut MsdConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref dir) = self.msd_dir {
            config.msd_dir = dir.trim().to_string();
        }
        if let Some(ref value) = self.flash_inquiry_string {
            config.flash_inquiry_string = value.trim().to_string();
        }
        if let Some(ref value) = self.cdrom_inquiry_string {
            config.cdrom_inquiry_string = value.trim().to_string();
        }
    }
}

/// Update for a single ATX output binding
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxOutputBindingUpdate {
    pub enabled: Option<bool>,
    pub device: Option<String>,
    pub pin: Option<u32>,
    pub active_level: Option<crate::atx::ActiveLevel>,
}

/// Update for ATX GPIO input sensing
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxInputBindingUpdate {
    pub enabled: Option<bool>,
    pub device: Option<String>,
    pub pin: Option<u32>,
    pub active_level: Option<crate::atx::ActiveLevel>,
}

/// ATX configuration update request
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxConfigUpdate {
    pub enabled: Option<bool>,
    pub driver: Option<crate::atx::AtxDriverType>,
    pub device: Option<String>,
    pub baud_rate: Option<u32>,
    /// Power button configuration
    pub power: Option<AtxOutputBindingUpdate>,
    /// Reset button configuration
    pub reset: Option<AtxOutputBindingUpdate>,
    /// LED sensing configuration
    pub led: Option<AtxInputBindingUpdate>,
    /// HDD activity sensing configuration
    pub hdd: Option<AtxInputBindingUpdate>,
    /// Network interface for WOL packets (empty = auto)
    pub wol_interface: Option<String>,
}

impl AtxConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(baud_rate) = self.baud_rate {
            if baud_rate == 0 {
                return Err(AppError::BadRequest(
                    "ATX baud_rate must be greater than 0".to_string(),
                ));
            }
        }

        for (name, binding) in [
            ("power", self.power.as_ref()),
            ("reset", self.reset.as_ref()),
        ] {
            if let Some(binding) = binding {
                Self::validate_output_binding_update(binding, name)?;
            }
        }

        for (name, binding) in [("led", self.led.as_ref()), ("hdd", self.hdd.as_ref())] {
            if let Some(binding) = binding {
                Self::validate_input_binding_update(binding, name)?;
            }
        }

        Ok(())
    }

    pub fn validate_with_current(&self, current: &AtxConfig) -> crate::error::Result<()> {
        self.validate()?;

        // Validate with full context after applying the patch payload.
        let mut merged = current.clone();
        self.apply_to(&mut merged);

        if merged.driver == crate::atx::AtxDriverType::None {
            if merged.enabled {
                return Err(AppError::BadRequest(
                    "ATX driver must not be none when ATX is enabled".to_string(),
                ));
            }
            return Ok(());
        }

        merged.normalize();

        if !merged.enabled {
            return Ok(());
        }

        Self::validate_effective_config(&merged)?;
        Ok(())
    }

    fn validate_device_path(device: &str, name: &str) -> crate::error::Result<()> {
        let trimmed = device.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "{} device cannot be empty",
                name
            )));
        }

        if !cfg!(windows) && !std::path::Path::new(trimmed).exists() {
            return Err(AppError::BadRequest(format!(
                "{} device '{}' does not exist",
                name, trimmed
            )));
        }

        Ok(())
    }

    fn validate_output_binding_update(
        binding: &AtxOutputBindingUpdate,
        name: &str,
    ) -> crate::error::Result<()> {
        if let Some(ref device) = binding.device {
            if !device.trim().is_empty() {
                Self::validate_device_path(device, name)?;
            }
        }

        Ok(())
    }

    fn validate_input_binding_update(
        binding: &AtxInputBindingUpdate,
        name: &str,
    ) -> crate::error::Result<()> {
        if let Some(ref device) = binding.device {
            if !device.trim().is_empty() {
                Self::validate_device_path(device, name)?;
            }
        }

        Ok(())
    }

    fn validate_effective_config(config: &AtxConfig) -> crate::error::Result<()> {
        match config.driver {
            crate::atx::AtxDriverType::Gpio => {
                for (name, binding) in [("power", &config.power), ("reset", &config.reset)] {
                    if binding.enabled {
                        Self::validate_device_path(&binding.device, name)?;
                    }
                }

                for (name, binding) in [("led", &config.led), ("hdd", &config.hdd)] {
                    if binding.enabled {
                        Self::validate_device_path(&binding.device, name)?;
                    }
                }
            }
            crate::atx::AtxDriverType::UsbRelay => {
                Self::validate_device_path(&config.device, "ATX LCUS HID relay")?;
                Self::validate_output_channel(
                    &config.power,
                    "power",
                    1,
                    crate::atx::LCUS_RELAY_MAX_CHANNEL as u32,
                )?;
                Self::validate_output_channel(
                    &config.reset,
                    "reset",
                    1,
                    crate::atx::LCUS_RELAY_MAX_CHANNEL as u32,
                )?;
            }
            crate::atx::AtxDriverType::Serial => {
                Self::validate_device_path(&config.device, "ATX LCUS serial relay")?;
                if config.baud_rate == 0 {
                    return Err(AppError::BadRequest(
                        "ATX baud_rate must be greater than 0".to_string(),
                    ));
                }
                Self::validate_output_channel(
                    &config.power,
                    "power",
                    1,
                    crate::atx::LCUS_RELAY_MAX_CHANNEL as u32,
                )?;
                Self::validate_output_channel(
                    &config.reset,
                    "reset",
                    1,
                    crate::atx::LCUS_RELAY_MAX_CHANNEL as u32,
                )?;
            }
            crate::atx::AtxDriverType::None => {}
        };

        Ok(())
    }

    fn validate_output_channel(
        binding: &crate::atx::AtxOutputBinding,
        name: &str,
        min: u32,
        max: u32,
    ) -> crate::error::Result<()> {
        if !binding.enabled {
            return Ok(());
        }

        if binding.pin < min || binding.pin > max {
            return Err(AppError::BadRequest(format!(
                "{} channel must be between {} and {}",
                name, min, max
            )));
        }

        Ok(())
    }

    pub fn apply_to(&self, config: &mut AtxConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(driver) = self.driver {
            config.driver = driver;
        }
        if let Some(ref device) = self.device {
            config.device = device.clone();
        }
        if let Some(baud_rate) = self.baud_rate {
            config.baud_rate = baud_rate;
        }
        if let Some(ref power) = self.power {
            Self::apply_output_binding_update(power, &mut config.power);
        }
        if let Some(ref reset) = self.reset {
            Self::apply_output_binding_update(reset, &mut config.reset);
        }
        if let Some(ref led) = self.led {
            Self::apply_input_binding_update(led, &mut config.led);
        }
        if let Some(ref hdd) = self.hdd {
            Self::apply_input_binding_update(hdd, &mut config.hdd);
        }
        if let Some(ref wol_interface) = self.wol_interface {
            config.wol_interface = wol_interface.clone();
        }
    }

    fn apply_output_binding_update(
        update: &AtxOutputBindingUpdate,
        config: &mut crate::atx::AtxOutputBinding,
    ) {
        if let Some(enabled) = update.enabled {
            config.enabled = enabled;
        }
        if let Some(ref device) = update.device {
            config.device = device.clone();
        }
        if let Some(pin) = update.pin {
            config.pin = pin;
        }
        if let Some(active_level) = update.active_level {
            config.active_level = active_level;
        }
    }

    fn apply_input_binding_update(
        update: &AtxInputBindingUpdate,
        config: &mut crate::atx::AtxInputBinding,
    ) {
        if let Some(enabled) = update.enabled {
            config.enabled = enabled;
        }
        if let Some(ref device) = update.device {
            config.device = device.clone();
        }
        if let Some(pin) = update.pin {
            config.pin = pin;
        }
        if let Some(active_level) = update.active_level {
            config.active_level = active_level;
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AudioConfigUpdate {
    pub enabled: Option<bool>,
    pub device: Option<String>,
    pub quality: Option<String>,
}

impl AudioConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(ref quality) = self.quality {
            if !["voice", "balanced", "high"].contains(&quality.as_str()) {
                return Err(AppError::BadRequest(
                    "Invalid quality: must be 'voice', 'balanced', or 'high'".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut AudioConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref device) = self.device {
            config.device = device.clone();
        }
        if let Some(ref quality) = self.quality {
            config.quality = quality.clone();
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebCertificateInfo {
    pub subject: String,
    pub issuer: String,
    /// Start of the certificate validity period, in RFC 3339 UTC.
    pub not_before: String,
    /// End of the certificate validity period, in RFC 3339 UTC.
    pub not_after: String,
    /// DNS names and IP addresses from the leaf certificate's subject alternative names.
    pub names: Vec<String>,
    /// Absent when the private key could not be read or parsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_matches: Option<bool>,
}

/// Web server settings returned by `GET` / `PATCH /api/config/web`.
///
/// Public API shape: certificate paths on disk are not exposed. The full stored model is `WebConfig` in `config::schema`.
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfigResponse {
    pub http_port: u16,
    pub https_port: u16,
    pub bind_addresses: Vec<String>,
    pub bind_address: String,
    pub https_enabled: bool,
    /// Whether a custom TLS certificate and key are configured.
    pub has_custom_cert: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_cert_info: Option<WebCertificateInfo>,
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct WebConfigUpdate {
    pub http_port: Option<u16>,
    pub https_port: Option<u16>,
    pub bind_addresses: Option<Vec<String>>,
    pub bind_address: Option<String>,
    pub https_enabled: Option<bool>,
    /// PEM-encoded certificate content (must be provided together with ssl_key_pem)
    pub ssl_cert_pem: Option<String>,
    /// PEM-encoded private key content (must be provided together with ssl_cert_pem)
    pub ssl_key_pem: Option<String>,
    /// Set to true to remove the custom certificate and revert to self-signed
    pub clear_custom_cert: Option<bool>,
}

impl WebConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(port) = self.http_port {
            if port == 0 {
                return Err(AppError::BadRequest("HTTP port cannot be 0".into()));
            }
        }
        if let Some(port) = self.https_port {
            if port == 0 {
                return Err(AppError::BadRequest("HTTPS port cannot be 0".into()));
            }
        }
        if let Some(ref addrs) = self.bind_addresses {
            for addr in addrs {
                if addr.parse::<std::net::IpAddr>().is_err() {
                    return Err(AppError::BadRequest("Invalid bind address".into()));
                }
            }
        }
        if let Some(ref addr) = self.bind_address {
            if addr.parse::<std::net::IpAddr>().is_err() {
                return Err(AppError::BadRequest("Invalid bind address".into()));
            }
        }
        // Cert and key must be provided together (cryptographic validity is checked in the
        // handler via `server_config_from_pem`, same stack as the running HTTPS server).
        match (&self.ssl_cert_pem, &self.ssl_key_pem) {
            (Some(_cert), Some(_key)) => {}
            (Some(_), None) => {
                return Err(AppError::BadRequest(
                    "ssl_key_pem is required when ssl_cert_pem is provided".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(AppError::BadRequest(
                    "ssl_cert_pem is required when ssl_key_pem is provided".into(),
                ));
            }
            (None, None) => {}
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut crate::config::WebConfig) {
        if let Some(port) = self.http_port {
            config.http_port = port;
        }
        if let Some(port) = self.https_port {
            config.https_port = port;
        }
        if let Some(ref addrs) = self.bind_addresses {
            config.bind_addresses = addrs.clone();
            if let Some(first) = addrs.first() {
                config.bind_address = first.clone();
            }
        } else if let Some(ref addr) = self.bind_address {
            config.bind_address = addr.clone();
            if config.bind_addresses.is_empty() {
                config.bind_addresses = vec![addr.clone()];
            }
        }
        if let Some(enabled) = self.https_enabled {
            config.https_enabled = enabled;
        }
        // ssl_cert_pem, ssl_key_pem, clear_custom_cert are handled at the handler level
        // (they require async file I/O before updating config paths)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedfishConfigResponse {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct RedfishConfigUpdate {
    pub enabled: Option<bool>,
}

impl RedfishConfigUpdate {
    pub fn apply_to(&self, config: &mut crate::config::RedfishConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_atx_update() -> AtxConfigUpdate {
        AtxConfigUpdate {
            enabled: None,
            driver: None,
            device: None,
            baud_rate: None,
            power: None,
            reset: None,
            led: None,
            hdd: None,
            wol_interface: None,
        }
    }

    #[test]
    fn test_atx_apply_update_applies_global_baud_rate() {
        let mut config = AtxConfig::default();
        let mut update = empty_atx_update();
        update.driver = Some(crate::atx::AtxDriverType::Serial);
        update.device = Some("/dev/null".to_string());
        update.baud_rate = Some(19200);
        update.power = Some(AtxOutputBindingUpdate {
            enabled: Some(true),
            device: Some("/dev/ignored".to_string()),
            pin: Some(1),
            active_level: None,
        });

        update.apply_to(&mut config);
        assert_eq!(config.driver, crate::atx::AtxDriverType::Serial);
        assert_eq!(config.device, "/dev/null");
        assert_eq!(config.baud_rate, 19200);
        assert!(config.power.enabled);
        assert_eq!(config.power.pin, 1);
    }

    #[test]
    fn test_atx_validate_with_current_rejects_serial_pin_zero() {
        let mut current = AtxConfig::default();
        current.enabled = true;
        current.driver = crate::atx::AtxDriverType::Serial;
        current.device = "/dev/null".to_string();
        current.power.enabled = true;
        current.power.pin = 1;
        current.baud_rate = 9600;

        let mut update = empty_atx_update();
        update.power = Some(AtxOutputBindingUpdate {
            enabled: None,
            device: None,
            pin: Some(0),
            active_level: None,
        });

        assert!(update.validate_with_current(&current).is_err());
    }

    #[test]
    fn test_atx_validate_with_current_rejects_lcus_serial_channel_over_8() {
        let mut current = AtxConfig::default();
        current.enabled = true;
        current.driver = crate::atx::AtxDriverType::Serial;
        current.device = "/dev/null".to_string();
        current.power.enabled = true;
        current.power.pin = 1;
        current.baud_rate = 9600;

        let mut update = empty_atx_update();
        update.power = Some(AtxOutputBindingUpdate {
            enabled: None,
            device: None,
            pin: Some(9),
            active_level: None,
        });

        assert!(update.validate_with_current(&current).is_err());
    }

    #[test]
    fn test_atx_validate_with_current_rejects_enabled_none_driver() {
        let mut current = AtxConfig::default();
        current.driver = crate::atx::AtxDriverType::None;

        let mut update = empty_atx_update();
        update.enabled = Some(true);

        assert!(update.validate_with_current(&current).is_err());
    }

    #[test]
    fn test_atx_validate_with_current_rejects_usb_relay_channel_over_8() {
        let mut current = AtxConfig::default();
        current.enabled = true;
        current.driver = crate::atx::AtxDriverType::UsbRelay;
        current.device = "/dev/null".to_string();
        current.power.enabled = true;
        current.power.pin = 1;

        let mut update = empty_atx_update();
        update.power = Some(AtxOutputBindingUpdate {
            enabled: None,
            device: None,
            pin: Some(9),
            active_level: None,
        });

        assert!(update.validate_with_current(&current).is_err());
    }
}
