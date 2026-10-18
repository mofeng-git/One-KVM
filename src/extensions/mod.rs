mod manager;
mod protected_config;
mod software;
mod types;
mod validation;

pub use manager::ExtensionManager;
#[cfg(unix)]
pub use manager::TTYD_SOCKET_PATH;
#[cfg(windows)]
pub use manager::TTYD_TCP_ADDR;
pub use types::*;
pub(crate) use validation::{
    validate_easyfrp_config, validate_easytier_config, validate_extension_config,
    validate_frpc_config,
};
