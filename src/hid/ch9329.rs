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
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, watch};
use tracing::{info, trace, warn};

use super::backend::{HidBackend, HidBackendRuntimeSnapshot};
use super::ch9329_proto::{
    build_packet, cmd, expected_response_cmd, try_extract_response, ChipInfo, LedStatus, Response,
    DEFAULT_ADDR, DEFAULT_BAUD_RATE, MAX_PACKET_SIZE,
};
use super::types::{KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent, MouseEventType};
use crate::config::{Ch9329DescriptorConfig, Ch9329DescriptorState};
use crate::error::{AppError, Result};
use crate::events::LedState;

const RESPONSE_TIMEOUT_MS: u64 = 500;

const CH9329_MOUSE_RESOLUTION: u32 = 4096;

const PROBE_INTERVAL_MS: u64 = 100;

const RECONNECT_DELAY_MS: u64 = 2000;

const INIT_WAIT_MS: u64 = 3000;

const RECONNECT_COMMAND_POLL_MS: u64 = 100;

const PARAM_CFG_LEN: usize = 50;
const PARAM_CFG_VID_PID_OFFSET: usize = 11;
const PARAM_CFG_STRING_FLAGS_OFFSET: usize = 36;
const DESCRIPTOR_READ_RETRIES: usize = 3;
const DESCRIPTOR_RETRY_DELAY_MS: u64 = 80;
const DESCRIPTOR_APPLY_RESET_WAIT_MS: u64 = 3000;
const USB_STRING_MAX_LEN: usize = 23;
const USB_STRING_FLAG_ENABLE: u8 = 0x80;
const USB_STRING_FLAG_MANUFACTURER: u8 = 0x04;
const USB_STRING_FLAG_PRODUCT: u8 = 0x02;
const USB_STRING_FLAG_SERIAL: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsbStringType {
    Manufacturer = 0x00,
    Product = 0x01,
    Serial = 0x02,
}

impl UsbStringType {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParameterConfig {
    bytes: [u8; PARAM_CFG_LEN],
}

impl ParameterConfig {
    fn from_response(data: &[u8]) -> Result<Self> {
        let bytes: [u8; PARAM_CFG_LEN] = data.try_into().map_err(|_| {
            Ch9329Backend::backend_error(
                "Invalid CH9329 parameter config length",
                "invalid_response",
            )
        })?;
        Self::validate_parameter_layout(&bytes)?;
        Ok(Self { bytes })
    }

    fn validate_parameter_layout(bytes: &[u8; PARAM_CFG_LEN]) -> Result<()> {
        const VALID_WORK_MODES: [u8; 8] = [0x00, 0x01, 0x02, 0x03, 0x80, 0x81, 0x82, 0x83];
        const VALID_SERIAL_MODES: [u8; 6] = [0x00, 0x01, 0x02, 0x80, 0x81, 0x82];

        if !VALID_WORK_MODES.contains(&bytes[0]) || !VALID_SERIAL_MODES.contains(&bytes[1]) {
            return Err(Ch9329Backend::backend_error(
                format!(
                    "CH9329 did not return parameter config; enter protocol configuration mode by pulling SET low before reading or writing descriptors; response [{}]: {}",
                    bytes.len(),
                    Ch9329Backend::hex_bytes(bytes),
                ),
                "invalid_response",
            ));
        }

        Ok(())
    }

    fn set_vid_pid(&mut self, vendor_id: u16, product_id: u16) {
        let offset = PARAM_CFG_VID_PID_OFFSET;
        self.bytes[offset..offset + 2].copy_from_slice(&vendor_id.to_le_bytes());
        self.bytes[offset + 2..offset + 4].copy_from_slice(&product_id.to_le_bytes());
    }

    fn set_string_flags(&mut self, descriptor: &Ch9329DescriptorConfig) {
        let mut flags = self.bytes[PARAM_CFG_STRING_FLAGS_OFFSET] & 0x78;
        flags |= USB_STRING_FLAG_ENABLE | USB_STRING_FLAG_MANUFACTURER | USB_STRING_FLAG_PRODUCT;
        if descriptor
            .serial_number
            .as_ref()
            .is_some_and(|s| !s.is_empty())
        {
            flags |= USB_STRING_FLAG_SERIAL;
        }
        self.bytes[PARAM_CFG_STRING_FLAGS_OFFSET] = flags;
    }

    fn descriptor_base(&self) -> Ch9329DescriptorConfig {
        let offset = PARAM_CFG_VID_PID_OFFSET;
        Ch9329DescriptorConfig {
            vendor_id: u16::from_le_bytes([self.bytes[offset], self.bytes[offset + 1]]),
            product_id: u16::from_le_bytes([self.bytes[offset + 2], self.bytes[offset + 3]]),
            manufacturer: String::new(),
            product: String::new(),
            serial_number: None,
        }
    }

