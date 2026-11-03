//! Linux gadget HID: `/dev/hidg*` opened from [`crate::otg::OtgService`].
//! Typical nodes: hidg0 keyboard, hidg1 relative mouse, hidg2 absolute, hidg3 consumer control.
//!
//! Polled timed writes (JetKVM-style). Treat `ESHUTDOWN` (108) by closing handles and reopening; keep fd on `EAGAIN` (11). Host/gadget teardown during MSD resembles PiKVM. <https://github.com/raspberrypi/linux/issues/4373>

use async_trait::async_trait;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, info, trace, warn};

use super::backend::{HidBackend, HidBackendRuntimeSnapshot};
use super::mouse_state::{IntentKind, MouseState};
use super::otg_device::OtgDeviceIo;
use super::types::{ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent};
use crate::error::{AppError, Result};
use crate::events::LedState;
use crate::otg::{wait_for_hid_devices, HidDevicePaths};

#[derive(Debug, Clone, Copy)]
enum DeviceType {
    Keyboard,
    MouseRelative,
    MouseAbsolute,
    ConsumerControl,
}

impl LedState {
    pub fn from_byte(b: u8) -> Self {
        Self {
            num_lock: b & 0x01 != 0,
            caps_lock: b & 0x02 != 0,
            scroll_lock: b & 0x04 != 0,
            compose: b & 0x08 != 0,
            kana: b & 0x10 != 0,
        }
    }

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

/// Opens `/dev/hidg*` nodes provisioned by `OtgService`; gadget lifecycle is not handled here.
pub struct OtgBackend {
    keyboard_path: Option<PathBuf>,
    mouse_rel_path: Option<PathBuf>,
    mouse_abs_path: Option<PathBuf>,
    consumer_path: Option<PathBuf>,
    keyboard_dev: Mutex<Option<File>>,
    mouse_rel_dev: Mutex<Option<File>>,
    mouse_abs_dev: Mutex<Option<File>>,
    consumer_dev: Mutex<Option<File>>,
    keyboard_leds_enabled: bool,
    keyboard_state: Mutex<KeyboardReport>,
    mouse_state: Mutex<MouseState>,
    mouse_recovery_needed: AtomicBool,
    #[cfg(test)]
    mouse_write_faults: Mutex<std::collections::VecDeque<std::io::ErrorKind>>,
    macos_drag: bool,
    macos_drag_state: Mutex<super::macos_drag::MacosDrag>,
    led_state: Arc<parking_lot::RwLock<LedState>>,
    screen_resolution: parking_lot::RwLock<Option<(u32, u32)>>,
    udc_name: Arc<parking_lot::RwLock<Option<String>>>,
    initialized: AtomicBool,
    online: AtomicBool,
    last_error: parking_lot::RwLock<Option<(String, String)>>,
    last_error_log: parking_lot::Mutex<std::time::Instant>,
    error_count: AtomicU8,
    runtime_notify_tx: watch::Sender<()>,
    runtime_worker_stop: Arc<AtomicBool>,
    runtime_worker: Mutex<Option<thread::JoinHandle<()>>>,
}

const HID_WRITE_TIMEOUT_MS: i32 = 20;
// Backpressure the sequential controller instead of dropping button edges.
const MOUSE_WRITE_TIMEOUT_MS: i32 = 100;
const OTG_RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(500);

impl OtgBackend {
    /// Gadget must already exist; paths come from `OtgService`.
    pub fn from_handles(paths: HidDevicePaths) -> Result<Self> {
        Self::with_macos_drag(paths, false)
    }

    pub fn with_macos_drag(paths: HidDevicePaths, macos_drag: bool) -> Result<Self> {
        if macos_drag && (paths.mouse_relative.is_none() || paths.mouse_absolute.is_none()) {
            return Err(AppError::Config(
                "macOS drag compatibility requires both OTG mouse interfaces".into(),
            ));
        }
        let (runtime_notify_tx, _runtime_notify_rx) = watch::channel(());
        Ok(Self {
            keyboard_path: paths.keyboard,
            mouse_rel_path: paths.mouse_relative,
            mouse_abs_path: paths.mouse_absolute,
            consumer_path: paths.consumer,
            keyboard_dev: Mutex::new(None),
            mouse_rel_dev: Mutex::new(None),
            mouse_abs_dev: Mutex::new(None),
            consumer_dev: Mutex::new(None),
            keyboard_leds_enabled: paths.keyboard_leds_enabled,
            keyboard_state: Mutex::new(KeyboardReport::default()),
            mouse_state: Mutex::new(MouseState::default()),
            mouse_recovery_needed: AtomicBool::new(false),
            #[cfg(test)]
            mouse_write_faults: Mutex::new(std::collections::VecDeque::new()),
            macos_drag,
            macos_drag_state: Mutex::new(super::macos_drag::MacosDrag::default()),
            led_state: Arc::new(parking_lot::RwLock::new(LedState::default())),
            screen_resolution: parking_lot::RwLock::new(Some((1920, 1080))),
            udc_name: Arc::new(parking_lot::RwLock::new(paths.udc)),
            initialized: AtomicBool::new(false),
            online: AtomicBool::new(false),
            last_error: parking_lot::RwLock::new(None),
            last_error_log: parking_lot::Mutex::new(std::time::Instant::now()),
            error_count: AtomicU8::new(0),
            runtime_notify_tx,
            runtime_worker_stop: Arc::new(AtomicBool::new(false)),
            runtime_worker: Mutex::new(None),
        })
    }

    fn notify_runtime_changed(&self) {
        let _ = self.runtime_notify_tx.send(());
    }

