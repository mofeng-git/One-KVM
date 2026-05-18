use async_trait::async_trait;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::traits::AtxKeyBackend;
use super::types::AtxKeyConfig;
use crate::error::{AppError, Result};

const USB_RELAY_MAX_CHANNEL: u8 = 8;
const USB_RELAY_REPORT_LEN: usize = 9;
const HIDIOCSFEATURE_9: libc::c_ulong = 0xC009_4806;

pub struct HidrawLinuxRelayBackend {
    config: AtxKeyConfig,
    handle: Mutex<Option<File>>,
    initialized: AtomicBool,
}

impl HidrawLinuxRelayBackend {
    pub fn new(config: AtxKeyConfig) -> Self {
        Self {
            config,
            handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.pin == 0 {
            return Err(AppError::Config(
                "USB relay channel must be 1-based (>= 1)".to_string(),
            ));
        }
        if self.config.pin > USB_RELAY_MAX_CHANNEL as u32 {
            return Err(AppError::Config(format!(
                "USB HID relay channel must be <= {}",
                USB_RELAY_MAX_CHANNEL
            )));
        }
        Ok(())
    }

    fn send_command(&self, on: bool) -> Result<()> {
        let channel = u8::try_from(self.config.pin).map_err(|_| {
            AppError::Config(format!(
                "USB relay channel {} exceeds max {}",
                self.config.pin,
                u8::MAX
            ))
        })?;
        if channel == 0 {
            return Err(AppError::Config(
                "USB relay channel must be 1-based (>= 1)".to_string(),
            ));
        }
        if channel > USB_RELAY_MAX_CHANNEL {
            return Err(AppError::Config(format!(
                "USB HID relay channel must be <= {}",
                USB_RELAY_MAX_CHANNEL
            )));
        }

        let cmd = Self::build_command(channel, on);
        let mut guard = self.handle.lock().unwrap();
        let device = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("USB relay not initialized".to_string()))?;

        if let Err(feature_err) = Self::send_feature_report(device, &cmd) {
            debug!(
                "USB relay feature report failed ({}), falling back to hidraw write",
                feature_err
            );
            device.write_all(&cmd).map_err(|write_err| {
                AppError::Internal(format!(
                    "USB relay feature report failed: {}; raw write failed: {}",
                    feature_err, write_err
                ))
            })?;
            device
                .flush()
                .map_err(|e| AppError::Internal(format!("USB relay flush failed: {}", e)))?;
        }

        Ok(())
    }

    pub fn build_command(channel: u8, on: bool) -> [u8; USB_RELAY_REPORT_LEN] {
        let mut cmd = [0x00; USB_RELAY_REPORT_LEN];
        cmd[1] = if on { 0xFF } else { 0xFD };
        cmd[2] = channel;
        cmd
    }

    fn send_feature_report(
        device: &File,
        report: &[u8; USB_RELAY_REPORT_LEN],
    ) -> std::io::Result<()> {
        let rc = unsafe { libc::ioctl(device.as_raw_fd(), HIDIOCSFEATURE_9, report.as_ptr()) };
        if rc < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl AtxKeyBackend for HidrawLinuxRelayBackend {
    async fn init(&mut self) -> Result<()> {
        self.validate_config()?;

        info!(
            "Initializing USB relay ATX backend on {} channel {}",
            self.config.device, self.config.pin
        );

        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.config.device)
            .map_err(|e| AppError::Internal(format!("USB relay device open failed: {}", e)))?;

        *self.handle.lock().unwrap() = Some(device);
        self.send_command(false)?;
        self.initialized.store(true, Ordering::Relaxed);

        debug!(
            "USB relay channel {} configured successfully",
            self.config.pin
        );
        Ok(())
    }

    async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_initialized() {
            return Err(AppError::Internal("USB relay not initialized".to_string()));
        }

        self.send_command(true)?;
        sleep(duration).await;
        self.send_command(false)?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.is_initialized() {
            let _ = self.send_command(false);
        }
        *self.handle.lock().unwrap() = None;
        self.initialized.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }
}

impl Drop for HidrawLinuxRelayBackend {
    fn drop(&mut self) {
        if self.is_initialized() {
            let _ = self.send_command(false);
        }
        *self.handle.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::HidrawLinuxRelayBackend;

    #[test]
    fn usb_relay_command_format() {
        assert_eq!(
            HidrawLinuxRelayBackend::build_command(1, true),
            [0x00, 0xFF, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            HidrawLinuxRelayBackend::build_command(1, false),
            [0x00, 0xFD, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }
}
