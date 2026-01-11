use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::atx::AtxController;
use crate::audio::AudioController;
use crate::auth::{SessionStore, UserStore};
use crate::config::ConfigStore;
use crate::events::{
    AtxDeviceInfo, AudioDeviceInfo, EventBus, HidDeviceInfo, MsdDeviceInfo, SystemEvent,
    VideoDeviceInfo,
};
use crate::extensions::ExtensionManager;
use crate::hid::HidController;
use crate::msd::MsdController;
use crate::otg::OtgService;
use crate::rustdesk::RustDeskService;
use crate::video::VideoStreamManager;

/// Application-wide state shared across handlers
///
/// # Video Streaming
///
/// All video operations should go through `stream_manager`:
/// - `stream_manager.webrtc_streamer()` - WebRTC streaming (H264, extensible to VP8/VP9/H265)
/// - `stream_manager.mjpeg_handler()` - MJPEG stream handler
/// - `stream_manager.streamer()` - Low-level video capture
/// - `stream_manager.start()` / `stream_manager.stop()` - Stream control
/// - `stream_manager.stats()` - Stream statistics
/// - `stream_manager.list_devices()` - List video devices
pub struct AppState {
    /// Configuration store
    pub config: ConfigStore,
    /// Session store
    pub sessions: SessionStore,
    /// User store
    pub users: UserStore,
    /// OTG Service - centralized USB gadget lifecycle management
    /// This is the single owner of OtgGadgetManager, coordinating HID and MSD functions
    pub otg_service: Arc<OtgService>,
    /// Video stream manager (unified MJPEG/WebRTC management)
    /// This is the single entry point for all video operations.
    pub stream_manager: Arc<VideoStreamManager>,
    /// HID controller
    pub hid: Arc<HidController>,
    /// MSD controller (optional, may not be initialized)
    pub msd: Arc<RwLock<Option<MsdController>>>,
    /// ATX controller (optional, may not be initialized)
    pub atx: Arc<RwLock<Option<AtxController>>>,
    /// Audio controller
    pub audio: Arc<AudioController>,
    /// RustDesk remote access service (optional)
    pub rustdesk: Arc<RwLock<Option<Arc<RustDeskService>>>>,
    /// Extension manager (ttyd, gostc, easytier)
    pub extensions: Arc<ExtensionManager>,
    /// Event bus for real-time notifications
    pub events: Arc<EventBus>,
    /// Shutdown signal sender
    pub shutdown_tx: broadcast::Sender<()>,
    /// Data directory path
    data_dir: std::path::PathBuf,
}

