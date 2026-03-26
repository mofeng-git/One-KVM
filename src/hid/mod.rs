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
pub mod otg;
pub mod types;
pub mod websocket;

pub use backend::{HidBackend, HidBackendStatus, HidBackendType};
pub use otg::LedState;
pub use types::{
    ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent,
    MouseEventType,
};

/// HID backend information
#[derive(Debug, Clone)]
pub struct HidInfo {
    /// Backend name
    pub name: String,
    /// Whether backend is initialized
    pub initialized: bool,
    /// Whether absolute mouse positioning is supported
    pub supports_absolute_mouse: bool,
    /// Screen resolution for absolute mouse
    pub screen_resolution: Option<(u32, u32)>,
}

/// Unified HID runtime state used by snapshots and events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidRuntimeState {
    /// Whether a backend is configured and expected to exist.
    pub available: bool,
    /// Stable backend key: "otg", "ch9329", "none".
    pub backend: String,
    /// Whether the backend is currently initialized and operational.
    pub initialized: bool,
    /// Whether the backend is currently online.
    pub online: bool,
    /// Whether absolute mouse positioning is supported.
    pub supports_absolute_mouse: bool,
    /// Screen resolution for absolute mouse mode.
    pub screen_resolution: Option<(u32, u32)>,
    /// Device path associated with the backend, if any.
    pub device: Option<String>,
    /// Current user-facing error, if any.
    pub error: Option<String>,
    /// Current programmatic error code, if any.
    pub error_code: Option<String>,
}

impl HidRuntimeState {
    fn from_backend_type(backend_type: &HidBackendType) -> Self {
        Self {
            available: !matches!(backend_type, HidBackendType::None),
            backend: backend_type.name_str().to_string(),
            initialized: false,
            online: false,
            supports_absolute_mouse: false,
            screen_resolution: None,
            device: device_for_backend_type(backend_type),
            error: None,
            error_code: None,
        }
    }

    fn from_backend(backend_type: &HidBackendType, backend: &dyn HidBackend) -> Self {
        let status = backend.status();
        Self {
            available: !matches!(backend_type, HidBackendType::None),
            backend: backend_type.name_str().to_string(),
            initialized: status.initialized,
            online: status.online,
            supports_absolute_mouse: backend.supports_absolute_mouse(),
            screen_resolution: backend.screen_resolution(),
            device: device_for_backend_type(backend_type),
            error: status.error,
            error_code: status.error_code,
        }
    }

    fn with_error(
        backend_type: &HidBackendType,
        current: &Self,
        reason: impl Into<String>,
        error_code: impl Into<String>,
    ) -> Self {
        let mut next = current.clone();
        next.available = !matches!(backend_type, HidBackendType::None);
        next.backend = backend_type.name_str().to_string();
        next.initialized = false;
        next.online = false;
        next.device = device_for_backend_type(backend_type);
        next.error = Some(reason.into());
        next.error_code = Some(error_code.into());
        next
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, warn};
use tokio::sync::RwLock;

use crate::error::{AppError, Result};
use crate::events::{EventBus, SystemEvent};
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
    events: Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    /// Unified HID runtime state.
    runtime_state: Arc<RwLock<HidRuntimeState>>,
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
    /// Backend initialization fast flag
    backend_available: Arc<AtomicBool>,
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
            backend_type: Arc::new(RwLock::new(backend_type.clone())),
            events: Arc::new(tokio::sync::RwLock::new(None)),
            runtime_state: Arc::new(RwLock::new(HidRuntimeState::from_backend_type(
                &backend_type,
            ))),
            hid_tx,
            hid_rx: Mutex::new(Some(hid_rx)),
            pending_move: Arc::new(parking_lot::Mutex::new(None)),
            pending_move_flag: Arc::new(AtomicBool::new(false)),
            hid_worker: Mutex::new(None),
            backend_available: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
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

        if let Err(e) = backend.init().await {
            self.backend_available.store(false, Ordering::Release);
            let error_state = {
                let backend_type = self.backend_type.read().await.clone();
                let current = self.runtime_state.read().await.clone();
                HidRuntimeState::with_error(
                    &backend_type,
                    &current,
                    format!("Failed to initialize HID backend: {}", e),
                    "init_failed",
                )
            };
            self.apply_runtime_state(error_state).await;
            return Err(e);
        }

        *self.backend.write().await = Some(backend);
        self.sync_runtime_state_from_backend().await;

        // Start HID event worker (once)
        self.start_event_worker().await;

