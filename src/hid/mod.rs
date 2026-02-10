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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{AppError, Result};
use crate::otg::OtgService;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const HID_EVENT_QUEUE_CAPACITY: usize = 64;
const HID_EVENT_SEND_TIMEOUT_MS: u64 = 30;

#[derive(Debug)]
enum HidEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Consumer(ConsumerEvent),
    Reset,
}

/// HID controller managing keyboard and mouse input
pub struct HidController {
    /// OTG Service reference (only used when backend is OTG)
    otg_service: Option<Arc<OtgService>>,
    /// Active backend
    backend: Arc<RwLock<Option<Arc<dyn HidBackend>>>>,
    /// Backend type (mutable for reload)
    backend_type: Arc<RwLock<HidBackendType>>,
    /// Event bus for broadcasting state changes (optional)
    events: tokio::sync::RwLock<Option<Arc<crate::events::EventBus>>>,
    /// Health monitor for error tracking and recovery
    monitor: Arc<HidHealthMonitor>,
    /// HID event queue sender (non-blocking)
    hid_tx: mpsc::Sender<HidEvent>,
    /// HID event queue receiver (moved into worker on first start)
    hid_rx: Mutex<Option<mpsc::Receiver<HidEvent>>>,
    /// Coalesced mouse move (latest)
    pending_move: Arc<parking_lot::Mutex<Option<MouseEvent>>>,
    /// Pending move flag (fast path)
    pending_move_flag: Arc<AtomicBool>,
    /// Worker task handle
    hid_worker: Mutex<Option<JoinHandle<()>>>,
    /// Backend availability fast flag
    backend_available: AtomicBool,
}

impl HidController {
    /// Create a new HID controller with specified backend
    ///
    /// For OTG backend, otg_service should be provided to support hot-reload
    pub fn new(backend_type: HidBackendType, otg_service: Option<Arc<OtgService>>) -> Self {
        let (hid_tx, hid_rx) = mpsc::channel(HID_EVENT_QUEUE_CAPACITY);
        Self {
            otg_service,
            backend: Arc::new(RwLock::new(None)),
            backend_type: Arc::new(RwLock::new(backend_type)),
            events: tokio::sync::RwLock::new(None),
            monitor: Arc::new(HidHealthMonitor::with_defaults()),
            hid_tx,
            hid_rx: Mutex::new(Some(hid_rx)),
            pending_move: Arc::new(parking_lot::Mutex::new(None)),
            pending_move_flag: Arc::new(AtomicBool::new(false)),
            hid_worker: Mutex::new(None),
            backend_available: AtomicBool::new(false),
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
        let backend: Arc<dyn HidBackend> = match backend_type {
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
                Arc::new(otg::OtgBackend::from_handles(handles)?)
            }
            HidBackendType::Ch9329 {
                ref port,
                baud_rate,
            } => {
                info!(
                    "Initializing CH9329 HID backend on {} @ {} baud",
                    port, baud_rate
                );
                Arc::new(ch9329::Ch9329Backend::with_baud_rate(port, baud_rate)?)
            }
            HidBackendType::None => {
                warn!("HID backend disabled");
                return Ok(());
            }
        };

        backend.init().await?;
        *self.backend.write().await = Some(backend);
        self.backend_available.store(true, Ordering::Release);

        // Start HID event worker (once)
        self.start_event_worker().await;

        info!("HID backend initialized: {:?}", backend_type);
        Ok(())
    }

    /// Shutdown the HID backend and release resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HID controller");

