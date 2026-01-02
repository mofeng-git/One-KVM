use serde::Deserialize;
use typeshare::typeshare;
use crate::config::*;
use crate::error::AppError;
use crate::rustdesk::config::RustDeskConfig;
use crate::video::encoder::BitratePreset;

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
                return Err(AppError::BadRequest("Invalid width: must be 320-7680".into()));
            }
        }
        if let Some(height) = self.height {
            if !(240..=4320).contains(&height) {
                return Err(AppError::BadRequest("Invalid height: must be 240-4320".into()));
            }
        }
        if let Some(fps) = self.fps {
            if !(1..=120).contains(&fps) {
                return Err(AppError::BadRequest("Invalid fps: must be 1-120".into()));
            }
        }
        if let Some(quality) = self.quality {
            if !(1..=100).contains(&quality) {
                return Err(AppError::BadRequest("Invalid quality: must be 1-100".into()));
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
    pub stun_server: Option<String>,
    pub turn_server: Option<String>,
    pub turn_username: Option<String>,
    /// 指示是否已设置 TURN 密码（实际密码不返回）
    pub has_turn_password: bool,
}

impl From<&StreamConfig> for StreamConfigResponse {
    fn from(config: &StreamConfig) -> Self {
        Self {
            mode: config.mode.clone(),
            encoder: config.encoder.clone(),
            bitrate_preset: config.bitrate_preset,
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
    pub stun_server: Option<String>,
    /// TURN server URL (e.g., "turn:turn.example.com:3478")
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
                    "STUN server must start with 'stun:' (e.g., stun:stun.l.google.com:19302)".into(),
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
        // STUN/TURN settings - empty string means clear, Some("value") means set
        if let Some(ref stun) = self.stun_server {
            config.stun_server = if stun.is_empty() { None } else { Some(stun.clone()) };
        }
        if let Some(ref turn) = self.turn_server {
            config.turn_server = if turn.is_empty() { None } else { Some(turn.clone()) };
        }
        if let Some(ref username) = self.turn_username {
            config.turn_username = if username.is_empty() { None } else { Some(username.clone()) };
        }
        if let Some(ref password) = self.turn_password {
            config.turn_password = if password.is_empty() { None } else { Some(password.clone()) };
        }
    }
}

// ===== HID Config =====
#[typeshare]
#[derive(Debug, Deserialize)]
pub struct HidConfigUpdate {
    pub backend: Option<HidBackend>,
    pub ch9329_port: Option<String>,
    pub ch9329_baudrate: Option<u32>,
    pub otg_udc: Option<String>,
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
    pub images_path: Option<String>,
    pub drive_path: Option<String>,
    pub virtual_drive_size_mb: Option<u32>,
}

impl MsdConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        if let Some(size) = self.virtual_drive_size_mb {
            if !(1..=10240).contains(&size) {
                return Err(AppError::BadRequest("Drive size must be 1-10240 MB".into()));
            }
        }
        Ok(())
    }

    pub fn apply_to(&self, config: &mut MsdConfig) {
        if let Some(enabled) = self.enabled {
            config.enabled = enabled;
        }
        if let Some(ref path) = self.images_path {
            config.images_path = path.clone();
        }
        if let Some(ref path) = self.drive_path {
            config.drive_path = path.clone();
        }
        if let Some(size) = self.virtual_drive_size_mb {
            config.virtual_drive_size_mb = size;
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
    pub device_password: Option<String>,
}

impl RustDeskConfigUpdate {
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate rendezvous server format (should be host:port)
        if let Some(ref server) = self.rendezvous_server {
            if !server.is_empty() && !server.contains(':') {
                return Err(AppError::BadRequest(
                    "Rendezvous server must be in format 'host:port' (e.g., rs.example.com:21116)".into(),
                ));
            }
        }
        // Validate relay server format if provided
        if let Some(ref server) = self.relay_server {
            if !server.is_empty() && !server.contains(':') {
                return Err(AppError::BadRequest(
                    "Relay server must be in format 'host:port' (e.g., rs.example.com:21117)".into(),
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
            config.relay_server = if server.is_empty() { None } else { Some(server.clone()) };
        }
        if let Some(ref password) = self.device_password {
            if !password.is_empty() {
                config.device_password = password.clone();
            }
        }
    }
}
