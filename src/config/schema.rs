use serde::{Deserialize, Serialize};
use typeshare::typeshare;

// Re-export domain config types that are embedded in AppConfig.
// These are simple data types defined in their respective modules;
// keeping the re-export here is acceptable since they flow inward.
pub use crate::extensions::ExtensionsConfig;
pub use crate::rustdesk::config::RustDeskConfig;

/// Bitrate preset for video encoding
///
/// Simplifies bitrate configuration by providing three intuitive presets
/// plus a custom option for advanced users.
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[derive(Default)]
pub enum BitratePreset {
    /// Speed priority: 1 Mbps, lowest latency, smaller GOP
    Speed,
    /// Balanced: 4 Mbps, good quality/latency tradeoff
    #[default]
    Balanced,
    /// Quality priority: 8 Mbps, best visual quality
    Quality,
    /// Custom bitrate in kbps (for advanced users)
    Custom(u32),
}

impl BitratePreset {
    /// Get bitrate value in kbps
    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            Self::Speed => 1000,
            Self::Balanced => 4000,
            Self::Quality => 8000,
            Self::Custom(kbps) => *kbps,
        }
    }

    /// Get recommended GOP size based on preset
    pub fn gop_size(&self, fps: u32) -> u32 {
        match self {
            Self::Speed => (fps / 2).max(15),
            Self::Balanced => fps,
            Self::Quality => fps * 2,
            Self::Custom(_) => fps,
        }
    }

    /// Get quality preset name for encoder configuration
    pub fn quality_level(&self) -> &'static str {
        match self {
            Self::Speed => "low",
            Self::Balanced => "medium",
            Self::Quality => "high",
            Self::Custom(_) => "medium",
        }
    }

    /// Create from kbps value, mapping to nearest preset or Custom
    pub fn from_kbps(kbps: u32) -> Self {
        match kbps {
            0..=1500 => Self::Speed,
            1501..=6000 => Self::Balanced,
            6001..=10000 => Self::Quality,
            _ => Self::Custom(kbps),
        }
    }
}

impl std::fmt::Display for BitratePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Speed => write!(f, "Speed (1 Mbps)"),
            Self::Balanced => write!(f, "Balanced (4 Mbps)"),
            Self::Quality => write!(f, "Quality (8 Mbps)"),
            Self::Custom(kbps) => write!(f, "Custom ({} kbps)", kbps),
        }
    }
}

/// Main application configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
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
    /// RTSP streaming settings
    pub rtsp: RtspConfig,
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
#[derive(Default)]
pub enum HidBackend {
    /// USB OTG HID gadget
    Otg,
    /// CH9329 serial HID controller
    Ch9329,
    /// Disabled
    #[default]
    None,
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
#[derive(Default)]
pub enum OtgHidProfile {
    /// Full HID device set (keyboard + relative mouse + absolute mouse + consumer control)
    #[default]
    #[serde(alias = "full_no_msd")]
    Full,
    /// Full HID device set without consumer control
    #[serde(alias = "full_no_consumer_no_msd")]
    FullNoConsumer,
    /// Legacy profile: only keyboard
    LegacyKeyboard,
    /// Legacy profile: only relative mouse
    LegacyMouseRelative,
    /// Custom function selection
    Custom,
}

/// OTG endpoint budget policy.
#[typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum OtgEndpointBudget {
    /// Derive a safe default from the selected UDC.
    #[default]
    Auto,
    /// Limit OTG gadget functions to 5 endpoints.
    Five,
    /// Limit OTG gadget functions to 6 endpoints.
    Six,
    /// Do not impose a software endpoint budget.
    Unlimited,
}

impl OtgEndpointBudget {
    /// Resolve endpoint limit assuming a known budget variant (not Auto).
    pub fn endpoint_limit_raw(&self) -> Option<u8> {
        match self {
            Self::Five => Some(5),
            Self::Six => Some(6),
            Self::Unlimited => None,
            Self::Auto => None, // resolved via `HidConfig::resolved_otg_endpoint_limit`
        }
    }
}

/// OTG HID function selection (used when profile is Custom)
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

    pub fn endpoint_cost(&self, keyboard_leds: bool) -> u8 {
        let mut endpoints = 0;
        if self.keyboard {
            endpoints += 1;
            if keyboard_leds {
                endpoints += 1;
            }
        }
        if self.mouse_relative {
            endpoints += 1;
        }
        if self.mouse_absolute {
            endpoints += 1;
        }
        if self.consumer {
            endpoints += 1;
        }
        endpoints
    }
}

impl Default for OtgHidFunctions {
    fn default() -> Self {
        Self::full()
    }
}

