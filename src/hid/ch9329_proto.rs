//! Shared CH9329 protocol types and packet helpers.

use serde::{Deserialize, Serialize};

const PACKET_HEADER: [u8; 2] = [0x57, 0xAB];
pub const RESPONSE_SUCCESS_MASK: u8 = 0x80;
pub const RESPONSE_ERROR_MASK: u8 = 0xC0;

pub const DEFAULT_ADDR: u8 = 0x00;
pub const DEFAULT_BAUD_RATE: u32 = 9600;
pub const MAX_DATA_LEN: usize = 64;
pub const MAX_PACKET_SIZE: usize = 70;
const EXTENDED_PARAMETER_RESPONSE_SIZES: [usize; 2] = [72, 88];

pub mod cmd {
    pub const GET_INFO: u8 = 0x01;
    pub const SEND_KB_GENERAL_DATA: u8 = 0x02;
    pub const SEND_KB_MEDIA_DATA: u8 = 0x03;
    pub const SEND_MS_ABS_DATA: u8 = 0x04;
    pub const SEND_MS_REL_DATA: u8 = 0x05;
    pub const GET_PARA_CFG: u8 = 0x08;
    pub const SET_PARA_CFG: u8 = 0x09;
    pub const GET_USB_STRING: u8 = 0x0A;
    pub const SET_USB_STRING: u8 = 0x0B;
    pub const RESET: u8 = 0x0F;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ch9329Error {
    Success = 0x00,
    Timeout = 0xE1,
    InvalidHeader = 0xE2,
    InvalidCommand = 0xE3,
    ChecksumError = 0xE4,
    ParameterError = 0xE5,
    OperationFailed = 0xE6,
}

impl From<u8> for Ch9329Error {
    fn from(code: u8) -> Self {
        match code {
            0x00 => Ch9329Error::Success,
            0xE1 => Ch9329Error::Timeout,
            0xE2 => Ch9329Error::InvalidHeader,
            0xE3 => Ch9329Error::InvalidCommand,
            0xE4 => Ch9329Error::ChecksumError,
            0xE5 => Ch9329Error::ParameterError,
            0xE6 => Ch9329Error::OperationFailed,
            _ => Ch9329Error::OperationFailed,
        }
    }
}

impl std::fmt::Display for Ch9329Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ch9329Error::Success => write!(f, "Success"),
            Ch9329Error::Timeout => write!(f, "Serial receive timeout"),
            Ch9329Error::InvalidHeader => write!(f, "Invalid packet header"),
            Ch9329Error::InvalidCommand => write!(f, "Invalid command code"),
            Ch9329Error::ChecksumError => write!(f, "Checksum mismatch"),
            Ch9329Error::ParameterError => write!(f, "Parameter error"),
            Ch9329Error::OperationFailed => write!(f, "Operation failed"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChipInfo {
    pub version: String,
    pub version_raw: u8,
    pub usb_connected: bool,
    pub num_lock: bool,
    pub caps_lock: bool,
    pub scroll_lock: bool,
}

impl ChipInfo {
    pub fn from_response(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let version_raw = data[0];
        let version = format!("V{}.{}", version_raw >> 4, version_raw & 0x0F);
        let usb_connected = data[1] == 0x01;
        let led_status = data[2];

        Some(Self {
            version,
            version_raw,
            usb_connected,
            num_lock: (led_status & 0x01) != 0,
            caps_lock: (led_status & 0x02) != 0,
            scroll_lock: (led_status & 0x04) != 0,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedStatus {
    pub num_lock: bool,
    pub caps_lock: bool,
    pub scroll_lock: bool,
}

impl From<u8> for LedStatus {
    fn from(byte: u8) -> Self {
        Self {
            num_lock: (byte & 0x01) != 0,
            caps_lock: (byte & 0x02) != 0,
            scroll_lock: (byte & 0x04) != 0,
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub cmd: u8,
    pub data: Vec<u8>,
    pub is_error: bool,
    pub error_code: Option<Ch9329Error>,
}

impl Response {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 6 || bytes[0] != PACKET_HEADER[0] || bytes[1] != PACKET_HEADER[1] {
            return None;
        }

        let cmd = bytes[3];
        let len = bytes[4] as usize;
        let expected_frame_len = 6 + len;
        if bytes.len() < expected_frame_len {
            return None;
        }

        let expected_checksum = bytes[5 + len];
        let calculated_checksum = bytes[..5 + len]
            .iter()
            .fold(0u8, |acc, &x| acc.wrapping_add(x));
        if expected_checksum != calculated_checksum {
            tracing::debug!(
                "CH9329 checksum mismatch: expected {:02X}, got {:02X}",
                expected_checksum,
                calculated_checksum
            );
            return None;
        }

        let data = bytes[5..5 + len].to_vec();
        let is_error = (cmd & RESPONSE_ERROR_MASK) == RESPONSE_ERROR_MASK;
        let error_code = if is_error && !data.is_empty() {
            Some(Ch9329Error::from(data[0]))
        } else {
            None
        };

        Some(Self {
            cmd,
            data,
            is_error,
            error_code,
        })
    }
}

#[inline]
pub fn calculate_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x))
}

#[inline]
pub fn build_packet_buf(address: u8, cmd: u8, data: &[u8]) -> ([u8; MAX_PACKET_SIZE], usize) {
    debug_assert!(
        data.len() <= MAX_DATA_LEN,
        "Data too long for CH9329 packet"
    );

    let len = data.len() as u8;
    let packet_len = 6 + data.len();
    let mut packet = [0u8; MAX_PACKET_SIZE];

    packet[0] = PACKET_HEADER[0];
    packet[1] = PACKET_HEADER[1];
    packet[2] = address;
    packet[3] = cmd;
    packet[4] = len;
    packet[5..5 + data.len()].copy_from_slice(data);
    packet[5 + data.len()] = calculate_checksum(&packet[..5 + data.len()]);

    (packet, packet_len)
}

#[inline]
pub fn build_packet(address: u8, cmd: u8, data: &[u8]) -> Vec<u8> {
    let (buf, len) = build_packet_buf(address, cmd, data);
    buf[..len].to_vec()
}

#[inline]
pub fn expected_response_cmd(cmd: u8, is_error: bool) -> u8 {
    cmd | if is_error {
        RESPONSE_ERROR_MASK
    } else {
        RESPONSE_SUCCESS_MASK
    }
}

pub fn try_extract_response(buffer: &[u8]) -> Option<(Response, usize)> {
    let mut offset = 0;
    while offset + 6 <= buffer.len() {
        if buffer[offset] != PACKET_HEADER[0] || buffer[offset + 1] != PACKET_HEADER[1] {
            offset += 1;
            continue;
        }

        let len = buffer[offset + 4] as usize;
        if len > MAX_DATA_LEN {
            offset += 1;
            continue;
        }

        let frame_len = 6 + len;
        if offset + frame_len > buffer.len() {
            return None;
        }

        let frame = &buffer[offset..offset + frame_len];
        if let Some(response) = Response::parse(frame) {
            return Some((response, offset + frame_len));
        }

        // Some CH9329F firmware appends reserved bytes to GET_PARA_CFG while
        // retaining the protocol LEN value of 50. Locate and validate the real
        // checksum, return the documented 50-byte payload, and consume the
        // complete extended frame. Other commands keep strict framing.
        let cmd = buffer[offset + 3];
        let data_start = offset + 5;
        let parameter_payload_is_plausible = cmd == expected_response_cmd(cmd::GET_PARA_CFG, false)
            && len == 50
            && matches!(buffer[data_start], 0x00..=0x03 | 0x80..=0x83)
            && matches!(buffer[data_start + 1], 0x00..=0x02 | 0x80..=0x82);
        if parameter_payload_is_plausible {
            for extended_size in EXTENDED_PARAMETER_RESPONSE_SIZES {
                let extended_end = offset + extended_size;
                if buffer.len() >= extended_end {
                    let checksum_index = extended_end - 1;
                    if calculate_checksum(&buffer[offset..checksum_index]) != buffer[checksum_index]
                    {
                        continue;
                    }
                    return Some((
                        Response {
                            cmd,
                            data: buffer[data_start..data_start + len].to_vec(),
                            is_error: false,
                            error_code: None,
                        },
                        extended_end,
                    ));
                }
            }

            if buffer.len() < offset + EXTENDED_PARAMETER_RESPONSE_SIZES[1] {
                return None;
            }
        }

        offset += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_response_and_checksum() {
        let frame = build_packet(DEFAULT_ADDR, 0x81, &[0x30, 0x01, 0x00, 0, 0, 0, 0, 0]);
        let response = Response::parse(&frame).expect("valid response");
        assert_eq!(response.cmd, 0x81);
        assert_eq!(response.data, vec![0x30, 0x01, 0x00, 0, 0, 0, 0, 0]);
        assert!(!response.is_error);
    }

    #[test]
    fn extracts_extended_parameter_response_with_valid_trailing_checksum() {
        let payload = [
            0x80, 0x80, 0x00, 0x00, 0x00, 0x25, 0x80, 0x08, 0x00, 0x00, 0x03, 0x86, 0x1A, 0x2A,
            0xE1, 0x00, 0x00, 0x00, 0x01, 0x00, 0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        for reserved_len in [16, 32] {
            let mut frame = vec![0x57, 0xAB, DEFAULT_ADDR, 0x88, 50];
            frame.extend_from_slice(&payload);
            frame.extend_from_slice(&vec![0; reserved_len]);
            frame.push(calculate_checksum(&frame));

            assert!(Response::parse(&frame[..56]).is_none());
            let (response, consumed) = try_extract_response(&frame).expect("extended response");
            assert_eq!(response.cmd, 0x88);
            assert_eq!(response.data, payload);
            assert!(!response.is_error);
            assert_eq!(consumed, frame.len());
        }
    }

    #[test]
    fn rejects_bad_checksum_for_other_commands() {
        let mut frame = build_packet(DEFAULT_ADDR, 0x89, &[0x00; 50]);
        *frame.last_mut().unwrap() ^= 0xFF;
        assert!(Response::parse(&frame).is_none());
        assert!(try_extract_response(&frame).is_none());
    }

    #[test]
    fn extracts_noise_and_adjacent_packets() {
        let first = build_packet(DEFAULT_ADDR, 0x81, &[0x30, 0x01, 0, 0, 0, 0, 0, 0]);
        let second = build_packet(DEFAULT_ADDR, 0x82, &[0x00]);
        let mut buffer = vec![0x00, 0xFF];
        buffer.extend_from_slice(&first);
        buffer.extend_from_slice(&second);

        let (_, consumed) = try_extract_response(&buffer).expect("first response");
        assert_eq!(consumed, 2 + first.len());
        let (response, _) = try_extract_response(&buffer[consumed..]).expect("second response");
        assert_eq!(response.cmd, 0x82);
    }
}
