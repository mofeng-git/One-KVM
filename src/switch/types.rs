//! KVM 切换器运行时类型与协议常量。

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

/// 默认串口波特率（BliSwitch / XH-HK4401 固定为 19200）。
pub const SWITCH_DEFAULT_BAUD_RATE: u32 = 19200;
/// 默认通道数（BliSwitch v2 支持 8 路）。
pub const SWITCH_DEFAULT_CHANNELS: u8 = 8;
/// 协议支持的最大通道数。
pub const SWITCH_MAX_CHANNELS: u8 = 8;

/// KVM 切换器的运行时状态。
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SwitchState {
    pub available: bool,
    /// 串口设备当前是否已打开并可读。
    pub connected: bool,
    pub device: String,
    pub baud_rate: u32,
    pub channel_count: u8,
    /// 当前激活通道（1 基），未知时为 `None`。
    pub current_channel: Option<u8>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_baud_rate() {
        assert_eq!(SWITCH_DEFAULT_BAUD_RATE, 19200);
    }

}
