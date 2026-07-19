use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub use crate::extensions::ExtensionsConfig;
pub use crate::rustdesk::config::RustDeskConfig;

mod atx;
mod common;
mod computer_use;
mod hid;
mod otg_network;
mod stream;
mod watchdog;
mod web;

pub use atx::*;
pub use common::*;
pub use computer_use::*;
pub use hid::*;
pub use otg_network::*;
pub use stream::*;
pub use watchdog::*;
pub use web::*;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct AppConfig {
    pub initialized: bool,
    pub auth: AuthConfig,
    pub video: VideoConfig,
    pub hid: HidConfig,
    pub otg_network: OtgNetworkConfig,
    pub msd: MsdConfig,
    pub atx: AtxConfig,
    pub audio: AudioConfig,
    pub stream: StreamConfig,
    pub web: WebConfig,
    pub computer_use: ComputerUseConfig,
    pub extensions: ExtensionsConfig,
    pub rustdesk: RustDeskConfig,
    pub vnc: VncConfig,
    pub rtsp: RtspConfig,
    pub redfish: RedfishConfig,
    pub watchdog: WatchdogConfig,
}

impl AppConfig {
    pub fn enforce_invariants(&mut self) {
        if self.hid.backend != HidBackend::Otg {
            self.msd.enabled = false;
            self.otg_network.enabled = false;
        }
        self.atx.normalize();
    }

    pub fn apply_platform_defaults(&mut self) {
        crate::platform::defaults::apply(self);
        self.enforce_invariants();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_watchdog_config_defaults_to_disabled() {
        let value = serde_json::to_value(AppConfig::default()).unwrap();
        let mut object = value.as_object().unwrap().clone();
        object.remove("watchdog");

        let config: AppConfig = serde_json::from_value(object.into()).unwrap();
        assert!(!config.watchdog.enabled);
    }
}