    fn clear_error(&self) {
        let mut error = self.last_error.write();
        if error.is_some() {
            *error = None;
            self.notify_runtime_changed();
        }
    }

    fn record_error(&self, reason: impl Into<String>, error_code: impl Into<String>) {
        let reason = reason.into();
        let error_code = error_code.into();
        let was_online = self.online.swap(false, Ordering::Relaxed);
        let mut error = self.last_error.write();
        let changed = error.as_ref() != Some(&(reason.clone(), error_code.clone()));
        *error = Some((reason, error_code));
        drop(error);
        if was_online || changed {
            self.notify_runtime_changed();
        }
    }

    fn mark_online(&self) {
        // Opening a node or writing a peer endpoint is not recovery proof.
        if self.mouse_recovery_needed.load(Ordering::Acquire) {
            return;
        }
        let was_online = self.online.swap(true, Ordering::Relaxed);
        let mut error = self.last_error.write();
        let cleared_error = error.take().is_some();
        drop(error);
        if !was_online || cleared_error {
            self.notify_runtime_changed();
        }
    }

    fn log_throttled_error(&self, msg: &str) {
        let mut last_log = self.last_error_log.lock();
        let now = std::time::Instant::now();
        if now.duration_since(*last_log).as_secs() >= 1 {
            let count = self.error_count.swap(0, Ordering::Relaxed);
            if count > 1 {
                debug!("{} (repeated {} times)", msg, count);
            } else {
                debug!("{}", msg);
            }
            *last_log = now;
        } else {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn reset_error_count(&self) {
        self.error_count.store(0, Ordering::Relaxed);
    }

    /// Poll-based write with `HID_WRITE_TIMEOUT_MS`; timeout → drop (JetKVM-style).
    fn write_with_timeout(&self, file: &mut File, data: &[u8]) -> std::io::Result<bool> {
        OtgDeviceIo::write_with_timeout(file, data, HID_WRITE_TIMEOUT_MS)
    }

    pub fn set_udc_name(&self, udc: &str) {
        *self.udc_name.write() = Some(udc.to_string());
    }

    fn read_udc_configured(udc_name: &parking_lot::RwLock<Option<String>>) -> bool {
        let current_udc = udc_name.read().clone().or_else(Self::find_udc);
        if let Some(udc) = current_udc {
            {
                let mut guard = udc_name.write();
                if guard.as_ref() != Some(&udc) {
                    *guard = Some(udc.clone());
                }
            }

            let state_path = format!("/sys/class/udc/{}/state", udc);
            match fs::read_to_string(&state_path) {
                Ok(content) => {
                    let state = content.trim().to_lowercase();
                    trace!("UDC {} state: {}", udc, state);
                    state == "configured"
                }
                Err(e) => {
                    debug!("Failed to read UDC state from {}: {}", state_path, e);
                    true
                }
            }
        } else {
            true
        }
    }

    /// `true` when `/sys/class/udc/<name>/state` reads `configured` (PiKVM-style).
    pub fn is_udc_configured(&self) -> bool {
        Self::read_udc_configured(&self.udc_name)
    }

    fn find_udc() -> Option<String> {
        crate::otg::configfs::find_udc()
    }

    /// PiKVM-style: drop handle if node missing; reopen when path reappears.
    fn ensure_device(&self, device_type: DeviceType) -> Result<()> {
        let (path_opt, dev_mutex) = match device_type {
            DeviceType::Keyboard => (&self.keyboard_path, &self.keyboard_dev),
            DeviceType::MouseRelative => (&self.mouse_rel_path, &self.mouse_rel_dev),
            DeviceType::MouseAbsolute => (&self.mouse_abs_path, &self.mouse_abs_dev),
            DeviceType::ConsumerControl => (&self.consumer_path, &self.consumer_dev),
        };

        let path = match path_opt {
            Some(p) => p,
            None => {
                let err = AppError::HidError {
                    backend: "otg".to_string(),
                    reason: "Device disabled".to_string(),
                    error_code: "disabled".to_string(),
                };
                self.record_error("Device disabled", "disabled");
                return Err(err);
            }
        };

        if !path.exists() {
            let mut dev = dev_mutex.lock();
            if dev.is_some() {
                debug!(
                    "Device path {} no longer exists, closing handle",
                    path.display()
                );
                *dev = None;
            }
            let reason = format!("Device not found: {}", path.display());
            self.record_error(reason.clone(), "enoent");
            return Err(AppError::HidError {
                backend: "otg".to_string(),
                reason,
                error_code: "enoent".to_string(),
            });
        }

        let mut dev = dev_mutex.lock();
        if dev.is_none() {
            match Self::open_device(path) {
                Ok(file) => {
                    info!("Reopened HID device: {}", path.display());
                    *dev = Some(file);
                }
                Err(e) => {
                    warn!("Failed to reopen HID device {}: {}", path.display(), e);
                    self.record_error(
                        format!("Failed to reopen HID device {}: {}", path.display(), e),
                        "not_opened",
                    );
                    return Err(e);
                }
            }
        }

        self.mark_online();
        Ok(())
    }

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

    fn io_error_code(e: &std::io::Error) -> &'static str {
        match e.raw_os_error() {
            Some(32) => "epipe",
            Some(108) => "eshutdown",
            Some(11) => "eagain",
            Some(6) => "enxio",
            Some(19) => "enodev",
            Some(5) => "eio",
            Some(2) => "enoent",
            _ => "io_error",
        }
    }

    fn io_error_to_hid_error(e: std::io::Error, operation: &str) -> AppError {
        let error_code = Self::io_error_code(&e);

        AppError::HidError {
            backend: "otg".to_string(),
            reason: format!("{}: {}", operation, e),
            error_code: error_code.to_string(),
        }
    }

    fn handle_write_error(
        &self,
        dev: &mut Option<File>,
        err: std::io::Error,
        operation: &str,
        device_label: &str,
    ) -> Result<()> {
        match err.raw_os_error() {
            Some(108) => {
                debug!("{} ESHUTDOWN, closing for recovery", device_label);
                *dev = None;
                self.record_error(format!("{}: {}", operation, err), "eshutdown");
                Err(Self::io_error_to_hid_error(err, operation))
            }
            Some(11) => {
                trace!("{} EAGAIN after poll, dropping", device_label);
                Ok(())
            }
            _ => {
                warn!("{} write error: {}", device_label, err);
                self.record_error(format!("{}: {}", operation, err), Self::io_error_code(&err));
                Err(Self::io_error_to_hid_error(err, operation))
            }
        }
    }

    pub fn check_devices_exist(&self) -> bool {
        self.keyboard_path.as_ref().is_none_or(|p| p.exists())
            && self.mouse_rel_path.as_ref().is_none_or(|p| p.exists())
            && self.mouse_abs_path.as_ref().is_none_or(|p| p.exists())
            && self.consumer_path.as_ref().is_none_or(|p| p.exists())
    }

    pub fn get_missing_devices(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if let Some(ref path) = self.keyboard_path {
            if !path.exists() {
                missing.push(path.display().to_string());
            }
        }
        if let Some(ref path) = self.mouse_rel_path {
            if !path.exists() {
                missing.push(path.display().to_string());
            }
        }
        if let Some(ref path) = self.mouse_abs_path {
            if !path.exists() {
                missing.push(path.display().to_string());
            }
        }
        missing
    }

    fn send_keyboard_report(&self, report: &KeyboardReport) -> Result<()> {
        self.send_keyboard_report_impl(report, false)
    }

    fn send_keyboard_report_impl(&self, report: &KeyboardReport, reliable: bool) -> Result<()> {
        if self.keyboard_path.is_none() {
            return Ok(());
        }

        self.ensure_device(DeviceType::Keyboard)?;

        let mut dev = self.keyboard_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = report.to_bytes();
            match if reliable {
                match OtgDeviceIo::write_with_timeout(file, &data, MOUSE_WRITE_TIMEOUT_MS) {
                    Ok(false) => Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Keyboard release timed out",
                    )),
                    result => result,
                }
            } else {
                self.write_with_timeout(file, &data)
            } {
                Ok(true) => {
                    self.mark_online();
                    self.reset_error_count();
                    debug!("Sent keyboard report: {:02X?}", data);
                    Ok(())
                }
                Ok(false) => {
                    self.log_throttled_error("HID keyboard write timeout, dropped");
                    Ok(())
                }
                Err(e) => self.handle_write_error(
                    &mut dev,
                    e,
                    "Failed to write keyboard report",
                    "Keyboard",
                ),
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Keyboard device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    // Success means kernel acceptance, not a guest delivery acknowledgement.
    fn write_mouse_with_timeout(&self, file: &mut File, data: &[u8]) -> std::io::Result<bool> {
        #[cfg(test)]
        if let Some(kind) = self.mouse_write_faults.lock().pop_front() {
            self.record_error("Mouse report delivery failed", "mouse_write_failed");
            return Err(kind.into());
        }
        let result = OtgDeviceIo::write_with_timeout(file, data, MOUSE_WRITE_TIMEOUT_MS);
        match result {
            Ok(true) => Ok(true),
            Ok(false) => {
                self.record_error(
                    "Mouse report timed out; delivery not confirmed",
                    "mouse_write_timeout",
                );
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Mouse HID report timed out",
                ))
            }
            Err(e) => {
                self.record_error(format!("Mouse report failed: {e}"), "mouse_write_failed");
                Err(e)
            }
        }
    }

