//! ATX Key Executor
//!
//! Lightweight executor for a single ATX key operation.
//! Each executor handles one button (power or reset) with its own hardware binding.

use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use serialport::SerialPort;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

use super::types::{ActiveLevel, AtxDriverType, AtxKeyConfig};
use crate::error::{AppError, Result};

/// Timing constants for ATX operations
pub mod timing {
    use std::time::Duration;

    /// Short press duration (power on/graceful shutdown)
    pub const SHORT_PRESS: Duration = Duration::from_millis(500);

    /// Long press duration (force power off)
    pub const LONG_PRESS: Duration = Duration::from_millis(5000);

    /// Reset press duration
    pub const RESET_PRESS: Duration = Duration::from_millis(500);
}

/// Executor for a single ATX key operation
///
/// Each executor manages one hardware button (power or reset).
/// It handles both GPIO and USB relay backends.
pub struct AtxKeyExecutor {
    config: AtxKeyConfig,
    gpio_handle: Mutex<Option<LineHandle>>,
    /// Cached USB relay file handle to avoid repeated open/close syscalls
    usb_relay_handle: Mutex<Option<File>>,
    /// Cached Serial port handle
    serial_handle: Mutex<Option<Box<dyn SerialPort>>>,
    initialized: AtomicBool,
}

impl AtxKeyExecutor {
    /// Create a new executor with the given configuration
    pub fn new(config: AtxKeyConfig) -> Self {
        Self {
            config,
            gpio_handle: Mutex::new(None),
            usb_relay_handle: Mutex::new(None),
            serial_handle: Mutex::new(None),
            initialized: AtomicBool::new(false),
        }
    }

    /// Check if this executor is configured
    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    /// Check if this executor is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }

    /// Initialize the executor
    pub async fn init(&mut self) -> Result<()> {
        if !self.config.is_configured() {
            debug!("ATX key executor not configured, skipping init");
            return Ok(());
        }

        self.validate_runtime_config()?;

        match self.config.driver {
            AtxDriverType::Gpio => self.init_gpio().await?,
            AtxDriverType::UsbRelay => self.init_usb_relay().await?,
            AtxDriverType::Serial => self.init_serial().await?,
            AtxDriverType::None => {}
        }

        self.initialized.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn validate_runtime_config(&self) -> Result<()> {
        match self.config.driver {
            AtxDriverType::Serial => {
                if self.config.pin == 0 {
                    return Err(AppError::Config(
                        "Serial ATX channel must be 1-based (>= 1)".to_string(),
                    ));
                }
                if self.config.pin > u8::MAX as u32 {
                    return Err(AppError::Config(format!(
                        "Serial ATX channel must be <= {}",
                        u8::MAX
                    )));
                }
                if self.config.baud_rate == 0 {
                    return Err(AppError::Config(
                        "Serial ATX baud_rate must be greater than 0".to_string(),
                    ));
                }
            }
            AtxDriverType::UsbRelay => {
                if self.config.pin > u8::MAX as u32 {
                    return Err(AppError::Config(format!(
                        "USB relay channel must be <= {}",
                        u8::MAX
                    )));
                }
            }
            AtxDriverType::Gpio | AtxDriverType::None => {}
        }
        Ok(())
    }

    /// Initialize GPIO backend
    async fn init_gpio(&mut self) -> Result<()> {
        info!(
            "Initializing GPIO ATX executor on {} pin {}",
            self.config.device, self.config.pin
        );

        let mut chip = Chip::new(&self.config.device)
            .map_err(|e| AppError::Internal(format!("GPIO chip open failed: {}", e)))?;

        let line = chip.get_line(self.config.pin).map_err(|e| {
            AppError::Internal(format!("GPIO line {} failed: {}", self.config.pin, e))
        })?;

        // Initial value depends on active level (start in inactive state)
        let initial_value = match self.config.active_level {
            ActiveLevel::High => 0, // Inactive = low
            ActiveLevel::Low => 1,  // Inactive = high
        };

        let handle = line
            .request(LineRequestFlags::OUTPUT, initial_value, "one-kvm-atx")
            .map_err(|e| AppError::Internal(format!("GPIO request failed: {}", e)))?;

        *self.gpio_handle.lock().unwrap() = Some(handle);
        debug!("GPIO pin {} configured successfully", self.config.pin);
        Ok(())
    }

    /// Initialize USB relay backend
    async fn init_usb_relay(&self) -> Result<()> {
        info!(
            "Initializing USB relay ATX executor on {} channel {}",
            self.config.device, self.config.pin
        );

        // Open and cache the device handle
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.config.device)
            .map_err(|e| AppError::Internal(format!("USB relay device open failed: {}", e)))?;

        *self.usb_relay_handle.lock().unwrap() = Some(device);

        // Ensure relay is off initially
        self.send_usb_relay_command(false)?;

        debug!(
            "USB relay channel {} configured successfully",
            self.config.pin
        );
        Ok(())
    }

