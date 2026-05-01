//! CH9329 over UART — WCH *Serial Communication Protocol V1.0*.
//! ```text
//! ┌──────┬──────┬──────┬────────┬──────────────┬──────────┐
//! │Header│ ADDR │ CMD  │  LEN   │     DATA     │   SUM    │
//! ├──────┼──────┼──────┼────────┼──────────────┼──────────┤
//! │57 AB │ 00   │ xx   │   N    │   N bytes    │Checksum  │
//! └──────┴──────┴──────┴────────┴──────────────┴──────────┘
//! ```
//! Sum of all octets modulo 256 (including header).

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{info, trace, warn};

use super::backend::{HidBackend, HidBackendRuntimeSnapshot};
use super::types::{KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent, MouseEventType};
use crate::error::{AppError, Result};
use crate::events::LedState;

const PACKET_HEADER: [u8; 2] = [0x57, 0xAB];

const DEFAULT_ADDR: u8 = 0x00;

pub const DEFAULT_BAUD_RATE: u32 = 9600;

const RESPONSE_TIMEOUT_MS: u64 = 500;

const MAX_DATA_LEN: usize = 64;

const CH9329_MOUSE_RESOLUTION: u32 = 4096;

const PROBE_INTERVAL_MS: u64 = 100;

const RECONNECT_DELAY_MS: u64 = 2000;

const INIT_WAIT_MS: u64 = 3000;

pub mod cmd {
    pub const GET_INFO: u8 = 0x01;
    pub const SEND_KB_GENERAL_DATA: u8 = 0x02;
    pub const SEND_KB_MEDIA_DATA: u8 = 0x03;
    pub const SEND_MS_ABS_DATA: u8 = 0x04;
    pub const SEND_MS_REL_DATA: u8 = 0x05;
    pub const SEND_MY_HID_DATA: u8 = 0x06;
    pub const SET_DEFAULT_CFG: u8 = 0x0C;
    pub const RESET: u8 = 0x0F;
}

const RESPONSE_SUCCESS_MASK: u8 = 0x80;
const RESPONSE_ERROR_MASK: u8 = 0xC0;

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
    pub address: u8,
    pub cmd: u8,
    pub data: Vec<u8>,
    pub is_error: bool,
    pub error_code: Option<Ch9329Error>,
}

impl Response {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 6 {
            return None;
        }

        if bytes[0] != PACKET_HEADER[0] || bytes[1] != PACKET_HEADER[1] {
            return None;
        }

        let address = bytes[2];
        let cmd = bytes[3];
        let len = bytes[4] as usize;

        if bytes.len() < 5 + len + 1 {
            return None;
        }

        let expected_checksum = bytes[5 + len];
        let calculated_checksum = bytes[..5 + len]
            .iter()
            .fold(0u8, |acc, &x| acc.wrapping_add(x));

        if expected_checksum != calculated_checksum {
            warn!(
                "CH9329 checksum mismatch: expected {:02X}, got {:02X}",
                expected_checksum, calculated_checksum
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
            address,
            cmd,
            data,
            is_error,
            error_code,
        })
    }

    pub fn is_success(&self) -> bool {
        !self.is_error && (self.data.is_empty() || self.data[0] == Ch9329Error::Success as u8)
    }
}

const MAX_PACKET_SIZE: usize = 70;

struct Ch9329RuntimeState {
    initialized: AtomicBool,
    online: AtomicBool,
    last_error: RwLock<Option<(String, String)>>,
    notify_tx: watch::Sender<()>,
}

impl Ch9329RuntimeState {
    fn new() -> Self {
        let (notify_tx, _notify_rx) = watch::channel(());
        Self {
            initialized: AtomicBool::new(false),
            online: AtomicBool::new(false),
            last_error: RwLock::new(None),
            notify_tx,
        }
    }

    fn subscribe(&self) -> watch::Receiver<()> {
        self.notify_tx.subscribe()
    }

    fn notify(&self) {
        let _ = self.notify_tx.send(());
    }

    fn clear_error(&self) {
        let mut guard = self.last_error.write();
        if guard.is_some() {
            *guard = None;
            self.notify();
        }
    }