    fn send_mouse_inner(&self, mouse: &mut MouseState, event: MouseEvent) -> Result<()> {
        // One lock covers transitions and timed writes, including reset.
        let buttons = mouse.buttons();
        let Some(intent) = mouse.apply(&event) else {
            return Ok(());
        };

        if self.macos_drag {
            use super::macos_drag::MouseReport;
            let mut state = self.macos_drag_state.lock();
            let extent = self.screen_resolution.read().unwrap_or((1920, 1080));
            let (buttons, reports) = state.plan(event, buttons, extent);
            debug_assert_eq!(buttons, mouse.buttons());
            for report in reports.iter().copied() {
                match report {
                    MouseReport::Absolute { buttons, x, y } => {
                        self.send_mouse_report_absolute(buttons, x, y, 0)?
                    }
                    MouseReport::Relative {
                        buttons,
                        dx,
                        dy,
                        wheel,
                    } => self.send_mouse_report_relative(buttons, dx, dy, wheel)?,
                }
            }
            return Ok(());
        }

        // Device availability and dual-interface policy remain OTG-specific.
        match intent.kind {
            IntentKind::RelativeMove => self.send_mouse_report_relative(
                intent.buttons,
                intent.relative.0,
                intent.relative.1,
                0,
            )?,
            IntentKind::AbsoluteMove => {
                let buttons = if self.mouse_rel_path.is_some() {
                    0
                } else {
                    intent.buttons
                };
                self.send_mouse_report_absolute(buttons, intent.absolute.0, intent.absolute.1, 0)?;
            }
            IntentKind::Buttons | IntentKind::Wheel => {
                if self.mouse_rel_path.is_some() {
                    self.send_mouse_report_relative(intent.buttons, 0, 0, intent.wheel)?;
                } else if self.mouse_abs_path.is_some() {
                    self.send_mouse_report_absolute(
                        intent.buttons,
                        intent.absolute.0,
                        intent.absolute.1,
                        intent.wheel,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn recover_mouse(&self, mouse: &mut MouseState) -> Result<()> {
        // Do not replay old movement/wheel/press after uncertainty. Release both
        // possible endpoints before admitting new input, even after partial dual
        // interface delivery. Retain the last committed absolute position.
        self.mouse_recovery_needed.store(true, Ordering::Release);
        let (x, y) = mouse.absolute();
        let relative = self.send_mouse_report_relative(0, 0, 0, 0);
        let absolute = self.send_mouse_report_absolute(0, x, y, 0);
        if relative.is_err() || absolute.is_err() {
            self.record_error("Mouse release recovery incomplete", "mouse_recovery_failed");
            relative?;
            absolute?;
        }
        mouse.release_buttons();
        self.macos_drag_state.lock().reset();
        self.mouse_recovery_needed.store(false, Ordering::Release);
        self.mark_online();
        Ok(())
    }

    fn send_mouse_report_relative(&self, buttons: u8, dx: i8, dy: i8, wheel: i8) -> Result<()> {
        if self.mouse_rel_path.is_none() {
            return Ok(());
        }

        self.ensure_device(DeviceType::MouseRelative)?;

        let mut dev = self.mouse_rel_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = [buttons, dx as u8, dy as u8, wheel as u8];
            match self.write_mouse_with_timeout(file, &data) {
                Ok(true) => {
                    self.mark_online();
                    self.reset_error_count();
                    trace!("Sent relative mouse report: {:02X?}", data);
                    Ok(())
                }
                Ok(false) => unreachable!("mouse timeout is an error"),
                Err(e) => self.handle_write_error(
                    &mut dev,
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Mouse HID remained blocked",
                        )
                    } else {
                        e
                    },
                    "Failed to write mouse report",
                    "Relative mouse",
                ),
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Relative mouse device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    fn send_mouse_report_absolute(&self, buttons: u8, x: u16, y: u16, wheel: i8) -> Result<()> {
        if self.mouse_abs_path.is_none() {
            return Ok(());
        }

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
            match self.write_mouse_with_timeout(file, &data) {
                Ok(true) => {
                    self.mark_online();
                    self.reset_error_count();
                    Ok(())
                }
                Ok(false) => unreachable!("mouse timeout is an error"),
                Err(e) => self.handle_write_error(
                    &mut dev,
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Mouse HID remained blocked",
                        )
                    } else {
                        e
                    },
                    "Failed to write mouse report",
                    "Absolute mouse",
                ),
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Absolute mouse device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    /// Press (`usage`) then release (`0x0000`).
    fn send_consumer_report(&self, usage: u16) -> Result<()> {
        if self.consumer_path.is_none() {
            return Ok(());
        }

        self.ensure_device(DeviceType::ConsumerControl)?;

        let mut dev = self.consumer_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = [(usage & 0xFF) as u8, (usage >> 8) as u8];
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    trace!("Sent consumer report: {:02X?}", data);
                    let release = [0u8, 0u8];
                    let _ = self.write_with_timeout(file, &release);
                    self.mark_online();
                    self.reset_error_count();
                    Ok(())
                }
                Ok(false) => Ok(()),
                Err(e) => self.handle_write_error(
                    &mut dev,
                    e,
                    "Failed to write consumer report",
                    "Consumer control",
                ),
            }
        } else {
            Err(AppError::HidError {
                backend: "otg".to_string(),
                reason: "Consumer control device not opened".to_string(),
                error_code: "not_opened".to_string(),
            })
        }
    }

    pub fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        self.send_consumer_report(event.usage)
    }

    pub fn led_state(&self) -> LedState {
        *self.led_state.read()
    }

    fn build_runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
        let initialized = self.initialized.load(Ordering::Relaxed);
        let mut online = initialized
            && self.online.load(Ordering::Relaxed)
            && !self.mouse_recovery_needed.load(Ordering::Acquire);
        let mut error = self.last_error.read().clone();

        if initialized && !self.check_devices_exist() {
            online = false;
            let missing = self.get_missing_devices();
            error = Some((
                format!("HID device node missing: {}", missing.join(", ")),
                "enoent".to_string(),
            ));
        } else if initialized && !self.is_udc_configured() {
            online = false;
            error = Some((
                "UDC is not in configured state".to_string(),
                "udc_not_configured".to_string(),
            ));
        }

        HidBackendRuntimeSnapshot {
            initialized,
            online,
            supports_absolute_mouse: self.mouse_abs_path.as_ref().is_some_and(|p| p.exists()),
            keyboard_leds_enabled: self.keyboard_leds_enabled,
            led_state: self.led_state(),
            screen_resolution: *self.screen_resolution.read(),
            device: self.udc_name.read().clone(),
            error: error.as_ref().map(|(reason, _)| reason.clone()),
            error_code: error.as_ref().map(|(_, code)| code.clone()),
        }
    }