        info!("HID backend initialized: {:?}", backend_type);
        Ok(())
    }

    /// Shutdown the HID backend and release resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HID controller");

        // Close the backend
        if let Some(backend) = self.backend.write().await.take() {
            if let Err(e) = backend.shutdown().await {
                warn!("Error shutting down HID backend: {}", e);
            }
        }
        self.backend_available.store(false, Ordering::Release);
        let backend_type = self.backend_type.read().await.clone();
        let mut shutdown_state = HidRuntimeState::from_backend_type(&backend_type);
        if matches!(backend_type, HidBackendType::None) {
            shutdown_state.available = false;
        } else {
            shutdown_state.error = Some("HID backend stopped".to_string());
            shutdown_state.error_code = Some("shutdown".to_string());
        }
        self.apply_runtime_state(shutdown_state).await;

        // If OTG backend, notify OtgService to disable HID
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
        self.backend_available.load(Ordering::Acquire)
    }

    /// Get backend type
    pub async fn backend_type(&self) -> HidBackendType {
        self.backend_type.read().await.clone()
    }

    /// Get backend info
    pub async fn info(&self) -> Option<HidInfo> {
        let state = self.runtime_state.read().await.clone();
        if !state.available {
            return None;
        }

        Some(HidInfo {
            name: state.backend,
            initialized: state.initialized,
            supports_absolute_mouse: state.supports_absolute_mouse,
            screen_resolution: state.screen_resolution,
        })
    }

    /// Get current HID runtime state snapshot.
    pub async fn snapshot(&self) -> HidRuntimeState {
        self.runtime_state.read().await.clone()
    }

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> crate::events::SystemEvent {
        let state = self.snapshot().await;
        SystemEvent::HidStateChanged {
            backend: state.backend,
            initialized: state.initialized,
            online: state.online,
            error: state.error,
            error_code: state.error_code,
        }
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

        if matches!(new_backend_type, HidBackendType::None) {
            *self.backend_type.write().await = HidBackendType::None;
            self.apply_runtime_state(HidRuntimeState::from_backend_type(&HidBackendType::None))
                .await;
            return Ok(());
        }

        if self.backend.read().await.is_some() {
            info!("HID backend reloaded successfully: {:?}", new_backend_type);
            self.start_event_worker().await;

            // Update backend_type on success
            *self.backend_type.write().await = new_backend_type.clone();

            self.sync_runtime_state_from_backend().await;

            Ok(())
        } else {
            warn!("HID backend reload resulted in no active backend");
            self.backend_available.store(false, Ordering::Release);

            // Update backend_type even on failure (to reflect the attempted change)
            *self.backend_type.write().await = new_backend_type.clone();

            let current = self.runtime_state.read().await.clone();
            let error_state = HidRuntimeState::with_error(
                &new_backend_type,
                &current,
                "Failed to initialize HID backend",
                "init_failed",
            );
            self.apply_runtime_state(error_state).await;

            Err(AppError::Internal(
                "Failed to reload HID backend".to_string(),
            ))
        }
    }

    async fn apply_runtime_state(&self, next: HidRuntimeState) {
        apply_runtime_state(&self.runtime_state, &self.events, next).await;
    }

    async fn sync_runtime_state_from_backend(&self) {
        let backend_opt = self.backend.read().await.clone();
        let backend_type = self.backend_type.read().await.clone();

        let next = match backend_opt.as_ref() {
            Some(backend) => HidRuntimeState::from_backend(&backend_type, backend.as_ref()),
            None => HidRuntimeState::from_backend_type(&backend_type),
        };

        self.backend_available
            .store(next.initialized, Ordering::Release);
        self.apply_runtime_state(next).await;
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
        let backend_type = self.backend_type.clone();
        let runtime_state = self.runtime_state.clone();
        let events = self.events.clone();
        let backend_available = self.backend_available.clone();
        let pending_move = self.pending_move.clone();
        let pending_move_flag = self.pending_move_flag.clone();

        let handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                let event = match rx.recv().await {
                    Some(ev) => ev,
                    None => break,
                };

                process_hid_event(
                    event,
                    &backend,
                    &backend_type,
                    &runtime_state,
                    &events,
                    backend_available.as_ref(),
                )
                .await;

                // After each event, flush latest move if pending
                if pending_move_flag.swap(false, Ordering::AcqRel) {
                    let move_event = { pending_move.lock().take() };
                    if let Some(move_event) = move_event {
                        process_hid_event(
                            HidEvent::Mouse(move_event),
                            &backend,
                            &backend_type,
                            &runtime_state,
                            &events,
                            backend_available.as_ref(),
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
    backend_type: &Arc<RwLock<HidBackendType>>,
    runtime_state: &Arc<RwLock<HidRuntimeState>>,
    events: &Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    backend_available: &AtomicBool,
) {
    let backend_opt = backend.read().await.clone();
    let backend = match backend_opt {
        Some(b) => b,
        None => return,
    };

    let backend_for_send = backend.clone();
    let result = tokio::task::spawn_blocking(move || {
        futures::executor::block_on(async move {
            match event {
                HidEvent::Keyboard(ev) => backend_for_send.send_keyboard(ev).await,
                HidEvent::Mouse(ev) => backend_for_send.send_mouse(ev).await,
                HidEvent::Consumer(ev) => backend_for_send.send_consumer(ev).await,
                HidEvent::Reset => backend_for_send.reset().await,
            }
        })
    })
    .await;

    let result = match result {
        Ok(r) => r,
        Err(_) => return,
    };

    match result {
        Ok(_) => {}
        Err(e) => {
            warn!("HID event processing failed: {}", e);
        }
    }

    let backend_kind = backend_type.read().await.clone();
    let next = HidRuntimeState::from_backend(&backend_kind, backend.as_ref());
    backend_available.store(next.initialized, Ordering::Release);
    apply_runtime_state(runtime_state, events, next).await;
}

impl Default for HidController {
    fn default() -> Self {
        Self::new(HidBackendType::None, None)
    }
}

fn device_for_backend_type(backend_type: &HidBackendType) -> Option<String> {
    match backend_type {
        HidBackendType::Ch9329 { port, .. } => Some(port.clone()),
        _ => None,
    }
}

async fn apply_runtime_state(
    runtime_state: &Arc<RwLock<HidRuntimeState>>,
    events: &Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    next: HidRuntimeState,
) {
    let changed = {
        let mut guard = runtime_state.write().await;
        if *guard == next {
            false
        } else {
            *guard = next.clone();
            true
        }
    };

    if !changed {
        return;
    }

    if let Some(events) = events.read().await.as_ref() {
        events.publish(SystemEvent::HidStateChanged {
            backend: next.backend,
            initialized: next.initialized,
            online: next.online,
            error: next.error,
            error_code: next.error_code,
        });
    }
}
