use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::Mutex;

use crate::config::ConfigStore;
use crate::hid::HidController;
#[cfg(unix)]
use crate::otg::OtgService;
use crate::runtime::{RemoteAccessCoordinator, UsbCoordinator};
use crate::state::AppState;

#[derive(Clone)]
pub(crate) struct RemoteAccessApiState {
    pub config: ConfigStore,
    pub coordinator: Arc<RemoteAccessCoordinator>,
    pub rustdesk_apply_lock: Arc<Mutex<()>>,
    pub vnc_apply_lock: Arc<Mutex<()>>,
    pub rtsp_apply_lock: Arc<Mutex<()>>,
}

impl FromRef<Arc<AppState>> for RemoteAccessApiState {
    fn from_ref(state: &Arc<AppState>) -> Self {
        Self {
            config: state.config.clone(),
            coordinator: state.remote_access.clone(),
            rustdesk_apply_lock: state.config_apply_locks.rustdesk.clone(),
            vnc_apply_lock: state.config_apply_locks.vnc.clone(),
            rtsp_apply_lock: state.config_apply_locks.rtsp.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct UsbApiState {
    pub config: ConfigStore,
    pub coordinator: Arc<UsbCoordinator>,
    pub hid: Arc<HidController>,
    pub apply_lock: Arc<Mutex<()>>,
    #[cfg(unix)]
    pub otg: Arc<OtgService>,
}

impl FromRef<Arc<AppState>> for UsbApiState {
    fn from_ref(state: &Arc<AppState>) -> Self {
        Self {
            config: state.config.clone(),
            coordinator: state.usb.clone(),
            hid: state.hid.clone(),
            apply_lock: state.config_apply_locks.otg.clone(),
            #[cfg(unix)]
            otg: state.otg_service.clone(),
        }
    }
}
