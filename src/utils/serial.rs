//! Cross-platform serial port discovery helpers.

/// Return serial port names that users can put directly into the config.
pub fn list_serial_ports() -> Vec<String> {
    let mut ports: Vec<String> = serialport::available_ports()
        .map(|ports| ports.into_iter().map(|port| port.port_name).collect())
        .unwrap_or_default();

    ports.sort();
    ports.dedup();
    ports
}