    fn poll_keyboard_led_once(
        file: &mut Option<File>,
        path: &PathBuf,
        led_state: &Arc<parking_lot::RwLock<LedState>>,
    ) -> bool {
        if file.is_none() {
            match OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)
            {
                Ok(opened) => {
                    *file = Some(opened);
                }
                Err(err) => {
                    warn!(
                        "Failed to open OTG keyboard LED listener {}: {}",
                        path.display(),
                        err
                    );
                    thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                    return false;
                }
            }
        }

        let Some(file_ref) = file.as_mut() else {
            return false;
        };

        let mut pollfd = [PollFd::new(
            file_ref.as_fd(),
            PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP,
        )];

        match poll(&mut pollfd, PollTimeout::from(500u16)) {
            Ok(0) => false,
            Ok(_) => {
                let Some(revents) = pollfd[0].revents() else {
                    return false;
                };

                if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP) {
                    *file = None;
                    thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                    return true;
                }

                if !revents.contains(PollFlags::POLLIN) {
                    return false;
                }

                let mut buf = [0u8; 1];
                match file_ref.read(&mut buf) {
                    Ok(1) => {
                        let next = LedState::from_byte(buf[0]);
                        let mut guard = led_state.write();
                        if *guard == next {
                            false
                        } else {
                            *guard = next;
                            true
                        }
                    }
                    Ok(_) => false,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => false,
                    Err(err) => {
                        if err.raw_os_error() == Some(libc::ESHUTDOWN) {
                            debug!("OTG keyboard LED listener disconnected: {}", err);
                        } else {
                            warn!("OTG keyboard LED listener read failed: {}", err);
                        }
                        *file = None;
                        thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                        true
                    }
                }
            }
            Err(err) => {
                warn!("OTG keyboard LED listener poll failed: {}", err);
                *file = None;
                thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                true
            }
        }
    }

    fn start_runtime_worker(&self) {
        let mut worker = self.runtime_worker.lock();
        if worker.is_some() {
            return;
        }

        self.runtime_worker_stop.store(false, Ordering::Relaxed);
        let stop = self.runtime_worker_stop.clone();
        let keyboard_leds_enabled = self.keyboard_leds_enabled;
        let keyboard_path = self.keyboard_path.clone();
        let led_state = self.led_state.clone();
        let udc_name = self.udc_name.clone();
        let runtime_notify_tx = self.runtime_notify_tx.clone();

        let handle = thread::Builder::new()
            .name("otg-runtime-monitor".to_string())
            .spawn(move || {
                let mut last_udc_configured = Some(Self::read_udc_configured(&udc_name));
                let mut keyboard_led_file: Option<File> = None;

                while !stop.load(Ordering::Relaxed) {
                    let mut changed = false;

                    let current_udc_configured = Self::read_udc_configured(&udc_name);
                    if last_udc_configured != Some(current_udc_configured) {
                        last_udc_configured = Some(current_udc_configured);
                        changed = true;
                    }

                    if keyboard_leds_enabled && current_udc_configured {
                        if let Some(path) = keyboard_path.as_ref() {
                            changed |= Self::poll_keyboard_led_once(
                                &mut keyboard_led_file,
                                path,
                                &led_state,
                            );
                        } else {
                            thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                        }
                    } else {
                        keyboard_led_file = None;
                        thread::sleep(OTG_RUNTIME_POLL_INTERVAL);
                    }

                    if changed {
                        let _ = runtime_notify_tx.send(());
                    }
                }
            });

        match handle {
            Ok(handle) => {
                *worker = Some(handle);
            }
            Err(err) => {
                warn!("Failed to spawn OTG runtime monitor: {}", err);
            }
        }
    }

    fn stop_runtime_worker(&self) {
        self.runtime_worker_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.runtime_worker.lock().take() {
            let _ = handle.join();
        }
    }
}

