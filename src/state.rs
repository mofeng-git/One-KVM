use std::{collections::VecDeque, path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, watch, Mutex, RwLock};

use crate::atx::AtxController;
use crate::audio::AudioController;
use crate::auth::{SessionStore, UserStore};
use crate::computer_use::ComputerUseManager;
use crate::config::ConfigStore;
use crate::db::DatabasePool;
use crate::events::{
    AtxDeviceInfo, AudioDeviceInfo, EventBus, HidDeviceInfo, LedState, MsdDeviceInfo,
    MsdDeviceMediaInfo, SystemEvent, TtydDeviceInfo, VideoDeviceInfo,
};
use crate::extensions::{ExtensionId, ExtensionManager};
use crate::hid::HidController;
#[cfg(unix)]
use crate::msd::MsdController;
#[cfg(unix)]
use crate::otg::OtgService;
use crate::rtsp::RtspService;
use crate::rustdesk::RustDeskService;
use crate::update::UpdateService;
use crate::video::VideoStreamManager;
use crate::vnc::VncService;
use crate::watchdog::WatchdogController;
use crate::webrtc::WebRtcStreamer;

#[derive(Clone)]
pub struct ConfigApplyLocks {
    pub video: Arc<Mutex<()>>,
    pub stream: Arc<Mutex<()>>,
    pub otg: Arc<Mutex<()>>,
    pub audio: Arc<Mutex<()>>,
    pub atx: Arc<Mutex<()>>,
    pub rustdesk: Arc<Mutex<()>>,
    pub vnc: Arc<Mutex<()>>,
    pub rtsp: Arc<Mutex<()>>,
    pub watchdog: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownAction {
    Exit,
    Restart { exe_path: Option<PathBuf> },
}

impl ConfigApplyLocks {
    fn new() -> Self {
        Self {
            video: Arc::new(Mutex::new(())),
            stream: Arc::new(Mutex::new(())),
            otg: Arc::new(Mutex::new(())),
            audio: Arc::new(Mutex::new(())),
            atx: Arc::new(Mutex::new(())),
            rustdesk: Arc::new(Mutex::new(())),
            vnc: Arc::new(Mutex::new(())),
            rtsp: Arc::new(Mutex::new(())),
            watchdog: Arc::new(Mutex::new(())),
        }
    }
}

/// Shared Axum/App state: video flows through [`VideoStreamManager`]; WebRTC SDP/ICE/sessions on [`WebRtcStreamer`].
pub struct AppState {
    pub db: DatabasePool,
    pub config: ConfigStore,
    pub sessions: SessionStore,
    pub users: UserStore,
    #[cfg(unix)]
    pub otg_service: Arc<OtgService>,
    pub stream_manager: Arc<VideoStreamManager>,
    pub webrtc: Arc<WebRtcStreamer>,
    pub hid: Arc<HidController>,
    pub computer_use: Arc<ComputerUseManager>,
    #[cfg(unix)]
    pub msd: Arc<RwLock<Option<MsdController>>>,
    pub atx: Arc<RwLock<Option<AtxController>>>,
    pub audio: Arc<AudioController>,
    pub rustdesk: Arc<RwLock<Option<Arc<RustDeskService>>>>,
    pub vnc: Arc<RwLock<Option<Arc<VncService>>>>,
    pub rtsp: Arc<RwLock<Option<Arc<RtspService>>>>,
    pub extensions: Arc<ExtensionManager>,
    pub events: Arc<EventBus>,
    device_info_tx: watch::Sender<Option<SystemEvent>>,
    pub update: Arc<UpdateService>,
    pub watchdog: Arc<WatchdogController>,
    pub shutdown_tx: broadcast::Sender<ShutdownAction>,
    pub revoked_sessions: Arc<RwLock<VecDeque<String>>>,
    pub config_apply_locks: ConfigApplyLocks,
    data_dir: std::path::PathBuf,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: DatabasePool,
        config: ConfigStore,
        sessions: SessionStore,
        users: UserStore,
        #[cfg(unix)] otg_service: Arc<OtgService>,
        stream_manager: Arc<VideoStreamManager>,
        webrtc: Arc<WebRtcStreamer>,
        hid: Arc<HidController>,
        computer_use: Arc<ComputerUseManager>,
        #[cfg(unix)] msd: Option<MsdController>,
        atx: Option<AtxController>,
        audio: Arc<AudioController>,
        rustdesk: Option<Arc<RustDeskService>>,
        vnc: Option<Arc<VncService>>,
        rtsp: Option<Arc<RtspService>>,
        extensions: Arc<ExtensionManager>,
        events: Arc<EventBus>,
        update: Arc<UpdateService>,
        shutdown_tx: broadcast::Sender<ShutdownAction>,
        data_dir: std::path::PathBuf,
    ) -> Arc<Self> {
        let (device_info_tx, _device_info_rx) = watch::channel(None);

        Arc::new(Self {
            db,
            config,
            sessions,
            users,
            #[cfg(unix)]
            otg_service,
            stream_manager,
            webrtc,
            hid,
            computer_use,
            #[cfg(unix)]
            msd: Arc::new(RwLock::new(msd)),
            atx: Arc::new(RwLock::new(atx)),
            audio,
            rustdesk: Arc::new(RwLock::new(rustdesk)),
            vnc: Arc::new(RwLock::new(vnc)),
            rtsp: Arc::new(RwLock::new(rtsp)),
            extensions,
            events,
            device_info_tx,
            update,
            watchdog: Arc::new(WatchdogController::new()),
            shutdown_tx,
            revoked_sessions: Arc::new(RwLock::new(VecDeque::new())),
            config_apply_locks: ConfigApplyLocks::new(),
            data_dir,
        })
    }

