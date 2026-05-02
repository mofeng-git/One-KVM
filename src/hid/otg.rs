//! Linux gadget HID: `/dev/hidg*` opened from [`crate::otg::OtgService`].
//! Typical nodes: hidg0 keyboard, hidg1 relative mouse, hidg2 absolute, hidg3 consumer control.
//!
//! Polled timed writes (JetKVM-style). Treat `ESHUTDOWN` (108) by closing handles and reopening; keep fd on `EAGAIN` (11). Host/gadget teardown during MSD resembles PiKVM. <https://github.com/raspberrypi/linux/issues/4373>

use async_trait::async_trait;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use parking_lot::Mutex;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, info, trace, warn};

use super::backend::{HidBackend, HidBackendRuntimeSnapshot};
use super::types::{
    ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardReport, MouseEvent, MouseEventType,
};
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
    mouse_buttons: AtomicU8,
    led_state: Arc<parking_lot::RwLock<LedState>>,
    screen_resolution: parking_lot::RwLock<Option<(u32, u32)>>,
    udc_name: Arc<parking_lot::RwLock<Option<String>>>,
    initialized: AtomicBool,
    online: AtomicBool,
    last_error: parking_lot::RwLock<Option<(String, String)>>,
    last_error_log: parking_lot::Mutex<std::time::Instant>,
    error_count: AtomicU8,
    eagain_count: AtomicU8,
    runtime_notify_tx: watch::Sender<()>,
    runtime_worker_stop: Arc<AtomicBool>,
    runtime_worker: Mutex<Option<thread::JoinHandle<()>>>,
}

const HID_WRITE_TIMEOUT_MS: i32 = 20;

