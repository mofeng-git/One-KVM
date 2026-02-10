use crate::video::encoder::BitratePreset;
use serde::{Deserialize, Serialize};
use typeshare::typeshare;

// Re-export ExtensionsConfig from extensions module
pub use crate::extensions::ExtensionsConfig;
// Re-export RustDeskConfig from rustdesk module
pub use crate::rustdesk::config::RustDeskConfig;

/// Main application configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Whether initial setup has been completed
    pub initialized: bool,
    /// Authentication settings
    pub auth: AuthConfig,
    /// Video capture settings
    pub video: VideoConfig,
    /// HID (keyboard/mouse) settings
    pub hid: HidConfig,
    /// Mass Storage Device settings
    pub msd: MsdConfig,
    /// ATX power control settings
    pub atx: AtxConfig,
    /// Audio settings
    pub audio: AudioConfig,
    /// Streaming settings
    pub stream: StreamConfig,
    /// Web server settings
    pub web: WebConfig,
    /// Extensions settings (ttyd, gostc, easytier)
    pub extensions: ExtensionsConfig,
    /// RustDesk remote access settings
    pub rustdesk: RustDeskConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            initialized: false,
            auth: AuthConfig::default(),
            video: VideoConfig::default(),
            hid: HidConfig::default(),
            msd: MsdConfig::default(),
            atx: AtxConfig::default(),
            audio: AudioConfig::default(),
            stream: StreamConfig::default(),
            web: WebConfig::default(),
            extensions: ExtensionsConfig::default(),
            rustdesk: RustDeskConfig::default(),
        }
    }
}

/// Authentication configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// Session timeout in seconds
    pub session_timeout_secs: u32,
    /// Allow multiple concurrent web sessions (single-user mode)
    pub single_user_allow_multiple_sessions: bool,
    /// Enable 2FA
    pub totp_enabled: bool,
    /// TOTP secret (encrypted)
    pub totp_secret: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_timeout_secs: 3600 * 24, // 24 hours
            single_user_allow_multiple_sessions: false,
            totp_enabled: false,
            totp_secret: None,
        }
    }
}

/// Video capture configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VideoConfig {
    /// Video device path (e.g., /dev/video0)
    pub device: Option<String>,
    /// Video pixel format (e.g., "MJPEG", "YUYV", "NV12")
    pub format: Option<String>,
    /// Resolution width
    pub width: u32,
    /// Resolution height
    pub height: u32,
    /// Frame rate
    pub fps: u32,
    /// JPEG quality (1-100)
    pub quality: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            device: None,
            format: None, // Auto-detect or use MJPEG as default
            width: 1920,
            height: 1080,
            fps: 30,
            quality: 80,
        }
    }
}

/// HID backend type
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HidBackend {
    /// USB OTG HID gadget
    Otg,
    /// CH9329 serial HID controller
    Ch9329,
    /// Disabled
    None,
}

impl Default for HidBackend {
    fn default() -> Self {
        Self::None
    }
}

/// OTG USB device descriptor configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OtgDescriptorConfig {
    /// USB Vendor ID (e.g., 0x1d6b)
    pub vendor_id: u16,
    /// USB Product ID (e.g., 0x0104)
    pub product_id: u16,
    /// Manufacturer string
    pub manufacturer: String,
    /// Product string
    pub product: String,
    /// Serial number (optional, auto-generated if not set)
    pub serial_number: Option<String>,
}

impl Default for OtgDescriptorConfig {
    fn default() -> Self {
        Self {
            vendor_id: 0x1d6b,  // Linux Foundation
            product_id: 0x0104, // Multifunction Composite Gadget
            manufacturer: "One-KVM".to_string(),
            product: "One-KVM USB Device".to_string(),
            serial_number: None,
        }
    }
}

/// OTG HID function profile
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OtgHidProfile {
    /// Full HID device set (keyboard + relative mouse + absolute mouse + consumer control)
    Full,
    /// Full HID device set without MSD
    FullNoMsd,
    /// Full HID device set without consumer control
    FullNoConsumer,
    /// Full HID device set without consumer control and MSD
    FullNoConsumerNoMsd,
    /// Legacy profile: only keyboard
    LegacyKeyboard,
    /// Legacy profile: only relative mouse
    LegacyMouseRelative,
    /// Custom function selection
    Custom,
}

impl Default for OtgHidProfile {
    fn default() -> Self {
        Self::Full
    }
}

/// OTG HID function selection (used when profile is Custom)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OtgHidFunctions {
    pub keyboard: bool,
    pub mouse_relative: bool,
    pub mouse_absolute: bool,
    pub consumer: bool,
}

impl OtgHidFunctions {
    pub fn full() -> Self {
        Self {
            keyboard: true,
            mouse_relative: true,
            mouse_absolute: true,
            consumer: true,
        }
    }

    pub fn full_no_consumer() -> Self {
        Self {
            keyboard: true,
            mouse_relative: true,
            mouse_absolute: true,
            consumer: false,
        }
    }

