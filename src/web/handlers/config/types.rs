use crate::config::*;
use crate::error::AppError;
use crate::rustdesk::config::RustDeskConfig;
use crate::video::encoder::BitratePreset;
use serde::Deserialize;
use std::path::Path;
use typeshare::typeshare;

// ===== Auth Config =====
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AuthConfigUpdate {
    pub single_user_allow_multiple_sessions: Option<bool>,
}

impl AuthConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        Ok(())
    }

    pub fn apply_to(&self, config: &mut AuthConfig) {
        if let Some(allow_multiple) = self.single_user_allow_multiple_sessions {
            config.single_user_allow_multiple_sessions = allow_multiple;
        }
    }
}

// ===== Video Config =====
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

// ===== Stream Config =====

/// Stream 配置响应（包含 has_turn_password 字段）
#[typeshare]
#[derive(Debug, serde::Serialize)]
pub struct StreamConfigResponse {
    pub mode: StreamMode,
    pub encoder: EncoderType,
    pub bitrate_preset: BitratePreset,
    /// 是否有公共 ICE 服务器可用（编译时确定）
    pub has_public_ice_servers: bool,
    /// 当前是否正在使用公共 ICE 服务器（STUN/TURN 都为空时）
    pub using_public_ice_servers: bool,
    pub stun_server: Option<String>,
    pub turn_server: Option<String>,
    pub turn_username: Option<String>,
    /// 指示是否已设置 TURN 密码（实际密码不返回）
    pub has_turn_password: bool,
}

impl From<&StreamConfig> for StreamConfigResponse {
    fn from(config: &StreamConfig) -> Self {
        use crate::webrtc::config::public_ice;
        Self {
            mode: config.mode.clone(),
            encoder: config.encoder.clone(),
            bitrate_preset: config.bitrate_preset,
            has_public_ice_servers: public_ice::is_configured(),
            using_public_ice_servers: config.is_using_public_ice_servers(),
            stun_server: config.stun_server.clone(),
            turn_server: config.turn_server.clone(),
            turn_username: config.turn_username.clone(),
            has_turn_password: config.turn_password.is_some(),
        }
    }
}

#[typeshare]
#[derive(Debug, Deserialize)]
pub struct StreamConfigUpdate {
    pub mode: Option<StreamMode>,
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

// ===== HID Config =====

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
pub struct HidConfigUpdate {
    pub backend: Option<HidBackend>,
    pub ch9329_port: Option<String>,
    pub ch9329_baudrate: Option<u32>,
    pub otg_udc: Option<String>,
    pub otg_descriptor: Option<OtgDescriptorConfigUpdate>,
    pub otg_profile: Option<OtgHidProfile>,
    pub otg_functions: Option<OtgHidFunctionsUpdate>,
    pub mouse_absolute: Option<bool>,
}

impl HidConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
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
        Ok(())
    }

    pub fn apply_to(&self, config: &mut HidConfig) {
        if let Some(backend) = self.backend.clone() {
            config.backend = backend;
        }
        if let Some(ref port) = self.ch9329_port {
            config.ch9329_port = port.clone();
        }
        if let Some(baudrate) = self.ch9329_baudrate {
            config.ch9329_baudrate = baudrate;
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
        if let Some(absolute) = self.mouse_absolute {
            config.mouse_absolute = absolute;
        }
    }
}

// ===== MSD Config =====
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct MsdConfigUpdate {
    pub enabled: Option<bool>,
    pub msd_dir: Option<String>,
}

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
        Ok(())
    }

    pub fn apply_to(&self, config: &mut MsdConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref dir) = self.msd_dir {
            config.msd_dir = dir.trim().to_string();
        }
    }
}

// ===== ATX Config =====

/// Update for a single ATX key configuration
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxKeyConfigUpdate {
    pub driver: Option<crate::atx::AtxDriverType>,
    pub device: Option<String>,
    pub pin: Option<u32>,
    pub active_level: Option<crate::atx::ActiveLevel>,
}

/// Update for LED sensing configuration
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxLedConfigUpdate {
    pub enabled: Option<bool>,
    pub gpio_chip: Option<String>,
    pub gpio_pin: Option<u32>,
    pub inverted: Option<bool>,
}

