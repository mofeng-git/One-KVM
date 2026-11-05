use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WatchdogConfig {
    pub enabled: bool,
}
