#![allow(dead_code)]

use super::types::{AtxLedConfig, PowerStatus};
use crate::error::Result;

pub struct LedSensor {
    config: AtxLedConfig,
}

impl LedSensor {
    pub fn new(config: AtxLedConfig) -> Self {
        Self { config }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    pub fn is_initialized(&self) -> bool {
        false
    }

    pub async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    pub async fn read(&self) -> Result<PowerStatus> {
        Ok(PowerStatus::Unknown)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
