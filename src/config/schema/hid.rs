use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum HidBackend {
    Otg,
    Ch9329,
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum OtgEndpointBudget {
    #[default]
    Auto,
    Five,
    Six,
    Unlimited,
}

impl OtgEndpointBudget {
    pub fn endpoint_limit_raw(&self) -> Option<u8> {
        match self {
            Self::Five => Some(5),
            Self::Six => Some(6),
            Self::Unlimited => None,
            Self::Auto => None,
        }
    }
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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HidConfig {
    pub backend: HidBackend,
    pub otg_udc: Option<String>,
    #[serde(default)]
    pub otg_descriptor: OtgDescriptorConfig,
    #[serde(default)]
    pub otg_profile: OtgHidProfile,
    #[serde(default)]
    pub otg_endpoint_budget: OtgEndpointBudget,
    #[serde(default)]
    pub otg_functions: OtgHidFunctions,
    #[serde(default)]
    pub otg_keyboard_leds: bool,
    pub ch9329_port: String,
    pub ch9329_baudrate: u32,
    #[serde(default)]
    pub ch9329_hybrid_mouse: bool,
    #[serde(default)]
    pub ch9329_descriptor: Ch9329DescriptorConfig,
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
            ch9329_hybrid_mouse: false,
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

    pub fn effective_otg_required_endpoints(&self, msd_enabled: bool) -> u8 {
        let functions = self.effective_otg_functions();
        let mut endpoints = functions.endpoint_cost(self.effective_otg_keyboard_leds());
        if msd_enabled {
            endpoints += 2;
        }
        endpoints
    }

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
                #[cfg(unix)]
                let udc = self.resolved_otg_udc().unwrap_or_default();
                #[cfg(unix)]
                if crate::otg::configfs::is_low_endpoint_udc(&udc) {
                    Some(5)
                } else {
                    Some(6)
                }
                #[cfg(not(unix))]
                {
                    Some(6)
                }
            }
        }
    }
}
