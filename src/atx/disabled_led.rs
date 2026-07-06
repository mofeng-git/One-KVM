#![allow(dead_code)]

use super::types::{AtxInputBinding, PowerStatus};
use crate::error::Result;

pub struct LedSensor {
    config: AtxInputBinding,
}

impl LedSensor {
    pub fn new(config: AtxInputBinding) -> Self {
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

    pub async fn read_active(&self) -> Result<bool> {
        Ok(false)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
