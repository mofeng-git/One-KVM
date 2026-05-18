use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::encoder::OpusConfig;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Voice,
    #[default]
    Balanced,
    High,
}

impl AudioQuality {
    pub fn bitrate(&self) -> u32 {
        match self {
            AudioQuality::Voice => 32000,
            AudioQuality::Balanced => 64000,
            AudioQuality::High => 128000,
        }
    }

    pub fn to_opus_config(&self) -> OpusConfig {
        match self {
            AudioQuality::Voice => OpusConfig::voice(),
            AudioQuality::Balanced => OpusConfig::default(),
            AudioQuality::High => OpusConfig::music(),
        }
    }
}

impl FromStr for AudioQuality {
    type Err = AppError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "voice" => Ok(Self::Voice),
            "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            _ => Err(AppError::BadRequest(format!(
                "invalid audio quality {:?} (expected voice, balanced, or high)",
                s.trim()
            ))),
        }
    }
}

impl std::fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioQuality::Voice => write!(f, "voice"),
            AudioQuality::Balanced => write!(f, "balanced"),
            AudioQuality::High => write!(f, "high"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioControllerConfig {
    pub enabled: bool,
    pub device: String,
    pub quality: AudioQuality,
}

impl Default for AudioControllerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device: String::new(),
            quality: AudioQuality::Balanced,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStatus {
    pub enabled: bool,
    pub streaming: bool,
    pub device: Option<String>,
    pub quality: AudioQuality,
    pub subscriber_count: usize,
    pub error: Option<String>,
}
