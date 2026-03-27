//! System event types
//!
//! Defines all event types that can be broadcast through the event bus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Device Info Structures (for system.device_info event)
// ============================================================================

/// Video device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    /// Whether video device is available
    pub available: bool,
    /// Device path (e.g., /dev/video0)
    pub device: Option<String>,
    /// Pixel format (e.g., "MJPEG", "YUYV")
    pub format: Option<String>,
    /// Resolution (width, height)
    pub resolution: Option<(u32, u32)>,
    /// Frames per second
    pub fps: u32,
    /// Whether stream is currently active
    pub online: bool,
    /// Current streaming mode: "mjpeg", "h264", "h265", "vp8", or "vp9"
    pub stream_mode: String,
    /// Whether video config is currently being changed (frontend should skip mode sync)
    pub config_changing: bool,
    /// Error message if any, None if OK
    pub error: Option<String>,
}

/// HID device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HidDeviceInfo {
    /// Whether HID backend is available
    pub available: bool,
    /// Backend type: "otg", "ch9329", "none"
    pub backend: String,
    /// Whether backend is initialized and ready
    pub initialized: bool,
    /// Whether backend is currently online
    pub online: bool,
    /// Whether absolute mouse positioning is supported
    pub supports_absolute_mouse: bool,
    /// Device path (e.g., serial port for CH9329)
    pub device: Option<String>,
    /// Error message if any, None if OK
    pub error: Option<String>,
    /// Error code if any, None if OK
    pub error_code: Option<String>,
}

/// MSD device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsdDeviceInfo {
    /// Whether MSD is available
    pub available: bool,
    /// Operating mode: "none", "image", "drive"
    pub mode: String,
    /// Whether storage is connected to target
    pub connected: bool,
    /// Currently mounted image ID
    pub image_id: Option<String>,
    /// Error message if any, None if OK
    pub error: Option<String>,
}

/// ATX device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtxDeviceInfo {
    /// Whether ATX controller is available
    pub available: bool,
    /// Backend type: "gpio", "usb_relay", "none"
    pub backend: String,
    /// Whether backend is initialized
    pub initialized: bool,
    /// Whether power is currently on
    pub power_on: bool,
    /// Error message if any, None if OK
    pub error: Option<String>,
}

/// Audio device information
///
/// Note: Sample rate is fixed at 48000Hz and channels at 2 (stereo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// Whether audio is enabled/available
    pub available: bool,
    /// Whether audio is currently streaming
    pub streaming: bool,
    /// Current audio device name
    pub device: Option<String>,
    /// Quality preset: "voice", "balanced", "high"
    pub quality: String,
    /// Error message if any, None if OK
    pub error: Option<String>,
}

/// ttyd status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtydDeviceInfo {
    /// Whether ttyd binary is available
    pub available: bool,
    /// Whether ttyd is currently running
    pub running: bool,
}

/// Per-client statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStats {
    /// Client ID
    pub id: String,
    /// Current FPS for this client (frames sent in last second)
    pub fps: u32,
    /// Connected duration (seconds)
    pub connected_secs: u64,
}

/// System event enumeration
///
/// All events are tagged with their event name for serialization.
/// The `serde(tag = "event", content = "data")` attribute creates a
/// JSON structure like:
/// ```json
/// {
///   "event": "stream.state_changed",
///   "data": { "state": "streaming", "device": "/dev/video0" }
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
#[allow(clippy::large_enum_variant)]
pub enum SystemEvent {
    // ============================================================================
    // Video Stream Events
    // ============================================================================
    /// Stream mode switching started (transactional, correlates all following events)
    ///
    /// Sent immediately after a mode switch request is accepted.
    /// Clients can use `transition_id` to correlate subsequent `stream.*` events.
    #[serde(rename = "stream.mode_switching")]
    StreamModeSwitching {
        /// Unique transition ID for this mode switch transaction
        transition_id: String,
        /// Target mode: "mjpeg", "h264", "h265", "vp8", "vp9"
        to_mode: String,
        /// Previous mode: "mjpeg", "h264", "h265", "vp8", "vp9"
        from_mode: String,
    },

    /// Stream state changed (e.g., started, stopped, error)
    #[serde(rename = "stream.state_changed")]
    StreamStateChanged {
        /// Current state: "uninitialized", "ready", "streaming", "no_signal", "error"
        state: String,
        /// Device path if available
        device: Option<String>,
    },

    /// Stream configuration is being changed
    ///
    /// Sent before applying new configuration to notify clients that
    /// the stream will be interrupted temporarily.
    #[serde(rename = "stream.config_changing")]
    StreamConfigChanging {
        /// Optional transition ID if this config change is part of a mode switch transaction
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        /// Reason for change: "device_switch", "resolution_change", "format_change"
        reason: String,
    },

    /// Stream configuration has been applied successfully
    ///
    /// Sent after new configuration is active. Clients can reconnect now.
    #[serde(rename = "stream.config_applied")]
    StreamConfigApplied {
        /// Optional transition ID if this config change is part of a mode switch transaction
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        /// Device path
        device: String,
        /// Resolution (width, height)
        resolution: (u32, u32),
        /// Pixel format: "mjpeg", "yuyv", etc.
        format: String,
        /// Frames per second
        fps: u32,
    },

    /// Stream device was lost (disconnected or error)
    #[serde(rename = "stream.device_lost")]
    StreamDeviceLost {
        /// Device path that was lost
        device: String,
        /// Reason for loss
        reason: String,
    },

