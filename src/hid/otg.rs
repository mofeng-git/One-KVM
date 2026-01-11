//! OTG USB Gadget HID backend
//!
//! This backend uses Linux USB Gadget API to emulate USB HID devices.
//! It creates and manages three HID devices:
//! - hidg0: Keyboard (8-byte reports, with LED feedback)
//! - hidg1: Relative Mouse (4-byte reports)
//! - hidg2: Absolute Mouse (6-byte reports)
//!
//! Requirements:
//! - USB OTG/Device controller (UDC)
//! - ConfigFS with USB gadget support
//! - Root privileges for gadget setup
//!
//! Error Recovery:
//! This module implements automatic device reconnection based on PiKVM's approach.
//! When ESHUTDOWN or EAGAIN errors occur (common during MSD operations), the device
//! file handles are closed and reopened on the next operation.
//! See: https://github.com/raspberrypi/linux/issues/4373

use async_trait::async_trait;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tracing::{debug, info, trace, warn};

use super::backend::HidBackend;
use super::keymap;
use super::types::{
    ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent, MouseEventType,
};
use crate::error::{AppError, Result};
use crate::otg::{wait_for_hid_devices, HidDevicePaths};

/// Device type for ensure_device operations
#[derive(Debug, Clone, Copy)]
enum DeviceType {
    Keyboard,
    MouseRelative,
    MouseAbsolute,
    ConsumerControl,
}

/// Keyboard LED state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LedState {
    /// Num Lock LED
    pub num_lock: bool,
    /// Caps Lock LED
    pub caps_lock: bool,
    /// Scroll Lock LED
    pub scroll_lock: bool,
    /// Compose LED
    pub compose: bool,
    /// Kana LED
    pub kana: bool,
}

impl LedState {
    /// Create from raw byte
    pub fn from_byte(b: u8) -> Self {
        Self {
            num_lock: b & 0x01 != 0,
            caps_lock: b & 0x02 != 0,
            scroll_lock: b & 0x04 != 0,
            compose: b & 0x08 != 0,
            kana: b & 0x10 != 0,
        }
    }

    /// Convert to raw byte
    pub fn to_byte(&self) -> u8 {
        let mut b = 0u8;
        if self.num_lock {
            b |= 0x01;
        }
        if self.caps_lock {
            b |= 0x02;
        }
        if self.scroll_lock {
            b |= 0x04;
        }
        if self.compose {
            b |= 0x08;
        }
        if self.kana {
            b |= 0x10;
        }
        b
    }
}

/// OTG HID backend with 4 devices
///
/// This backend opens HID device files created by OtgService.
/// It does NOT manage the USB gadget itself - that's handled by OtgService.
///
/// ## Error Recovery
///
/// Based on PiKVM's implementation, this backend automatically handles:
/// - EAGAIN (errno 11): Resource temporarily unavailable - just retry later, don't close device
/// - ESHUTDOWN (errno 108): Transport endpoint shutdown - close and reopen device
///
/// When ESHUTDOWN occurs, the device file handle is closed and will be
/// reopened on the next operation attempt.
pub struct OtgBackend {
    /// Keyboard device path (/dev/hidg0)
    keyboard_path: PathBuf,
    /// Relative mouse device path (/dev/hidg1)
    mouse_rel_path: PathBuf,
    /// Absolute mouse device path (/dev/hidg2)
    mouse_abs_path: PathBuf,
    /// Consumer control device path (/dev/hidg3)
    consumer_path: PathBuf,
    /// Keyboard device file
    keyboard_dev: Mutex<Option<File>>,
    /// Relative mouse device file
    mouse_rel_dev: Mutex<Option<File>>,
    /// Absolute mouse device file
    mouse_abs_dev: Mutex<Option<File>>,
    /// Consumer control device file
    consumer_dev: Mutex<Option<File>>,
    /// Current keyboard state
    keyboard_state: Mutex<KeyboardReport>,
    /// Current mouse button state
    mouse_buttons: AtomicU8,
    /// Last known LED state (using parking_lot::RwLock for sync access)
    led_state: parking_lot::RwLock<LedState>,
    /// Screen resolution for absolute mouse (using parking_lot::RwLock for sync access)
    screen_resolution: parking_lot::RwLock<Option<(u32, u32)>>,
    /// UDC name for state checking (e.g., "fcc00000.usb")
    udc_name: parking_lot::RwLock<Option<String>>,
    /// Whether the device is currently online (UDC configured and devices accessible)
    online: AtomicBool,
    /// Last error log time for throttling (using parking_lot for sync)
    last_error_log: parking_lot::Mutex<std::time::Instant>,
    /// Error count since last successful operation (for log throttling)
    error_count: AtomicU8,
    /// Consecutive EAGAIN count (for offline threshold detection)
    eagain_count: AtomicU8,
}

