//! Shared utilities.

pub mod fs;
pub mod host;
#[cfg(unix)]
pub mod net;
#[cfg(not(unix))]
#[path = "net_disabled.rs"]
pub mod net;
pub mod serial;
pub mod throttle;

pub use fs::{list_dir_names, read_trimmed};
pub use host::{hostname_from_etc, hostname_uname};
pub use net::{bind_tcp_listener, bind_udp_socket};
pub use serial::list_serial_ports;
pub use throttle::LogThrottler;
