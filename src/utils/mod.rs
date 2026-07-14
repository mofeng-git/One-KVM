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

use std::net::{IpAddr, SocketAddr};

pub fn bind_socket_addr(bind: &str, port: u16) -> Result<SocketAddr, std::net::AddrParseError> {
    bind.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port))
}

#[cfg(test)]
mod tests {
    use super::bind_socket_addr;

    #[test]
    fn ipv6_bind_socket_addr_formats_ipv4_and_ipv6() {
        assert_eq!(
            bind_socket_addr("0.0.0.0", 5900).unwrap().to_string(),
            "0.0.0.0:5900"
        );
        assert_eq!(
            bind_socket_addr("127.0.0.1", 5900).unwrap().to_string(),
            "127.0.0.1:5900"
        );
        assert_eq!(
            bind_socket_addr("::", 8554).unwrap().to_string(),
            "[::]:8554"
        );
        assert_eq!(
            bind_socket_addr("::1", 8554).unwrap().to_string(),
            "[::1]:8554"
        );
        assert_eq!(
            bind_socket_addr("2001:db8::1", 8554).unwrap().to_string(),
            "[2001:db8::1]:8554"
        );
    }
}