/// Write timeout in milliseconds (same as JetKVM's hidWriteTimeout)
const HID_WRITE_TIMEOUT_MS: i32 = 500;

impl OtgBackend {
    /// Create OTG backend from device paths provided by OtgService
    ///
    /// This is the ONLY way to create an OtgBackend - it no longer manages
    /// the USB gadget itself. The gadget must already be set up by OtgService.
    pub fn from_handles(paths: HidDevicePaths) -> Result<Self> {
        Ok(Self {
            keyboard_path: paths.keyboard,
            mouse_rel_path: paths.mouse_relative,
            mouse_abs_path: paths.mouse_absolute,
            consumer_path: paths
                .consumer
                .unwrap_or_else(|| PathBuf::from("/dev/hidg3")),
            keyboard_dev: Mutex::new(None),
            mouse_rel_dev: Mutex::new(None),
            mouse_abs_dev: Mutex::new(None),
            consumer_dev: Mutex::new(None),
            keyboard_state: Mutex::new(KeyboardReport::default()),
            mouse_buttons: AtomicU8::new(0),
            led_state: parking_lot::RwLock::new(LedState::default()),
            screen_resolution: parking_lot::RwLock::new(Some((1920, 1080))),
            udc_name: parking_lot::RwLock::new(None),
            online: AtomicBool::new(false),
            last_error_log: parking_lot::Mutex::new(std::time::Instant::now()),
            error_count: AtomicU8::new(0),
            eagain_count: AtomicU8::new(0),
        })
    }

