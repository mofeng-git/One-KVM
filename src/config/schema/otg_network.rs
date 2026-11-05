use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OtgNetworkDriverMode {
    #[default]
    Ncm,
    Ecm,
    Rndis,
}

impl OtgNetworkDriverMode {
    pub fn function_name(self) -> &'static str {
        match self {
            Self::Ncm => "ncm",
            Self::Ecm => "ecm",
            Self::Rndis => "rndis",
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct OtgNetworkConfig {
    pub enabled: bool,
    pub driver_mode: OtgNetworkDriverMode,
    /// Empty means select the connected NetworkManager Ethernet interface.
    pub bridge_interface: String,
    /// Empty values are resolved from the machine identity at runtime.
    pub host_mac: String,
    pub device_mac: String,
}

impl OtgNetworkConfig {
    pub fn validate(&self) -> crate::error::Result<()> {
        for (name, value) in [
            ("host_mac", self.host_mac.as_str()),
            ("device_mac", self.device_mac.as_str()),
        ] {
            if !value.is_empty() && !is_valid_unicast_mac(value) {
                return Err(crate::error::AppError::BadRequest(format!(
                    "OTG network {name} must be a locally administered unicast MAC address"
                )));
            }
        }
        if !self.host_mac.is_empty()
            && !self.device_mac.is_empty()
            && self.host_mac.eq_ignore_ascii_case(&self.device_mac)
        {
            return Err(crate::error::AppError::BadRequest(
                "OTG network host_mac and device_mac must be different".to_string(),
            ));
        }
        if self.bridge_interface.contains('/') || self.bridge_interface.contains('\0') {
            return Err(crate::error::AppError::BadRequest(
                "Invalid OTG network bridge interface".to_string(),
            ));
        }
        Ok(())
    }
}

fn is_valid_unicast_mac(value: &str) -> bool {
    let bytes = value
        .split(':')
        .map(|part| u8::from_str_radix(part, 16))
        .collect::<Result<Vec<_>, _>>();
    matches!(bytes, Ok(ref bytes) if bytes.len() == 6 && bytes[0] & 0x03 == 0x02)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_local_unicast_mac_addresses() {
        assert!(is_valid_unicast_mac("02:00:00:00:10:01"));
        assert!(!is_valid_unicast_mac("01:00:00:00:10:01"));
        assert!(!is_valid_unicast_mac("00:00:00:00:10:01"));
        assert!(!is_valid_unicast_mac("bad"));
    }
}
