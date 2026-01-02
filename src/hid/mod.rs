//! HID (Human Interface Device) control module
//!
//! This module provides keyboard and mouse control for remote KVM:
//! - USB OTG gadget mode (native Linux USB gadget)
//! - CH9329 serial HID controller
//!
//! Architecture:
//! ```text
//! Web Client --> WebSocket/DataChannel --> HID Events --> Backend --> Target PC
//!                                              |
//!                                      [OTG | CH9329]
//! ```

pub mod backend;
pub mod ch9329;
pub mod consumer;
pub mod datachannel;
pub mod keymap;
pub mod monitor;
pub mod otg;
pub mod types;
pub mod websocket;

pub use backend::{HidBackend, HidBackendType};
pub use monitor::{HidHealthMonitor, HidHealthStatus, HidMonitorConfig};
pub use otg::LedState;
pub use types::{
    ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent,
    MouseEventType,
};

/// HID backend information
#[derive(Debug, Clone)]
pub struct HidInfo {
    /// Backend name
    pub name: &'static str,
    /// Whether backend is initialized
    pub initialized: bool,
    /// Whether absolute mouse positioning is supported
    pub supports_absolute_mouse: bool,
    /// Screen resolution for absolute mouse
    pub screen_resolution: Option<(u32, u32)>,
}

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{AppError, Result};
use crate::otg::OtgService;

/// HID controller managing keyboard and mouse input
pub struct HidController {
    /// OTG Service reference (only used when backend is OTG)
    otg_service: Option<Arc<OtgService>>,
    /// Active backend
    backend: Arc<RwLock<Option<Box<dyn HidBackend>>>>,
    /// Backend type (mutable for reload)
    backend_type: RwLock<HidBackendType>,
    /// Event bus for broadcasting state changes (optional)
    events: tokio::sync::RwLock<Option<Arc<crate::events::EventBus>>>,
    /// Health monitor for error tracking and recovery
    monitor: Arc<HidHealthMonitor>,
}