    fn set_online(&self) {
        let was_online = self.online.swap(true, Ordering::Relaxed);
        let mut error = self.last_error.write();
        let cleared_error = error.take().is_some();
        drop(error);
        if !was_online || cleared_error {
            self.notify();
        }
    }

    fn set_error(&self, reason: impl Into<String>, error_code: impl Into<String>) {
        let reason = reason.into();
        let error_code = error_code.into();
        let was_online = self.online.swap(false, Ordering::Relaxed);
        let mut error = self.last_error.write();
        let changed = error.as_ref() != Some(&(reason.clone(), error_code.clone()));
        *error = Some((reason, error_code));
        drop(error);
        if was_online || changed {
            self.notify();
        }
    }

    fn set_initialized(&self, initialized: bool) {
        if self.initialized.swap(initialized, Ordering::Relaxed) != initialized {
            self.notify();
        }
    }

    fn set_offline(&self) {
        if self.online.swap(false, Ordering::Relaxed) {
            self.notify();
        }
    }
}

enum WorkerCommand {
    Packet { cmd: u8, data: Vec<u8> },
    ResetState,
    Shutdown,
}

pub struct Ch9329Backend {
    port_path: String,
    baud_rate: u32,
    worker_tx: Mutex<Option<mpsc::Sender<WorkerCommand>>>,
    worker_handle: Mutex<Option<thread::JoinHandle<()>>>,
    keyboard_state: Mutex<KeyboardReport>,
    mouse_buttons: AtomicU8,
    screen_resolution: RwLock<(u32, u32)>,
    chip_info: Arc<RwLock<Option<ChipInfo>>>,
    led_status: Arc<RwLock<LedStatus>>,
    address: u8,
    last_abs_x: AtomicU16,
    last_abs_y: AtomicU16,
    relative_mouse_active: AtomicBool,
    runtime: Arc<Ch9329RuntimeState>,
}

impl Ch9329Backend {
    pub fn new(port_path: &str) -> Result<Self> {
        Self::with_baud_rate(port_path, DEFAULT_BAUD_RATE)
    }

    pub fn with_baud_rate(port_path: &str, baud_rate: u32) -> Result<Self> {
        Ok(Self {
            port_path: port_path.to_string(),
            baud_rate,
            worker_tx: Mutex::new(None),
            worker_handle: Mutex::new(None),
            keyboard_state: Mutex::new(KeyboardReport::default()),
            mouse_buttons: AtomicU8::new(0),
            screen_resolution: RwLock::new((1920, 1080)),
            chip_info: Arc::new(RwLock::new(None)),
            led_status: Arc::new(RwLock::new(LedStatus::default())),
            address: DEFAULT_ADDR,
            last_abs_x: AtomicU16::new(0),
            last_abs_y: AtomicU16::new(0),
            relative_mouse_active: AtomicBool::new(false),
            runtime: Arc::new(Ch9329RuntimeState::new()),
        })
    }

    fn record_error(&self, reason: impl Into<String>, error_code: impl Into<String>) {
        self.runtime.set_error(reason, error_code);
    }

    pub fn check_port_exists(&self) -> bool {
        std::path::Path::new(&self.port_path).exists()
    }

    fn serial_error_to_hid_error(e: serialport::Error, operation: &str) -> AppError {
        let error_code = match e.kind() {
            serialport::ErrorKind::NoDevice => "port_not_found",
            serialport::ErrorKind::InvalidInput => "invalid_config",
            serialport::ErrorKind::Io(_) => "io_error",
            _ => "serial_error",
        };

        AppError::HidError {
            backend: "ch9329".to_string(),
            reason: format!("{}: {}", operation, e),
            error_code: error_code.to_string(),
        }
    }

    fn backend_error(reason: impl Into<String>, error_code: impl Into<String>) -> AppError {
        AppError::HidError {
            backend: "ch9329".to_string(),
            reason: reason.into(),
            error_code: error_code.into(),
        }
    }