    /// Initialize Serial relay backend
    async fn init_serial(&self) -> Result<()> {
        info!(
            "Initializing Serial relay ATX executor on {} channel {}",
            self.config.device, self.config.pin
        );

        let baud_rate = self.config.baud_rate;

        let port = serialport::new(&self.config.device, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| AppError::Internal(format!("Serial port open failed: {}", e)))?;

        *self.serial_handle.lock().unwrap() = Some(port);

        // Ensure relay is off initially
        self.send_serial_relay_command(false)?;

        debug!(
            "Serial relay channel {} configured successfully",
            self.config.pin
        );
        Ok(())
    }

    /// Pulse the button for the specified duration
    pub async fn pulse(&self, duration: Duration) -> Result<()> {
        if !self.is_configured() {
            return Err(AppError::Internal("ATX key not configured".to_string()));
        }

        if !self.is_initialized() {
            return Err(AppError::Internal("ATX key not initialized".to_string()));
        }

        match self.config.driver {
            AtxDriverType::Gpio => self.pulse_gpio(duration).await,
            AtxDriverType::UsbRelay => self.pulse_usb_relay(duration).await,
            AtxDriverType::Serial => self.pulse_serial(duration).await,
            AtxDriverType::None => Ok(()),
        }
    }

    /// Pulse GPIO pin
    async fn pulse_gpio(&self, duration: Duration) -> Result<()> {
        let (active, inactive) = match self.config.active_level {
            ActiveLevel::High => (1u8, 0u8),
            ActiveLevel::Low => (0u8, 1u8),
        };

        // Set to active state
        {
            let guard = self.gpio_handle.lock().unwrap();
            let handle = guard
                .as_ref()
                .ok_or_else(|| AppError::Internal("GPIO not initialized".to_string()))?;
            handle
                .set_value(active)
                .map_err(|e| AppError::Internal(format!("GPIO set failed: {}", e)))?;
        }

        // Wait for duration (no lock held)
        sleep(duration).await;

        // Set to inactive state
        {
            let guard = self.gpio_handle.lock().unwrap();
            if let Some(handle) = guard.as_ref() {
                handle.set_value(inactive).ok();
            }
        }

        Ok(())
    }

    /// Pulse USB relay
    async fn pulse_usb_relay(&self, duration: Duration) -> Result<()> {
        // Turn relay on
        self.send_usb_relay_command(true)?;

        // Wait for duration
        sleep(duration).await;

        // Turn relay off
        self.send_usb_relay_command(false)?;

        Ok(())
    }

    /// Send USB relay command using cached handle
    fn send_usb_relay_command(&self, on: bool) -> Result<()> {
        let channel = u8::try_from(self.config.pin).map_err(|_| {
            AppError::Config(format!(
                "USB relay channel {} exceeds max {}",
                self.config.pin,
                u8::MAX
            ))
        })?;

        // Standard HID relay command format
        let cmd = if on {
            [0x00, channel + 1, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00]
        } else {
            [0x00, channel + 1, 0xFD, 0x00, 0x00, 0x00, 0x00, 0x00]
        };

        let mut guard = self.usb_relay_handle.lock().unwrap();
        let device = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("USB relay not initialized".to_string()))?;

        device
            .write_all(&cmd)
            .map_err(|e| AppError::Internal(format!("USB relay write failed: {}", e)))?;

