use std::path::PathBuf;

use super::ExtensionId;

#[cfg_attr(windows, path = "software_windows.rs")]
#[cfg_attr(not(windows), path = "software_linux.rs")]
mod platform;

pub fn binary_path(id: ExtensionId) -> PathBuf {
    platform::binary_path(id)
}

pub fn default_ttyd_shell() -> &'static str {
    platform::default_ttyd_shell()
}