    /// Log throttled error message (max once per second)
    fn log_throttled_error(&self, msg: &str) {
        let mut last_log = self.last_error_log.lock();
        let now = std::time::Instant::now();
        if now.duration_since(*last_log).as_secs() >= 1 {
            let count = self.error_count.swap(0, Ordering::Relaxed);
            if count > 1 {
                warn!("{} (repeated {} times)", msg, count);
            } else {
                warn!("{}", msg);
            }
            *last_log = now;
        } else {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Reset error count on successful operation
    fn reset_error_count(&self) {
        self.error_count.store(0, Ordering::Relaxed);
        // Also reset EAGAIN count - successful operation means device is working
        self.eagain_count.store(0, Ordering::Relaxed);
    }

    /// Write data to HID device with timeout (JetKVM style)
    ///
    /// Uses poll() to wait for device to be ready for writing.
    /// If timeout expires, silently drops the data (acceptable for mouse movement).
    /// Returns Ok(true) if write succeeded, Ok(false) if timed out (silently dropped).
    fn write_with_timeout(&self, file: &mut File, data: &[u8]) -> std::io::Result<bool> {
        let mut pollfd = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];

        match poll(&mut pollfd, PollTimeout::from(HID_WRITE_TIMEOUT_MS as u16)) {
            Ok(1) => {
                // Device ready, check for errors
                if let Some(revents) = pollfd[0].revents() {
                    if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP)
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Device error or hangup",
                        ));
                    }
                }
                // Write the data
                file.write_all(data)?;
                Ok(true)
            }
            Ok(0) => {
                // Timeout - silently drop (JetKVM behavior)
                trace!("HID write timeout, dropping data");
                Ok(false)
            }
            Ok(_) => Ok(false),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        }
    }

    /// Set the UDC name for state checking
    pub fn set_udc_name(&self, udc: &str) {
        *self.udc_name.write() = Some(udc.to_string());
    }

    /// Check if the UDC is in "configured" state
    ///
    /// This is based on PiKVM's `__is_udc_configured()` method.
    /// The UDC state file indicates whether the USB host has enumerated and configured the gadget.
    pub fn is_udc_configured(&self) -> bool {
        let udc_name = self.udc_name.read();
        if let Some(ref udc) = *udc_name {
            let state_path = format!("/sys/class/udc/{}/state", udc);
            match fs::read_to_string(&state_path) {
                Ok(content) => {
                    let state = content.trim().to_lowercase();
                    trace!("UDC {} state: {}", udc, state);
                    state == "configured"
                }
                Err(e) => {
                    debug!("Failed to read UDC state from {}: {}", state_path, e);
                    // If we can't read the state, assume it might be configured
                    // to avoid blocking operations unnecessarily
                    true
                }
            }
        } else {
            // No UDC name set, try to auto-detect
            if let Some(udc) = Self::find_udc() {
                drop(udc_name);
                *self.udc_name.write() = Some(udc.clone());
                let state_path = format!("/sys/class/udc/{}/state", udc);
                fs::read_to_string(&state_path)
                    .map(|s| s.trim().to_lowercase() == "configured")
                    .unwrap_or(true)
            } else {
                true
            }
        }
    }

    /// Find the first available UDC
    fn find_udc() -> Option<String> {
        let udc_path = PathBuf::from("/sys/class/udc");
        if let Ok(entries) = fs::read_dir(&udc_path) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// Check if device is online
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    /// Ensure a device is open and ready for I/O
    ///
    /// This method is based on PiKVM's `__ensure_device()` pattern:
    /// 1. Check if device path exists, close handle if not
    /// 2. If handle is None but path exists, reopen the device
    /// 3. Return whether the device is ready for I/O
    fn ensure_device(&self, device_type: DeviceType) -> Result<()> {
        let (path, dev_mutex) = match device_type {
            DeviceType::Keyboard => (&self.keyboard_path, &self.keyboard_dev),
            DeviceType::MouseRelative => (&self.mouse_rel_path, &self.mouse_rel_dev),
            DeviceType::MouseAbsolute => (&self.mouse_abs_path, &self.mouse_abs_dev),
            DeviceType::ConsumerControl => (&self.consumer_path, &self.consumer_dev),
        };

        // Check if device path exists
        if !path.exists() {
            // Close the device if open (device was removed)
            let mut dev = dev_mutex.lock();
            if dev.is_some() {
                debug!(
                    "Device path {} no longer exists, closing handle",
                    path.display()
                );
                *dev = None;
            }
            self.online.store(false, Ordering::Relaxed);
            return Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: format!("Device not found: {}", path.display()),
                error_code: "enoent".to_string(),
            });
        }

        // If device is not open, try to open it
        let mut dev = dev_mutex.lock();
        if dev.is_none() {
            match Self::open_device(path) {
                Ok(file) => {
                    info!("Reopened HID device: {}", path.display());
                    *dev = Some(file);
                }
                Err(e) => {
                    warn!("Failed to reopen HID device {}: {}", path.display(), e);
                    return Err(e);
                }
            }
        }

        self.online.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Open a HID device file with read/write access
    fn open_device(path: &PathBuf) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| {
                AppError::Internal(format!(
                    "Failed to open HID device {}: {}",
                    path.display(),
                    e
                ))
            })
    }

    /// Convert I/O error to HidError with appropriate error code
    fn io_error_to_hid_error(e: std::io::Error, operation: &str) -> AppError {
        let error_code = match e.raw_os_error() {
            Some(32) => "epipe",      // EPIPE - broken pipe
            Some(108) => "eshutdown", // ESHUTDOWN - transport endpoint shutdown
            Some(11) => "eagain",     // EAGAIN - resource temporarily unavailable
            Some(6) => "enxio",       // ENXIO - no such device or address
            Some(19) => "enodev",     // ENODEV - no such device
            Some(5) => "eio",         // EIO - I/O error
            Some(2) => "enoent",      // ENOENT - no such file or directory
            _ => "io_error",
        };

        AppError::HidError {
            backend: "otg".to_string(),
            reason: format!("{}: {}", operation, e),
            error_code: error_code.to_string(),
        }
    }

    /// Check if all HID device files exist
    pub fn check_devices_exist(&self) -> bool {
        self.keyboard_path.exists() && self.mouse_rel_path.exists() && self.mouse_abs_path.exists()
    }

    /// Get list of missing device paths
    pub fn get_missing_devices(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if !self.keyboard_path.exists() {
            missing.push(self.keyboard_path.display().to_string());
        }
        if !self.mouse_rel_path.exists() {
            missing.push(self.mouse_rel_path.display().to_string());
        }
        if !self.mouse_abs_path.exists() {
            missing.push(self.mouse_abs_path.display().to_string());
        }
        missing
    }

    /// Send keyboard report (8 bytes)
    ///
    /// This method ensures the device is open before writing, and handles
    /// ESHUTDOWN errors by closing the device handle for later reconnection.
    /// Uses write_with_timeout to avoid blocking on busy devices.
    fn send_keyboard_report(&self, report: &KeyboardReport) -> Result<()> {
        // Ensure device is ready
        self.ensure_device(DeviceType::Keyboard)?;

        let mut dev = self.keyboard_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = report.to_bytes();
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    self.online.store(true, Ordering::Relaxed);
                    self.reset_error_count();
                    debug!("Sent keyboard report: {:02X?}", data);
                    Ok(())
                }
                Ok(false) => {
                    // Timeout - silently dropped (JetKVM behavior)
                    self.log_throttled_error("HID keyboard write timeout, dropped");
                    Ok(())
                }
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            // ESHUTDOWN - endpoint closed, need to reopen device
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Keyboard ESHUTDOWN, closing for recovery");
                            *dev = None;
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write keyboard report",
                            ))
                        }
                        Some(11) => {
                            // EAGAIN after poll - should be rare, silently drop
                            trace!("Keyboard EAGAIN after poll, dropping");
                            Ok(())
                        }
                        _ => {
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Keyboard write error: {}", e);
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write keyboard report",
                            ))
                        }
                    }
                }
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Keyboard device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    /// Send relative mouse report (4 bytes: buttons, dx, dy, wheel)
    ///
    /// This method ensures the device is open before writing, and handles
    /// ESHUTDOWN errors by closing the device handle for later reconnection.
    /// Uses write_with_timeout to avoid blocking on busy devices.
    fn send_mouse_report_relative(&self, buttons: u8, dx: i8, dy: i8, wheel: i8) -> Result<()> {
        // Ensure device is ready
        self.ensure_device(DeviceType::MouseRelative)?;

        let mut dev = self.mouse_rel_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = [buttons, dx as u8, dy as u8, wheel as u8];
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    self.online.store(true, Ordering::Relaxed);
                    self.reset_error_count();
                    trace!("Sent relative mouse report: {:02X?}", data);
                    Ok(())
                }
                Ok(false) => {
                    // Timeout - silently dropped (JetKVM behavior)
                    Ok(())
                }
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Relative mouse ESHUTDOWN, closing for recovery");
                            *dev = None;
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                        Some(11) => {
                            // EAGAIN after poll - should be rare, silently drop
                            Ok(())
                        }
                        _ => {
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Relative mouse write error: {}", e);
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                    }
                }
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Relative mouse device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    /// Send absolute mouse report (6 bytes: buttons, x_lo, x_hi, y_lo, y_hi, wheel)
    ///
    /// This method ensures the device is open before writing, and handles
    /// ESHUTDOWN errors by closing the device handle for later reconnection.
    /// Uses write_with_timeout to avoid blocking on busy devices.
    fn send_mouse_report_absolute(&self, buttons: u8, x: u16, y: u16, wheel: i8) -> Result<()> {
        // Ensure device is ready
        self.ensure_device(DeviceType::MouseAbsolute)?;

        let mut dev = self.mouse_abs_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = [
                buttons,
                (x & 0xFF) as u8,
                (x >> 8) as u8,
                (y & 0xFF) as u8,
                (y >> 8) as u8,
                wheel as u8,
            ];
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    self.online.store(true, Ordering::Relaxed);
                    self.reset_error_count();
                    Ok(())
                }
                Ok(false) => {
                    // Timeout - silently dropped (JetKVM behavior)
                    Ok(())
                }
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Absolute mouse ESHUTDOWN, closing for recovery");
                            *dev = None;
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                        Some(11) => {
                            // EAGAIN after poll - should be rare, silently drop
                            Ok(())
                        }
                        _ => {
                            self.online.store(false, Ordering::Relaxed);
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Absolute mouse write error: {}", e);
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                    }
                }
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Absolute mouse device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    /// Send consumer control report (2 bytes: usage_lo, usage_hi)
    ///
    /// Sends a consumer control usage code and then releases it (sends 0x0000).
    fn send_consumer_report(&self, usage: u16) -> Result<()> {
        // Ensure device is ready
        self.ensure_device(DeviceType::ConsumerControl)?;

        let mut dev = self.consumer_dev.lock();
        if let Some(ref mut file) = *dev {
            // Send the usage code
            let data = [(usage & 0xFF) as u8, (usage >> 8) as u8];
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    trace!("Sent consumer report: {:02X?}", data);
                    // Send release (0x0000)
                    let release = [0u8, 0u8];
                    let _ = self.write_with_timeout(file, &release);
                    self.online.store(true, Ordering::Relaxed);
                    self.reset_error_count();
                    Ok(())
                }
                Ok(false) => {
                    // Timeout - silently dropped
                    Ok(())
                }
                Err(e) => {
                    let error_code = e.raw_os_error();
                    match error_code {
                        Some(108) => {
                            self.online.store(false, Ordering::Relaxed);
                            debug!("Consumer control ESHUTDOWN, closing for recovery");
                            *dev = None;
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write consumer report",
                            ))
                        }
                        Some(11) => {
                            // EAGAIN after poll - silently drop
                            Ok(())
                        }
                        _ => {
                            self.online.store(false, Ordering::Relaxed);
                            warn!("Consumer control write error: {}", e);
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write consumer report",
                            ))
                        }
                    }
                }
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Consumer control device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    /// Send consumer control event
    pub fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        self.send_consumer_report(event.usage)
    }

    /// Read keyboard LED state (non-blocking)
    pub fn read_led_state(&self) -> Result<Option<LedState>> {
        let mut dev = self.keyboard_dev.lock();
        if let Some(ref mut file) = *dev {
            let mut buf = [0u8; 1];
            match file.read(&mut buf) {
                Ok(1) => {
                    let state = LedState::from_byte(buf[0]);
                    // Update LED state (using parking_lot RwLock)
                    *self.led_state.write() = state;
                    Ok(Some(state))
                }
                Ok(_) => Ok(None), // No data available
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(e) => Err(AppError::Internal(format!(
                    "Failed to read LED state: {}",
                    e
                ))),
            }
        } else {
            Ok(None)
        }
    }

    /// Get last known LED state
    pub fn led_state(&self) -> LedState {
        *self.led_state.read()
    }
}

