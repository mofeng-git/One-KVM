use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[derive(Default)]
pub enum BitratePreset {
    Speed,
    #[default]
    Balanced,
    Quality,
    Custom(u32),
}

impl BitratePreset {
    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            Self::Speed => 1000,
            Self::Balanced => 4000,
            Self::Quality => 8000,
            Self::Custom(kbps) => *kbps,
        }
    }

    pub fn gop_size(&self, fps: u32) -> u32 {
        match self {
            Self::Speed => (fps / 2).max(15),
            Self::Balanced => fps,
            Self::Quality => fps * 2,
            Self::Custom(_) => fps,
        }
    }

    pub fn quality_level(&self) -> &'static str {
        match self {
            Self::Speed => "low",
            Self::Balanced => "medium",
            Self::Quality => "high",
            Self::Custom(_) => "medium",
        }
    }

    pub fn from_kbps(kbps: u32) -> Self {
        match kbps {
            0..=1500 => Self::Speed,
            1501..=6000 => Self::Balanced,
            6001..=10000 => Self::Quality,
            _ => Self::Custom(kbps),
        }
    }
}

impl std::fmt::Display for BitratePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Speed => write!(f, "Speed (1 Mbps)"),
            Self::Balanced => write!(f, "Balanced (4 Mbps)"),
            Self::Quality => write!(f, "Quality (8 Mbps)"),
            Self::Custom(kbps) => write!(f, "Custom ({} kbps)", kbps),
        }
    }
}

