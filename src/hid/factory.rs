use std::sync::Arc;

use tracing::{info, warn};

use super::{ch9329, HidBackend, HidBackendType};
use crate::error::{AppError, Result};
#[cfg(unix)]
use crate::otg::OtgService;

pub struct HidBackendFactory {
    #[cfg(unix)]
    otg_service: Option<Arc<OtgService>>,
}

impl HidBackendFactory {
    #[cfg(unix)]
    pub fn new(otg_service: Option<Arc<OtgService>>) -> Self {
        Self { otg_service }
    }

    #[cfg(not(unix))]
    pub fn new() -> Self {
        Self {}
    }

    pub async fn create_initialized(
        &self,
        backend_type: &HidBackendType,
    ) -> Result<Option<Arc<dyn HidBackend>>> {
        let backend = match self.create(backend_type).await? {
            Some(backend) => backend,
            None => return Ok(None),
        };

        backend.init().await?;
        Ok(Some(backend))
    }

    async fn create(&self, backend_type: &HidBackendType) -> Result<Option<Arc<dyn HidBackend>>> {
        match backend_type {
            HidBackendType::Otg => self.create_otg_backend().await.map(Some),
            HidBackendType::Ch9329 { port, baud_rate } => {
                info!(
                    "Initializing CH9329 HID backend on {} @ {} baud",
                    port, baud_rate
                );
                Ok(Some(Arc::new(ch9329::Ch9329Backend::with_baud_rate(
                    port, *baud_rate,
                )?)))
            }
            HidBackendType::None => {
                warn!("HID backend disabled");
                Ok(None)
            }
        }
    }

    #[cfg(unix)]
    async fn create_otg_backend(&self) -> Result<Arc<dyn HidBackend>> {
        let otg_service = self
            .otg_service
            .as_ref()
            .ok_or_else(|| AppError::Config("OTG backend not available".to_string()))?;

        let handles = otg_service
            .hid_device_paths()
            .await
            .ok_or_else(|| AppError::Config("OTG HID paths are not available".to_string()))?;

        info!("Creating OTG HID backend from device paths");
        Ok(Arc::new(super::otg::OtgBackend::from_handles(handles)?))
    }

    #[cfg(not(unix))]
    async fn create_otg_backend(&self) -> Result<Arc<dyn HidBackend>> {
        Err(AppError::Config(
            "OTG HID is only available on Linux".to_string(),
        ))
    }
}
