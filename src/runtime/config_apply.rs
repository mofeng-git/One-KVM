use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigApplyOptions {
    pub force: bool,
    pub preserve_service_state: bool,
    pub runtime_only: bool,
}

impl ConfigApplyOptions {
    pub const fn forced() -> Self {
        Self {
            force: true,
            preserve_service_state: false,
            runtime_only: false,
        }
    }

    pub const fn preserving_service_state() -> Self {
        Self {
            force: false,
            preserve_service_state: true,
            runtime_only: false,
        }
    }

    pub const fn runtime_only() -> Self {
        Self {
            force: false,
            preserve_service_state: false,
            runtime_only: true,
        }
    }
}

pub fn try_apply_lock(lock: &Arc<Mutex<()>>, domain: &str) -> Result<OwnedMutexGuard<()>> {
    lock.clone().try_lock_owned().map_err(|_| {
        AppError::ServiceUnavailable(format!("{domain} configuration is already applying"))
    })
}
