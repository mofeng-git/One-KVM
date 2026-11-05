use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::error::{AppError, Result};

/// Configuration for the USB Audio Class microphone gadget.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UacConfig {
    pub enabled: bool,
    pub sample_rate: u32,
    pub channels: u8,
}

impl Default for UacConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_rate: 48_000,
            channels: 2,
        }
    }
}

impl UacConfig {
    pub fn validate(&self) -> Result<()> {
        // Older configurations stored zero-valued placeholders while UAC was
        // disabled. Accept them until the feature is enabled and normalized.
        if !self.enabled {
            return Ok(());
        }
        if self.sample_rate != 48_000 {
            return Err(AppError::BadRequest(format!(
                "unsupported UAC sample rate {} (expected 48000)",
                self.sample_rate
            )));
        }
        if self.channels != 2 {
            return Err(AppError::BadRequest(format!(
                "unsupported UAC channel count {} (expected 2)",
                self.channels
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_audio_transport() {
        let config = UacConfig::default();
        assert_eq!(config.sample_rate, 48_000);
        assert_eq!(config.channels, 2);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_formats_the_transport_cannot_convert() {
        assert!(UacConfig {
            enabled: true,
            sample_rate: 44_100,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(UacConfig {
            enabled: true,
            channels: 1,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
