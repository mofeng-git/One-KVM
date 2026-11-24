use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::Mutex;

use crate::config::ConfigStore;
use crate::hid::HidController;
#[cfg(unix)]
use crate::otg::OtgService;
use crate::runtime::UsbCoordinator;
use crate::state::AppState;

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