impl AppState {
    /// Create new application state
    pub fn new(
        config: ConfigStore,
        sessions: SessionStore,
        users: UserStore,
        otg_service: Arc<OtgService>,
        stream_manager: Arc<VideoStreamManager>,
        hid: Arc<HidController>,
        msd: Option<MsdController>,
        atx: Option<AtxController>,
        audio: Arc<AudioController>,
        rustdesk: Option<Arc<RustDeskService>>,
        extensions: Arc<ExtensionManager>,
        events: Arc<EventBus>,
        shutdown_tx: broadcast::Sender<()>,
        data_dir: std::path::PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            sessions,
            users,
            otg_service,
            stream_manager,
            hid,
            msd: Arc::new(RwLock::new(msd)),
            atx: Arc::new(RwLock::new(atx)),
            audio,
            rustdesk: Arc::new(RwLock::new(rustdesk)),
            extensions,
            events,
            shutdown_tx,
            data_dir,
        })
    }

    /// Get data directory path
    pub fn data_dir(&self) -> &std::path::PathBuf {
        &self.data_dir
    }

    /// Subscribe to shutdown signal
    pub fn shutdown_signal(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Get complete device information for WebSocket clients
    ///
    /// This method collects the current state of all devices (video, HID, MSD, ATX, Audio)
    /// and returns a DeviceInfo event that can be sent to clients.
    /// Uses tokio::join! to collect all device info in parallel for better performance.
    pub async fn get_device_info(&self) -> SystemEvent {
        // Collect all device info in parallel
        let (video, hid, msd, atx, audio) = tokio::join!(
            self.collect_video_info(),
            self.collect_hid_info(),
            self.collect_msd_info(),
            self.collect_atx_info(),
            self.collect_audio_info(),
        );

        SystemEvent::DeviceInfo {
            video,
            hid,
            msd,
            atx,
            audio,
        }
    }

    /// Publish DeviceInfo event to all connected WebSocket clients
    pub async fn publish_device_info(&self) {
        let device_info = self.get_device_info().await;
        self.events.publish(device_info);
    }

    /// Collect video device information
    async fn collect_video_info(&self) -> VideoDeviceInfo {
        // Use stream_manager to get video info (includes stream_mode)
        self.stream_manager.get_video_info().await
    }

    /// Collect HID device information
    async fn collect_hid_info(&self) -> HidDeviceInfo {
        let info = self.hid.info().await;
        let backend_type = self.hid.backend_type().await;

        match info {
            Some(hid_info) => HidDeviceInfo {
                available: true,
                backend: hid_info.name.to_string(),
                initialized: hid_info.initialized,
                supports_absolute_mouse: hid_info.supports_absolute_mouse,
                device: match backend_type {
                    crate::hid::HidBackendType::Ch9329 { ref port, .. } => Some(port.clone()),
                    _ => None,
                },
                error: None,
            },
            None => HidDeviceInfo {
                available: false,
                backend: backend_type.name_str().to_string(),
                initialized: false,
                supports_absolute_mouse: false,
                device: match backend_type {
                    crate::hid::HidBackendType::Ch9329 { ref port, .. } => Some(port.clone()),
                    _ => None,
                },
                error: Some("HID backend not available".to_string()),
            },
        }
    }

    /// Collect MSD device information (optional)
    async fn collect_msd_info(&self) -> Option<MsdDeviceInfo> {
        let msd_guard = self.msd.read().await;
        let msd = msd_guard.as_ref()?;

        let state = msd.state().await;
        Some(MsdDeviceInfo {
            available: state.available,
            mode: match state.mode {
                crate::msd::MsdMode::None => "none",
                crate::msd::MsdMode::Image => "image",
                crate::msd::MsdMode::Drive => "drive",
            }
            .to_string(),
            connected: state.connected,
            image_id: state.current_image.map(|img| img.id),
            error: None,
        })
    }

    /// Collect ATX device information (optional)
    async fn collect_atx_info(&self) -> Option<AtxDeviceInfo> {
        // Predefined backend strings to avoid repeated allocations
        const BACKEND_POWER_ONLY: &str = "power: configured, reset: none";
        const BACKEND_RESET_ONLY: &str = "power: none, reset: configured";
        const BACKEND_BOTH: &str = "power: configured, reset: configured";
        const BACKEND_NONE: &str = "none";

        let atx_guard = self.atx.read().await;
        let atx = atx_guard.as_ref()?;

        let state = atx.state().await;
        Some(AtxDeviceInfo {
            available: state.available,
            backend: match (state.power_configured, state.reset_configured) {
                (true, true) => BACKEND_BOTH,
                (true, false) => BACKEND_POWER_ONLY,
                (false, true) => BACKEND_RESET_ONLY,
                (false, false) => BACKEND_NONE,
            }
            .to_string(),
            initialized: state.power_configured || state.reset_configured,
            power_on: state.power_status == crate::atx::PowerStatus::On,
            error: None,
        })
    }

    /// Collect Audio device information (optional)
    async fn collect_audio_info(&self) -> Option<AudioDeviceInfo> {
        let status = self.audio.status().await;

        Some(AudioDeviceInfo {
            available: status.enabled,
            streaming: status.streaming,
            device: status.device,
            quality: status.quality.to_string(),
            error: status.error,
        })
    }
}