    pub fn legacy_keyboard() -> Self {
        Self {
            keyboard: true,
            mouse_relative: false,
            mouse_absolute: false,
            consumer: false,
        }
    }

    pub fn legacy_mouse_relative() -> Self {
        Self {
            keyboard: false,
            mouse_relative: true,
            mouse_absolute: false,
            consumer: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.keyboard && !self.mouse_relative && !self.mouse_absolute && !self.consumer
    }
}

impl Default for OtgHidFunctions {
    fn default() -> Self {
        Self::full()
    }
}

impl OtgHidProfile {
    pub fn resolve_functions(&self, custom: &OtgHidFunctions) -> OtgHidFunctions {
        match self {
            Self::Full => OtgHidFunctions::full(),
            Self::FullNoMsd => OtgHidFunctions::full(),
            Self::FullNoConsumer => OtgHidFunctions::full_no_consumer(),
            Self::FullNoConsumerNoMsd => OtgHidFunctions::full_no_consumer(),
            Self::LegacyKeyboard => OtgHidFunctions::legacy_keyboard(),
            Self::LegacyMouseRelative => OtgHidFunctions::legacy_mouse_relative(),
            Self::Custom => custom.clone(),
        }
    }
}

/// HID configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HidConfig {
    /// HID backend type
    pub backend: HidBackend,
    /// OTG keyboard device path
    pub otg_keyboard: String,
    /// OTG mouse device path
    pub otg_mouse: String,
    /// OTG UDC (USB Device Controller) name
    pub otg_udc: Option<String>,
    /// OTG USB device descriptor configuration
    #[serde(default)]
    pub otg_descriptor: OtgDescriptorConfig,
    /// OTG HID function profile
    #[serde(default)]
    pub otg_profile: OtgHidProfile,
    /// OTG HID function selection (used when profile is Custom)
    #[serde(default)]
    pub otg_functions: OtgHidFunctions,
    /// CH9329 serial port
    pub ch9329_port: String,
    /// CH9329 baud rate
    pub ch9329_baudrate: u32,
    /// Mouse mode: absolute or relative
    pub mouse_absolute: bool,
}

impl Default for HidConfig {
    fn default() -> Self {
        Self {
            backend: HidBackend::None,
            otg_keyboard: "/dev/hidg0".to_string(),
            otg_mouse: "/dev/hidg1".to_string(),
            otg_udc: None,
            otg_descriptor: OtgDescriptorConfig::default(),
            otg_profile: OtgHidProfile::default(),
            otg_functions: OtgHidFunctions::default(),
            ch9329_port: "/dev/ttyUSB0".to_string(),
            ch9329_baudrate: 9600,
            mouse_absolute: true,
        }
    }
}

impl HidConfig {
    pub fn effective_otg_functions(&self) -> OtgHidFunctions {
        self.otg_profile.resolve_functions(&self.otg_functions)
    }
}

/// MSD configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MsdConfig {
    /// Enable MSD functionality
    pub enabled: bool,
    /// MSD base directory (absolute path)
    pub msd_dir: String,
}

impl Default for MsdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            msd_dir: String::new(),
        }
    }
}

impl MsdConfig {
    pub fn msd_dir_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.msd_dir)
    }

    pub fn images_dir(&self) -> std::path::PathBuf {
        self.msd_dir_path().join("images")
    }

    pub fn ventoy_dir(&self) -> std::path::PathBuf {
        self.msd_dir_path().join("ventoy")
    }

    pub fn drive_path(&self) -> std::path::PathBuf {
        self.ventoy_dir().join("ventoy.img")
    }
}

// Re-export ATX types from atx module for configuration
pub use crate::atx::{ActiveLevel, AtxDriverType, AtxKeyConfig, AtxLedConfig};

/// ATX power control configuration
///
/// Each ATX action (power, reset) can be independently configured with its own
/// hardware binding using the four-tuple: (driver, device, pin, active_level).
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtxConfig {
    /// Enable ATX functionality
    pub enabled: bool,
    /// Power button configuration (used for both short and long press)
    pub power: AtxKeyConfig,
    /// Reset button configuration
    pub reset: AtxKeyConfig,
    /// LED sensing configuration (optional)
    pub led: AtxLedConfig,
    /// Network interface for WOL packets (empty = auto)
    pub wol_interface: String,
}

impl Default for AtxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            power: AtxKeyConfig::default(),
            reset: AtxKeyConfig::default(),
            led: AtxLedConfig::default(),
            wol_interface: String::new(),
        }
    }
}

impl AtxConfig {
    /// Convert to AtxControllerConfig for the controller
    pub fn to_controller_config(&self) -> crate::atx::AtxControllerConfig {
        crate::atx::AtxControllerConfig {
            enabled: self.enabled,
            power: self.power.clone(),
            reset: self.reset.clone(),
            led: self.led.clone(),
        }
    }
}

