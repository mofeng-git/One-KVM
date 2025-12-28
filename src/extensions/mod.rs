//! Extensions module - manage external processes like ttyd, gostc, easytier

mod manager;
mod types;

pub use manager::{ExtensionManager, TTYD_SOCKET_PATH};
pub use types::*;