#[async_trait]
impl HidBackend for OtgBackend {
    async fn init(&self) -> Result<()> {
        debug!("Initializing OTG HID backend");

        if self.udc_name.read().is_none() {
            if let Some(udc) = Self::find_udc() {
                debug!("Auto-detected UDC: {}", udc);
                self.set_udc_name(&udc);
            }
        } else if let Some(udc) = self.udc_name.read().clone() {
            debug!("Using configured UDC: {}", udc);
        }

        let mut device_paths = Vec::new();
        if let Some(ref path) = self.keyboard_path {
            device_paths.push(path.clone());
        }
        if let Some(ref path) = self.mouse_rel_path {
            device_paths.push(path.clone());
        }
        if let Some(ref path) = self.mouse_abs_path {
            device_paths.push(path.clone());
        }
        if let Some(ref path) = self.consumer_path {
            device_paths.push(path.clone());
        }

        if device_paths.is_empty() {
            return Err(AppError::Internal(
                "No HID devices configured for OTG backend".into(),
            ));
        }

        if !wait_for_hid_devices(&device_paths, 2000).await {
            return Err(AppError::Internal("HID devices did not appear".into()));
        }

        if let Some(ref path) = self.keyboard_path {
            if path.exists() {
                let file = Self::open_device(path)?;
                *self.keyboard_dev.lock() = Some(file);
                debug!("Keyboard device opened: {}", path.display());
            } else {
                warn!("Keyboard device not found: {}", path.display());
            }
        }

        if let Some(ref path) = self.mouse_rel_path {
            if path.exists() {
                let file = Self::open_device(path)?;
                *self.mouse_rel_dev.lock() = Some(file);
                debug!("Relative mouse device opened: {}", path.display());
            } else {
                warn!("Relative mouse device not found: {}", path.display());
            }
        }

        if let Some(ref path) = self.mouse_abs_path {
            if path.exists() {
                let file = Self::open_device(path)?;
                *self.mouse_abs_dev.lock() = Some(file);
                debug!("Absolute mouse device opened: {}", path.display());
            } else {
                warn!("Absolute mouse device not found: {}", path.display());
            }
        }

        if let Some(ref path) = self.consumer_path {
            if path.exists() {
                let file = Self::open_device(path)?;
                *self.consumer_dev.lock() = Some(file);
                debug!("Consumer control device opened: {}", path.display());
            } else {
                debug!("Consumer control device not found: {}", path.display());
            }
        }

        self.initialized.store(true, Ordering::Relaxed);
        self.notify_runtime_changed();
        self.start_runtime_worker();
        self.mark_online();

        Ok(())
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
        let mut committed = self.mouse_state.lock();
        if self.mouse_recovery_needed.load(Ordering::Acquire) {
            self.recover_mouse(&mut committed)?;
        }
        let mut next = committed.clone();
        match self.send_mouse_inner(&mut next, event) {
            Ok(()) => {
                *committed = next;
                Ok(())
            }
            Err(error) => {
                self.mouse_recovery_needed.store(true, Ordering::Release);
                self.macos_drag_state.lock().reset();
                Err(error)
            }
        }
    }

