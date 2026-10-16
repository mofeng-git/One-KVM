//! KVM 输入切换器（BliSwitch / XH-HK4401）模块
//!
//! 通过串口（UART）控制 BliSwitch / XH-HK4401 多路 KVM 切换器，实现最多 8 路输入源
//! 之间的切换，并读取切换器上报的当前通道。
//!
//! 通信协议与 BliSwitch v2 官方文档《Control Protocol》一致（波特率 19200）：
//! - 切换到第 N 路：发送 `SW{N}\r\nAG{NN}gA`；
//! - 切换器周期性上报当前通道：`G{NN}gA` 帧。
//!
//! 参见 <https://www.blikvm.com/zh-CN/docs/device-guides/BliSwitch-v2-guide>。

mod controller;
mod types;

pub use controller::{SwitchController, SwitchControllerConfig};
pub use types::{
    SwitchState, SWITCH_DEFAULT_BAUD_RATE, SWITCH_DEFAULT_CHANNELS,
    SWITCH_MAX_CHANNELS,
};

/// 返回可用于 KVM 切换器的串口设备列表。
pub fn available_serial_ports() -> Vec<String> {
    crate::utils::list_serial_ports()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _: SwitchState = SwitchState::default();
    }
}
