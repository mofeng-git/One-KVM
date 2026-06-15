use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub use crate::extensions::ExtensionsConfig;
pub use crate::rustdesk::config::RustDeskConfig;

mod atx;
mod common;
mod computer_use;
mod hid;
mod stream;
mod web;

pub use atx::*;
pub use common::*;
pub use computer_use::*;
pub use hid::*;
pub use stream::*;
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
    pub msd: MsdConfig,
    pub atx: AtxConfig,
    pub audio: AudioConfig,
    pub stream: StreamConfig,
    pub web: WebConfig,
    pub computer_use: ComputerUseConfig,
    pub extensions: ExtensionsConfig,
    pub rustdesk: RustDeskConfig,
    pub rtsp: RtspConfig,
    pub redfish: RedfishConfig,
}

impl AppConfig {
    pub fn enforce_invariants(&mut self) {
        if self.hid.backend != HidBackend::Otg {
            self.msd.enabled = false;
        }
    }

    pub fn apply_platform_defaults(&mut self) {
        crate::platform::defaults::apply(self);
        self.enforce_invariants();
    }
}