    async fn reset(&self) -> Result<()> {
        self.mouse_recovery_needed.store(true, Ordering::Release);
        let keyboard_result = {
            let mut state = self.keyboard_state.lock();
            state.clear();
            self.send_keyboard_report_impl(&state, true)
        };
        let mut mouse = self.mouse_state.lock();
        self.mouse_recovery_needed.store(true, Ordering::Release);
        let mut next = mouse.clone();
        let release = next.release_buttons();
        self.macos_drag_state.lock().reset();
        let relative_result = self.send_mouse_report_relative(0, 0, 0, 0);
        let absolute_result = if self.mouse_rel_path.is_none() && self.mouse_abs_path.is_some() {
            self.send_mouse_report_absolute(0, release.absolute.0, release.absolute.1, 0)
        } else {
            next = MouseState::default();
            self.send_mouse_report_absolute(0, 0, 0, 0)
        };
        // Always attempt every release, then report failure. A later endpoint's
        // successful write must not turn a partially failed reset into online.
        let errors: Vec<String> = [keyboard_result, relative_result, absolute_result]
            .into_iter()
            .filter_map(|result| result.err().map(|e| e.to_string()))
            .collect();
        if !errors.is_empty() {
            let reason = format!("HID reset incomplete: {}", errors.join("; "));
            self.record_error(reason.clone(), "reset_incomplete");
            return Err(AppError::HidError {
                backend: "otg".into(),
                reason,
                error_code: "reset_incomplete".into(),
            });
        }
        *mouse = next;
        self.mouse_recovery_needed.store(false, Ordering::Release);
        self.mark_online();
        info!("HID state reset");
        Ok(())
    }

    async fn prepare_rebuild(&self) -> Result<()> {
        self.stop_runtime_worker();
        *self.keyboard_dev.lock() = None;
        *self.mouse_rel_dev.lock() = None;
        *self.mouse_abs_dev.lock() = None;
        *self.consumer_dev.lock() = None;
        self.initialized.store(false, Ordering::Relaxed);
        self.online.store(false, Ordering::Relaxed);
        self.notify_runtime_changed();
        info!("OTG backend prepared for gadget rebuild");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        self.stop_runtime_worker();

        self.reset().await?;

        *self.keyboard_dev.lock() = None;
        *self.mouse_rel_dev.lock() = None;
        *self.mouse_abs_dev.lock() = None;
        *self.consumer_dev.lock() = None;

        self.initialized.store(false, Ordering::Relaxed);
        self.online.store(false, Ordering::Relaxed);
        self.clear_error();
        self.notify_runtime_changed();

        info!("OTG backend shutdown");
        Ok(())
    }

    fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
        self.build_runtime_snapshot()
    }

    fn subscribe_runtime(&self) -> watch::Receiver<()> {
        self.runtime_notify_tx.subscribe()
    }

    async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        self.send_consumer_report(event.usage)
    }

    fn set_screen_resolution(&self, width: u32, height: u32) {
        *self.screen_resolution.write() = Some((width, height));
        self.notify_runtime_changed();
    }
}

impl Drop for OtgBackend {
    fn drop(&mut self) {
        self.runtime_worker_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.runtime_worker.get_mut().take() {
            let _ = handle.join();
        }
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
    use std::io::{Seek, SeekFrom, Write};

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
        let kb_report = KeyboardReport::default();
        assert_eq!(kb_report.to_bytes().len(), 8);
    }

    #[tokio::test]
    async fn mouse_compatibility_writes_both_endpoints_and_preserves_default() {
        use crate::hid::MouseButton;
        for enabled in [false, true] {
            let relative = tempfile::NamedTempFile::new().unwrap();
            let absolute = tempfile::NamedTempFile::new().unwrap();
            let backend = OtgBackend::with_macos_drag(
                HidDevicePaths {
                    mouse_relative: Some(relative.path().to_path_buf()),
                    mouse_absolute: Some(absolute.path().to_path_buf()),
                    ..Default::default()
                },
                enabled,
            )
            .unwrap();
            for event in [
                MouseEvent::move_abs(8000, 8000),
                MouseEvent::button_down(MouseButton::Left),
                MouseEvent::move_abs(16000, 16000),
                MouseEvent::button_up(MouseButton::Left),
            ] {
                backend.send_mouse(event).await.unwrap();
            }
            let abs = fs::read(absolute.path()).unwrap();
            let rel = fs::read(relative.path()).unwrap();
            let buttons: Vec<_> = abs.chunks_exact(6).map(|packet| packet[0]).collect();
            assert_eq!(buttons, if enabled { vec![0, 1, 0] } else { vec![0, 0] });
            let dx: i32 = rel
                .chunks_exact(4)
                .map(|packet| i32::from(packet[1] as i8))
                .sum();
            let dy: i32 = rel
                .chunks_exact(4)
                .map(|packet| i32::from(packet[2] as i8))
                .sum();
            assert_eq!((dx, dy), if enabled { (468, 263) } else { (0, 0) });
            assert_eq!(&rel[rel.len() - 4..], &[0, 0, 0, 0]);
        }
    }

