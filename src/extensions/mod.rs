mod manager;
mod software;
mod types;

pub use manager::ExtensionManager;
#[cfg(unix)]
pub use manager::TTYD_SOCKET_PATH;
#[cfg(windows)]
pub use manager::TTYD_TCP_ADDR;
pub use types::*;