    fn string_flags(&self) -> u8 {
        self.bytes[PARAM_CFG_STRING_FLAGS_OFFSET]
    }
}

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
    Packet {
        cmd: u8,
        data: Vec<u8>,
    },
    ApplyDescriptor {
        descriptor: Ch9329DescriptorConfig,
        result_tx: oneshot::Sender<Result<Ch9329DescriptorState>>,
    },
    ReadDescriptor {
        result_tx: oneshot::Sender<Result<Ch9329DescriptorState>>,
    },
    ResetState,
    Shutdown,
}

pub struct Ch9329Backend {
    port_path: String,
    baud_rate: u32,
    worker_tx: Mutex<Option<mpsc::Sender<WorkerCommand>>>,
    worker_handle: Mutex<Option<thread::JoinHandle<()>>>,
    keyboard_state: Arc<Mutex<KeyboardReport>>,
    mouse_buttons: Arc<AtomicU8>,
    screen_resolution: RwLock<(u32, u32)>,
    chip_info: Arc<RwLock<Option<ChipInfo>>>,
    led_status: Arc<RwLock<LedStatus>>,
    address: u8,
    last_abs_x: Arc<AtomicU16>,
    last_abs_y: Arc<AtomicU16>,
    relative_mouse_active: Arc<AtomicBool>,
    hybrid_mouse: bool,
    runtime: Arc<Ch9329RuntimeState>,
}

impl Ch9329Backend {
    pub fn new(port_path: &str) -> Result<Self> {
        Self::with_baud_rate(port_path, DEFAULT_BAUD_RATE)
    }

    pub fn with_baud_rate(port_path: &str, baud_rate: u32) -> Result<Self> {
        Self::with_options(port_path, baud_rate, false)
    }

    pub fn with_options(port_path: &str, baud_rate: u32, hybrid_mouse: bool) -> Result<Self> {
        Ok(Self {
            port_path: port_path.to_string(),
            baud_rate,
            worker_tx: Mutex::new(None),
            worker_handle: Mutex::new(None),
            keyboard_state: Arc::new(Mutex::new(KeyboardReport::default())),
            mouse_buttons: Arc::new(AtomicU8::new(0)),
            screen_resolution: RwLock::new((1920, 1080)),
            chip_info: Arc::new(RwLock::new(None)),
            led_status: Arc::new(RwLock::new(LedStatus::default())),
            address: DEFAULT_ADDR,
            last_abs_x: Arc::new(AtomicU16::new(0)),
            last_abs_y: Arc::new(AtomicU16::new(0)),
            relative_mouse_active: Arc::new(AtomicBool::new(false)),
            hybrid_mouse,
            runtime: Arc::new(Ch9329RuntimeState::new()),
        })
    }

    fn record_error(&self, reason: impl Into<String>, error_code: impl Into<String>) {
        self.runtime.set_error(reason, error_code);
    }

    pub fn check_port_exists(&self) -> bool {
        #[cfg(windows)]
        {
            return crate::utils::list_serial_ports()
                .iter()
                .any(|port| port.eq_ignore_ascii_case(&self.port_path));
        }
        #[cfg(not(windows))]
        std::path::Path::new(&self.port_path).exists()
    }

