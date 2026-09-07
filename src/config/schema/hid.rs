use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BluetoothHidConfig {
    pub adapter: String,
    pub name: String,
    pub peer: Option<String>,
}
impl Default for BluetoothHidConfig {
    fn default() -> Self {
        Self {
            adapter: "hci0".into(),
            name: "One-KVM HID".into(),
            peer: None,
        }
    }
}
impl BluetoothHidConfig {
    pub fn validate(&self) -> crate::error::Result<()> {
        let invalid = |reason: &str| crate::error::AppError::BadRequest(reason.into());
        if !self
            .adapter
            .strip_prefix("hci")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        {
            return Err(invalid(
                "Bluetooth adapter must be hci followed by an index",
            ));
        }
        if self.name.is_empty() || self.name.len() > 64 || self.name.chars().any(char::is_control) {
            return Err(invalid(
                "Bluetooth name must contain 1–64 UTF-8 bytes without control characters",
            ));
        }
        if let Some(peer) = &self.peer {
            let parts: Vec<_> = peer.split(':').collect();
            if parts.len() != 6
                || parts
                    .iter()
                    .any(|p| p.len() != 2 || !p.bytes().all(|c| c.is_ascii_hexdigit()))
            {
                return Err(invalid("Invalid Bluetooth peer address"));
            }
        }
        Ok(())
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum HidBackend {
    Otg,
    Ch9329,
    Bluetooth,
    #[default]
    None,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OtgDescriptorConfig {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: Option<String>,
}

impl Default for OtgDescriptorConfig {
    fn default() -> Self {
        Self {
            vendor_id: 0x1d6b,
            product_id: 0x0104,
            manufacturer: "One-KVM".to_string(),
            product: "One-KVM USB Device".to_string(),
            serial_number: None,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ch9329DescriptorConfig {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: Option<String>,
}

impl Default for Ch9329DescriptorConfig {
    fn default() -> Self {
        Self {
            vendor_id: 0x1a86,
            product_id: 0xe129,
            manufacturer: "WCH.CN".to_string(),
            product: "CH9329".to_string(),
            serial_number: None,
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ch9329DescriptorState {
    pub descriptor: Ch9329DescriptorConfig,
    pub manufacturer_enabled: bool,
    pub product_enabled: bool,
    pub serial_enabled: bool,
    pub config_mode_available: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum OtgHidProfile {
    #[default]
    #[serde(alias = "full_no_msd")]
    Full,
    #[serde(alias = "full_no_consumer_no_msd")]
    FullNoConsumer,
    LegacyKeyboard,
    LegacyMouseRelative,
    Custom,
}

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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HidConfig {
    pub bluetooth: BluetoothHidConfig,
    pub backend: HidBackend,
    pub otg_udc: Option<String>,
    #[serde(default)]
    pub otg_descriptor: OtgDescriptorConfig,
    #[serde(default)]
    pub otg_profile: OtgHidProfile,
    #[serde(default)]
    pub otg_functions: OtgHidFunctions,
    #[serde(default)]
    pub otg_keyboard_leds: bool,
    pub ch9329_port: String,
    pub ch9329_baudrate: u32,
    #[serde(default)]
    pub ch9329_hybrid_mouse: bool,
    #[serde(default)]
    pub ch9329_macos_drag: bool,
    #[serde(default)]
    pub ch9329_descriptor: Ch9329DescriptorConfig,
    pub mouse_absolute: bool,
}

impl Default for HidConfig {
    fn default() -> Self {
        Self {
            backend: HidBackend::None,
            bluetooth: BluetoothHidConfig::default(),
            otg_udc: None,
            otg_descriptor: OtgDescriptorConfig::default(),
            otg_profile: OtgHidProfile::default(),
            otg_functions: OtgHidFunctions::default(),
            otg_keyboard_leds: false,
            ch9329_port: "/dev/ttyUSB0".to_string(),
            ch9329_baudrate: 9600,
            ch9329_hybrid_mouse: false,
            ch9329_macos_drag: false,
            ch9329_descriptor: Ch9329DescriptorConfig::default(),
            mouse_absolute: true,
        }
    }
}

impl HidConfig {
    pub fn effective_otg_functions(&self) -> OtgHidFunctions {
        self.otg_profile.resolve_functions(&self.otg_functions)
    }

    pub fn effective_otg_keyboard_leds(&self) -> bool {
        self.otg_keyboard_leds && self.effective_otg_functions().keyboard
    }

    pub fn constrained_otg_functions(&self) -> OtgHidFunctions {
        self.effective_otg_functions()
    }

    pub fn validate_otg_functions(&self) -> crate::error::Result<()> {
        if self.backend != HidBackend::Otg {
            return Ok(());
        }

        let functions = self.effective_otg_functions();
        if functions.is_empty() {
            return Err(crate::error::AppError::BadRequest(
                "OTG HID functions cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    #[inline]
    pub fn resolved_otg_udc(&self) -> Option<String> {
        if self.backend != HidBackend::Otg {
            return None;
        }
        self.otg_udc
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                #[cfg(unix)]
                {
                    crate::otg::OtgGadgetManager::find_udc()
                }
                #[cfg(not(unix))]
                {
                    None
                }
            })
    }
}

#[cfg(test)]
mod bluetooth_tests {
    use super::*;
    #[test]
    fn old_configs_keep_bluetooth_disabled_and_get_defaults() {
        let config: HidConfig = serde_json::from_str(r#"{"backend":"otg"}"#).unwrap();
        assert_eq!(config.backend, HidBackend::Otg);
        assert_eq!(config.bluetooth, BluetoothHidConfig::default());
    }
    #[test]
    fn obsolete_ble_flag_is_ignored_and_not_saved() {
        let config: BluetoothHidConfig =
            serde_json::from_str(r#"{"adapter":"hci0","name":"My keyboard","le_only":true}"#)
                .unwrap();
        config.validate().unwrap();
        assert!(serde_json::to_value(config)
            .unwrap()
            .get("le_only")
            .is_none());
    }
    #[test]
    fn bluetooth_uses_relative_mouse_and_existing_usb_constraints() {
        let mut config = crate::config::AppConfig::default();
        config.hid.backend = HidBackend::Bluetooth;
        config.hid.mouse_absolute = true;
        config.msd.enabled = true;
        config.uac.enabled = true;
        config.otg_network.enabled = true;
        config.enforce_invariants();
        assert!(!config.hid.mouse_absolute);
        assert!(!config.msd.enabled && !config.uac.enabled && !config.otg_network.enabled);
    }
    #[test]
    fn reject_invalid_adapter_address_and_oversize_advertisement_name() {
        let mut config = BluetoothHidConfig::default();
        config.adapter = "/dev/hci0".into();
        assert!(config.validate().is_err());
        config.adapter = "hci0".into();
        config.peer = Some("not-a-mac".into());
        assert!(config.validate().is_err());
        config.peer = None;
        config.name = "蓝".repeat(24);
        assert!(config.validate().is_err());
    }
}
