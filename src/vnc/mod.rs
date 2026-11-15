#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VncServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

impl std::fmt::Display for VncServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error(err) => write!(f, "error: {}", err),
        }
    }
}


#[cfg(feature = "vnc")]
pub mod rfb;
#[cfg(feature = "vnc")]
mod service;
#[cfg(feature = "vnc")]
pub use service::VncService;