        Ok(())
    }

    /// Pulse Serial relay
    async fn pulse_serial(&self, duration: Duration) -> Result<()> {
        info!(
            "Pulse serial relay on {} pin {}",
            self.config.device, self.config.pin
        );
        // Turn relay on
        self.send_serial_relay_command(true)?;

        // Wait for duration
        sleep(duration).await;

        // Turn relay off
        self.send_serial_relay_command(false)?;

        Ok(())
    }

    /// Send Serial relay command using cached handle
    fn send_serial_relay_command(&self, on: bool) -> Result<()> {
        let channel = u8::try_from(self.config.pin).map_err(|_| {
            AppError::Config(format!(
                "Serial relay channel {} exceeds max {}",
                self.config.pin,
                u8::MAX
            ))
        })?;
        if channel == 0 {
            return Err(AppError::Config(
                "Serial relay channel must be 1-based (>= 1)".to_string(),
            ));
        }

        // LCUS-Type Protocol
        // Frame: [StopByte(A0), Channel, State, Checksum]
        // Checksum = A0 + channel + state
        let state = if on { 1 } else { 0 };
        let checksum = 0xA0u8.wrapping_add(channel).wrapping_add(state);

        // Example for Channel 1:
        // ON:  A0 01 01 A2
        // OFF: A0 01 00 A1
        let cmd = [0xA0, channel, state, checksum];

        let mut guard = self.serial_handle.lock().unwrap();
        let port = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("Serial relay not initialized".to_string()))?;

        port.write_all(&cmd)
            .map_err(|e| AppError::Internal(format!("Serial relay write failed: {}", e)))?;

        Ok(())
    }

    /// Shutdown the executor
    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.is_initialized() {
            return Ok(());
        }

        match self.config.driver {
            AtxDriverType::Gpio => {
                // Release GPIO handle
                *self.gpio_handle.lock().unwrap() = None;
            }
            AtxDriverType::UsbRelay => {
                // Ensure relay is off before closing handle
                let _ = self.send_usb_relay_command(false);
                // Release USB relay handle
                *self.usb_relay_handle.lock().unwrap() = None;
            }
            AtxDriverType::Serial => {
                // Ensure relay is off before closing handle
                let _ = self.send_serial_relay_command(false);
                // Release Serial relay handle
                *self.serial_handle.lock().unwrap() = None;
            }
            AtxDriverType::None => {}
        }

        self.initialized.store(false, Ordering::Relaxed);
        debug!("ATX key executor shutdown complete");
        Ok(())
    }
}

impl Drop for AtxKeyExecutor {
    fn drop(&mut self) {
        // Ensure GPIO lines are released
        *self.gpio_handle.lock().unwrap() = None;

        // Ensure USB relay is off and handle released
        if self.config.driver == AtxDriverType::UsbRelay && self.is_initialized() {
            let _ = self.send_usb_relay_command(false);
        }
        *self.usb_relay_handle.lock().unwrap() = None;

        // Ensure Serial relay is off and handle released
        if self.config.driver == AtxDriverType::Serial && self.is_initialized() {
            let _ = self.send_serial_relay_command(false);
        }
        *self.serial_handle.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let config = AtxKeyConfig::default();
        let executor = AtxKeyExecutor::new(config);
        assert!(!executor.is_configured());
        assert!(!executor.is_initialized());
    }

    #[test]
    fn test_executor_with_gpio_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Gpio,
            device: "/dev/gpiochip0".to_string(),
            pin: 5,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
        assert!(!executor.is_initialized());
    }

    #[test]
    fn test_executor_with_usb_relay_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::UsbRelay,
            device: "/dev/hidraw0".to_string(),
            pin: 0,
            active_level: ActiveLevel::High, // Ignored for USB relay
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
    }

    #[test]
    fn test_executor_with_serial_config() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 1,
            active_level: ActiveLevel::High, // Ignored
            baud_rate: 9600,
        };
        let executor = AtxKeyExecutor::new(config);
        assert!(executor.is_configured());
    }

    #[test]
    fn test_timing_constants() {
        assert_eq!(timing::SHORT_PRESS.as_millis(), 500);
        assert_eq!(timing::LONG_PRESS.as_millis(), 5000);
        assert_eq!(timing::RESET_PRESS.as_millis(), 500);
    }

    #[tokio::test]
    async fn test_executor_init_rejects_serial_channel_zero() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 0,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let mut executor = AtxKeyExecutor::new(config);
        let err = executor.init().await.unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
    }

    #[tokio::test]
    async fn test_executor_init_rejects_serial_channel_overflow() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 256,
            active_level: ActiveLevel::High,
            baud_rate: 9600,
        };
        let mut executor = AtxKeyExecutor::new(config);
        let err = executor.init().await.unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
    }

    #[tokio::test]
    async fn test_executor_init_rejects_zero_serial_baud_rate() {
        let config = AtxKeyConfig {
            driver: AtxDriverType::Serial,
            device: "/dev/ttyUSB0".to_string(),
            pin: 1,
            active_level: ActiveLevel::High,
            baud_rate: 0,
        };
        let mut executor = AtxKeyExecutor::new(config);
        let err = executor.init().await.unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
    }
}