#[async_trait]
impl HidBackend for OtgBackend {
    fn name(&self) -> &'static str {
        "OTG USB Gadget"
    }

    async fn init(&self) -> Result<()> {
        info!("Initializing OTG HID backend");

        // Auto-detect UDC name for state checking
        if let Some(udc) = Self::find_udc() {
            info!("Auto-detected UDC: {}", udc);
            self.set_udc_name(&udc);
        }

        // Wait for devices to appear (they should already exist from OtgService)
        let device_paths = vec![
            self.keyboard_path.clone(),
            self.mouse_rel_path.clone(),
            self.mouse_abs_path.clone(),
        ];

        if !wait_for_hid_devices(&device_paths, 2000).await {
            return Err(AppError::Internal("HID devices did not appear".into()));
        }

        // Open keyboard device
        if self.keyboard_path.exists() {
            let file = Self::open_device(&self.keyboard_path)?;
            *self.keyboard_dev.lock() = Some(file);
            info!("Keyboard device opened: {}", self.keyboard_path.display());
        } else {
            warn!(
                "Keyboard device not found: {}",
                self.keyboard_path.display()
            );
        }

        // Open relative mouse device
        if self.mouse_rel_path.exists() {
            let file = Self::open_device(&self.mouse_rel_path)?;
            *self.mouse_rel_dev.lock() = Some(file);
            info!(
                "Relative mouse device opened: {}",
                self.mouse_rel_path.display()
            );
        } else {
            warn!(
                "Relative mouse device not found: {}",
                self.mouse_rel_path.display()
            );
        }

        // Open absolute mouse device
        if self.mouse_abs_path.exists() {
            let file = Self::open_device(&self.mouse_abs_path)?;
            *self.mouse_abs_dev.lock() = Some(file);
            info!(
                "Absolute mouse device opened: {}",
                self.mouse_abs_path.display()
            );
        } else {
            warn!(
                "Absolute mouse device not found: {}",
                self.mouse_abs_path.display()
            );
        }

        // Open consumer control device (optional, may not exist on older setups)
        if self.consumer_path.exists() {
            let file = Self::open_device(&self.consumer_path)?;
            *self.consumer_dev.lock() = Some(file);
            info!(
                "Consumer control device opened: {}",
                self.consumer_path.display()
            );
        } else {
            debug!(
                "Consumer control device not found: {}",
                self.consumer_path.display()
            );
        }

        // Mark as online if all devices opened successfully
        self.online.store(true, Ordering::Relaxed);

        Ok(())
    }

    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        // Convert JS keycode to USB HID if needed (skip if already USB HID)
        let usb_key = if event.is_usb_hid {
            event.key
        } else {
            keymap::js_to_usb(event.key).unwrap_or(event.key)
        };

        // Handle modifier keys separately
        if keymap::is_modifier_key(usb_key) {
            let mut state = self.keyboard_state.lock();

            if let Some(bit) = keymap::modifier_bit(usb_key) {
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

            // Update modifiers from event
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
                // Relative movement - use hidg1
                let dx = event.x.clamp(-127, 127) as i8;
                let dy = event.y.clamp(-127, 127) as i8;
                self.send_mouse_report_relative(buttons, dx, dy, 0)?;
            }
            MouseEventType::MoveAbs => {
                // Absolute movement - use hidg2
                // Frontend sends 0-32767 range directly (standard HID absolute mouse range)
                // Don't send button state with move - buttons are handled separately on relative device
                let x = event.x.clamp(0, 32767) as u16;
                let y = event.y.clamp(0, 32767) as u16;
                self.send_mouse_report_absolute(0, x, y, 0)?;
            }
            MouseEventType::Down => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_or(bit, Ordering::Relaxed) | bit;
                    // Send on relative device for button clicks
                    self.send_mouse_report_relative(new_buttons, 0, 0, 0)?;
                }
            }
            MouseEventType::Up => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_and(!bit, Ordering::Relaxed) & !bit;
                    self.send_mouse_report_relative(new_buttons, 0, 0, 0)?;
                }
            }
            MouseEventType::Scroll => {
                self.send_mouse_report_relative(buttons, 0, 0, event.scroll)?;
            }
        }

        Ok(())
    }

    async fn reset(&self) -> Result<()> {
        // Reset keyboard
        {
            let mut state = self.keyboard_state.lock();
            state.clear();
            let report = state.clone();
            drop(state);
            self.send_keyboard_report(&report)?;
        }

        // Reset mouse
        self.mouse_buttons.store(0, Ordering::Relaxed);
        self.send_mouse_report_relative(0, 0, 0, 0)?;
        self.send_mouse_report_absolute(0, 0, 0, 0)?;

        info!("HID state reset");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // Reset before closing
        self.reset().await?;

        // Close devices
        *self.keyboard_dev.lock() = None;
        *self.mouse_rel_dev.lock() = None;
        *self.mouse_abs_dev.lock() = None;
        *self.consumer_dev.lock() = None;

        // Gadget cleanup is handled by OtgService, not here

        info!("OTG backend shutdown");
        Ok(())
    }

    fn supports_absolute_mouse(&self) -> bool {
        self.mouse_abs_path.exists()
    }

    async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        self.send_consumer_report(event.usage)
    }

    fn screen_resolution(&self) -> Option<(u32, u32)> {
        *self.screen_resolution.read()
    }

    fn set_screen_resolution(&mut self, width: u32, height: u32) {
        *self.screen_resolution.write() = Some((width, height));
    }
}