        // Close the backend
        *self.backend.write().await = None;
        self.backend_available.store(false, Ordering::Release);

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
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }
        self.enqueue_event(HidEvent::Keyboard(event)).await
    }

    /// Send mouse event
    pub async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }

        if matches!(
            event.event_type,
            MouseEventType::Move | MouseEventType::MoveAbs
        ) {
            // Best-effort: drop/merge move events if queue is full
            self.enqueue_mouse_move(event)
        } else {
            self.enqueue_event(HidEvent::Mouse(event)).await
        }
    }

    /// Send consumer control event (multimedia keys)
    pub async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }
        self.enqueue_event(HidEvent::Consumer(event)).await
    }

    /// Reset all keys (release all pressed keys)
    pub async fn reset(&self) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Ok(());
        }
        // Reset is important but best-effort; enqueue to avoid blocking
        self.enqueue_event(HidEvent::Reset).await
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
            HidHealthStatus::Error {
                reason, error_code, ..
            } => (Some(reason), Some(error_code)),
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
        self.backend_available.store(false, Ordering::Release);

        // Shutdown existing backend first
        if let Some(backend) = self.backend.write().await.take() {
            if let Err(e) = backend.shutdown().await {
                warn!("Error shutting down old HID backend: {}", e);
            }
        }

        // Create and initialize new backend
        let new_backend: Option<Arc<dyn HidBackend>> = match new_backend_type {
            HidBackendType::Otg => {
                info!("Initializing OTG HID backend");

                // Get OtgService reference
                let otg_service = match self.otg_service.as_ref() {
                    Some(svc) => svc,
                    None => {
                        warn!("OTG backend requires OtgService, but it's not available");
                        return Err(AppError::Config(
                            "OTG backend not available (OtgService missing)".to_string(),
                        ));
                    }
                };

                // Request HID functions from OtgService
                match otg_service.enable_hid().await {
                    Ok(handles) => {
                        // Create OtgBackend from handles
                        match otg::OtgBackend::from_handles(handles) {
                            Ok(backend) => {
                                let backend = Arc::new(backend);
                                match backend.init().await {
                                    Ok(_) => {
                                        info!("OTG backend initialized successfully");
                                        Some(backend)
                                    }
                                    Err(e) => {
                                        warn!("Failed to initialize OTG backend: {}", e);
                                        // Cleanup: disable HID in OtgService
                                        if let Err(e2) = otg_service.disable_hid().await {
                                            warn!(
                                                "Failed to cleanup HID after init failure: {}",
                                                e2
                                            );
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
            HidBackendType::Ch9329 {
                ref port,
                baud_rate,
            } => {
                info!(
                    "Initializing CH9329 HID backend on {} @ {} baud",
                    port, baud_rate
                );
                match ch9329::Ch9329Backend::with_baud_rate(port, baud_rate) {
                    Ok(b) => {
                        let backend = Arc::new(b);
                        match backend.init().await {
                            Ok(_) => Some(backend),
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
            self.backend_available.store(true, Ordering::Release);
            self.start_event_worker().await;

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
            self.backend_available.store(false, Ordering::Release);

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

    async fn start_event_worker(&self) {
        let mut worker_guard = self.hid_worker.lock().await;
        if worker_guard.is_some() {
            return;
        }

        let mut rx_guard = self.hid_rx.lock().await;
        let rx = match rx_guard.take() {
            Some(rx) => rx,
            None => return,
        };

        let backend = self.backend.clone();
        let monitor = self.monitor.clone();
        let backend_type = self.backend_type.clone();
        let pending_move = self.pending_move.clone();
        let pending_move_flag = self.pending_move_flag.clone();

        let handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                let event = match rx.recv().await {
                    Some(ev) => ev,
                    None => break,
                };

                process_hid_event(event, &backend, &monitor, &backend_type).await;

                // After each event, flush latest move if pending
                if pending_move_flag.swap(false, Ordering::AcqRel) {
                    let move_event = { pending_move.lock().take() };
                    if let Some(move_event) = move_event {
                        process_hid_event(
                            HidEvent::Mouse(move_event),
                            &backend,
                            &monitor,
                            &backend_type,
                        )
                        .await;
                    }
                }
            }
        });

        *worker_guard = Some(handle);
    }

    fn enqueue_mouse_move(&self, event: MouseEvent) -> Result<()> {
        match self.hid_tx.try_send(HidEvent::Mouse(event.clone())) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                *self.pending_move.lock() = Some(event);
                self.pending_move_flag.store(true, Ordering::Release);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(AppError::BadRequest("HID event queue closed".to_string()))
            }
        }
    }

    async fn enqueue_event(&self, event: HidEvent) -> Result<()> {
        match self.hid_tx.try_send(event) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(ev)) => {
                // For non-move events, wait briefly to avoid dropping critical input
                let tx = self.hid_tx.clone();
                let send_result = tokio::time::timeout(
                    Duration::from_millis(HID_EVENT_SEND_TIMEOUT_MS),
                    tx.send(ev),
                )
                .await;
                if send_result.is_ok() {
                    Ok(())
                } else {
                    warn!("HID event queue full, dropping event");
                    Ok(())
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(AppError::BadRequest("HID event queue closed".to_string()))
            }
        }
    }
}

async fn process_hid_event(
    event: HidEvent,
    backend: &Arc<RwLock<Option<Arc<dyn HidBackend>>>>,
    monitor: &Arc<HidHealthMonitor>,
    backend_type: &Arc<RwLock<HidBackendType>>,
) {
    let backend_opt = backend.read().await.clone();
    let backend = match backend_opt {
        Some(b) => b,
        None => return,
    };

    let result = tokio::task::spawn_blocking(move || {
        futures::executor::block_on(async move {
            match event {
                HidEvent::Keyboard(ev) => backend.send_keyboard(ev).await,
                HidEvent::Mouse(ev) => backend.send_mouse(ev).await,
                HidEvent::Consumer(ev) => backend.send_consumer(ev).await,
                HidEvent::Reset => backend.reset().await,
            }
        })
    })
    .await;

    let result = match result {
        Ok(r) => r,
        Err(_) => return,
    };

    match result {
        Ok(_) => {
            if monitor.is_error().await {
                let backend_type = backend_type.read().await;
                monitor.report_recovered(backend_type.name_str()).await;
            }
        }
        Err(e) => {
            if let AppError::HidError {
                ref backend,
                ref reason,
                ref error_code,
            } = e
            {
                if error_code != "eagain_retry" {
                    monitor
                        .report_error(backend, None, reason, error_code)
                        .await;
                }
            }
        }
    }
}

impl Default for HidController {
    fn default() -> Self {
        Self::new(HidBackendType::None, None)
    }
}