    fn serial_error_to_hid_error(
        port_path: &str,
        e: serialport::Error,
        operation: &str,
    ) -> AppError {
        let port_present = {
            #[cfg(windows)]
            {
                crate::utils::list_serial_ports()
                    .iter()
                    .any(|port| port.eq_ignore_ascii_case(port_path))
            }
            #[cfg(not(windows))]
            {
                std::path::Path::new(port_path).exists()
            }
        };

        let error_code = match e.kind() {
            serialport::ErrorKind::NoDevice if !port_present => "port_not_found",
            serialport::ErrorKind::NoDevice => "device_unavailable",
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
    fn open_port(port_path: &str, baud_rate: u32) -> Result<Box<dyn serialport::SerialPort>> {
        #[cfg(not(windows))]
        if !std::path::Path::new(port_path).exists() {
            return Err(Self::backend_error(
                format!("Serial port {} not found", port_path),
                "port_not_found",
            ));
        }

        let port = serialport::new(port_path, baud_rate)
            .timeout(Duration::from_millis(RESPONSE_TIMEOUT_MS))
            .open()
            .map_err(|e| {
                Self::serial_error_to_hid_error(port_path, e, "Failed to open serial port")
            })?;

        let _ = port.clear(serialport::ClearBuffer::All);

        Ok(port)
    }

    fn write_packet(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        cmd: u8,
        data: &[u8],
    ) -> Result<()> {
        let packet = build_packet(address, cmd, data);
        port.write_all(&packet).map_err(|e| {
            Self::backend_error(format!("Failed to write to CH9329: {}", e), "write_failed")
        })?;
        Ok(())
    }

    fn xfer_packet(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        cmd: u8,
        data: &[u8],
    ) -> Result<Response> {
        let _ = port.clear(serialport::ClearBuffer::Input);

        Self::write_packet(port, address, cmd, data)?;

        let mut pending = Vec::with_capacity(128);
        let deadline = Instant::now() + Duration::from_millis(RESPONSE_TIMEOUT_MS);
        let expected_ok = expected_response_cmd(cmd, false);
        let expected_err = expected_response_cmd(cmd, true);

        loop {
            let mut chunk = [0u8; 128];
            match port.read(&mut chunk) {
                Ok(n) if n > 0 => {
                    pending.extend_from_slice(&chunk[..n]);

                    while let Some((response, consumed)) = try_extract_response(&pending) {
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

    fn ensure_success(response: Response) -> Result<Response> {
        if response.is_error {
            let reason = response
                .error_code
                .map(|e| format!("CH9329 error response: {}", e))
                .unwrap_or_else(|| "CH9329 returned error response".to_string());
            return Err(Self::backend_error(reason, "protocol_error"));
        }
        Ok(response)
    }

    fn query_chip_info_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
    ) -> Result<ChipInfo> {
        let response = Self::ensure_success(Self::xfer_packet(port, address, cmd::GET_INFO, &[])?)?;

        ChipInfo::from_response(&response.data)
            .ok_or_else(|| Self::backend_error("Failed to parse chip info", "invalid_response"))
    }

    fn read_parameter_config_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
    ) -> Result<ParameterConfig> {
        let mut last_error = None;
        for attempt in 0..DESCRIPTOR_READ_RETRIES {
            let result =
                Self::ensure_success(Self::xfer_packet(port, address, cmd::GET_PARA_CFG, &[])?)
                    .and_then(|response| ParameterConfig::from_response(&response.data));

            match result {
                Ok(config) => return Ok(config),
                Err(err)
                    if attempt + 1 < DESCRIPTOR_READ_RETRIES && Self::is_invalid_response(&err) =>
                {
                    last_error = Some(err);
                    thread::sleep(Duration::from_millis(DESCRIPTOR_RETRY_DELAY_MS));
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Self::backend_error("Failed to read CH9329 parameter config", "invalid_response")
        }))
    }

    fn write_parameter_config_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        config: &ParameterConfig,
    ) -> Result<()> {
        Self::ensure_success(Self::xfer_packet(
            port,
            address,
            cmd::SET_PARA_CFG,
            &config.bytes,
        )?)?;
        Ok(())
    }

    fn hex_bytes(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().saturating_mul(3).saturating_sub(1));
        for (index, byte) in data.iter().enumerate() {
            if index > 0 {
                out.push(' ');
            }
            use std::fmt::Write as _;
            let _ = write!(&mut out, "{:02x}", byte);
        }
        out
    }

    fn write_usb_string_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        string_type: UsbStringType,
        value: &str,
    ) -> Result<()> {
        let data = Self::build_usb_string_data(string_type, value)?;
        Self::ensure_success(Self::xfer_packet(
            port,
            address,
            cmd::SET_USB_STRING,
            &data,
        )?)?;
        Ok(())
    }

    fn read_usb_string_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        string_type: UsbStringType,
    ) -> Result<String> {
        let data = [string_type.as_u8()];
        let response = Self::ensure_success(Self::xfer_packet(
            port,
            address,
            cmd::GET_USB_STRING,
            &data,
        )?)?;
        Self::parse_usb_string_response(&response.data)
    }

    fn read_usb_string_with_retry_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        string_type: UsbStringType,
    ) -> Result<String> {
        let mut last_error = None;
        for attempt in 0..DESCRIPTOR_READ_RETRIES {
            match Self::read_usb_string_on_port(port, address, string_type) {
                Ok(value) => return Ok(value),
                Err(err)
                    if attempt + 1 < DESCRIPTOR_READ_RETRIES && Self::is_invalid_response(&err) =>
                {
                    last_error = Some(err);
                    thread::sleep(Duration::from_millis(DESCRIPTOR_RETRY_DELAY_MS));
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Self::backend_error("Failed to read CH9329 USB string", "invalid_response")
        }))
    }