/// Check if OTG HID gadget is available
pub fn is_otg_available() -> bool {
    // Check for existing HID devices (they should be created by OtgService)
    let kb = PathBuf::from("/dev/hidg0");
    let mouse_rel = PathBuf::from("/dev/hidg1");
    let mouse_abs = PathBuf::from("/dev/hidg2");

    kb.exists() && mouse_rel.exists() && mouse_abs.exists()
}

/// Implement Drop for OtgBackend to close device files
impl Drop for OtgBackend {
    fn drop(&mut self) {
        // Close device files
        // Note: Gadget cleanup is handled by OtgService, not here
        *self.keyboard_dev.lock() = None;
        *self.mouse_rel_dev.lock() = None;
        *self.mouse_abs_dev.lock() = None;
        *self.consumer_dev.lock() = None;
        debug!("OtgBackend dropped, device files closed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otg_availability_check() {
        // This just tests the function runs without panicking
        let _available = is_otg_available();
    }

    #[test]
    fn test_led_state() {
        let state = LedState::from_byte(0b00000011);
        assert!(state.num_lock);
        assert!(state.caps_lock);
        assert!(!state.scroll_lock);

        assert_eq!(state.to_byte(), 0b00000011);
    }

    #[test]
    fn test_report_sizes() {
        // Keyboard report is 8 bytes
        let kb_report = KeyboardReport::default();
        assert_eq!(kb_report.to_bytes().len(), 8);
    }
}
