use async_trait::async_trait;
use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::types::{AtxKeyConfig, LCUS_RELAY_MAX_CHANNEL};
use crate::error::Result;

pub type SharedSerialHandle = Arc<Mutex<Box<dyn SerialPort>>>;

#[async_trait]
pub trait AtxKeyBackend: Send + Sync {
    async fn init(&mut self) -> Result<()>;

    async fn pulse(&self, duration: Duration) -> Result<()>;

    async fn shutdown(&mut self) -> Result<()>;

    fn is_initialized(&self) -> bool;
}

#[derive(Debug, Clone)]
pub enum AtxKeyBackendContext {
    Standalone,
    SharedSerial(SharedSerialHandle),
}

pub fn validate_serial_config(config: &AtxKeyConfig) -> Result<()> {
    if config.device.trim().is_empty() {
        return Err(crate::error::AppError::Config(
            "Serial ATX device cannot be empty".to_string(),
        ));
    }
    if config.pin == 0 {
        return Err(crate::error::AppError::Config(
            "Serial ATX channel must be 1-based (>= 1)".to_string(),
        ));
    }
    if config.pin > LCUS_RELAY_MAX_CHANNEL as u32 {
        return Err(crate::error::AppError::Config(format!(
            "LCUS serial relay channel must be <= {}",
            LCUS_RELAY_MAX_CHANNEL
        )));
    }
    if config.baud_rate == 0 {
        return Err(crate::error::AppError::Config(
            "Serial ATX baud_rate must be greater than 0".to_string(),
        ));
    }
    Ok(())
}