/// ATX configuration update request
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct AtxConfigUpdate {
    pub enabled: Option<bool>,
    /// Power button configuration
    pub power: Option<AtxKeyConfigUpdate>,
    /// Reset button configuration
    pub reset: Option<AtxKeyConfigUpdate>,
    /// LED sensing configuration
    pub led: Option<AtxLedConfigUpdate>,
    /// Network interface for WOL packets (empty = auto)
    pub wol_interface: Option<String>,
}

impl AtxConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate power key config if present
        if let Some(ref power) = self.power {
            Self::validate_key_config(power, "power")?;
        }
        // Validate reset key config if present
        if let Some(ref reset) = self.reset {
            Self::validate_key_config(reset, "reset")?;
        }
        Ok(())
    }

    fn validate_key_config(key: &AtxKeyConfigUpdate, name: &str) -> crate::error::Result<()> {
        if let Some(ref device) = key.device {
            if !device.is_empty() && !std::path::Path::new(device).exists() {
                return Err(AppError::BadRequest(format!(
                    "{} device '{}' does not exist",
                    name, device
                )));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut AtxConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref power) = self.power {
            Self::apply_key_update(power, &mut config.power);
        }
        if let Some(ref reset) = self.reset {
            Self::apply_key_update(reset, &mut config.reset);
        }
        if let Some(ref led) = self.led {
            Self::apply_led_update(led, &mut config.led);
        }
        if let Some(ref wol_interface) = self.wol_interface {
            config.wol_interface = wol_interface.clone();
        }
    }

    fn apply_key_update(update: &AtxKeyConfigUpdate, config: &mut crate::atx::AtxKeyConfig) {
        if let Some(driver) = update.driver {
            config.driver = driver;
        }
        if let Some(ref device) = update.device {
            config.device = device.clone();
        }
        if let Some(pin) = update.pin {
            config.pin = pin;
        }
        if let Some(level) = update.active_level {
            config.active_level = level;
        }
    }

    fn apply_led_update(update: &AtxLedConfigUpdate, config: &mut crate::atx::AtxLedConfig) {
        if let Some(enabled) = update.enabled {
            config.enabled = enabled;
        }
        if let Some(ref chip) = update.gpio_chip {
            config.gpio_chip = chip.clone();
        }
        if let Some(pin) = update.gpio_pin {
            config.gpio_pin = pin;
        }
        if let Some(inverted) = update.inverted {
            config.inverted = inverted;
        }
    }
}

// ===== Audio Config =====
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

// ===== RustDesk Config =====
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct RustDeskConfigUpdate {
    pub enabled: Option<bool>,
    pub rendezvous_server: Option<String>,
    pub relay_server: Option<String>,
    pub relay_key: Option<String>,
    pub device_password: Option<String>,
}

impl RustDeskConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate rendezvous server format (should be host:port)
        if let Some(ref server) = self.rendezvous_server {
            if !server.is_empty() && !server.contains(':') {
                return Err(AppError::BadRequest(
                    "Rendezvous server must be in format 'host:port' (e.g., rs.example.com:21116)"
                        .into(),
                ));
            }
        }
        // Validate relay server format if provided
        if let Some(ref server) = self.relay_server {
            if !server.is_empty() && !server.contains(':') {
                return Err(AppError::BadRequest(
                    "Relay server must be in format 'host:port' (e.g., rs.example.com:21117)"
                        .into(),
                ));
            }
        }
        // Validate password (minimum 6 characters if provided)
        if let Some(ref password) = self.device_password {
            if !password.is_empty() && password.len() < 6 {
                return Err(AppError::BadRequest(
                    "Device password must be at least 6 characters".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut RustDeskConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref server) = self.rendezvous_server {
            config.rendezvous_server = server.clone();
        }
        if let Some(ref server) = self.relay_server {
            config.relay_server = if server.is_empty() {
                None
            } else {
                Some(server.clone())
            };
        }
        if let Some(ref key) = self.relay_key {
            config.relay_key = if key.is_empty() {
                None
            } else {
                Some(key.clone())
            };
        }
        if let Some(ref password) = self.device_password {
            if !password.is_empty() {
                config.device_password = password.clone();
            }
        }
    }
}

// ===== Web Config =====
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct WebConfigUpdate {
    pub http_port: Option<u16>,
    pub https_port: Option<u16>,
    pub bind_addresses: Option<Vec<String>>,
    pub bind_address: Option<String>,
    pub https_enabled: Option<bool>,
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
    }
}
