//! HID path: browser (WebSocket or WebRTC DataChannel) → queue → OTG gadget or CH9329.

pub mod backend;
pub mod ch9329;
pub mod consumer;
pub mod datachannel;
pub mod keyboard;
pub mod otg;
pub mod types;
pub mod websocket;

pub use crate::events::LedState;
pub use backend::{HidBackend, HidBackendRuntimeSnapshot, HidBackendType};
pub use keyboard::CanonicalKey;
pub use types::{
    ConsumerEvent, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent,
    MouseEventType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidRuntimeState {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub online: bool,
    pub supports_absolute_mouse: bool,
    pub keyboard_leds_enabled: bool,
    pub led_state: LedState,
    pub screen_resolution: Option<(u32, u32)>,
    pub device: Option<String>,
    pub error: Option<String>,
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
            keyboard_leds_enabled: false,
            led_state: LedState::default(),
            screen_resolution: None,
            device: device_for_backend_type(backend_type),
            error: None,
            error_code: None,
        }
    }

    fn from_backend(backend_type: &HidBackendType, snapshot: HidBackendRuntimeSnapshot) -> Self {
        Self {
            available: !matches!(backend_type, HidBackendType::None),
            backend: backend_type.name_str().to_string(),
            initialized: snapshot.initialized,
            online: snapshot.online,
            supports_absolute_mouse: snapshot.supports_absolute_mouse,
            keyboard_leds_enabled: snapshot.keyboard_leds_enabled,
            led_state: snapshot.led_state,
            screen_resolution: snapshot.screen_resolution,
            device: snapshot
                .device
                .or_else(|| device_for_backend_type(backend_type)),
            error: snapshot.error,
            error_code: snapshot.error_code,
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
        next.keyboard_leds_enabled = false;
        next.led_state = LedState::default();
        next.device = device_for_backend_type(backend_type);
        next.error = Some(reason.into());
        next.error_code = Some(error_code.into());
        next
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{AppError, Result};
use crate::events::EventBus;
use crate::otg::OtgService;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

const HID_EVENT_QUEUE_CAPACITY: usize = 64;
const HID_EVENT_SEND_TIMEOUT_MS: u64 = 30;

#[derive(Debug)]
enum QueuedHidEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Consumer(ConsumerEvent),
    Reset,
}

pub struct HidController {
    otg_service: Option<Arc<OtgService>>,
    backend: Arc<RwLock<Option<Arc<dyn HidBackend>>>>,
    backend_type: Arc<RwLock<HidBackendType>>,
    events: Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    runtime_state: Arc<RwLock<HidRuntimeState>>,
    hid_tx: mpsc::Sender<QueuedHidEvent>,
    hid_rx: Mutex<Option<mpsc::Receiver<QueuedHidEvent>>>,
    pending_move: Arc<parking_lot::Mutex<Option<MouseEvent>>>,
    pending_move_flag: Arc<AtomicBool>,
    hid_worker: Mutex<Option<JoinHandle<()>>>,
    runtime_worker: Mutex<Option<JoinHandle<()>>>,
    backend_available: Arc<AtomicBool>,
}

impl HidController {
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
            runtime_worker: Mutex::new(None),
            backend_available: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    pub async fn init(&self) -> Result<()> {
        let backend_type = self.backend_type.read().await.clone();
        let backend: Arc<dyn HidBackend> = match backend_type {
            HidBackendType::Otg => {
                let otg_service = self
                    .otg_service
                    .as_ref()
                    .ok_or_else(|| AppError::Internal("OtgService not available".into()))?;

                let handles = otg_service.hid_device_paths().await.ok_or_else(|| {
                    AppError::Config("OTG HID paths are not available".to_string())
                })?;

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

        self.start_event_worker().await;
        self.restart_runtime_worker().await;

        info!("HID backend initialized: {:?}", backend_type);
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HID controller");
        self.stop_runtime_worker().await;

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

        info!("HID controller shutdown complete");
        Ok(())
    }

    pub async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }
        self.enqueue_event(QueuedHidEvent::Keyboard(event)).await
    }

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
            self.enqueue_mouse_move(event)
        } else {
            self.enqueue_event(QueuedHidEvent::Mouse(event)).await
        }
    }

    pub async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }
        self.enqueue_event(QueuedHidEvent::Consumer(event)).await
    }

    pub async fn reset(&self) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Ok(());
        }
        self.enqueue_event(QueuedHidEvent::Reset).await
    }

    pub async fn is_available(&self) -> bool {
        self.backend_available.load(Ordering::Acquire)
    }

    pub async fn backend_type(&self) -> HidBackendType {
        self.backend_type.read().await.clone()
    }

    pub async fn snapshot(&self) -> HidRuntimeState {
        self.runtime_state.read().await.clone()
    }

    pub async fn reload(&self, new_backend_type: HidBackendType) -> Result<()> {
        info!("Reloading HID backend: {:?}", new_backend_type);
        self.backend_available.store(false, Ordering::Release);
        self.stop_runtime_worker().await;

        if let Some(backend) = self.backend.write().await.take() {
            if let Err(e) = backend.shutdown().await {
                warn!("Error shutting down old HID backend: {}", e);
            }
        }

        let new_backend: Option<Arc<dyn HidBackend>> = match new_backend_type {
            HidBackendType::Otg => {
                info!("Initializing OTG HID backend");

                let otg_service = match self.otg_service.as_ref() {
                    Some(svc) => svc,
                    None => {
                        warn!("OTG backend requires OtgService, but it's not available");
                        return Err(AppError::Config(
                            "OTG backend not available (OtgService missing)".to_string(),
                        ));
                    }
                };

                match otg_service.hid_device_paths().await {
                    Some(handles) => match otg::OtgBackend::from_handles(handles) {
                        Ok(backend) => {
                            let backend = Arc::new(backend);
                            match backend.init().await {
                                Ok(_) => {
                                    info!("OTG backend initialized successfully");
                                    Some(backend)
                                }
                                Err(e) => {
                                    warn!("Failed to initialize OTG backend: {}", e);
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create OTG backend: {}", e);
                            None
                        }
                    },
                    None => {
                        warn!("OTG HID paths are not available");
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

            *self.backend_type.write().await = new_backend_type.clone();

            self.sync_runtime_state_from_backend().await;
            self.restart_runtime_worker().await;

            Ok(())
        } else {
            warn!("HID backend reload resulted in no active backend");
            self.backend_available.store(false, Ordering::Release);

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
        apply_backend_runtime_state(
            &self.backend_type,
            &self.runtime_state,
            &self.events,
            self.backend_available.as_ref(),
            backend_opt.as_deref(),
        )
        .await;
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
        let pending_move = self.pending_move.clone();
        let pending_move_flag = self.pending_move_flag.clone();

        let handle = tokio::spawn(async move {
            let mut rx = rx;
            loop {
                let event = match rx.recv().await {
                    Some(ev) => ev,
                    None => break,
                };

                process_hid_event(event, &backend).await;

                if pending_move_flag.swap(false, Ordering::AcqRel) {
                    let move_event = { pending_move.lock().take() };
                    if let Some(move_event) = move_event {
                        process_hid_event(QueuedHidEvent::Mouse(move_event), &backend).await;
                    }
                }
            }
        });

        *worker_guard = Some(handle);
    }

    async fn restart_runtime_worker(&self) {
        self.stop_runtime_worker().await;

        let backend_opt = self.backend.read().await.clone();
        let Some(backend) = backend_opt else {
            return;
        };

        let mut runtime_rx = backend.subscribe_runtime();
        let runtime_state = self.runtime_state.clone();
        let events = self.events.clone();
        let backend_available = self.backend_available.clone();
        let backend_type = self.backend_type.clone();

        let handle = tokio::spawn(async move {
            loop {
                if runtime_rx.changed().await.is_err() {
                    break;
                }

                apply_backend_runtime_state(
                    &backend_type,
                    &runtime_state,
                    &events,
                    backend_available.as_ref(),
                    Some(backend.as_ref()),
                )
                .await;
            }
        });

        *self.runtime_worker.lock().await = Some(handle);
    }

    async fn stop_runtime_worker(&self) {
        if let Some(handle) = self.runtime_worker.lock().await.take() {
            handle.abort();
        }
    }

    fn enqueue_mouse_move(&self, event: MouseEvent) -> Result<()> {
        match self.hid_tx.try_send(QueuedHidEvent::Mouse(event.clone())) {
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

    async fn enqueue_event(&self, event: QueuedHidEvent) -> Result<()> {
        match self.hid_tx.try_send(event) {
            Ok(_) => Ok(()),
            Err(mpsc::error::TrySendError::Full(ev)) => {
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

async fn apply_backend_runtime_state(
    backend_type: &Arc<RwLock<HidBackendType>>,
    runtime_state: &Arc<RwLock<HidRuntimeState>>,
    events: &Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    backend_available: &AtomicBool,
    backend: Option<&dyn HidBackend>,
) {
    let backend_kind = backend_type.read().await.clone();
    let next = match backend {
        Some(backend) => HidRuntimeState::from_backend(&backend_kind, backend.runtime_snapshot()),
        None => HidRuntimeState::from_backend_type(&backend_kind),
    };
    backend_available.store(next.initialized, Ordering::Release);
    apply_runtime_state(runtime_state, events, next).await;
}

async fn process_hid_event(
    event: QueuedHidEvent,
    backend: &Arc<RwLock<Option<Arc<dyn HidBackend>>>>,
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
                QueuedHidEvent::Keyboard(ev) => backend_for_send.send_keyboard(ev).await,
                QueuedHidEvent::Mouse(ev) => backend_for_send.send_mouse(ev).await,
                QueuedHidEvent::Consumer(ev) => backend_for_send.send_consumer(ev).await,
                QueuedHidEvent::Reset => backend_for_send.reset().await,
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
        events.mark_device_info_dirty();
    }
}