impl OtgBackend {
    /// Gadget must already exist; paths come from `OtgService`.
    pub fn from_handles(paths: HidDevicePaths) -> Result<Self> {
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
            mouse_buttons: AtomicU8::new(0),
            led_state: Arc::new(parking_lot::RwLock::new(LedState::default())),
            screen_resolution: parking_lot::RwLock::new(Some((1920, 1080))),
            udc_name: Arc::new(parking_lot::RwLock::new(paths.udc)),
            initialized: AtomicBool::new(false),
            online: AtomicBool::new(false),
            last_error: parking_lot::RwLock::new(None),
            last_error_log: parking_lot::Mutex::new(std::time::Instant::now()),
            error_count: AtomicU8::new(0),
            eagain_count: AtomicU8::new(0),
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
                warn!("{} (repeated {} times)", msg, count);
            } else {
                warn!("{}", msg);
            }
            *last_log = now;
        } else {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn reset_error_count(&self) {
        self.error_count.store(0, Ordering::Relaxed);
        self.eagain_count.store(0, Ordering::Relaxed);
    }

    /// Poll-based write with `HID_WRITE_TIMEOUT_MS`; timeout → drop (JetKVM-style).
    fn write_with_timeout(&self, file: &mut File, data: &[u8]) -> std::io::Result<bool> {
        let mut pollfd = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];

        match poll(&mut pollfd, PollTimeout::from(HID_WRITE_TIMEOUT_MS as u16)) {
            Ok(1) => {
                if let Some(revents) = pollfd[0].revents() {
                    if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP)
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Device error or hangup",
                        ));
                    }
                }
                file.write_all(data)?;
                Ok(true)
            }
            Ok(0) => {
                trace!("HID write timeout, dropping data");
                Ok(false)
            }
            Ok(_) => Ok(false),
            Err(e) => Err(std::io::Error::other(e)),
        }
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
        if self.keyboard_path.is_none() {
            return Ok(());
        }

        self.ensure_device(DeviceType::Keyboard)?;

        let mut dev = self.keyboard_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = report.to_bytes();
            match self.write_with_timeout(file, &data) {
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
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Keyboard ESHUTDOWN, closing for recovery");
                            *dev = None;
                            self.record_error(
                                format!("Failed to write keyboard report: {}", e),
                                "eshutdown",
                            );
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write keyboard report",
                            ))
                        }
                        Some(11) => {
                            trace!("Keyboard EAGAIN after poll, dropping");
                            Ok(())
                        }
                        _ => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Keyboard write error: {}", e);
                            self.record_error(
                                format!("Failed to write keyboard report: {}", e),
                                Self::io_error_code(&e),
                            );
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

    fn send_mouse_report_relative(&self, buttons: u8, dx: i8, dy: i8, wheel: i8) -> Result<()> {
        if self.mouse_rel_path.is_none() {
            return Ok(());
        }

        self.ensure_device(DeviceType::MouseRelative)?;

        let mut dev = self.mouse_rel_dev.lock();
        if let Some(ref mut file) = *dev {
            let data = [buttons, dx as u8, dy as u8, wheel as u8];
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    self.mark_online();
                    self.reset_error_count();
                    trace!("Sent relative mouse report: {:02X?}", data);
                    Ok(())
                }
                Ok(false) => Ok(()),
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Relative mouse ESHUTDOWN, closing for recovery");
                            *dev = None;
                            self.record_error(
                                format!("Failed to write mouse report: {}", e),
                                "eshutdown",
                            );
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                        Some(11) => Ok(()),
                        _ => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Relative mouse write error: {}", e);
                            self.record_error(
                                format!("Failed to write mouse report: {}", e),
                                Self::io_error_code(&e),
                            );
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
            match self.write_with_timeout(file, &data) {
                Ok(true) => {
                    self.mark_online();
                    self.reset_error_count();
                    Ok(())
                }
                Ok(false) => Ok(()),
                Err(e) => {
                    let error_code = e.raw_os_error();

                    match error_code {
                        Some(108) => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            debug!("Absolute mouse ESHUTDOWN, closing for recovery");
                            *dev = None;
                            self.record_error(
                                format!("Failed to write mouse report: {}", e),
                                "eshutdown",
                            );
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write mouse report",
                            ))
                        }
                        Some(11) => Ok(()),
                        _ => {
                            self.eagain_count.store(0, Ordering::Relaxed);
                            warn!("Absolute mouse write error: {}", e);
                            self.record_error(
                                format!("Failed to write mouse report: {}", e),
                                Self::io_error_code(&e),
                            );
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
                Err(e) => {
                    let error_code = e.raw_os_error();
                    match error_code {
                        Some(108) => {
                            debug!("Consumer control ESHUTDOWN, closing for recovery");
                            *dev = None;
                            self.record_error(
                                format!("Failed to write consumer report: {}", e),
                                "eshutdown",
                            );
                            Err(Self::io_error_to_hid_error(
                                e,
                                "Failed to write consumer report",
                            ))
                        }
                        Some(11) => Ok(()),
                        _ => {
                            warn!("Consumer control write error: {}", e);
                            self.record_error(
                                format!("Failed to write consumer report: {}", e),
                                Self::io_error_code(&e),
                            );
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

    pub fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        self.send_consumer_report(event.usage)
    }

    pub fn led_state(&self) -> LedState {
        *self.led_state.read()
    }

    fn build_runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
        let initialized = self.initialized.load(Ordering::Relaxed);
        let mut online = initialized && self.online.load(Ordering::Relaxed);
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
                    thread::sleep(Duration::from_millis(500));
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
                        warn!("OTG keyboard LED listener read failed: {}", err);
                        *file = None;
                        true
                    }
                }
            }
            Err(err) => {
                warn!("OTG keyboard LED listener poll failed: {}", err);
                *file = None;
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

                    if keyboard_leds_enabled {
                        if let Some(path) = keyboard_path.as_ref() {
                            changed |= Self::poll_keyboard_led_once(
                                &mut keyboard_led_file,
                                path,
                                &led_state,
                            );
                        } else {
                            thread::sleep(Duration::from_millis(500));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(500));
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
        let buttons = self.mouse_buttons.load(Ordering::Relaxed);

        match event.event_type {
            MouseEventType::Move => {
                let dx = event.x.clamp(-127, 127) as i8;
                let dy = event.y.clamp(-127, 127) as i8;
                self.send_mouse_report_relative(buttons, dx, dy, 0)?;
            }
            MouseEventType::MoveAbs => {
                // Coordinates 0–32767; buttons are sent only on the relative endpoint.
                let x = event.x.clamp(0, 32767) as u16;
                let y = event.y.clamp(0, 32767) as u16;
                self.send_mouse_report_absolute(0, x, y, 0)?;
            }
            MouseEventType::Down => {
                if let Some(button) = event.button {
                    let bit = button.to_hid_bit();
                    let new_buttons = self.mouse_buttons.fetch_or(bit, Ordering::Relaxed) | bit;
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
        {
            let mut state = self.keyboard_state.lock();
            state.clear();
            let report = state.clone();
            drop(state);
            self.send_keyboard_report(&report)?;
        }

        self.mouse_buttons.store(0, Ordering::Relaxed);
        self.send_mouse_report_relative(0, 0, 0, 0)?;
        self.send_mouse_report_absolute(0, 0, 0, 0)?;

        info!("HID state reset");
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
}