    #[inline]
    fn calculate_checksum(data: &[u8]) -> u8 {
        data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x))
    }

    #[inline]
    fn build_packet_buf(address: u8, cmd: u8, data: &[u8]) -> ([u8; MAX_PACKET_SIZE], usize) {
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
        let checksum = Self::calculate_checksum(&packet[..5 + data.len()]);
        packet[5 + data.len()] = checksum;

        (packet, packet_len)
    }

    fn build_packet(address: u8, cmd: u8, data: &[u8]) -> Vec<u8> {
        let (buf, len) = Self::build_packet_buf(address, cmd, data);
        buf[..len].to_vec()
    }

    fn open_port(port_path: &str, baud_rate: u32) -> Result<Box<dyn serialport::SerialPort>> {
        if !std::path::Path::new(port_path).exists() {
            return Err(Self::backend_error(
                format!("Serial port {} not found", port_path),
                "port_not_found",
            ));
        }

        serialport::new(port_path, baud_rate)
            .timeout(Duration::from_millis(RESPONSE_TIMEOUT_MS))
            .open()
            .map_err(|e| Self::serial_error_to_hid_error(e, "Failed to open serial port"))
    }

    fn write_packet(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        cmd: u8,
        data: &[u8],
    ) -> Result<()> {
        let packet = Self::build_packet(address, cmd, data);
        port.write_all(&packet).map_err(|e| {
            Self::backend_error(format!("Failed to write to CH9329: {}", e), "write_failed")
        })?;
        Ok(())
    }

    fn try_extract_response(buffer: &[u8]) -> Option<(Response, usize)> {
        let mut offset = 0;
        while offset + 6 <= buffer.len() {
            if buffer[offset] != PACKET_HEADER[0] || buffer[offset + 1] != PACKET_HEADER[1] {
                offset += 1;
                continue;
            }

            let len = buffer[offset + 4] as usize;
            let frame_len = 6 + len;
            if offset + frame_len > buffer.len() {
                return None;
            }

            let frame = &buffer[offset..offset + frame_len];
            if let Some(response) = Response::parse(frame) {
                return Some((response, offset + frame_len));
            }

            offset += 1;
        }

        None
    }

    fn expected_response_cmd(cmd: u8, is_error: bool) -> u8 {
        cmd | if is_error {
            RESPONSE_ERROR_MASK
        } else {
            RESPONSE_SUCCESS_MASK
        }
    }

    fn xfer_packet(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        cmd: u8,
        data: &[u8],
    ) -> Result<Response> {
        Self::write_packet(port, address, cmd, data)?;

        let mut pending = Vec::with_capacity(128);
        let deadline = Instant::now() + Duration::from_millis(RESPONSE_TIMEOUT_MS);
        let expected_ok = Self::expected_response_cmd(cmd, false);
        let expected_err = Self::expected_response_cmd(cmd, true);

        loop {
            let mut chunk = [0u8; 128];
            match port.read(&mut chunk) {
                Ok(n) if n > 0 => {
                    pending.extend_from_slice(&chunk[..n]);

                    while let Some((response, consumed)) = Self::try_extract_response(&pending) {
                        pending.drain(..consumed);
                        if response.cmd == expected_ok || response.cmd == expected_err {
                            return Ok(response);
                        }

                        trace!(
                            "CH9329 ignored out-of-order response: expected 0x{:02X}/0x{:02X}, got 0x{:02X}",
                            expected_ok,
                            expected_err,
                            response.cmd
                        );
                    }

                    if pending.len() > MAX_PACKET_SIZE * 4 {
                        let keep = MAX_PACKET_SIZE;
                        pending.drain(..pending.len().saturating_sub(keep));
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    return Err(Self::backend_error(
                        format!("Failed to read from CH9329: {}", e),
                        "read_failed",
                    ));
                }
            }

            if Instant::now() >= deadline {
                return Err(Self::backend_error(
                    format!("No matching response from CH9329 for cmd 0x{:02X}", cmd),
                    "no_response",
                ));
            }

            thread::sleep(Duration::from_millis(1));
        }
    }

    fn try_best_effort_reset(port: &mut dyn serialport::SerialPort, address: u8) {
        if let Err(err) = Self::write_packet(port, address, cmd::RESET, &[]) {
            trace!("CH9329 best-effort reset failed: {}", err);
        }
    }

    fn query_chip_info_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
    ) -> Result<ChipInfo> {
        let response = Self::xfer_packet(port, address, cmd::GET_INFO, &[])?;
        if response.is_error {
            let reason = response
                .error_code
                .map(|e| format!("CH9329 error response: {}", e))
                .unwrap_or_else(|| "CH9329 returned error response".to_string());
            return Err(Self::backend_error(reason, "protocol_error"));
        }

        ChipInfo::from_response(&response.data)
            .ok_or_else(|| Self::backend_error("Failed to parse chip info", "invalid_response"))
    }

    fn update_chip_info_cache(
        chip_info: &Arc<RwLock<Option<ChipInfo>>>,
        led_status: &Arc<RwLock<LedStatus>>,
        info: ChipInfo,
    ) -> bool {
        let next_led_status = LedStatus {
            num_lock: info.num_lock,
            caps_lock: info.caps_lock,
            scroll_lock: info.scroll_lock,
        };
        *chip_info.write() = Some(info);
        let mut led_guard = led_status.write();
        let changed = *led_guard != next_led_status;
        *led_guard = next_led_status;
        changed
    }

    fn enqueue_command(&self, command: WorkerCommand) -> Result<()> {
        let guard = self.worker_tx.lock();
        let Some(sender) = guard.as_ref() else {
            self.record_error("CH9329 worker is not running", "worker_stopped");
            return Err(Self::backend_error(
                "CH9329 worker is not running",
                "worker_stopped",
            ));
        };

        sender.send(command).map_err(|_| {
            self.record_error("CH9329 worker stopped", "worker_stopped");
            Self::backend_error("CH9329 worker stopped", "worker_stopped")
        })
    }

    fn send_packet(&self, cmd: u8, data: &[u8]) -> Result<()> {
        self.enqueue_command(WorkerCommand::Packet {
            cmd,
            data: data.to_vec(),
        })
    }

    fn worker_reconnect_loop(
        rx: &mpsc::Receiver<WorkerCommand>,
        port_path: &str,
        baud_rate: u32,
        address: u8,
        chip_info: &Arc<RwLock<Option<ChipInfo>>>,
        led_status: &Arc<RwLock<LedStatus>>,
        runtime: &Arc<Ch9329RuntimeState>,
    ) -> Option<Box<dyn serialport::SerialPort>> {
        loop {
            match rx.recv_timeout(Duration::from_millis(RECONNECT_DELAY_MS)) {
                Ok(WorkerCommand::Shutdown) => return None,
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            match Self::open_port(port_path, baud_rate).and_then(|mut port| {
                let info = Self::query_chip_info_on_port(port.as_mut(), address)?;
                Ok((port, info))
            }) {
                Ok((port, info)) => {
                    info!(
                        "CH9329 reconnected: {}, USB: {}",
                        info.version,
                        if info.usb_connected {
                            "connected"
                        } else {
                            "disconnected"
                        }
                    );
                    if Self::update_chip_info_cache(chip_info, led_status, info) {
                        runtime.notify();
                    }
                    runtime.set_online();
                    return Some(port);
                }
                Err(err) => {
                    if let AppError::HidError {
                        reason, error_code, ..
                    } = err
                    {
                        runtime.set_error(reason, error_code);
                    }
                }
            }
        }
    }

    fn send_keyboard_report(&self, report: &KeyboardReport) -> Result<()> {
        let data = report.to_bytes();
        self.send_packet(cmd::SEND_KB_GENERAL_DATA, &data)
    }

    pub fn send_media_key(&self, data: &[u8]) -> Result<()> {
        if data.len() < 2 || data.len() > 4 {
            return Err(AppError::Internal(
                "Invalid media key data length".to_string(),
            ));
        }
        self.send_packet(cmd::SEND_KB_MEDIA_DATA, data)
    }

    pub fn release_media_keys(&self) -> Result<()> {
        self.send_media_key(&[0x02, 0x00, 0x00, 0x00])
    }

    fn send_mouse_relative(&self, buttons: u8, dx: i8, dy: i8, wheel: i8) -> Result<()> {
        let data = [0x01, buttons, dx as u8, dy as u8, wheel as u8];
        self.send_packet(cmd::SEND_MS_REL_DATA, &data)
    }

    fn send_mouse_absolute(&self, buttons: u8, x: u16, y: u16, wheel: i8) -> Result<()> {
        let data = [
            0x02,
            buttons,
            (x & 0xFF) as u8,
            (x >> 8) as u8,
            (y & 0xFF) as u8,
            (y >> 8) as u8,
            wheel as u8,
        ];
        self.send_packet(cmd::SEND_MS_ABS_DATA, &data)?;
        trace!("CH9329 mouse: buttons=0x{:02X} pos=({},{})", buttons, x, y);
        Ok(())
    }

    fn worker_loop(
        port_path: String,
        baud_rate: u32,
        address: u8,
        rx: mpsc::Receiver<WorkerCommand>,
        chip_info: Arc<RwLock<Option<ChipInfo>>>,
        led_status: Arc<RwLock<LedStatus>>,
        runtime: Arc<Ch9329RuntimeState>,
        init_tx: mpsc::Sender<Result<ChipInfo>>,
    ) {
        runtime.set_initialized(true);

        let mut port = match Self::open_port(&port_path, baud_rate).and_then(|mut port| {
            let info = Self::query_chip_info_on_port(port.as_mut(), address)?;
            Ok((port, info))
        }) {
            Ok((port, info)) => {
                info!(
                    "CH9329 serial port opened: {} @ {} baud",
                    port_path, baud_rate
                );
                if Self::update_chip_info_cache(&chip_info, &led_status, info.clone()) {
                    runtime.notify();
                }
                runtime.set_online();
                let _ = init_tx.send(Ok(info));
                port
            }
            Err(err) => {
                if let AppError::HidError {
                    reason, error_code, ..
                } = &err
                {
                    runtime.set_error(reason.clone(), error_code.clone());
                }
                let _ = init_tx.send(Err(err));
                runtime.set_initialized(false);
                return;
            }
        };

        loop {
            match rx.recv_timeout(Duration::from_millis(PROBE_INTERVAL_MS)) {
                Ok(WorkerCommand::Packet { cmd, data }) => {
                    if let Err(err) = Self::xfer_packet(port.as_mut(), address, cmd, &data) {
                        if let AppError::HidError {
                            reason, error_code, ..
                        } = err
                        {
                            runtime.set_error(reason, error_code);
                        }

                        Self::try_best_effort_reset(port.as_mut(), address);

                        let Some(new_port) = Self::worker_reconnect_loop(
                            &rx,
                            &port_path,
                            baud_rate,
                            address,
                            &chip_info,
                            &led_status,
                            &runtime,
                        ) else {
                            break;
                        };
                        port = new_port;
                    } else {
                        runtime.set_online();
                    }
                }
                Ok(WorkerCommand::ResetState) => {
                    let reset_sequence = [
                        (cmd::SEND_KB_GENERAL_DATA, vec![0; 8]),
                        (cmd::SEND_MS_ABS_DATA, vec![0x02, 0, 0, 0, 0, 0, 0]),
                        (cmd::SEND_KB_MEDIA_DATA, vec![0x02, 0x00, 0x00, 0x00]),
                    ];

                    let mut reset_failed = false;
                    for (cmd, data) in reset_sequence {
                        if let Err(err) = Self::xfer_packet(port.as_mut(), address, cmd, &data) {
                            if let AppError::HidError {
                                reason, error_code, ..
                            } = err
                            {
                                runtime.set_error(reason, error_code);
                            }
                            reset_failed = true;
                            Self::try_best_effort_reset(port.as_mut(), address);
                            break;
                        }
                    }

                    if reset_failed {
                        let Some(new_port) = Self::worker_reconnect_loop(
                            &rx,
                            &port_path,
                            baud_rate,
                            address,
                            &chip_info,
                            &led_status,
                            &runtime,
                        ) else {
                            break;
                        };
                        port = new_port;
                    } else {
                        runtime.set_online();
                    }
                }
                Ok(WorkerCommand::Shutdown) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    match Self::query_chip_info_on_port(port.as_mut(), address) {
                        Ok(info) => {
                            if Self::update_chip_info_cache(&chip_info, &led_status, info) {
                                runtime.notify();
                            }
                            runtime.set_online();
                        }
                        Err(err) => {
                            if let AppError::HidError {
                                reason, error_code, ..
                            } = err
                            {
                                runtime.set_error(reason, error_code);
                            }

                            Self::try_best_effort_reset(port.as_mut(), address);

                            let Some(new_port) = Self::worker_reconnect_loop(
                                &rx,
                                &port_path,
                                baud_rate,
                                address,
                                &chip_info,
                                &led_status,
                                &runtime,
                            ) else {
                                break;
                            };
                            port = new_port;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        runtime.set_offline();
        runtime.set_initialized(false);
    }
}

#[async_trait]
impl HidBackend for Ch9329Backend {
    async fn init(&self) -> Result<()> {
        if self.worker_handle.lock().is_some() {
            return Ok(());
        }

        let (tx, rx) = mpsc::channel();
        let (init_tx, init_rx) = mpsc::channel();
        let port_path = self.port_path.clone();
        let baud_rate = self.baud_rate;
        let address = self.address;
        let chip_info = self.chip_info.clone();
        let led_status = self.led_status.clone();
        let runtime = self.runtime.clone();

        let handle = thread::Builder::new()
            .name("ch9329-worker".to_string())
            .spawn(move || {
                Self::worker_loop(
                    port_path, baud_rate, address, rx, chip_info, led_status, runtime, init_tx,
                );
            })
            .map_err(|e| AppError::Internal(format!("Failed to spawn CH9329 worker: {}", e)))?;

        match init_rx.recv_timeout(Duration::from_millis(INIT_WAIT_MS)) {
            Ok(Ok(info)) => {
                info!(
                    "CH9329 chip detected: {}, USB: {}, LEDs: NumLock={}, CapsLock={}, ScrollLock={}",
                    info.version,
                    if info.usb_connected {
                        "connected"
                    } else {
                        "disconnected"
                    },
                    info.num_lock,
                    info.caps_lock,
                    info.scroll_lock
                );
                *self.worker_tx.lock() = Some(tx);
                *self.worker_handle.lock() = Some(handle);
                self.runtime.set_online();
                Ok(())
            }
            Ok(Err(err)) => {
                let _ = handle.join();
                self.record_error(
                    format!(
                        "CH9329 not responding on {} @ {} baud: {}",
                        self.port_path, self.baud_rate, err
                    ),
                    "init_failed",
                );
                Err(AppError::Internal(format!(
                    "CH9329 not responding on {} @ {} baud: {}",
                    self.port_path, self.baud_rate, err
                )))
            }
            Err(_) => {
                let _ = tx.send(WorkerCommand::Shutdown);
                let _ = handle.join();
                self.record_error("Timed out waiting for CH9329 worker init", "init_timeout");
                Err(AppError::Internal(
                    "Timed out waiting for CH9329 initialization".to_string(),
                ))
            }
        }
    }

    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        let usb_key = event.key.to_hid_usage();

        if event.key.is_modifier() {
            let mut state = self.keyboard_state.lock();

            if let Some(bit) = event.key.modifier_bit() {
                match event.event_type {
                    KeyEventType::Down => state.modifiers |= bit,
                    KeyEventType::Up => state.modifiers &= !bit,
                }
            }

            let report = state.clone();
            drop(state);

            self.send_keyboard_report(&report)?;
        } else {
            let mut state = self.keyboard_state.lock();

            state.modifiers = event.modifiers.to_hid_byte();

            match event.event_type {
                KeyEventType::Down => {
                    state.add_key(usb_key);
                }
                KeyEventType::Up => {
                    state.remove_key(usb_key);
                }
            }

            let report = state.clone();
            drop(state);

            self.send_keyboard_report(&report)?;
        }

        Ok(())
    }

    async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
        let buttons = self.mouse_buttons.load(Ordering::Relaxed);

        match event.event_type {
            MouseEventType::Move => {
                self.relative_mouse_active.store(true, Ordering::Relaxed);
                let dx = event.x.clamp(-127, 127) as i8;
                let dy = event.y.clamp(-127, 127) as i8;
                self.send_mouse_relative(buttons, dx, dy, 0)?;
            }
            MouseEventType::MoveAbs => {
                self.relative_mouse_active.store(false, Ordering::Relaxed);
                let x = ((event.x.clamp(0, 32767) as u32) * CH9329_MOUSE_RESOLUTION / 32768) as u16;
                let y = ((event.y.clamp(0, 32767) as u32) * CH9329_MOUSE_RESOLUTION / 32768) as u16;
                self.last_abs_x.store(x, Ordering::Relaxed);
                self.last_abs_y.store(y, Ordering::Relaxed);
                self.send_mouse_absolute(buttons, x, y, 0)?;
            }
            MouseEventType::Down => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_or(bit, Ordering::Relaxed) | bit;
                    trace!("Mouse down: {:?} buttons=0x{:02X}", button, new_buttons);
                    if self.relative_mouse_active.load(Ordering::Relaxed) {
                        self.send_mouse_relative(new_buttons, 0, 0, 0)?;
                    } else {
                        let x = self.last_abs_x.load(Ordering::Relaxed);
                        let y = self.last_abs_y.load(Ordering::Relaxed);
                        self.send_mouse_absolute(new_buttons, x, y, 0)?;
                    }
                }
            }
            MouseEventType::Up => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_and(!bit, Ordering::Relaxed) & !bit;
                    trace!("Mouse up: {:?} buttons=0x{:02X}", button, new_buttons);
                    if self.relative_mouse_active.load(Ordering::Relaxed) {
                        self.send_mouse_relative(new_buttons, 0, 0, 0)?;
                    } else {
                        let x = self.last_abs_x.load(Ordering::Relaxed);
                        let y = self.last_abs_y.load(Ordering::Relaxed);
                        self.send_mouse_absolute(new_buttons, x, y, 0)?;
                    }
                }
            }
            MouseEventType::Scroll => {
                if self.relative_mouse_active.load(Ordering::Relaxed) {
                    self.send_mouse_relative(buttons, 0, 0, event.scroll)?;
                } else {
                    let x = self.last_abs_x.load(Ordering::Relaxed);
                    let y = self.last_abs_y.load(Ordering::Relaxed);
                    self.send_mouse_absolute(buttons, x, y, event.scroll)?;
                }
            }
        }

        Ok(())
    }

    async fn reset(&self) -> Result<()> {
        {
            let mut state = self.keyboard_state.lock();
            state.clear();
            let report = state.clone();
            drop(state);
            self.send_keyboard_report(&report)?;
        }

        self.mouse_buttons.store(0, Ordering::Relaxed);
        self.last_abs_x.store(0, Ordering::Relaxed);
        self.last_abs_y.store(0, Ordering::Relaxed);
        self.relative_mouse_active.store(false, Ordering::Relaxed);
        self.send_mouse_absolute(0, 0, 0, 0)?;

        let _ = self.release_media_keys();

        info!("CH9329 HID state reset");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = self.enqueue_command(WorkerCommand::ResetState);
        let sender = self.worker_tx.lock().take();
        if let Some(sender) = sender {
            let _ = sender.send(WorkerCommand::Shutdown);
        }
        if let Some(handle) = self.worker_handle.lock().take() {
            let _ = handle.join();
        }
        self.runtime.set_offline();
        self.runtime.set_initialized(false);
        self.runtime.clear_error();

        info!("CH9329 backend shutdown");
        Ok(())
    }

    fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
        let initialized = self.runtime.initialized.load(Ordering::Relaxed);
        let mut online = initialized && self.runtime.online.load(Ordering::Relaxed);
        let mut error = self.runtime.last_error.read().clone();

        if initialized && !self.check_port_exists() {
            online = false;
            error = Some((
                format!("Serial port {} not found", self.port_path),
                "port_not_found".to_string(),
            ));
        }

        HidBackendRuntimeSnapshot {
            initialized,
            online,
            supports_absolute_mouse: true,
            keyboard_leds_enabled: true,
            led_state: {
                let led = *self.led_status.read();
                LedState {
                    num_lock: led.num_lock,
                    caps_lock: led.caps_lock,
                    scroll_lock: led.scroll_lock,
                    compose: false,
                    kana: false,
                }
            },
            screen_resolution: Some(*self.screen_resolution.read()),
            device: Some(self.port_path.clone()),
            error: error.as_ref().map(|(reason, _)| reason.clone()),
            error_code: error.as_ref().map(|(_, code)| code.clone()),
        }
    }

    fn subscribe_runtime(&self) -> watch::Receiver<()> {
        self.runtime.subscribe()
    }

    fn set_screen_resolution(&self, width: u32, height: u32) {
        *self.screen_resolution.write() = (width, height);
        self.runtime.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_building() {
        let packet = Ch9329Backend::build_packet(DEFAULT_ADDR, cmd::GET_INFO, &[]);
        assert_eq!(packet, vec![0x57, 0xAB, 0x00, 0x01, 0x00, 0x03]);

        let data = [0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]; // 'A' key
        let packet = Ch9329Backend::build_packet(DEFAULT_ADDR, cmd::SEND_KB_GENERAL_DATA, &data);

        assert_eq!(packet[0], 0x57); // Header
        assert_eq!(packet[1], 0xAB); // Header
        assert_eq!(packet[2], 0x00); // Address
        assert_eq!(packet[3], cmd::SEND_KB_GENERAL_DATA); // Command
        assert_eq!(packet[4], 8); // Length (8 data bytes)
        assert_eq!(&packet[5..13], &data); // Data
        let expected_checksum: u8 = packet[..13]
            .iter()
            .fold(0u8, |acc: u8, &x| acc.wrapping_add(x));
        assert_eq!(packet[13], expected_checksum);
    }

    #[test]
    fn test_relative_mouse_packet() {
        let data = [0x01, 0x00, 50u8, 0x00, 0x00];
        let packet = Ch9329Backend::build_packet(DEFAULT_ADDR, cmd::SEND_MS_REL_DATA, &data);

        assert_eq!(packet[0], 0x57);
        assert_eq!(packet[1], 0xAB);
        assert_eq!(packet[2], 0x00); // Address
        assert_eq!(packet[3], 0x05); // CMD_SEND_MS_REL_DATA
        assert_eq!(packet[4], 5); // Length = 5
        assert_eq!(packet[5], 0x01); // Mode marker
        assert_eq!(packet[6], 0x00); // Buttons
        assert_eq!(packet[7], 50); // X delta
    }

    #[test]
    fn test_checksum_calculation() {
        let packet = [0x57u8, 0xAB, 0x00, 0x01, 0x00];
        let checksum = Ch9329Backend::calculate_checksum(&packet);
        assert_eq!(checksum, 0x03);

        let packet = [
            0x57u8, 0xAB, 0x00, 0x02, 0x08, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let checksum = Ch9329Backend::calculate_checksum(&packet);
        assert_eq!(checksum, 0x10);
    }

    #[test]
    fn test_response_parsing() {
        let response_bytes = [
            0x57, 0xAB, // Header
            0x00, // Address
            0x81, // Command (GET_INFO | 0x80 = success)
            0x08, // Length
            0x31, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, // Data
            0xE0, // Checksum (calculated)
        ];

        let _result = Response::parse(&response_bytes);
    }

    #[test]
    fn test_chip_info_parsing() {
        let data = [0x31, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
        let info = ChipInfo::from_response(&data).unwrap();

        assert_eq!(info.version, "V3.1");
        assert_eq!(info.version_raw, 0x31);
        assert!(info.usb_connected);
        assert!(info.num_lock);
        assert!(info.caps_lock);
        assert!(!info.scroll_lock);
    }

    #[test]
    fn test_led_status() {
        let led = LedStatus::from(0x07);
        assert!(led.num_lock);
        assert!(led.caps_lock);
        assert!(led.scroll_lock);

        let led = LedStatus::from(0x00);
        assert!(!led.num_lock);
        assert!(!led.caps_lock);
        assert!(!led.scroll_lock);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(Ch9329Error::from(0x00), Ch9329Error::Success);
        assert_eq!(Ch9329Error::from(0xE1), Ch9329Error::Timeout);
        assert_eq!(Ch9329Error::from(0xE4), Ch9329Error::ChecksumError);
    }
}
