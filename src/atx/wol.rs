//! Wake-on-LAN (WOL) implementation
//!
//! Sends magic packets to wake up remote machines.

use std::net::{SocketAddr, UdpSocket};
use tracing::info;

use crate::error::{AppError, Result};

const WOL_HISTORY_MAX_ENTRIES: i64 = 50;

const MAGIC_PACKET_SIZE: usize = 102;

fn parse_mac_address(mac: &str) -> Result<[u8; 6]> {
    let mac = mac.trim().to_uppercase();
    let parts: Vec<&str> = if mac.contains(':') {
        mac.split(':').collect()
    } else if mac.contains('-') {
        mac.split('-').collect()
    } else {
        return Err(AppError::Config(format!(
            "Invalid MAC address format: {}",
            mac
        )));
    };

    if parts.len() != 6 {
        return Err(AppError::Config(format!(
            "Invalid MAC address: expected 6 parts, got {}",
            parts.len()
        )));
    }

    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16)
            .map_err(|_| AppError::Config(format!("Invalid MAC address byte: {}", part)))?;
    }

    Ok(bytes)
}

fn build_magic_packet(mac: &[u8; 6]) -> [u8; MAGIC_PACKET_SIZE] {
    let mut packet = [0u8; MAGIC_PACKET_SIZE];

    for byte in packet.iter_mut().take(6) {
        *byte = 0xFF;
    }

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

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| AppError::Internal(format!("Failed to create UDP socket: {}", e)))?;

    socket
        .set_broadcast(true)
        .map_err(|e| AppError::Internal(format!("Failed to enable broadcast: {}", e)))?;

    #[cfg(target_os = "linux")]
    if let Some(iface) = interface {
        if !iface.is_empty() {
            use std::os::unix::io::AsRawFd;
            let fd = socket.as_raw_fd();
            let iface_bytes = iface.as_bytes();

            let mut iface_buf = [0u8; 16];
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
            tracing::debug!("Bound to interface: {}", iface);
        }
    }

    let broadcast_addr: SocketAddr = "255.255.255.255:9".parse().unwrap();

    socket
        .send_to(&packet, broadcast_addr)
        .map_err(|e| AppError::Internal(format!("Failed to send WOL packet: {}", e)))?;

    let broadcast_addr_7: SocketAddr = "255.255.255.255:7".parse().unwrap();
    let _ = socket.send_to(&packet, broadcast_addr_7);

    info!("WOL packet sent successfully to {}", mac_address);
    Ok(())
}

pub async fn record_wol_history(pool: &sqlx::Pool<sqlx::Sqlite>, mac_address: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO wol_history (mac_address, updated_at)
        VALUES (?1, CAST(strftime('%s', 'now') AS INTEGER))
        ON CONFLICT(mac_address) DO UPDATE SET
            updated_at = excluded.updated_at
        "#,
    )
    .bind(mac_address)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM wol_history
        WHERE mac_address NOT IN (
            SELECT mac_address FROM wol_history
            ORDER BY updated_at DESC
            LIMIT ?1
        )
        "#,
    )
    .bind(WOL_HISTORY_MAX_ENTRIES)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_wol_history(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    limit: usize,
) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query_as(
        r#"
        SELECT mac_address, updated_at
        FROM wol_history
        ORDER BY updated_at DESC
        LIMIT ?1
        "#,
    )
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows)
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

        for byte in packet.iter().take(6) {
            assert_eq!(*byte, 0xFF);
        }

        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }
    }
}