    /// Stream device is reconnecting
    #[serde(rename = "stream.reconnecting")]
    StreamReconnecting {
        /// Device path being reconnected
        device: String,
        /// Retry attempt number
        attempt: u32,
    },

    /// Stream device has recovered
    #[serde(rename = "stream.recovered")]
    StreamRecovered {
        /// Device path that was recovered
        device: String,
    },

    /// WebRTC is ready to accept connections
    ///
    /// Sent after video frame source is connected to WebRTC pipeline.
    /// Clients should wait for this event before attempting to create WebRTC sessions.
    #[serde(rename = "stream.webrtc_ready")]
    WebRTCReady {
        /// Optional transition ID if this readiness is part of a mode switch transaction
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        /// Current video codec
        codec: String,
        /// Whether hardware encoding is being used
        hardware: bool,
    },

    /// WebRTC ICE candidate (server -> client trickle)
    #[serde(rename = "webrtc.ice_candidate")]
    WebRTCIceCandidate {
        /// WebRTC session ID
        session_id: String,
        /// ICE candidate data
        candidate: crate::webrtc::signaling::IceCandidate,
    },

    /// WebRTC ICE gathering complete (server -> client)
    #[serde(rename = "webrtc.ice_complete")]
    WebRTCIceComplete {
        /// WebRTC session ID
        session_id: String,
    },

    /// Stream statistics update (sent periodically for client stats)
    #[serde(rename = "stream.stats_update")]
    StreamStatsUpdate {
        /// Number of connected clients
        clients: u64,
        /// Per-client statistics (client_id -> client stats)
        /// Each client's FPS reflects the actual frames sent in the last second
        clients_stat: HashMap<String, ClientStats>,
    },

    /// Stream mode changed (MJPEG <-> WebRTC)
    ///
    /// Sent when the streaming mode is switched. Clients should disconnect
    /// from the current stream and reconnect using the new mode.
    #[serde(rename = "stream.mode_changed")]
    StreamModeChanged {
        /// Optional transition ID if this change is part of a mode switch transaction
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_id: Option<String>,
        /// New mode: "mjpeg", "h264", "h265", "vp8", or "vp9"
        mode: String,
        /// Previous mode: "mjpeg", "h264", "h265", "vp8", or "vp9"
        previous_mode: String,
    },

    /// Stream mode switching completed (transactional end marker)
    ///
    /// Sent when the backend considers the new mode ready for clients to connect.
    #[serde(rename = "stream.mode_ready")]
    StreamModeReady {
        /// Unique transition ID for this mode switch transaction
        transition_id: String,
        /// Active mode after switch: "mjpeg", "h264", "h265", "vp8", "vp9"
        mode: String,
    },

    // ============================================================================
    // MSD (Mass Storage Device) Events
    // ============================================================================
    /// File upload progress (for large file uploads)
    #[serde(rename = "msd.upload_progress")]
    MsdUploadProgress {
        /// Upload operation ID
        upload_id: String,
        /// Filename being uploaded
        filename: String,
        /// Bytes uploaded so far
        bytes_uploaded: u64,
        /// Total file size
        total_bytes: u64,
        /// Progress percentage (0.0 - 100.0)
        progress_pct: f32,
    },

    /// Image download progress (for URL downloads)
    #[serde(rename = "msd.download_progress")]
    MsdDownloadProgress {
        /// Download operation ID
        download_id: String,
        /// Source URL
        url: String,
        /// Target filename
        filename: String,
        /// Bytes downloaded so far
        bytes_downloaded: u64,
        /// Total file size (None if unknown)
        total_bytes: Option<u64>,
        /// Progress percentage (0.0 - 100.0, None if total unknown)
        progress_pct: Option<f32>,
        /// Download status: "started", "in_progress", "completed", "failed"
        status: String,
    },

    /// Complete device information (sent on WebSocket connect and state changes)
    #[serde(rename = "system.device_info")]
    DeviceInfo {
        /// Video device information
        video: VideoDeviceInfo,
        /// HID device information
        hid: HidDeviceInfo,
        /// MSD device information (None if MSD not enabled)
        msd: Option<MsdDeviceInfo>,
        /// ATX device information (None if ATX not enabled)
        atx: Option<AtxDeviceInfo>,
        /// Audio device information (None if audio not enabled)
        audio: Option<AudioDeviceInfo>,
        /// ttyd status information
        ttyd: TtydDeviceInfo,
    },

    /// WebSocket error notification (for connection-level errors like lag)
    #[serde(rename = "error")]
    Error {
        /// Error message
        message: String,
    },
}

impl SystemEvent {
    /// Get the event name (for filtering/routing)
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

    /// Check if event name matches a topic pattern
    ///
    /// Supports wildcards:
    /// - `*` matches all events
    /// - `stream.*` matches all stream events
    /// - `stream.state_changed` matches exact event
    pub fn matches_topic(&self, topic: &str) -> bool {
        if topic == "*" {
            return true;
        }

        let event_name = self.event_name();

        if topic.ends_with(".*") {
            let prefix = topic.trim_end_matches(".*");
            event_name.starts_with(prefix)
        } else {
            event_name == topic
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
        };
        assert_eq!(event.event_name(), "stream.state_changed");
    }

    #[test]
    fn test_matches_topic() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: None,
        };

        assert!(event.matches_topic("*"));
        assert!(event.matches_topic("stream.*"));
        assert!(event.matches_topic("stream.state_changed"));
        assert!(!event.matches_topic("msd.*"));
        assert!(!event.matches_topic("stream.config_changed"));
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