impl OtgHidProfile {
    pub fn from_legacy_str(value: &str) -> Option<Self> {
        match value {
            "full" | "full_no_msd" => Some(Self::Full),
            "full_no_consumer" | "full_no_consumer_no_msd" => Some(Self::FullNoConsumer),
            "legacy_keyboard" => Some(Self::LegacyKeyboard),
            "legacy_mouse_relative" => Some(Self::LegacyMouseRelative),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn resolve_functions(&self, custom: &OtgHidFunctions) -> OtgHidFunctions {
        match self {
            Self::Full => OtgHidFunctions::full(),
            Self::FullNoConsumer => OtgHidFunctions::full_no_consumer(),
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
    /// OTG UDC (USB Device Controller) name
    pub otg_udc: Option<String>,
    /// OTG USB device descriptor configuration
    #[serde(default)]
    pub otg_descriptor: OtgDescriptorConfig,
    /// OTG HID function profile
    #[serde(default)]
    pub otg_profile: OtgHidProfile,
    /// OTG endpoint budget policy
    #[serde(default)]
    pub otg_endpoint_budget: OtgEndpointBudget,
    /// OTG HID function selection (used when profile is Custom)
    #[serde(default)]
    pub otg_functions: OtgHidFunctions,
    /// Enable keyboard LED/status feedback for OTG keyboard
    #[serde(default)]
    pub otg_keyboard_leds: bool,
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
            otg_udc: None,
            otg_descriptor: OtgDescriptorConfig::default(),
            otg_profile: OtgHidProfile::default(),
            otg_endpoint_budget: OtgEndpointBudget::default(),
            otg_functions: OtgHidFunctions::default(),
            otg_keyboard_leds: false,
            ch9329_port: "/dev/ttyUSB0".to_string(),
            ch9329_baudrate: 9600,
            mouse_absolute: true,
        }
    }
}

impl HidConfig {
    /// Resolve effective OTG HID functions from profile + custom selection.
    /// Pure logic, no external dependency.
    pub fn effective_otg_functions(&self) -> OtgHidFunctions {
        self.otg_profile.resolve_functions(&self.otg_functions)
    }

    /// Whether keyboard LED feedback is effectively enabled.
    pub fn effective_otg_keyboard_leds(&self) -> bool {
        self.otg_keyboard_leds && self.effective_otg_functions().keyboard
    }

    /// Effective HID functions after applying all constraints.
    pub fn constrained_otg_functions(&self) -> OtgHidFunctions {
        self.effective_otg_functions()
    }

    /// Calculate required endpoint count for the current function selection.
    pub fn effective_otg_required_endpoints(&self, msd_enabled: bool) -> u8 {
        let functions = self.effective_otg_functions();
        let mut endpoints = functions.endpoint_cost(self.effective_otg_keyboard_leds());
        if msd_enabled {
            endpoints += 2;
        }
        endpoints
    }

    /// Validate endpoint budget for the current OTG configuration (UDC-aware when budget is Auto).
    pub fn validate_otg_endpoint_budget(&self, msd_enabled: bool) -> crate::error::Result<()> {
        if self.backend != HidBackend::Otg {
            return Ok(());
        }

        let functions = self.effective_otg_functions();
        if functions.is_empty() {
            return Err(crate::error::AppError::BadRequest(
                "OTG HID functions cannot be empty".to_string(),
            ));
        }

        let resolved_limit = self.resolved_otg_endpoint_limit();
        let required = self.effective_otg_required_endpoints(msd_enabled);
        if let Some(limit) = resolved_limit {
            if required > limit {
                return Err(crate::error::AppError::BadRequest(format!(
                    "OTG selection requires {} endpoints, but the configured limit is {}",
                    required, limit
                )));
            }
        }

        Ok(())
    }

    /// Effective OTG UDC name (for change detection / service).
    #[inline]
    pub fn resolved_otg_udc(&self) -> Option<String> {
        if self.backend != HidBackend::Otg {
            return None;
        }
        self.otg_udc
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| crate::otg::OtgGadgetManager::find_udc())
    }

    /// Resolved endpoint limit used for OTG gadget allocator / validation.
    #[inline]
    pub fn resolved_otg_endpoint_limit(&self) -> Option<u8> {
        if self.backend != HidBackend::Otg {
            return None;
        }
        match self.otg_endpoint_budget {
            OtgEndpointBudget::Five => Some(5),
            OtgEndpointBudget::Six => Some(6),
            OtgEndpointBudget::Unlimited => None,
            OtgEndpointBudget::Auto => {
                let udc = self.resolved_otg_udc().unwrap_or_default();
                if crate::otg::configfs::is_low_endpoint_udc(&udc) {
                    Some(5)
                } else {
                    Some(6)
                }
            }
        }
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
#[derive(Default)]
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
            device: String::new(),
            quality: "balanced".to_string(),
        }
    }
}

/// Stream mode
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StreamMode {
    /// WebRTC with H264/H265
    WebRTC,
    /// MJPEG over HTTP
    #[default]
    Mjpeg,
}

/// RTSP output codec
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RtspCodec {
    #[default]
    H264,
    H265,
}

/// RTSP configuration
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RtspConfig {
    /// Enable RTSP output
    pub enabled: bool,
    /// Bind IP address
    pub bind: String,
    /// RTSP TCP listen port
    pub port: u16,
    /// Stream path (without leading slash)
    pub path: String,
    /// Allow only one client connection at a time
    pub allow_one_client: bool,
    /// Output codec (H264/H265)
    pub codec: RtspCodec,
    /// Optional username for authentication
    pub username: Option<String>,
    /// Optional password for authentication
    #[typeshare(skip)]
    pub password: Option<String>,
}

impl Default for RtspConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "0.0.0.0".to_string(),
            port: 8554,
            path: "live".to_string(),
            allow_one_client: true,
            codec: RtspCodec::H264,
            username: None,
            password: None,
        }
    }
}

/// Encoder type
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum EncoderType {
    /// Auto-detect best encoder
    #[default]
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

impl EncoderType {
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
    /// Whether built-in / public ICE is used (no custom STUN or TURN URL configured).
    pub fn is_using_public_ice_servers(&self) -> bool {
        let no_custom_stun = self
            .stun_server
            .as_ref()
            .map_or(true, |s| s.trim().is_empty());
        let no_custom_turn = self
            .turn_server
            .as_ref()
            .map_or(true, |s| s.trim().is_empty());
        no_custom_stun && no_custom_turn
    }
}

/// Web server configuration persisted in the database (includes on-disk TLS paths).
///
/// The HTTP API for `/api/config/web` uses `WebConfigResponse` instead: no path fields, includes `has_custom_cert`.
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
