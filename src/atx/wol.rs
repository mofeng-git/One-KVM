//! Wake-on-LAN (WOL) implementation
//!
//! Sends magic packets to wake up remote machines.

use std::net::{SocketAddr, UdpSocket};
use tracing::{debug, info};

use crate::error::{AppError, Result};

/// WOL magic packet structure:
/// - 6 bytes of 0xFF
/// - 16 repetitions of the target MAC address (6 bytes each)
/// Total: 6 + 16 * 6 = 102 bytes
const MAGIC_PACKET_SIZE: usize = 102;

/// Parse MAC address string into bytes
/// Supports formats: "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF"
fn parse_mac_address(mac: &str) -> Result<[u8; 6]> {
    let mac = mac.trim().to_uppercase();
    let parts: Vec<&str> = if mac.contains(':') {
        mac.split(':').collect()
    } else if mac.contains('-') {
        mac.split('-').collect()
    } else {
        return Err(AppError::Config(format!("Invalid MAC address format: {}", mac)));
    };

    if parts.len() != 6 {
        return Err(AppError::Config(format!(
            "Invalid MAC address: expected 6 parts, got {}",
            parts.len()
        )));
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16).map_err(|_| {
            AppError::Config(format!("Invalid MAC address byte: {}", part))
        })?;
    }

    Ok(bytes)
}

/// Build WOL magic packet
fn build_magic_packet(mac: &[u8; 6]) -> [u8; MAGIC_PACKET_SIZE] {
    let mut packet = [0u8; MAGIC_PACKET_SIZE];

    // First 6 bytes are 0xFF
    for byte in packet.iter_mut().take(6) {
        *byte = 0xFF;
    }

    // Next 96 bytes are 16 repetitions of the MAC address
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(mac);
    }

    packet
}

/// Send WOL magic packet
///
/// # Arguments
/// * `mac_address` - Target MAC address (e.g., "AA:BB:CC:DD:EE:FF")
/// * `interface` - Optional network interface name (e.g., "eth0"). If None, uses default routing.
pub fn send_wol(mac_address: &str, interface: Option<&str>) -> Result<()> {
    let mac = parse_mac_address(mac_address)?;
    let packet = build_magic_packet(&mac);

    info!("Sending WOL packet to {} via {:?}", mac_address, interface);

    // Create UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| AppError::Internal(format!("Failed to create UDP socket: {}", e)))?;

    // Enable broadcast
    socket
        .set_broadcast(true)
        .map_err(|e| AppError::Internal(format!("Failed to enable broadcast: {}", e)))?;

    // Bind to specific interface if specified
    #[cfg(target_os = "linux")]
    if let Some(iface) = interface {
        if !iface.is_empty() {
            use std::os::unix::io::AsRawFd;
            let fd = socket.as_raw_fd();
            let iface_bytes = iface.as_bytes();

            // SO_BINDTODEVICE requires interface name as null-terminated string
            let mut iface_buf = [0u8; 16]; // IFNAMSIZ is typically 16
            let len = iface_bytes.len().min(15);
            iface_buf[..len].copy_from_slice(&iface_bytes[..len]);

            let ret = unsafe {
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_BINDTODEVICE,
                    iface_buf.as_ptr() as *const libc::c_void,
                    (len + 1) as libc::socklen_t,
                )
            };

            if ret < 0 {
                let err = std::io::Error::last_os_error();
                return Err(AppError::Internal(format!(
                    "Failed to bind to interface {}: {}",
                    iface, err
                )));
            }
            debug!("Bound to interface: {}", iface);
        }
    }

    // Send to broadcast address on port 9 (discard protocol, commonly used for WOL)
    let broadcast_addr: SocketAddr = "255.255.255.255:9".parse().unwrap();

    socket
        .send_to(&packet, broadcast_addr)
        .map_err(|e| AppError::Internal(format!("Failed to send WOL packet: {}", e)))?;

    // Also try sending to port 7 (echo protocol, alternative WOL port)
    let broadcast_addr_7: SocketAddr = "255.255.255.255:7".parse().unwrap();
    let _ = socket.send_to(&packet, broadcast_addr_7);

    info!("WOL packet sent successfully to {}", mac_address);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mac_address_colon() {
        let mac = parse_mac_address("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_address_dash() {
        let mac = parse_mac_address("aa-bb-cc-dd-ee-ff").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_parse_mac_address_invalid() {
        assert!(parse_mac_address("invalid").is_err());
        assert!(parse_mac_address("AA:BB:CC:DD:EE").is_err());
        assert!(parse_mac_address("AA:BB:CC:DD:EE:GG").is_err());
    }

    #[test]
    fn test_build_magic_packet() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let packet = build_magic_packet(&mac);

        // Check header (6 bytes of 0xFF)
        for i in 0..6 {
            assert_eq!(packet[i], 0xFF);
        }

        // Check MAC repetitions
        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }
    }
}
