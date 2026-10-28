use super::types::{AtxInputBinding, PowerStatus};
use crate::error::Result;

pub struct LedSensor;

impl LedSensor {
    pub fn new(_config: AtxInputBinding) -> Self {
        Self
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