    #[test]
    fn compatibility_requires_both_mouse_endpoints() {
        assert!(OtgBackend::with_macos_drag(HidDevicePaths::default(), true).is_err());
    }

    #[tokio::test]
    async fn absolute_only_click_wheel_drag_and_reset_keep_position() {
        use crate::hid::MouseButton;
        let absolute = tempfile::NamedTempFile::new().unwrap();
        let backend = OtgBackend::from_handles(HidDevicePaths {
            mouse_absolute: Some(absolute.path().to_path_buf()),
            ..Default::default()
        })
        .unwrap();
        for event in [
            MouseEvent::move_abs(18022, 16384),
            MouseEvent::move_rel(10, 10), // No relative output exists; no extra report.
            MouseEvent::button_down(MouseButton::Left),
            MouseEvent::scroll(-1),
            MouseEvent::move_abs(8192, 12288),
            MouseEvent::button_up(MouseButton::Left),
        ] {
            backend.send_mouse(event).await.unwrap();
        }
        backend.reset().await.unwrap();
        assert_eq!(
            fs::read(absolute.path()).unwrap(),
            [
                [0, 102, 70, 0, 64, 0],
                [1, 102, 70, 0, 64, 0],
                [1, 102, 70, 0, 64, 255],
                [1, 0, 32, 0, 48, 0],
                [0, 0, 32, 0, 48, 0],
                [0, 0, 32, 0, 48, 0],
            ]
            .concat()
        );
    }

    #[tokio::test]
    async fn relative_only_ignores_missing_absolute_output_and_preserves_reports() {
        use crate::hid::MouseButton;
        let relative = tempfile::NamedTempFile::new().unwrap();
        let backend = OtgBackend::from_handles(HidDevicePaths {
            mouse_relative: Some(relative.path().to_path_buf()),
            ..Default::default()
        })
        .unwrap();
        for event in [
            MouseEvent::move_abs(10000, 20000),
            MouseEvent::button_down(MouseButton::Right),
            MouseEvent::move_rel(200, -200),
            MouseEvent::scroll(2),
            MouseEvent::button_up(MouseButton::Right),
        ] {
            backend.send_mouse(event).await.unwrap();
        }
        backend.reset().await.unwrap();
        assert_eq!(
            fs::read(relative.path()).unwrap(),
            [
                [2, 0, 0, 0],
                [2, 127, 129, 0],
                [2, 0, 0, 2],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
            ]
            .concat()
        );
        let empty = OtgBackend::from_handles(HidDevicePaths::default()).unwrap();
        empty
            .send_mouse(MouseEvent::button_down(MouseButton::Left))
            .await
            .unwrap();
        empty.send_mouse(MouseEvent::scroll(1)).await.unwrap();
        empty.reset().await.unwrap();
        assert_eq!(empty.mouse_state.lock().buttons(), 0);
    }

