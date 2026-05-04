//! [`SystemEvent`] and device snapshot types (WebSocket / JSON).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LedState {
    pub num_lock: bool,
    pub caps_lock: bool,
    pub scroll_lock: bool,
    pub compose: bool,
    pub kana: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    pub available: bool,
    pub device: Option<String>,
    pub format: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub fps: u32,
    pub online: bool,
    pub stream_mode: String,
    pub config_changing: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HidDeviceInfo {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub online: bool,
    pub supports_absolute_mouse: bool,
    pub keyboard_leds_enabled: bool,
    pub led_state: LedState,
    pub device: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsdDeviceInfo {
    pub available: bool,
    pub mode: String,
    pub connected: bool,
    pub image_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtxDeviceInfo {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub power_on: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub available: bool,
    pub streaming: bool,
    pub device: Option<String>,
    pub quality: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtydDeviceInfo {
    pub available: bool,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStats {
    pub id: String,
    pub fps: u32,
    pub connected_secs: u64,
}

/// Video vs audio source for [`SystemEvent::StreamDeviceLost`] (WebSocket `stream.device_lost`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamDeviceLostKind {
    Video,
    Audio,
}

/// JSON: `{"event": "<name>", "data": { ... }}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
#[allow(clippy::large_enum_variant)]
pub enum SystemEvent {
    #[serde(rename = "stream.mode_switching")]
    StreamModeSwitching {
        transition_id: String,
        to_mode: String,
        from_mode: String,
    },

    #[serde(rename = "stream.state_changed")]
    StreamStateChanged {
        state: String,
        device: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_retry_ms: Option<u64>,
    },

    #[serde(rename = "stream.config_changing")]
    StreamConfigChanging {
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        reason: String,
    },

    #[serde(rename = "stream.config_applied")]
    StreamConfigApplied {
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        device: String,
        resolution: (u32, u32),
        format: String,
        fps: u32,
    },

    #[serde(rename = "stream.device_lost")]
    StreamDeviceLost {
        kind: StreamDeviceLostKind,
        device: String,
        reason: String,
    },

    #[serde(rename = "stream.reconnecting")]
    StreamReconnecting { device: String, attempt: u32 },

    #[serde(rename = "stream.recovered")]
    StreamRecovered { device: String },

    #[serde(rename = "stream.webrtc_ready")]
    WebRTCReady {
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        codec: String,
        hardware: bool,
    },

    #[serde(rename = "webrtc.ice_candidate")]
    WebRTCIceCandidate {
        session_id: String,
        candidate: serde_json::Value,
    },

    #[serde(rename = "webrtc.ice_complete")]
    WebRTCIceComplete { session_id: String },

    #[serde(rename = "stream.stats_update")]
    StreamStatsUpdate {
        clients: u64,
        clients_stat: HashMap<String, ClientStats>,
    },

    #[serde(rename = "stream.mode_changed")]
    StreamModeChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        mode: String,
        previous_mode: String,
    },

    #[serde(rename = "stream.mode_ready")]
    StreamModeReady { transition_id: String, mode: String },

    #[serde(rename = "msd.upload_progress")]
    MsdUploadProgress {
        upload_id: String,
        filename: String,
        bytes_uploaded: u64,
        total_bytes: u64,
        progress_pct: f32,
    },

    #[serde(rename = "msd.download_progress")]
    MsdDownloadProgress {
        download_id: String,
        url: String,
        filename: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
        progress_pct: Option<f32>,
        status: String,
    },

    #[serde(rename = "system.device_info")]
    DeviceInfo {
        video: VideoDeviceInfo,
        hid: HidDeviceInfo,
        msd: Option<MsdDeviceInfo>,
        atx: Option<AtxDeviceInfo>,
        audio: Option<AudioDeviceInfo>,
        ttyd: TtydDeviceInfo,
    },

    #[serde(rename = "error")]
    Error { message: String },
}

/// One entry per [`SystemEvent::event_name`]. `EventBus` builds `*.`-wildcard channels from the first segment; names without `.` (e.g. `error`) have no wildcard channel.
pub(crate) const EXACT_EVENT_TOPICS: &[&str] = &[
    "stream.mode_switching",
    "stream.state_changed",
    "stream.config_changing",
    "stream.config_applied",
    "stream.device_lost",
    "stream.reconnecting",
    "stream.recovered",
    "stream.webrtc_ready",
    "stream.stats_update",
    "stream.mode_changed",
    "stream.mode_ready",
    "webrtc.ice_candidate",
    "webrtc.ice_complete",
    "msd.upload_progress",
    "msd.download_progress",
    "system.device_info",
    "error",
];

impl SystemEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::StreamModeSwitching { .. } => "stream.mode_switching",
            Self::StreamStateChanged { .. } => "stream.state_changed",
            Self::StreamConfigChanging { .. } => "stream.config_changing",
            Self::StreamConfigApplied { .. } => "stream.config_applied",
            Self::StreamDeviceLost { .. } => "stream.device_lost",
            Self::StreamReconnecting { .. } => "stream.reconnecting",
            Self::StreamRecovered { .. } => "stream.recovered",
            Self::WebRTCReady { .. } => "stream.webrtc_ready",
            Self::StreamStatsUpdate { .. } => "stream.stats_update",
            Self::StreamModeChanged { .. } => "stream.mode_changed",
            Self::StreamModeReady { .. } => "stream.mode_ready",
            Self::WebRTCIceCandidate { .. } => "webrtc.ice_candidate",
            Self::WebRTCIceComplete { .. } => "webrtc.ice_complete",
            Self::MsdUploadProgress { .. } => "msd.upload_progress",
            Self::MsdDownloadProgress { .. } => "msd.download_progress",
            Self::DeviceInfo { .. } => "system.device_info",
            Self::Error { .. } => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_name() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: Some("/dev/video0".to_string()),
            reason: None,
            next_retry_ms: None,
        };
        assert_eq!(event.event_name(), "stream.state_changed");
    }

    #[test]
    fn stream_device_lost_json_snake_case_kind() {
        let event = SystemEvent::StreamDeviceLost {
            kind: StreamDeviceLostKind::Audio,
            device: "hw:0,0".to_string(),
            reason: "test".to_string(),
        };
        let v = serde_json::to_value(&event).unwrap();
        let data = v.get("data").unwrap();
        assert_eq!(data.get("kind").and_then(|x| x.as_str()), Some("audio"));
        assert_eq!(data.get("device").and_then(|x| x.as_str()), Some("hw:0,0"));
    }

    #[test]
    fn exact_topics_covers_all_variants() {
        use std::collections::HashSet;

        let samples = vec![
            SystemEvent::StreamModeSwitching {
                transition_id: String::new(),
                to_mode: String::new(),
                from_mode: String::new(),
            },
            SystemEvent::StreamStateChanged {
                state: String::new(),
                device: None,
                reason: None,
                next_retry_ms: None,
            },
            SystemEvent::StreamConfigChanging {
                transition_id: None,
                reason: String::new(),
            },
            SystemEvent::StreamConfigApplied {
                transition_id: None,
                device: String::new(),
                resolution: (0, 0),
                format: String::new(),
                fps: 0,
            },
            SystemEvent::StreamDeviceLost {
                kind: StreamDeviceLostKind::Video,
                device: String::new(),
                reason: String::new(),
            },
            SystemEvent::StreamReconnecting {
                device: String::new(),
                attempt: 0,
            },
            SystemEvent::StreamRecovered {
                device: String::new(),
            },
            SystemEvent::WebRTCReady {
                transition_id: None,
                codec: String::new(),
                hardware: false,
            },
            SystemEvent::StreamStatsUpdate {
                clients: 0,
                clients_stat: HashMap::new(),
            },
            SystemEvent::StreamModeChanged {
                transition_id: None,
                mode: String::new(),
                previous_mode: String::new(),
            },
            SystemEvent::StreamModeReady {
                transition_id: String::new(),
                mode: String::new(),
            },
            SystemEvent::WebRTCIceCandidate {
                session_id: String::new(),
                candidate: serde_json::Value::Null,
            },
            SystemEvent::WebRTCIceComplete {
                session_id: String::new(),
            },
            SystemEvent::MsdUploadProgress {
                upload_id: String::new(),
                filename: String::new(),
                bytes_uploaded: 0,
                total_bytes: 0,
                progress_pct: 0.0,
            },
            SystemEvent::MsdDownloadProgress {
                download_id: String::new(),
                url: String::new(),
                filename: String::new(),
                bytes_downloaded: 0,
                total_bytes: None,
                progress_pct: None,
                status: String::new(),
            },
            SystemEvent::DeviceInfo {
                video: VideoDeviceInfo {
                    available: false,
                    device: None,
                    format: None,
                    resolution: None,
                    fps: 0,
                    online: false,
                    stream_mode: String::new(),
                    config_changing: false,
                    error: None,
                },
                hid: HidDeviceInfo {
                    available: false,
                    backend: String::new(),
                    initialized: false,
                    online: false,
                    supports_absolute_mouse: false,
                    keyboard_leds_enabled: false,
                    led_state: LedState::default(),
                    device: None,
                    error: None,
                    error_code: None,
                },
                msd: None,
                atx: None,
                audio: None,
                ttyd: TtydDeviceInfo {
                    available: false,
                    running: false,
                },
            },
            SystemEvent::Error {
                message: String::new(),
            },
        ];

        let from_enum: HashSet<_> = samples.iter().map(|e| e.event_name()).collect();
        let from_const: HashSet<_> = super::EXACT_EVENT_TOPICS.iter().copied().collect();
        assert_eq!(from_enum, from_const);
    }

    #[test]
    fn test_serialization() {
        let event = SystemEvent::StreamConfigApplied {
            transition_id: None,
            device: "/dev/video0".to_string(),
            resolution: (1920, 1080),
            format: "mjpeg".to_string(),
            fps: 30,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("stream.config_applied"));
        assert!(json.contains("/dev/video0"));

        let deserialized: SystemEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            SystemEvent::StreamConfigApplied { .. }
        ));
    }
}