impl HidController {
    /// Create a new HID controller with specified backend
    ///
    /// For OTG backend, otg_service should be provided to support hot-reload
    pub fn new(backend_type: HidBackendType, otg_service: Option<Arc<OtgService>>) -> Self {
        Self {
            otg_service,
            backend: Arc::new(RwLock::new(None)),
            backend_type: RwLock::new(backend_type),
            events: tokio::sync::RwLock::new(None),
            monitor: Arc::new(HidHealthMonitor::with_defaults()),
        }
    }

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<crate::events::EventBus>) {
        *self.events.write().await = Some(events.clone());
        // Also set event bus on the monitor for health notifications
        self.monitor.set_event_bus(events).await;
    }

    /// Initialize the HID backend
    pub async fn init(&self) -> Result<()> {
        let backend_type = self.backend_type.read().await.clone();
        let backend: Box<dyn HidBackend> = match backend_type {
            HidBackendType::Otg => {
                // Request HID functions from OtgService
                let otg_service = self
                    .otg_service
                    .as_ref()
                    .ok_or_else(|| AppError::Internal("OtgService not available".into()))?;

                info!("Requesting HID functions from OtgService");
                let handles = otg_service.enable_hid().await?;

                // Create OtgBackend from handles (no longer manages gadget itself)
                info!("Creating OTG HID backend from device paths");
                Box::new(otg::OtgBackend::from_handles(handles)?)
            }
            HidBackendType::Ch9329 { ref port, baud_rate } => {
                info!("Initializing CH9329 HID backend on {} @ {} baud", port, baud_rate);
                Box::new(ch9329::Ch9329Backend::with_baud_rate(port, baud_rate)?)
            }
            HidBackendType::None => {
                warn!("HID backend disabled");
                return Ok(());
            }
        };

        backend.init().await?;
        *self.backend.write().await = Some(backend);

        info!("HID backend initialized: {:?}", backend_type);
        Ok(())
    }

    /// Shutdown the HID backend and release resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HID controller");

        // Close the backend
        *self.backend.write().await = None;

        // If OTG backend, notify OtgService to disable HID
        let backend_type = self.backend_type.read().await.clone();
        if matches!(backend_type, HidBackendType::Otg) {
            if let Some(ref otg_service) = self.otg_service {
                info!("Disabling HID functions in OtgService");
                otg_service.disable_hid().await?;
            }
        }

        info!("HID controller shutdown complete");
        Ok(())
    }

    /// Send keyboard event
    pub async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        let backend = self.backend.read().await;
        match backend.as_ref() {
            Some(b) => {
                match b.send_keyboard(event).await {
                    Ok(_) => {
                        // Check if we were in an error state and now recovered
                        if self.monitor.is_error().await {
                            let backend_type = self.backend_type.read().await;
                            self.monitor.report_recovered(backend_type.name_str()).await;
                        }
                        Ok(())
                    }
                    Err(e) => {
                        // Report error to monitor, but skip temporary EAGAIN retries
                        // - "eagain_retry": within threshold, just temporary busy
                        // - "eagain": exceeded threshold, report as error
                        if let AppError::HidError { ref backend, ref reason, ref error_code } = e {
                            if error_code != "eagain_retry" {
                                self.monitor.report_error(backend, None, reason, error_code).await;
                            }
                        }
                        Err(e)
                    }
                }
            }
            None => Err(AppError::BadRequest("HID backend not available".to_string())),
        }
    }

    /// Send mouse event
    pub async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
        let backend = self.backend.read().await;
        match backend.as_ref() {
            Some(b) => {
                match b.send_mouse(event).await {
                    Ok(_) => {
                        // Check if we were in an error state and now recovered
                        if self.monitor.is_error().await {
                            let backend_type = self.backend_type.read().await;
                            self.monitor.report_recovered(backend_type.name_str()).await;
                        }
                        Ok(())
                    }
                    Err(e) => {
                        // Report error to monitor, but skip temporary EAGAIN retries
                        // - "eagain_retry": within threshold, just temporary busy
                        // - "eagain": exceeded threshold, report as error
                        if let AppError::HidError { ref backend, ref reason, ref error_code } = e {
                            if error_code != "eagain_retry" {
                                self.monitor.report_error(backend, None, reason, error_code).await;
                            }
                        }
                        Err(e)
                    }
                }
            }
            None => Err(AppError::BadRequest("HID backend not available".to_string())),
        }
    }

    /// Send consumer control event (multimedia keys)
    pub async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        let backend = self.backend.read().await;
        match backend.as_ref() {
            Some(b) => {
                match b.send_consumer(event).await {
                    Ok(_) => {
                        if self.monitor.is_error().await {
                            let backend_type = self.backend_type.read().await;
                            self.monitor.report_recovered(backend_type.name_str()).await;
                        }
                        Ok(())
                    }
                    Err(e) => {
                        if let AppError::HidError { ref backend, ref reason, ref error_code } = e {
                            if error_code != "eagain_retry" {
                                self.monitor.report_error(backend, None, reason, error_code).await;
                            }
                        }
                        Err(e)
                    }
                }
            }
            None => Err(AppError::BadRequest("HID backend not available".to_string())),
        }
    }

    /// Reset all keys (release all pressed keys)
    pub async fn reset(&self) -> Result<()> {
        let backend = self.backend.read().await;
        match backend.as_ref() {
            Some(b) => b.reset().await,
            None => Ok(()),
        }
    }

    /// Check if backend is available
    pub async fn is_available(&self) -> bool {
        self.backend.read().await.is_some()
    }

    /// Get backend type
    pub async fn backend_type(&self) -> HidBackendType {
        self.backend_type.read().await.clone()
    }

    /// Get backend info
    pub async fn info(&self) -> Option<HidInfo> {
        let backend = self.backend.read().await;
        backend.as_ref().map(|b| HidInfo {
            name: b.name(),
            initialized: true,
            supports_absolute_mouse: b.supports_absolute_mouse(),
            screen_resolution: b.screen_resolution(),
        })
    }

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> crate::events::SystemEvent {
        let backend = self.backend.read().await;
        let backend_type = self.backend_type().await;
        let (backend_name, initialized) = match backend.as_ref() {
            Some(b) => (b.name(), true),
            None => (backend_type.name_str(), false),
        };

        // Include error information from monitor
        let (error, error_code) = match self.monitor.status().await {
            HidHealthStatus::Error { reason, error_code, .. } => {
                (Some(reason), Some(error_code))
            }
            _ => (None, None),
        };

        crate::events::SystemEvent::HidStateChanged {
            backend: backend_name.to_string(),
            initialized,
            error,
            error_code,
        }
    }

    /// Get the health monitor reference
    pub fn monitor(&self) -> &Arc<HidHealthMonitor> {
        &self.monitor
    }

    /// Get current health status
    pub async fn health_status(&self) -> HidHealthStatus {
        self.monitor.status().await
    }

    /// Check if the HID backend is healthy
    pub async fn is_healthy(&self) -> bool {
        self.monitor.is_healthy().await
    }

    /// Reload the HID backend with new type
    pub async fn reload(&self, new_backend_type: HidBackendType) -> Result<()> {
        info!("Reloading HID backend: {:?}", new_backend_type);

        // Shutdown existing backend first
        if let Some(backend) = self.backend.write().await.take() {
            if let Err(e) = backend.shutdown().await {
                warn!("Error shutting down old HID backend: {}", e);
            }
        }

        // Create and initialize new backend
        let new_backend: Option<Box<dyn HidBackend>> = match new_backend_type {
            HidBackendType::Otg => {
                info!("Initializing OTG HID backend");

                // Get OtgService reference
                let otg_service = match self.otg_service.as_ref() {
                    Some(svc) => svc,
                    None => {
                        warn!("OTG backend requires OtgService, but it's not available");
                        return Err(AppError::Config(
                            "OTG backend not available (OtgService missing)".to_string()
                        ));
                    }
                };

                // Request HID functions from OtgService
                match otg_service.enable_hid().await {
                    Ok(handles) => {
                        // Create OtgBackend from handles
                        match otg::OtgBackend::from_handles(handles) {
                            Ok(backend) => {
                                let boxed: Box<dyn HidBackend> = Box::new(backend);
                                match boxed.init().await {
                                    Ok(_) => {
                                        info!("OTG backend initialized successfully");
                                        Some(boxed)
                                    }
                                    Err(e) => {
                                        warn!("Failed to initialize OTG backend: {}", e);
                                        // Cleanup: disable HID in OtgService
                                        if let Err(e2) = otg_service.disable_hid().await {
                                            warn!("Failed to cleanup HID after init failure: {}", e2);
                                        }
                                        None
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to create OTG backend: {}", e);
                                // Cleanup: disable HID in OtgService
                                if let Err(e2) = otg_service.disable_hid().await {
                                    warn!("Failed to cleanup HID after creation failure: {}", e2);
                                }
                                None
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to enable HID in OtgService: {}", e);
                        None
                    }
                }
            }
            HidBackendType::Ch9329 { ref port, baud_rate } => {
                info!("Initializing CH9329 HID backend on {} @ {} baud", port, baud_rate);
                match ch9329::Ch9329Backend::with_baud_rate(port, baud_rate) {
                    Ok(b) => {
                        let boxed = Box::new(b);
                        match boxed.init().await {
                            Ok(_) => Some(boxed),
                            Err(e) => {
                                warn!("Failed to initialize CH9329 backend: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to create CH9329 backend: {}", e);
                        None
                    }
                }
            }
            HidBackendType::None => {
                warn!("HID backend disabled");
                None
            }
        };

        *self.backend.write().await = new_backend;

        if self.backend.read().await.is_some() {
            info!("HID backend reloaded successfully: {:?}", new_backend_type);

            // Update backend_type on success
            *self.backend_type.write().await = new_backend_type.clone();

            // Reset monitor state on successful reload
            self.monitor.reset().await;

            // Publish HID state changed event
            let backend_name = new_backend_type.name_str().to_string();
            self.publish_event(crate::events::SystemEvent::HidStateChanged {
                backend: backend_name,
                initialized: true,
                error: None,
                error_code: None,
            })
            .await;

            Ok(())
        } else {
            warn!("HID backend reload resulted in no active backend");

            // Update backend_type even on failure (to reflect the attempted change)
            *self.backend_type.write().await = new_backend_type.clone();

            // Publish event with initialized=false
            self.publish_event(crate::events::SystemEvent::HidStateChanged {
                backend: new_backend_type.name_str().to_string(),
                initialized: false,
                error: Some("Failed to initialize HID backend".to_string()),
                error_code: Some("init_failed".to_string()),
            })
            .await;

            Err(AppError::Internal(
                "Failed to reload HID backend".to_string(),
            ))
        }
    }

    /// Publish event to event bus if available
    async fn publish_event(&self, event: crate::events::SystemEvent) {
        if let Some(events) = self.events.read().await.as_ref() {
            events.publish(event);
        }
    }
}

impl Default for HidController {
    fn default() -> Self {
        Self::new(HidBackendType::None, None)
    }
}