    #[test]
    fn concurrent_absolute_button_transitions_match_device_write_order() {
        use crate::hid::MouseButton;
        let absolute = tempfile::NamedTempFile::new().unwrap();
        let backend = Arc::new(
            OtgBackend::from_handles(HidDevicePaths {
                mouse_absolute: Some(absolute.path().to_path_buf()),
                ..Default::default()
            })
            .unwrap(),
        );
        let barrier = Arc::new(std::sync::Barrier::new(5));
        let workers: Vec<_> = [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
        ]
        .into_iter()
        .map(|button| {
            let backend = backend.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();
                barrier.wait();
                runtime
                    .block_on(backend.send_mouse(MouseEvent::button_down(button)))
                    .unwrap();
            })
        })
        .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let reports = fs::read(absolute.path()).unwrap();
        assert_eq!(reports.len(), 30);
        let mut previous: u8 = 0;
        for report in reports.chunks_exact(6) {
            assert_eq!(report[0] & previous, previous);
            assert_eq!(report[0].count_ones(), previous.count_ones() + 1);
            previous = report[0];
        }
        assert_eq!(previous, 31);
    }

    #[tokio::test]
    async fn prepare_rebuild_closes_devices_without_writing_reset_reports() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"sentinel").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let backend = OtgBackend::from_handles(HidDevicePaths::default()).unwrap();
        *backend.keyboard_dev.lock() = Some(file);
        backend.initialized.store(true, Ordering::Relaxed);
        backend.online.store(true, Ordering::Relaxed);

        backend.prepare_rebuild().await.unwrap();

        assert!(backend.keyboard_dev.lock().is_none());
        assert!(!backend.initialized.load(Ordering::Relaxed));
        assert!(!backend.online.load(Ordering::Relaxed));
    }
    fn fault_backend(absolute: bool) -> (tempfile::NamedTempFile, OtgBackend) {
        let file = tempfile::NamedTempFile::new().unwrap();
        let paths = HidDevicePaths {
            mouse_absolute: absolute.then(|| file.path().to_path_buf()),
            mouse_relative: (!absolute).then(|| file.path().to_path_buf()),
            ..Default::default()
        };
        (file, OtgBackend::from_handles(paths).unwrap())
    }

    #[tokio::test]
    async fn mouse_timeout_is_error_and_state_is_not_committed() {
        use crate::hid::MouseButton;
        for absolute in [false, true] {
            let (_file, backend) = fault_backend(absolute);
            backend
                .mouse_write_faults
                .lock()
                .push_back(std::io::ErrorKind::TimedOut);
            assert!(backend
                .send_mouse(MouseEvent::button_down(MouseButton::Right))
                .await
                .is_err());
            assert_eq!(backend.mouse_state.lock().buttons(), 0);
            assert!(backend.mouse_recovery_needed.load(Ordering::Acquire));
            assert!(!backend.online.load(Ordering::Relaxed));
        }
    }

    #[tokio::test]
    async fn failed_release_requires_neutral_before_new_mouse_input() {
        use crate::hid::MouseButton;
        for absolute in [false, true] {
            let (file, backend) = fault_backend(absolute);
            backend
                .send_mouse(MouseEvent::button_down(MouseButton::Right))
                .await
                .unwrap();
            backend
                .mouse_write_faults
                .lock()
                .push_back(std::io::ErrorKind::TimedOut);
            assert!(backend
                .send_mouse(MouseEvent::button_up(MouseButton::Right))
                .await
                .is_err());
            assert_eq!(backend.mouse_state.lock().buttons(), 2);
            backend
                .send_mouse(MouseEvent::button_down(MouseButton::Left))
                .await
                .unwrap();
            let data = std::fs::read(file.path()).unwrap();
            let size = if absolute { 6 } else { 4 };
            assert_eq!(
                data.chunks_exact(size).map(|r| r[0]).collect::<Vec<_>>(),
                [2, 0, 1]
            );
            assert_eq!(backend.mouse_state.lock().buttons(), 1);
            assert!(!backend.mouse_recovery_needed.load(Ordering::Acquire));
        }
    }

    #[tokio::test]
    async fn failed_recovery_rejects_press_and_reset_retries_release() {
        use crate::hid::MouseButton;
        let (file, backend) = fault_backend(true);
        backend
            .send_mouse(MouseEvent::button_down(MouseButton::Right))
            .await
            .unwrap();
        backend
            .mouse_write_faults
            .lock()
            .extend([std::io::ErrorKind::TimedOut; 3]);
        assert!(backend
            .send_mouse(MouseEvent::button_up(MouseButton::Right))
            .await
            .is_err());
        assert!(backend
            .send_mouse(MouseEvent::button_down(MouseButton::Left))
            .await
            .is_err());
        assert!(backend.reset().await.is_err());
        assert_eq!(std::fs::read(file.path()).unwrap().len(), 6);
        assert_eq!(backend.mouse_state.lock().buttons(), 2);
        backend.reset().await.unwrap();
        assert_eq!(std::fs::read(file.path()).unwrap()[6], 0);
        assert_eq!(backend.mouse_state.lock().buttons(), 0);
        assert!(!backend.mouse_recovery_needed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn keyboard_reset_failure_does_not_skip_mouse_release() {
        use crate::hid::MouseButton;
        let file = tempfile::NamedTempFile::new().unwrap();
        let backend = OtgBackend::from_handles(HidDevicePaths {
            keyboard: Some(file.path().join("missing-keyboard")),
            mouse_absolute: Some(file.path().to_path_buf()),
            ..Default::default()
        })
        .unwrap();
        backend
            .send_mouse(MouseEvent::button_down(MouseButton::Right))
            .await
            .unwrap();
        assert!(backend.reset().await.is_err());
        assert_eq!(std::fs::read(file.path()).unwrap()[6], 0);
        assert!(backend.mouse_recovery_needed.load(Ordering::Acquire));
        assert!(!backend.online.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn failed_position_does_not_replace_last_committed_reset_position() {
        let (file, backend) = fault_backend(true);
        backend
            .send_mouse(MouseEvent::move_abs(100, 200))
            .await
            .unwrap();
        backend
            .mouse_write_faults
            .lock()
            .push_back(std::io::ErrorKind::TimedOut);
        assert!(backend
            .send_mouse(MouseEvent::move_abs(300, 400))
            .await
            .is_err());
        backend.reset().await.unwrap();
        assert_eq!(
            &std::fs::read(file.path()).unwrap()[6..],
            &[0, 100, 0, 200, 0, 0]
        );
    }

    #[tokio::test]
    async fn macos_failure_resets_adapter_policy_before_recovery() {
        use crate::hid::MouseButton;
        let rel = tempfile::NamedTempFile::new().unwrap();
        let abs = tempfile::NamedTempFile::new().unwrap();
        let backend = OtgBackend::with_macos_drag(
            HidDevicePaths {
                mouse_relative: Some(rel.path().to_path_buf()),
                mouse_absolute: Some(abs.path().to_path_buf()),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        backend
            .send_mouse(MouseEvent::move_abs(100, 200))
            .await
            .unwrap();
        backend
            .mouse_write_faults
            .lock()
            .push_back(std::io::ErrorKind::TimedOut);
        assert!(backend
            .send_mouse(MouseEvent::button_down(MouseButton::Right))
            .await
            .is_err());
        assert_eq!(backend.mouse_state.lock().buttons(), 0);
        backend.reset().await.unwrap();
        backend
            .send_mouse(MouseEvent::button_down(MouseButton::Left))
            .await
            .unwrap();
        assert_eq!(backend.mouse_state.lock().buttons(), 1);
        assert!(!backend.mouse_recovery_needed.load(Ordering::Acquire));
        assert_eq!(
            std::fs::read(abs.path())
                .unwrap()
                .chunks_exact(6)
                .last()
                .unwrap()[0],
            1
        );
    }
}
