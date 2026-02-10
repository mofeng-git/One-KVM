//! System event types
//!
//! Defines all event types that can be broadcast through the event bus.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::atx::PowerStatus;
use crate::msd::MsdMode;

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
    /// Whether absolute mouse positioning is supported
    pub supports_absolute_mouse: bool,
    /// Device path (e.g., serial port for CH9329)
    pub device: Option<String>,
    /// Error message if any, None if OK
    pub error: Option<String>,
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
    // HID Events
    // ============================================================================
    /// HID backend state changed
    #[serde(rename = "hid.state_changed")]
    HidStateChanged {
        /// Backend type: "otg", "ch9329", "none"
        backend: String,
        /// Whether backend is initialized and ready
        initialized: bool,
        /// Error message if any, None if OK
        error: Option<String>,
        /// Error code for programmatic handling: "epipe", "eagain", "port_not_found", etc.
        error_code: Option<String>,
    },

    /// HID backend is being switched
    #[serde(rename = "hid.backend_switching")]
    HidBackendSwitching {
        /// Current backend
        from: String,
        /// New backend
        to: String,
    },

    /// HID device lost (device file missing or I/O error)
    #[serde(rename = "hid.device_lost")]
    HidDeviceLost {
        /// Backend type: "otg", "ch9329"
        backend: String,
        /// Device path that was lost (e.g., /dev/hidg0 or /dev/ttyUSB0)
        device: Option<String>,
        /// Human-readable reason for loss
        reason: String,
        /// Error code: "epipe", "eshutdown", "eagain", "enxio", "port_not_found", "io_error"
        error_code: String,
    },

    /// HID device is reconnecting
    #[serde(rename = "hid.reconnecting")]
    HidReconnecting {
        /// Backend type: "otg", "ch9329"
        backend: String,
        /// Current retry attempt number
        attempt: u32,
    },

    /// HID device has recovered after error
    #[serde(rename = "hid.recovered")]
    HidRecovered {
        /// Backend type: "otg", "ch9329"
        backend: String,
    },

    // ============================================================================
    // MSD (Mass Storage Device) Events
    // ============================================================================
    /// MSD state changed
    #[serde(rename = "msd.state_changed")]
    MsdStateChanged {
        /// Operating mode
        mode: MsdMode,
        /// Whether storage is connected to target
        connected: bool,
    },

    /// Image has been mounted
    #[serde(rename = "msd.image_mounted")]
    MsdImageMounted {
        /// Image ID
        image_id: String,
        /// Image filename
        image_name: String,
        /// Image size in bytes
        size: u64,
        /// Mount as CD-ROM (read-only)
        cdrom: bool,
    },

    /// Image has been unmounted
    #[serde(rename = "msd.image_unmounted")]
    MsdImageUnmounted,

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

    /// USB gadget connection status changed (host connected/disconnected)
    #[serde(rename = "msd.usb_status_changed")]
    MsdUsbStatusChanged {
        /// Whether host is connected to USB device
        connected: bool,
        /// USB device state from kernel (e.g., "configured", "not attached")
        device_state: String,
    },

    /// MSD operation error (configfs, image mount, etc.)
    #[serde(rename = "msd.error")]
    MsdError {
        /// Human-readable reason for error
        reason: String,
        /// Error code: "configfs_error", "image_not_found", "mount_failed", "io_error"
        error_code: String,
    },

    /// MSD has recovered after error
    #[serde(rename = "msd.recovered")]
    MsdRecovered,

    // ============================================================================
    // ATX (Power Control) Events
    // ============================================================================
    /// ATX power state changed
    #[serde(rename = "atx.state_changed")]
    AtxStateChanged {
        /// Power status
        power_status: PowerStatus,
    },

    /// ATX action was executed
    #[serde(rename = "atx.action_executed")]
    AtxActionExecuted {
        /// Action: "short", "long", "reset"
        action: String,
        /// When the action was executed
        timestamp: DateTime<Utc>,
    },

    // ============================================================================
    // Audio Events
    // ============================================================================
    /// Audio state changed (streaming started/stopped)
    #[serde(rename = "audio.state_changed")]
    AudioStateChanged {
        /// Whether audio is currently streaming
        streaming: bool,
        /// Current device (None if stopped)
        device: Option<String>,
    },

    /// Audio device was selected
    #[serde(rename = "audio.device_selected")]
    AudioDeviceSelected {
        /// Selected device name
        device: String,
    },

    /// Audio quality was changed
    #[serde(rename = "audio.quality_changed")]
    AudioQualityChanged {
        /// New quality setting: "voice", "balanced", "high"
        quality: String,
    },

    /// Audio device lost (capture error or device disconnected)
    #[serde(rename = "audio.device_lost")]
    AudioDeviceLost {
        /// Audio device name (e.g., "hw:0,0")
        device: Option<String>,
        /// Human-readable reason for loss
        reason: String,
        /// Error code: "device_busy", "device_disconnected", "capture_error", "io_error"
        error_code: String,
    },

    /// Audio device is reconnecting
    #[serde(rename = "audio.reconnecting")]
    AudioReconnecting {
        /// Current retry attempt number
        attempt: u32,
    },

    /// Audio device has recovered after error
    #[serde(rename = "audio.recovered")]
    AudioRecovered {
        /// Audio device name
        device: Option<String>,
    },

    // ============================================================================
    // System Events
    // ============================================================================
    /// A device was added (hot-plug)
    #[serde(rename = "system.device_added")]
    SystemDeviceAdded {
        /// Device type: "video", "audio", "hid", etc.
        device_type: String,
        /// Device path
        device_path: String,
        /// Device name/description
        device_name: String,
    },

    /// A device was removed (hot-unplug)
    #[serde(rename = "system.device_removed")]
    SystemDeviceRemoved {
        /// Device type
        device_type: String,
        /// Device path that was removed
        device_path: String,
    },

    /// System error or warning
    #[serde(rename = "system.error")]
    SystemError {
        /// Module that generated the error: "stream", "hid", "msd", "atx"
        module: String,
        /// Severity: "warning", "error", "critical"
        severity: String,
        /// Error message
        message: String,
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
            Self::HidStateChanged { .. } => "hid.state_changed",
            Self::HidBackendSwitching { .. } => "hid.backend_switching",
            Self::HidDeviceLost { .. } => "hid.device_lost",
            Self::HidReconnecting { .. } => "hid.reconnecting",
            Self::HidRecovered { .. } => "hid.recovered",
            Self::MsdStateChanged { .. } => "msd.state_changed",
            Self::MsdImageMounted { .. } => "msd.image_mounted",
            Self::MsdImageUnmounted => "msd.image_unmounted",
            Self::MsdUploadProgress { .. } => "msd.upload_progress",
            Self::MsdDownloadProgress { .. } => "msd.download_progress",
            Self::MsdUsbStatusChanged { .. } => "msd.usb_status_changed",
            Self::MsdError { .. } => "msd.error",
            Self::MsdRecovered => "msd.recovered",
            Self::AtxStateChanged { .. } => "atx.state_changed",
            Self::AtxActionExecuted { .. } => "atx.action_executed",
            Self::AudioStateChanged { .. } => "audio.state_changed",
            Self::AudioDeviceSelected { .. } => "audio.device_selected",
            Self::AudioQualityChanged { .. } => "audio.quality_changed",
            Self::AudioDeviceLost { .. } => "audio.device_lost",
            Self::AudioReconnecting { .. } => "audio.reconnecting",
            Self::AudioRecovered { .. } => "audio.recovered",
            Self::SystemDeviceAdded { .. } => "system.device_added",
            Self::SystemDeviceRemoved { .. } => "system.device_removed",
            Self::SystemError { .. } => "system.error",
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

        let event = SystemEvent::MsdImageMounted {
            image_id: "123".to_string(),
            image_name: "ubuntu.iso".to_string(),
            size: 1024,
            cdrom: true,
        };
        assert_eq!(event.event_name(), "msd.image_mounted");
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
