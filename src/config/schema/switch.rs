use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::switch::{
    SwitchControllerConfig, SWITCH_DEFAULT_BAUD_RATE, SWITCH_DEFAULT_CHANNELS, SWITCH_MAX_CHANNELS,
};


/// BliSwitch / XH-HK4401 KVM 输入切换器的持久化配置。
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SwitchConfig {
    pub enabled: bool,
    /// 切换器所在串口设备（如 `/dev/ttyUSB1`）。
    pub device: String,
    pub baud_rate: u32,
    /// 可切换的通道数（1..=8）。
    pub channel_count: u8,
    /// 各通道的显示名（超出 `channel_count` 的部分忽略）。
    pub channel_names: Vec<String>,
}

impl Default for SwitchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            baud_rate: SWITCH_DEFAULT_BAUD_RATE,
            channel_count: SWITCH_DEFAULT_CHANNELS,
            channel_names: Vec::new(),
        }
    }
}

impl SwitchConfig {
    /// 将配置规范化到合法范围。
    pub fn normalize(&mut self) {
        if self.device.trim().is_empty() || self.channel_count == 0 {
            self.enabled = false;
        }
        if self.baud_rate == 0 {
            self.baud_rate = SWITCH_DEFAULT_BAUD_RATE;
        }
        if self.channel_count > SWITCH_MAX_CHANNELS {
            self.channel_count = SWITCH_MAX_CHANNELS;
        }
        self.device = self.device.trim().to_string();
        self.channel_names = self
            .channel_names
            .iter()
            .take(self.channel_count as usize)
            .cloned()
            .collect();
    }

    /// 第 N 路（1 基）通道的显示名。
    pub fn channel_label(&self, channel: u8) -> String {
        let idx = (channel as usize).saturating_sub(1);
        self.channel_names
            .get(idx)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Input {}", channel))
    }

    pub fn to_controller_config(&self) -> SwitchControllerConfig {
        SwitchControllerConfig {
            enabled: self.enabled,
            device: self.device.clone(),
            baud_rate: self.baud_rate,
            channel_count: self.channel_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SwitchConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.baud_rate, 19200);
        assert_eq!(config.channel_count, 8);
    }

    #[test]
    fn test_normalize_disables_without_device() {
        let mut config = SwitchConfig::default();
        config.enabled = true;
        config.normalize();
        assert!(!config.enabled);
    }

    #[test]
    fn test_normalize_clamps_channels() {
        let mut config = SwitchConfig {
            enabled: true,
            device: "/dev/ttyUSB0".to_string(),
            channel_count: 16,
            ..Default::default()
        };
        config.normalize();
        assert_eq!(config.channel_count, SWITCH_MAX_CHANNELS);
    }

    #[test]
    fn test_channel_label_uses_name() {
        let mut config = SwitchConfig::default();
        config.channel_names = vec!["M2Pro".to_string()];
        assert_eq!(config.channel_label(1), "M2Pro");
        assert_eq!(config.channel_label(2), "Input 2");
    }
}
