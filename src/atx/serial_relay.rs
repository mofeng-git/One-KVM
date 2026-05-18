use async_trait::async_trait;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::traits::{validate_serial_config, AtxKeyBackend, SharedSerialHandle};
use super::types::AtxKeyConfig;
use crate::error::{AppError, Result};

pub struct SerialRelayBackend {
    config: AtxKeyConfig,
    serial_handle: Mutex<Option<SharedSerialHandle>>,
    initialized: AtomicBool,
}

impl SerialRelayBackend {
    pub fn new(config: AtxKeyConfig) -> Self {
        Self {
            config,
            serial_handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    pub fn new_with_shared_serial(config: AtxKeyConfig, serial_handle: SharedSerialHandle) -> Self {
        Self {
            config,
            serial_handle: Mutex::new(Some(serial_handle)),
            initialized: AtomicBool::new(false),
        }
    }

    pub fn open_shared_serial(device: &str, baud_rate: u32) -> Result<SharedSerialHandle> {
        let port = serialport::new(device, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| AppError::Internal(format!("Serial port open failed: {}", e)))?;
        Ok(Arc::new(Mutex::new(port)))
    }

    fn send_command(&self, on: bool) -> Result<()> {
        let channel = u8::try_from(self.config.pin).map_err(|_| {
            AppError::Config(format!(
                "Serial relay channel {} exceeds max {}",
                self.config.pin,
                u8::MAX
            ))
        })?;

        let state = if on { 1 } else { 0 };
        let checksum = 0xA0u8.wrapping_add(channel).wrapping_add(state);
        let cmd = [0xA0, channel, state, checksum];

        let serial_handle = self
            .serial_handle
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or_else(|| AppError::Internal("Serial relay not initialized".to_string()))?;
        let mut port = serial_handle.lock().unwrap();

        port.write_all(&cmd)
            .map_err(|e| AppError::Internal(format!("Serial relay write failed: {}", e)))?;
        port.flush()
            .map_err(|e| AppError::Internal(format!("Serial relay flush failed: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl AtxKeyBackend for SerialRelayBackend {
    async fn init(&mut self) -> Result<()> {
        validate_serial_config(&self.config)?;

        info!(
            "Initializing Serial relay ATX backend on {} channel {}",
            self.config.device, self.config.pin
        );

        let existing_handle = self.serial_handle.lock().unwrap().as_ref().cloned();
        if existing_handle.is_none() {
            let shared = Self::open_shared_serial(&self.config.device, self.config.baud_rate)?;
            *self.serial_handle.lock().unwrap() = Some(shared);
        }

        self.send_command(false)?;
        self.initialized.store(true, Ordering::Relaxed);

        debug!(
            "Serial relay channel {} configured successfully",
            self.config.pin
        );
        Ok(())
    }

    async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_initialized() {
            return Err(AppError::Internal(
                "Serial relay not initialized".to_string(),
            ));
        }

        info!(
            "Pulse serial relay on {} pin {}",
            self.config.device, self.config.pin
        );
        self.send_command(true)?;
        sleep(duration).await;
        self.send_command(false)?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if !self.is_initialized() {
            return Ok(());
        }

        let _ = self.send_command(false);
        *self.serial_handle.lock().unwrap() = None;
        self.initialized.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }
}

impl Drop for SerialRelayBackend {
    fn drop(&mut self) {
        if self.is_initialized() {
            let _ = self.send_command(false);
        }
        *self.serial_handle.lock().unwrap() = None;
    }
}