    pub fn data_dir(&self) -> &std::path::PathBuf {
        &self.data_dir
    }

    pub fn subscribe_device_info(&self) -> watch::Receiver<Option<SystemEvent>> {
        self.device_info_tx.subscribe()
    }

    pub async fn remember_revoked_sessions(&self, session_ids: Vec<String>) {
        if session_ids.is_empty() {
            return;
        }
        let mut guard = self.revoked_sessions.write().await;
        for id in session_ids {
            guard.push_back(id);
        }
        while guard.len() > 32 {
            guard.pop_front();
        }
    }

    pub async fn is_session_revoked(&self, session_id: &str) -> bool {
        let guard = self.revoked_sessions.read().await;
        guard.iter().any(|id| id == session_id)
    }

    pub async fn get_device_info(&self) -> SystemEvent {
        let (video, hid, msd, atx, audio, ttyd) = tokio::join!(
            self.collect_video_info(),
            self.collect_hid_info(),
            self.collect_msd_info(),
            self.collect_atx_info(),
            self.collect_audio_info(),
            self.collect_ttyd_info(),
        );

        SystemEvent::DeviceInfo {
            video,
            hid,
            msd,
            atx,
            audio,
            ttyd,
        }
    }

    pub async fn publish_device_info(&self) {
        let device_info = self.get_device_info().await;
        let _ = self.device_info_tx.send(Some(device_info));
    }

    async fn collect_video_info(&self) -> VideoDeviceInfo {
        self.stream_manager.get_video_info().await
    }

    async fn collect_hid_info(&self) -> HidDeviceInfo {
        let state = self.hid.snapshot().await;

        HidDeviceInfo {
            available: state.available,
            backend: state.backend,
            initialized: state.initialized,
            online: state.online,
            supports_absolute_mouse: state.supports_absolute_mouse,
            keyboard_leds_enabled: state.keyboard_leds_enabled,
            led_state: LedState {
                num_lock: state.led_state.num_lock,
                caps_lock: state.led_state.caps_lock,
                scroll_lock: state.led_state.scroll_lock,
                compose: state.led_state.compose,
                kana: state.led_state.kana,
            },
            device: state.device,
            error: state.error,
            error_code: state.error_code,
        }
    }

    async fn collect_msd_info(&self) -> Option<MsdDeviceInfo> {
        #[cfg(not(unix))]
        {
            None
        }
        #[cfg(unix)]
        {
            let msd_guard = self.msd.read().await;
            let msd = msd_guard.as_ref()?;

            let state = msd.state().await;
            let error = msd.monitor().error_message().await;
            let mounted_media = state
                .mounted_media
                .iter()
                .map(|media| MsdDeviceMediaInfo {
                    id: media.id.clone(),
                    kind: match media.kind {
                        crate::msd::MountedMediaKind::Drive => "drive",
                        crate::msd::MountedMediaKind::Image => "image",
                    }
                    .to_string(),
                    name: media.name.clone(),
                    cdrom: media.cdrom,
                    read_only: media.read_only,
                    size: media.size,
                })
                .collect::<Vec<_>>();
            Some(MsdDeviceInfo {
                available: state.available,
                disk_mode: match state.disk_mode {
                    crate::msd::DiskMode::Single => "single",
                    crate::msd::DiskMode::Multi => "multi",
                }
                .to_string(),
                slot_capacity: state.disk_mode.capacity(),
                mounted_count: state.mounted_media.len() as u8,
                mounted_media,
                usb_reenumerating: state.usb_reenumerating,
                error,
            })
        }
    }

    async fn collect_atx_info(&self) -> Option<AtxDeviceInfo> {
        let atx_guard = self.atx.read().await;
        let atx = atx_guard.as_ref()?;

        let state = atx.state().await;
        Some(AtxDeviceInfo {
            available: state.available,
            backend: match state.driver {
                crate::atx::AtxDriverType::Gpio => "gpio",
                crate::atx::AtxDriverType::UsbRelay => "usbrelay",
                crate::atx::AtxDriverType::Serial => "serial",
                crate::atx::AtxDriverType::None => "none",
            }
            .to_string(),
            initialized: state.power_configured || state.reset_configured,
            power_on: state.power_status == crate::atx::PowerStatus::On,
            hdd_status: match state.hdd_status {
                crate::atx::HddStatus::Active => "active",
                crate::atx::HddStatus::Inactive => "inactive",
                crate::atx::HddStatus::Unknown => "unknown",
            }
            .to_string(),
            error: None,
        })
    }

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

    async fn collect_ttyd_info(&self) -> TtydDeviceInfo {
        let status = self.extensions.status(ExtensionId::Ttyd).await;

        TtydDeviceInfo {
            available: self.extensions.check_available(ExtensionId::Ttyd),
            running: status.is_running(),
        }
    }
}