/// Audio configuration
///
/// Note: Sample rate is fixed at 48000Hz and channels at 2 (stereo).
/// These are optimal for Opus encoding and match WebRTC requirements.
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Enable audio capture
    pub enabled: bool,
    /// ALSA device name
    pub device: String,
    /// Audio quality preset: "voice", "balanced", "high"
    pub quality: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: "default".to_string(),
            quality: "balanced".to_string(),
        }
    }
}

/// Stream mode
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamMode {
    /// WebRTC with H264/H265
    WebRTC,
    /// MJPEG over HTTP
    Mjpeg,
}

impl Default for StreamMode {
    fn default() -> Self {
        Self::Mjpeg
    }
}

/// Encoder type
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EncoderType {
    /// Auto-detect best encoder
    Auto,
    /// Software encoder (libx264)
    Software,
    /// VAAPI hardware encoder
    Vaapi,
    /// NVIDIA NVENC hardware encoder
    Nvenc,
    /// Intel Quick Sync hardware encoder
    Qsv,
    /// AMD AMF hardware encoder
    Amf,
    /// Rockchip MPP hardware encoder
    Rkmpp,
    /// V4L2 M2M hardware encoder
    V4l2m2m,
}

impl Default for EncoderType {
    fn default() -> Self {
        Self::Auto
    }
}

impl EncoderType {
    /// Convert to EncoderBackend for registry queries
    pub fn to_backend(&self) -> Option<crate::video::encoder::registry::EncoderBackend> {
        use crate::video::encoder::registry::EncoderBackend;
        match self {
            EncoderType::Auto => None,
            EncoderType::Software => Some(EncoderBackend::Software),
            EncoderType::Vaapi => Some(EncoderBackend::Vaapi),
            EncoderType::Nvenc => Some(EncoderBackend::Nvenc),
            EncoderType::Qsv => Some(EncoderBackend::Qsv),
            EncoderType::Amf => Some(EncoderBackend::Amf),
            EncoderType::Rkmpp => Some(EncoderBackend::Rkmpp),
            EncoderType::V4l2m2m => Some(EncoderBackend::V4l2m2m),
        }
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            EncoderType::Auto => "Auto (Recommended)",
            EncoderType::Software => "Software (CPU)",
            EncoderType::Vaapi => "VAAPI",
            EncoderType::Nvenc => "NVIDIA NVENC",
            EncoderType::Qsv => "Intel Quick Sync",
            EncoderType::Amf => "AMD AMF",
            EncoderType::Rkmpp => "Rockchip MPP",
            EncoderType::V4l2m2m => "V4L2 M2M",
        }
    }
}

/// Streaming configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamConfig {
    /// Stream mode
    pub mode: StreamMode,
    /// Encoder type for H264/H265
    pub encoder: EncoderType,
    /// Bitrate preset (Speed/Balanced/Quality)
    pub bitrate_preset: BitratePreset,
    /// Custom STUN server (e.g., "stun:stun.l.google.com:19302")
    /// If empty, uses public ICE servers from secrets.toml
    pub stun_server: Option<String>,
    /// Custom TURN server (e.g., "turn:turn.example.com:3478")
    /// If empty, uses public ICE servers from secrets.toml
    pub turn_server: Option<String>,
    /// TURN username
    pub turn_username: Option<String>,
    /// TURN password (stored encrypted in DB, not exposed via API)
    pub turn_password: Option<String>,
    /// Auto-pause when no clients connected
    #[typeshare(skip)]
    pub auto_pause_enabled: bool,
    /// Auto-pause delay (seconds)
    #[typeshare(skip)]
    pub auto_pause_delay_secs: u64,
    /// Client timeout for cleanup (seconds)
    #[typeshare(skip)]
    pub client_timeout_secs: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            mode: StreamMode::Mjpeg,
            encoder: EncoderType::Auto,
            bitrate_preset: BitratePreset::Balanced,
            // Empty means use public ICE servers (like RustDesk)
            stun_server: None,
            turn_server: None,
            turn_username: None,
            turn_password: None,
            auto_pause_enabled: false,
            auto_pause_delay_secs: 10,
            client_timeout_secs: 30,
        }
    }
}

impl StreamConfig {
    /// Check if using public ICE servers (user left fields empty)
    pub fn is_using_public_ice_servers(&self) -> bool {
        use crate::webrtc::config::public_ice;
        self.stun_server
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
            && self
                .turn_server
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
            && public_ice::is_configured()
    }
}

/// Web server configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    /// HTTP port
    pub http_port: u16,
    /// HTTPS port
    pub https_port: u16,
    /// Bind addresses (preferred)
    pub bind_addresses: Vec<String>,
    /// Bind address (legacy)
    pub bind_address: String,
    /// Enable HTTPS
    pub https_enabled: bool,
    /// Custom SSL certificate path
    pub ssl_cert_path: Option<String>,
    /// Custom SSL key path
    pub ssl_key_path: Option<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            http_port: 8080,
            https_port: 8443,
            bind_addresses: Vec::new(),
            bind_address: "0.0.0.0".to_string(),
            https_enabled: false,
            ssl_cert_path: None,
            ssl_key_path: None,
        }
    }
}