    fn build_usb_string_data(string_type: UsbStringType, value: &str) -> Result<Vec<u8>> {
        let value = value.as_bytes();
        if value.len() > USB_STRING_MAX_LEN {
            return Err(Self::backend_error(
                "CH9329 USB string is too long",
                "invalid_config",
            ));
        }

        let mut data = Vec::with_capacity(2 + value.len());
        data.push(string_type.as_u8());
        data.push(value.len() as u8);
        data.extend_from_slice(value);
        Ok(data)
    }

    fn parse_usb_string_response(data: &[u8]) -> Result<String> {
        if data.len() < 2 {
            return Err(Self::backend_error(
                "Invalid CH9329 USB string response length",
                "invalid_response",
            ));
        }

        let len = data[1] as usize;
        if data.len() < 2 + len || len > USB_STRING_MAX_LEN {
            return Err(Self::backend_error(
                "Invalid CH9329 USB string response payload",
                "invalid_response",
            ));
        }

        String::from_utf8(data[2..2 + len].to_vec()).map_err(|_| {
            Self::backend_error("Invalid CH9329 USB string encoding", "invalid_response")
        })
    }

    fn read_device_descriptor_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
    ) -> Result<Ch9329DescriptorState> {
        let config = Self::read_parameter_config_on_port(port, address)?;
        let flags = config.string_flags();
        let strings_enabled = flags & USB_STRING_FLAG_ENABLE != 0;
        let manufacturer_enabled = strings_enabled && flags & USB_STRING_FLAG_MANUFACTURER != 0;
        let product_enabled = strings_enabled && flags & USB_STRING_FLAG_PRODUCT != 0;
        let serial_enabled = strings_enabled && flags & USB_STRING_FLAG_SERIAL != 0;
        let mut descriptor = config.descriptor_base();

        descriptor.manufacturer = if manufacturer_enabled {
            Self::read_usb_string_with_retry_on_port(port, address, UsbStringType::Manufacturer)?
        } else {
            String::new()
        };
        descriptor.product = if product_enabled {
            Self::read_usb_string_with_retry_on_port(port, address, UsbStringType::Product)?
        } else {
            String::new()
        };
        descriptor.serial_number = if serial_enabled {
            let value =
                Self::read_usb_string_with_retry_on_port(port, address, UsbStringType::Serial)?;
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        } else {
            None
        };

        Ok(Ch9329DescriptorState {
            descriptor,
            manufacturer_enabled,
            product_enabled,
            serial_enabled,
            config_mode_available: true,
        })
    }

    fn apply_device_descriptor_on_port(
        port: &mut dyn serialport::SerialPort,
        address: u8,
        descriptor: &Ch9329DescriptorConfig,
    ) -> Result<Ch9329DescriptorState> {
        let mut config = Self::read_parameter_config_on_port(&mut *port, address)?;
        config.set_vid_pid(descriptor.vendor_id, descriptor.product_id);
        config.set_string_flags(descriptor);
        Self::write_parameter_config_on_port(&mut *port, address, &config)?;
        Self::write_usb_string_on_port(
            &mut *port,
            address,
            UsbStringType::Manufacturer,
            &descriptor.manufacturer,
        )?;
        Self::write_usb_string_on_port(
            &mut *port,
            address,
            UsbStringType::Product,
            &descriptor.product,
        )?;
        if let Some(serial_number) = descriptor.serial_number.as_deref() {
            if !serial_number.is_empty() {
                Self::write_usb_string_on_port(
                    &mut *port,
                    address,
                    UsbStringType::Serial,
                    serial_number,
                )?;
            }
        }

        Self::try_best_effort_reset(port, address);
        thread::sleep(Duration::from_millis(DESCRIPTOR_APPLY_RESET_WAIT_MS));
        Self::query_chip_info_on_port(port, address)?;

        Ok(Ch9329DescriptorState {
            descriptor: descriptor.clone(),
            manufacturer_enabled: true,
            product_enabled: true,
            serial_enabled: descriptor
                .serial_number
                .as_ref()
                .is_some_and(|value| !value.is_empty()),
            config_mode_available: true,
        })
    }

    pub fn apply_device_descriptor(
        port_path: &str,
        baud_rate: u32,
        descriptor: &Ch9329DescriptorConfig,
    ) -> Result<Ch9329DescriptorState> {
        let mut port = Self::open_port(port_path, baud_rate)?;
        Self::apply_device_descriptor_on_port(port.as_mut(), DEFAULT_ADDR, descriptor)
    }

    pub fn read_device_descriptor(
        port_path: &str,
        baud_rate: u32,
    ) -> Result<Ch9329DescriptorState> {
        let mut port = Self::open_port(port_path, baud_rate)?;
        Self::read_device_descriptor_on_port(port.as_mut(), DEFAULT_ADDR)
    }

    fn open_ready_port(
        port_path: &str,
        baud_rate: u32,
        address: u8,
    ) -> Result<(Box<dyn serialport::SerialPort>, ChipInfo)> {
        Self::open_port(port_path, baud_rate).and_then(|mut port| {
            let info = Self::query_chip_info_on_port(port.as_mut(), address)?;
            Ok((port, info))
        })
    }

    fn record_runtime_error(runtime: &Arc<Ch9329RuntimeState>, err: &AppError) {
        if let AppError::HidError {
            reason, error_code, ..
        } = err
        {
            runtime.set_error(reason.clone(), error_code.clone());
        } else {
            runtime.set_error(err.to_string(), "error");
        }
    }

    fn is_invalid_response(err: &AppError) -> bool {
        matches!(
            err,
            AppError::HidError {
                backend,
                error_code,
                ..
            } if backend == "ch9329" && error_code == "invalid_response"
        )
    }

    fn should_recover_descriptor_error(err: &AppError) -> bool {
        match err {
            AppError::HidError {
                backend,
                error_code,
                ..
            } if backend == "ch9329" => matches!(
                error_code.as_str(),
                "no_response"
                    | "write_failed"
                    | "read_failed"
                    | "io_error"
                    | "device_unavailable"
                    | "serial_error"
                    | "enxio"
                    | "enodev"
                    | "epipe"
                    | "eshutdown"
            ),
            AppError::HidError { .. } => false,
            _ => true,
        }
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

    fn wait_reconnect_delay(rx: &mpsc::Receiver<WorkerCommand>) -> bool {
        let deadline = Instant::now() + Duration::from_millis(RECONNECT_DELAY_MS);
        loop {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }

            let remaining = deadline.saturating_duration_since(now);
            let timeout = remaining.min(Duration::from_millis(RECONNECT_COMMAND_POLL_MS));
            match rx.recv_timeout(timeout) {
                Ok(WorkerCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return false;
                }
                Ok(command) => Self::reject_command_while_reconnecting(command),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn reject_command_while_reconnecting(command: WorkerCommand) {
        let err = || Self::backend_error("CH9329 is reconnecting", "reconnecting");
        match command {
            WorkerCommand::ApplyDescriptor { result_tx, .. }
            | WorkerCommand::ReadDescriptor { result_tx } => {
                let _ = result_tx.send(Err(err()));
            }
            WorkerCommand::Packet { .. } | WorkerCommand::ResetState | WorkerCommand::Shutdown => {}
        }
    }

    fn release_state_on_port(port: &mut dyn serialport::SerialPort, address: u8) -> Result<()> {
        let reset_sequence = [
            (cmd::SEND_KB_GENERAL_DATA, vec![0; 8]),
            (cmd::SEND_MS_REL_DATA, vec![0x01, 0, 0, 0, 0]),
            (cmd::SEND_MS_ABS_DATA, vec![0x02, 0, 0, 0, 0, 0, 0]),
        ];

        for (cmd, data) in reset_sequence {
            Self::xfer_packet(port, address, cmd, &data)?;
        }

        Ok(())
    }

    fn clear_local_state(
        keyboard_state: &Arc<Mutex<KeyboardReport>>,
        mouse_buttons: &Arc<AtomicU8>,
        last_abs_x: &Arc<AtomicU16>,
        last_abs_y: &Arc<AtomicU16>,
        relative_mouse_active: &Arc<AtomicBool>,
    ) {
        keyboard_state.lock().clear();
        mouse_buttons.store(0, Ordering::Relaxed);
        last_abs_x.store(0, Ordering::Relaxed);
        last_abs_y.store(0, Ordering::Relaxed);
        relative_mouse_active.store(false, Ordering::Relaxed);
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
        runtime.set_offline();
        loop {
            match Self::open_ready_port(port_path, baud_rate, address) {
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
                    Self::record_runtime_error(runtime, &err);
                    if !Self::wait_reconnect_delay(rx) {
                        return None;
                    }
                }
            }
        }
    }

    fn recover_worker_port(
        mut port: Box<dyn serialport::SerialPort>,
        rx: &mpsc::Receiver<WorkerCommand>,
        port_path: &str,
        baud_rate: u32,
        address: u8,
        chip_info: &Arc<RwLock<Option<ChipInfo>>>,
        led_status: &Arc<RwLock<LedStatus>>,
        runtime: &Arc<Ch9329RuntimeState>,
    ) -> Option<Box<dyn serialport::SerialPort>> {
        Self::try_best_effort_reset(port.as_mut(), address);
        drop(port);
        Self::worker_reconnect_loop(
            rx, port_path, baud_rate, address, chip_info, led_status, runtime,
        )
    }

    fn finish_oneshot_command<T>(
        runtime: &Arc<Ch9329RuntimeState>,
        result: Result<T>,
        result_tx: oneshot::Sender<Result<T>>,
    ) -> bool {
        let success = result.is_ok();
        if let Err(ref err) = result {
            Self::record_runtime_error(runtime, err);
        }
        let _ = result_tx.send(result);
        if success {
            runtime.set_online();
        }
        success
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

    fn should_send_button_wheel_relative(&self) -> bool {
        self.hybrid_mouse || self.relative_mouse_active.load(Ordering::Relaxed)
    }

    fn absolute_move_buttons(&self, buttons: u8) -> u8 {
        if self.hybrid_mouse {
            0
        } else {
            buttons
        }
    }

    fn worker_loop(
        port_path: String,
        baud_rate: u32,
        address: u8,
        rx: mpsc::Receiver<WorkerCommand>,
        chip_info: Arc<RwLock<Option<ChipInfo>>>,
        led_status: Arc<RwLock<LedStatus>>,
        runtime: Arc<Ch9329RuntimeState>,
        keyboard_state: Arc<Mutex<KeyboardReport>>,
        mouse_buttons: Arc<AtomicU8>,
        last_abs_x: Arc<AtomicU16>,
        last_abs_y: Arc<AtomicU16>,
        relative_mouse_active: Arc<AtomicBool>,
        init_tx: mpsc::Sender<Result<ChipInfo>>,
    ) {
        runtime.set_initialized(true);

        let mut init_tx = Some(init_tx);
        let mut port = loop {
            match Self::open_ready_port(&port_path, baud_rate, address) {
                Ok((port, info)) => {
                    info!(
                        "CH9329 serial port opened: {} @ {} baud",
                        port_path, baud_rate
                    );
                    if Self::update_chip_info_cache(&chip_info, &led_status, info.clone()) {
                        runtime.notify();
                    }
                    runtime.set_online();
                    if let Some(init_tx) = init_tx.take() {
                        let _ = init_tx.send(Ok(info));
                    }
                    break port;
                }
                Err(err) => {
                    Self::record_runtime_error(&runtime, &err);
                    if let Some(init_tx) = init_tx.take() {
                        let _ = init_tx.send(Err(err));
                    }
                    if !Self::wait_reconnect_delay(&rx) {
                        runtime.set_offline();
                        runtime.set_initialized(false);
                        return;
                    }
                }
            }
        };

        loop {
            let recover_port = |port| {
                Self::recover_worker_port(
                    port,
                    &rx,
                    &port_path,
                    baud_rate,
                    address,
                    &chip_info,
                    &led_status,
                    &runtime,
                )
            };

            match rx.recv_timeout(Duration::from_millis(PROBE_INTERVAL_MS)) {
                Ok(WorkerCommand::Packet { cmd, data }) => {
                    if let Err(err) = Self::xfer_packet(port.as_mut(), address, cmd, &data) {
                        Self::record_runtime_error(&runtime, &err);
                        let Some(new_port) = recover_port(port) else {
                            break;
                        };
                        port = new_port;
                    } else {
                        runtime.set_online();
                    }
                }
                Ok(WorkerCommand::ApplyDescriptor {
                    descriptor,
                    result_tx,
                }) => {
                    let result =
                        Self::apply_device_descriptor_on_port(port.as_mut(), address, &descriptor);
                    let should_recover = result
                        .as_ref()
                        .is_err_and(Self::should_recover_descriptor_error);
                    if !Self::finish_oneshot_command(&runtime, result, result_tx) && should_recover
                    {
                        let Some(new_port) = recover_port(port) else {
                            break;
                        };
                        port = new_port;
                    }
                }
                Ok(WorkerCommand::ReadDescriptor { result_tx }) => {
                    let result = Self::read_device_descriptor_on_port(port.as_mut(), address);
                    let should_recover = result
                        .as_ref()
                        .is_err_and(Self::should_recover_descriptor_error);
                    if !Self::finish_oneshot_command(&runtime, result, result_tx) && should_recover
                    {
                        let Some(new_port) = recover_port(port) else {
                            break;
                        };
                        port = new_port;
                    }
                }
                Ok(WorkerCommand::ResetState) => {
                    Self::clear_local_state(
                        &keyboard_state,
                        &mouse_buttons,
                        &last_abs_x,
                        &last_abs_y,
                        &relative_mouse_active,
                    );
                    if let Err(err) = Self::release_state_on_port(port.as_mut(), address) {
                        Self::record_runtime_error(&runtime, &err);
                        let Some(new_port) = recover_port(port) else {
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
                            Self::record_runtime_error(&runtime, &err);
                            let Some(new_port) = recover_port(port) else {
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
        let keyboard_state = self.keyboard_state.clone();
        let mouse_buttons = self.mouse_buttons.clone();
        let last_abs_x = self.last_abs_x.clone();
        let last_abs_y = self.last_abs_y.clone();
        let relative_mouse_active = self.relative_mouse_active.clone();

        let handle = thread::Builder::new()
            .name("ch9329-worker".to_string())
            .spawn(move || {
                Self::worker_loop(
                    port_path,
                    baud_rate,
                    address,
                    rx,
                    chip_info,
                    led_status,
                    runtime,
                    keyboard_state,
                    mouse_buttons,
                    last_abs_x,
                    last_abs_y,
                    relative_mouse_active,
                    init_tx,
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
                self.record_error(
                    format!(
                        "CH9329 not responding on {} @ {} baud: {}",
                        self.port_path, self.baud_rate, err
                    ),
                    "init_failed",
                );
                warn!(
                    "CH9329 not responding on {} @ {} baud, retrying in background: {}",
                    self.port_path, self.baud_rate, err
                );
                *self.worker_tx.lock() = Some(tx);
                *self.worker_handle.lock() = Some(handle);
                Ok(())
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
                self.send_mouse_absolute(self.absolute_move_buttons(buttons), x, y, 0)?;
            }
            MouseEventType::Down => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_or(bit, Ordering::Relaxed) | bit;
                    trace!("Mouse down: {:?} buttons=0x{:02X}", button, new_buttons);
                    if self.should_send_button_wheel_relative() {
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
                    if self.should_send_button_wheel_relative() {
                        self.send_mouse_relative(new_buttons, 0, 0, 0)?;
                    } else {
                        let x = self.last_abs_x.load(Ordering::Relaxed);
                        let y = self.last_abs_y.load(Ordering::Relaxed);
                        self.send_mouse_absolute(new_buttons, x, y, 0)?;
                    }
                }
            }
            MouseEventType::Scroll => {
                if self.should_send_button_wheel_relative() {
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

    async fn apply_ch9329_descriptor(
        &self,
        descriptor: &Ch9329DescriptorConfig,
    ) -> Result<Ch9329DescriptorState> {
        let (result_tx, result_rx) = oneshot::channel();
        self.enqueue_command(WorkerCommand::ApplyDescriptor {
            descriptor: descriptor.clone(),
            result_tx,
        })?;
        result_rx
            .await
            .map_err(|_| Self::backend_error("CH9329 worker stopped", "worker_stopped"))?
    }

    async fn read_ch9329_descriptor(&self) -> Result<Ch9329DescriptorState> {
        let (result_tx, result_rx) = oneshot::channel();
        self.enqueue_command(WorkerCommand::ReadDescriptor { result_tx })?;
        result_rx
            .await
            .map_err(|_| Self::backend_error("CH9329 worker stopped", "worker_stopped"))?
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
        self.send_mouse_relative(0, 0, 0, 0)?;
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

        #[cfg(windows)]
        let port_still_present = crate::utils::list_serial_ports()
            .iter()
            .any(|port| port.eq_ignore_ascii_case(&self.port_path));
        #[cfg(not(windows))]
        let port_still_present = self.check_port_exists();

        if initialized && !port_still_present {
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
    use crate::hid::ch9329_proto::{build_packet, calculate_checksum, Ch9329Error};

    #[test]
    fn test_packet_building() {
        let packet = build_packet(DEFAULT_ADDR, cmd::GET_INFO, &[]);
        assert_eq!(packet, vec![0x57, 0xAB, 0x00, 0x01, 0x00, 0x03]);

        let data = [0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]; // 'A' key
        let packet = build_packet(DEFAULT_ADDR, cmd::SEND_KB_GENERAL_DATA, &data);

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
        let packet = build_packet(DEFAULT_ADDR, cmd::SEND_MS_REL_DATA, &data);

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
        let checksum = calculate_checksum(&packet);
        assert_eq!(checksum, 0x03);

        let packet = [
            0x57u8, 0xAB, 0x00, 0x02, 0x08, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let checksum = calculate_checksum(&packet);
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

    #[test]
    fn test_parameter_config_updates_vid_pid_and_string_flags() {
        let mut raw = [0u8; PARAM_CFG_LEN];
        raw[0] = 0x80;
        raw[1] = 0x80;
        raw[PARAM_CFG_STRING_FLAGS_OFFSET] = 0x7F;
        let mut config = ParameterConfig::from_response(&raw).unwrap();
        let descriptor = Ch9329DescriptorConfig {
            vendor_id: 0x1209,
            product_id: 0x9329,
            manufacturer: "One-KVM".to_string(),
            product: "One-KVM HID".to_string(),
            serial_number: Some("ABC123".to_string()),
        };

        config.set_vid_pid(descriptor.vendor_id, descriptor.product_id);
        config.set_string_flags(&descriptor);

        assert_eq!(
            &config.bytes[PARAM_CFG_VID_PID_OFFSET..PARAM_CFG_VID_PID_OFFSET + 4],
            &[0x09, 0x12, 0x29, 0x93]
        );
        assert_eq!(
            config.bytes[PARAM_CFG_STRING_FLAGS_OFFSET],
            0x78 | USB_STRING_FLAG_ENABLE
                | USB_STRING_FLAG_MANUFACTURER
                | USB_STRING_FLAG_PRODUCT
                | USB_STRING_FLAG_SERIAL
        );
    }

    #[test]
    fn test_parameter_config_disables_custom_serial_when_empty() {
        let mut raw = [0u8; PARAM_CFG_LEN];
        raw[0] = 0x80;
        raw[1] = 0x80;
        let mut config = ParameterConfig::from_response(&raw).unwrap();
        let descriptor = Ch9329DescriptorConfig {
            vendor_id: 0x1a86,
            product_id: 0xe129,
            manufacturer: "WCH.CN".to_string(),
            product: "CH9329".to_string(),
            serial_number: None,
        };

        config.set_string_flags(&descriptor);

        assert_eq!(
            config.bytes[PARAM_CFG_STRING_FLAGS_OFFSET],
            USB_STRING_FLAG_ENABLE | USB_STRING_FLAG_MANUFACTURER | USB_STRING_FLAG_PRODUCT
        );
    }

    #[test]
    fn test_parameter_config_parses_vid_pid_and_string_flags() {
        let mut raw = [0u8; PARAM_CFG_LEN];
        raw[0] = 0x80;
        raw[1] = 0x80;
        raw[PARAM_CFG_VID_PID_OFFSET..PARAM_CFG_VID_PID_OFFSET + 4]
            .copy_from_slice(&[0x86, 0x1a, 0x29, 0xe1]);
        raw[PARAM_CFG_STRING_FLAGS_OFFSET] =
            USB_STRING_FLAG_ENABLE | USB_STRING_FLAG_MANUFACTURER | USB_STRING_FLAG_SERIAL;

        let config = ParameterConfig::from_response(&raw).unwrap();
        let descriptor = config.descriptor_base();

        assert_eq!(descriptor.vendor_id, 0x1a86);
        assert_eq!(descriptor.product_id, 0xe129);
        assert_eq!(
            config.string_flags(),
            USB_STRING_FLAG_ENABLE | USB_STRING_FLAG_MANUFACTURER | USB_STRING_FLAG_SERIAL
        );
    }

    #[test]
    fn test_parameter_config_rejects_usb_string_descriptor_response() {
        let mut raw = [0u8; PARAM_CFG_LEN];
        raw[..24].copy_from_slice(b"W\0C\0H\0 \0U\0A\0R\0T\0 \0T\0O\0 \0");

        let err = ParameterConfig::from_response(&raw).unwrap_err();

        assert!(err.to_string().contains("protocol configuration mode"));
    }

    #[test]
    fn test_usb_string_data() {
        let data = Ch9329Backend::build_usb_string_data(UsbStringType::Product, "One-KVM").unwrap();

        assert_eq!(
            data,
            vec![0x01, 7, b'O', b'n', b'e', b'-', b'K', b'V', b'M']
        );
    }

    #[test]
    fn test_usb_string_data_rejects_overlong_value() {
        let err = Ch9329Backend::build_usb_string_data(
            UsbStringType::Manufacturer,
            "x".repeat(24).as_str(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn test_usb_string_response_parsing() {
        let value = Ch9329Backend::parse_usb_string_response(&[
            UsbStringType::Manufacturer.as_u8(),
            7,
            b'O',
            b'n',
            b'e',
            b'-',
            b'K',
            b'V',
            b'M',
        ])
        .unwrap();

        assert_eq!(value, "One-KVM");
    }

    #[test]
    fn test_hybrid_mouse_routes_buttons_and_wheel_to_relative_reports() {
        let backend = Ch9329Backend::with_options("/dev/null", DEFAULT_BAUD_RATE, true).unwrap();

        assert!(backend.should_send_button_wheel_relative());
        assert_eq!(backend.absolute_move_buttons(0x07), 0);
    }

    #[test]
    fn test_default_mouse_mode_preserves_absolute_report_buttons() {
        let backend = Ch9329Backend::with_baud_rate("/dev/null", DEFAULT_BAUD_RATE).unwrap();

        assert!(!backend.should_send_button_wheel_relative());
        assert_eq!(backend.absolute_move_buttons(0x07), 0x07);
    }
}
