//! HID path: browser (WebSocket or WebRTC DataChannel) → queue → OTG gadget or CH9329.

pub mod backend;
#[cfg(target_os = "linux")]
mod bluetooth;
pub mod ch9329;
mod ch9329_proto;
pub mod consumer;
pub mod datachannel;
mod event_queue;
mod factory;
pub(crate) mod input_session;
mod jiggler;
pub mod keyboard;
mod macos_drag;
mod mouse_state;
#[cfg(unix)]
pub mod otg;
#[cfg(unix)]
mod otg_device;
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

use crate::config::{Ch9329DescriptorConfig, Ch9329DescriptorState};
use crate::error::{AppError, Result};
use crate::events::EventBus;
#[cfg(unix)]
use crate::otg::OtgService;
use factory::HidBackendFactory;
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
    Confirmed(
        Box<QueuedHidEvent>,
        tokio::sync::oneshot::Sender<Result<()>>,
    ),
}

pub struct HidController {
    backend_factory: HidBackendFactory,
    backend: Arc<RwLock<Option<Arc<dyn HidBackend>>>>,
    backend_type: Arc<RwLock<HidBackendType>>,
    events: Arc<tokio::sync::RwLock<Option<Arc<EventBus>>>>,
    runtime_state: Arc<RwLock<HidRuntimeState>>,
    hid_tx: event_queue::Sender<QueuedHidEvent>,
    hid_rx: Mutex<Option<event_queue::Receiver<QueuedHidEvent>>>,
    hid_worker: Mutex<Option<JoinHandle<()>>>,
    runtime_worker: Mutex<Option<JoinHandle<()>>>,
    backend_available: Arc<AtomicBool>,
    screen_resolution: parking_lot::RwLock<(u32, u32)>,
    jiggler: Arc<parking_lot::Mutex<jiggler::JigglerState>>,
    jiggler_changed: Arc<tokio::sync::Notify>,
}

impl HidController {
    #[cfg(unix)]
    pub fn new(backend_type: HidBackendType, otg_service: Option<Arc<OtgService>>) -> Self {
        let (hid_tx, hid_rx) = event_queue::channel(
            HID_EVENT_QUEUE_CAPACITY,
            Duration::from_millis(HID_EVENT_SEND_TIMEOUT_MS),
        );
        Self {
            backend: Arc::new(RwLock::new(None)),
            backend_factory: HidBackendFactory::new(otg_service),
            backend_type: Arc::new(RwLock::new(backend_type.clone())),
            events: Arc::new(tokio::sync::RwLock::new(None)),
            runtime_state: Arc::new(RwLock::new(HidRuntimeState::from_backend_type(
                &backend_type,
            ))),
            hid_tx,
            hid_rx: Mutex::new(Some(hid_rx)),
            hid_worker: Mutex::new(None),
            runtime_worker: Mutex::new(None),
            backend_available: Arc::new(AtomicBool::new(false)),
            screen_resolution: parking_lot::RwLock::new((1920, 1080)),
            jiggler: Arc::new(parking_lot::Mutex::new(jiggler::JigglerState::default())),
            jiggler_changed: Arc::new(tokio::sync::Notify::new()),
        }
    }

    #[cfg(not(unix))]
    pub fn new(backend_type: HidBackendType) -> Self {
        let (hid_tx, hid_rx) = event_queue::channel(
            HID_EVENT_QUEUE_CAPACITY,
            Duration::from_millis(HID_EVENT_SEND_TIMEOUT_MS),
        );
        Self {
            backend: Arc::new(RwLock::new(None)),
            backend_factory: HidBackendFactory::new(),
            backend_type: Arc::new(RwLock::new(backend_type.clone())),
            events: Arc::new(tokio::sync::RwLock::new(None)),
            runtime_state: Arc::new(RwLock::new(HidRuntimeState::from_backend_type(
                &backend_type,
            ))),
            hid_tx,
            hid_rx: Mutex::new(Some(hid_rx)),
            hid_worker: Mutex::new(None),
            runtime_worker: Mutex::new(None),
            backend_available: Arc::new(AtomicBool::new(false)),
            screen_resolution: parking_lot::RwLock::new((1920, 1080)),
            jiggler: Arc::new(parking_lot::Mutex::new(jiggler::JigglerState::default())),
            jiggler_changed: Arc::new(tokio::sync::Notify::new()),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_bond_store(&self, store: crate::db::hid_bonds::HidBondStore) {
        let _ = self.backend_factory.bonds.set(Arc::new(store));
    }

    pub async fn set_event_bus(&self, events: Arc<EventBus>) {
        *self.events.write().await = Some(events);
    }

    pub async fn init(&self) -> Result<()> {
        let backend_type = self.backend_type.read().await.clone();
        let backend = match self.backend_factory.create_initialized(&backend_type).await {
            Ok(Some(backend)) => backend,
            Ok(None) => return Ok(()),
            Err(error) => {
                self.backend_available.store(false, Ordering::Release);
                let current = self.runtime_state.read().await.clone();
                let error_state = HidRuntimeState::with_error(
                    &backend_type,
                    &current,
                    format!("Failed to initialize HID backend: {}", error),
                    "init_failed",
                );
                self.apply_runtime_state(error_state).await;
                return Err(error);
            }
        };

        {
            let mut slot = self.backend.write().await;
            let (width, height) = *self.screen_resolution.read();
            backend.set_screen_resolution(width, height);
            *slot = Some(backend);
        }
        self.sync_runtime_state_from_backend().await;

        self.start_event_worker().await;
        self.restart_runtime_worker().await;

        info!("HID backend initialized: {:?}", backend_type);
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HID controller");
        self.backend_available.store(false, Ordering::Release);
        self.jiggler.lock().suspend();
        self.jiggler_changed.notify_one();
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

    pub async fn prepare_otg_rebuild(&self) -> Result<()> {
        let backend_type = self.backend_type.read().await.clone();
        if !matches!(backend_type, HidBackendType::Otg { .. }) {
            return Ok(());
        }

        info!("Preparing OTG HID backend for gadget rebuild");
        self.backend_available.store(false, Ordering::Release);
        self.jiggler.lock().suspend();
        self.stop_runtime_worker().await;

        if let Some(backend) = self.backend.write().await.take() {
            backend.prepare_rebuild().await?;
        }

        let current = self.runtime_state.read().await.clone();
        let rebuilding_state = HidRuntimeState::with_error(
            &backend_type,
            &current,
            "OTG gadget is rebuilding",
            "rebuilding",
        );
        self.apply_runtime_state(rebuilding_state).await;
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

        self.enqueue_event(QueuedHidEvent::Mouse(event)).await
    }

    pub async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::BadRequest(
                "HID backend not available".to_string(),
            ));
        }
        self.enqueue_event(QueuedHidEvent::Consumer(event)).await
    }

    /// MCP uses acknowledged events so timing starts after the hardware send.
    pub async fn send_keyboard_confirmed(&self, event: KeyboardEvent) -> Result<()> {
        self.send_confirmed(QueuedHidEvent::Keyboard(event)).await
    }

    pub async fn send_mouse_confirmed(&self, event: MouseEvent) -> Result<()> {
        self.send_confirmed(QueuedHidEvent::Mouse(event)).await
    }

    pub async fn reset_confirmed(&self) -> Result<()> {
        self.send_confirmed(QueuedHidEvent::Reset).await
    }

    async fn send_confirmed(&self, event: QueuedHidEvent) -> Result<()> {
        if !self.backend_available.load(Ordering::Acquire) {
            return Err(AppError::ServiceUnavailable(
                "HID backend not available".into(),
            ));
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue_event(QueuedHidEvent::Confirmed(Box::new(event), tx))
            .await?;
        tokio::time::timeout(Duration::from_secs(3), rx)
            .await
            .map_err(|_| AppError::ServiceUnavailable("HID hardware send timed out".into()))?
            .map_err(|_| {
                AppError::ServiceUnavailable("HID event discarded before sending".into())
            })?
    }

    pub async fn bluetooth_status(&self) -> Result<serde_json::Value> {
        let backend = self
            .backend
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError::BadRequest("HID unavailable".into()))?;
        backend.bluetooth_status().await
    }
    pub async fn bluetooth_action(&self, action: &str, seconds: u32) -> Result<()> {
        let backend = self
            .backend
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError::BadRequest("HID unavailable".into()))?;
        backend.bluetooth_action(action, seconds).await
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

    /// Keep the active capture dimensions across HID reloads and USB rebuilds.
    pub async fn set_screen_resolution(&self, width: u32, height: u32) {
        if width == 0 || height == 0 || width > 65535 || height > 65535 {
            return;
        }
        {
            let mut resolution = self.screen_resolution.write();
            if *resolution == (width, height) {
                return;
            }
            *resolution = (width, height);
        }
        if let Some(backend) = self.backend.read().await.as_ref() {
            let (width, height) = *self.screen_resolution.read();
            backend.set_screen_resolution(width, height);
        }
    }

    pub async fn snapshot(&self) -> HidRuntimeState {
        self.runtime_state.read().await.clone()
    }

    /// Jiggler preferences are independent of USB descriptors and device reloads.
    pub fn configure_jiggler(&self, config: &crate::config::HidConfig) {
        self.jiggler
            .lock()
            .configure(config, std::time::Instant::now());
        self.jiggler_changed.notify_one();
    }

    async fn ch9329_backend(&self) -> Result<Arc<dyn HidBackend>> {
        if !matches!(
            *self.backend_type.read().await,
            HidBackendType::Ch9329 { .. }
        ) {
            return Err(AppError::BadRequest(
                "Current HID backend is not CH9329".to_string(),
            ));
        }
        self.backend
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError::BadRequest("CH9329 backend not available".to_string()))
    }

    pub async fn apply_ch9329_descriptor(
        &self,
        descriptor: &Ch9329DescriptorConfig,
    ) -> Result<Ch9329DescriptorState> {
        self.ch9329_backend()
            .await?
            .apply_ch9329_descriptor(descriptor)
            .await
    }

    pub async fn read_ch9329_descriptor(&self) -> Result<Ch9329DescriptorState> {
        self.ch9329_backend().await?.read_ch9329_descriptor().await
    }

    pub async fn reload(&self, new_backend_type: HidBackendType) -> Result<()> {
        info!("Reloading HID backend: {:?}", new_backend_type);
        self.backend_available.store(false, Ordering::Release);
        self.jiggler.lock().suspend();
        self.stop_runtime_worker().await;

        if let Some(backend) = self.backend.write().await.take() {
            if let Err(e) = backend.shutdown().await {
                // A Bluetooth shutdown may fail to restore adapter settings. Surface
                // that failure so the config transaction can roll back.
                if matches!(
                    *self.backend_type.read().await,
                    HidBackendType::Bluetooth { .. }
                ) {
                    return Err(e);
                }
                warn!("Error shutting down old HID backend: {}", e);
            }
        }

        let new_backend = match self
            .backend_factory
            .create_initialized(&new_backend_type)
            .await
        {
            Ok(backend) => backend,
            Err(error) if matches!(&new_backend_type, HidBackendType::None) => return Err(error),
            Err(error) => {
                warn!("Failed to initialize HID backend: {}", error);
                None
            }
        };

        {
            let mut slot = self.backend.write().await;
            if let Some(backend) = new_backend.as_ref() {
                let (width, height) = *self.screen_resolution.read();
                backend.set_screen_resolution(width, height);
            }
            *slot = new_backend;
        }

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
        let jiggler = self.jiggler.clone();
        let jiggler_changed = self.jiggler_changed.clone();
        let backend_available = self.backend_available.clone();

        let handle = tokio::spawn(async move {
            let mut rx = rx;
            let mut next_poll = tokio::time::Instant::now();
            loop {
                let delay = jiggler.lock().delay();
                let work = tokio::select! {
                    biased;
                    event = rx.next() => match event {
                        Some(work) => work,
                        None => break,
                    },
                    _ = jiggler_changed.notified() => continue,
                    _ = tokio::time::sleep_until(next_poll), if delay.is_some() => {
                        // Synthetic motion is sent by this same worker, outside the
                        // input queue, so opposite movements cannot cancel out.
                        let active_backend = backend.read().await.clone();
                        let snapshot = active_backend.as_ref().map(|backend| backend.runtime_snapshot());
                        let ready = !rx.recovering() && backend_available.load(Ordering::Acquire)
                            && snapshot.as_ref().is_some_and(|state| state.initialized && state.online);
                        let resolution = snapshot.and_then(|state| state.screen_resolution).unwrap_or((1920, 1080));
                        let movement = jiggler.lock().tick(ready, resolution, std::time::Instant::now());
                        if let (Some((event, generation)), Some(active_backend)) = (movement, active_backend) {
                            let state = jiggler.clone();
                            let available = backend_available.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                // Check again after dispatch to the blocking pool:
                                // real input, shutdown or a config change may win.
                                if !available.load(Ordering::Acquire) || !state.lock().valid(generation) {
                                    return Ok(());
                                }
                                futures::executor::block_on(active_backend.send_mouse(event))
                            }).await;
                            if !matches!(result, Ok(Ok(()))) {
                                jiggler.lock().failed(generation, std::time::Instant::now());
                                warn!("Mouse jiggler movement failed: {:?}", result);
                                rx.delivery_failed().await;
                            }
                        }
                        // Keep an absolute deadline: periodic backend status
                        // notifications must not restart the timer indefinitely.
                        next_poll = tokio::time::Instant::now()
                            + jiggler.lock().delay().unwrap_or(Duration::from_secs(1));
                        continue;
                    }
                };

                match work {
                    event_queue::Work::Reset => {
                        let result =
                            process_hid_event(QueuedHidEvent::Reset, &backend, &jiggler).await;
                        if let Err(ref error) = result {
                            warn!(
                                "HID recovery release failed; input remains blocked: {}",
                                error
                            );
                        }
                        rx.reset_completed(result.is_ok()).await;
                    }
                    event_queue::Work::Event(event) => {
                        if let Err(error) = process_hid_event(event, &backend, &jiggler).await {
                            warn!(
                                "HID delivery failed; discarding backlog and releasing input: {}",
                                error
                            );
                            rx.delivery_failed().await;
                        }
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
        let jiggler = self.jiggler.clone();
        let jiggler_changed = self.jiggler_changed.clone();

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
                let state = runtime_state.read().await;
                if !state.initialized || !state.online {
                    jiggler.lock().suspend();
                }
                jiggler_changed.notify_one();
            }
        });

        *self.runtime_worker.lock().await = Some(handle);
    }

    async fn stop_runtime_worker(&self) {
        if let Some(handle) = self.runtime_worker.lock().await.take() {
            handle.abort();
        }
    }

    async fn enqueue_event(&self, event: QueuedHidEvent) -> Result<()> {
        self.jiggler.lock().activity(std::time::Instant::now());
        self.jiggler_changed.notify_one();
        self.hid_tx.send(event).await.map_err(|error| match error {
            event_queue::SendError::Closed => AppError::BadRequest("HID event queue closed".into()),
            event_queue::SendError::Full => AppError::ServiceUnavailable(
                "HID input queue full; input state will be reset".into(),
            ),
            event_queue::SendError::Recovering => AppError::ServiceUnavailable(
                "HID input blocked until release recovery succeeds".into(),
            ),
        })
    }
}

#[cfg(test)]
pub(crate) async fn test_controller(backend: Arc<dyn HidBackend>) -> Arc<HidController> {
    #[cfg(unix)]
    let controller = Arc::new(HidController::new(HidBackendType::None, None));
    #[cfg(not(unix))]
    let controller = Arc::new(HidController::new(HidBackendType::None));
    *controller.backend.write().await = Some(backend);
    controller.sync_runtime_state_from_backend().await;
    controller.start_event_worker().await;
    controller
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) struct RecordingBackend {
        pub events: parking_lot::Mutex<Vec<MouseEvent>>,
        changed: tokio::sync::Notify,
        runtime: tokio::sync::watch::Sender<()>,
    }

    impl RecordingBackend {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {
                events: parking_lot::Mutex::new(Vec::new()),
                changed: tokio::sync::Notify::new(),
                runtime: tokio::sync::watch::channel(()).0,
            })
        }

        pub async fn wait_for_events(&self, count: usize) {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let changed = self.changed.notified();
                    if self.events.lock().len() >= count {
                        return;
                    }
                    changed.await;
                }
            })
            .await
            .expect("HID worker should deliver mouse events");
        }
    }

    #[async_trait::async_trait]
    impl HidBackend for RecordingBackend {
        async fn init(&self) -> Result<()> {
            Ok(())
        }
        async fn send_keyboard(&self, _: KeyboardEvent) -> Result<()> {
            Ok(())
        }
        async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
            self.events.lock().push(event);
            self.changed.notify_one();
            Ok(())
        }
        async fn reset(&self) -> Result<()> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
        fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
            HidBackendRuntimeSnapshot {
                initialized: true,
                online: true,
                supports_absolute_mouse: true,
                ..Default::default()
            }
        }
        fn subscribe_runtime(&self) -> tokio::sync::watch::Receiver<()> {
            self.runtime.subscribe()
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
    jiggler: &Arc<parking_lot::Mutex<jiggler::JigglerState>>,
) -> Result<()> {
    let (event, reply) = match event {
        QueuedHidEvent::Confirmed(event, reply) => (*event, Some(reply)),
        event => (event, None),
    };
    {
        let mut state = jiggler.lock();
        let now = std::time::Instant::now();
        match &event {
            QueuedHidEvent::Keyboard(event) => state.keyboard(event, now),
            QueuedHidEvent::Mouse(event) => state.mouse(event, now),
            QueuedHidEvent::Consumer(event) => state.consumer(event.usage, now),
            QueuedHidEvent::Reset => state.reset(now),
            QueuedHidEvent::Confirmed(..) => unreachable!("nested confirmed HID event"),
        }
    }
    let backend = backend.read().await.clone();
    let result = match backend {
        Some(backend) => tokio::task::spawn_blocking(move || {
            futures::executor::block_on(async move {
                match event {
                    QueuedHidEvent::Keyboard(ev) => backend.send_keyboard(ev).await,
                    QueuedHidEvent::Mouse(ev) => send_mouse_to_backend(backend.as_ref(), ev).await,
                    QueuedHidEvent::Consumer(ev) => backend.send_consumer(ev).await,
                    QueuedHidEvent::Reset => backend.reset().await,
                    QueuedHidEvent::Confirmed(..) => unreachable!("nested confirmed HID event"),
                }
            })
        })
        .await
        .unwrap_or_else(|_| Err(AppError::ServiceUnavailable("HID worker failed".into()))),
        None => Err(AppError::ServiceUnavailable(
            "HID backend not available".into(),
        )),
    };
    if let Err(error) = &result {
        warn!("HID event processing failed: {}", error);
    }
    if let Some(reply) = reply {
        let response = result
            .as_ref()
            .map(|_| ())
            .map_err(|e| AppError::ServiceUnavailable(e.to_string()));
        let _ = reply.send(response);
    }
    result
}

async fn send_mouse_to_backend(backend: &dyn HidBackend, event: MouseEvent) -> Result<()> {
    if event.event_type != MouseEventType::Move {
        return backend.send_mouse(event).await;
    }

    // Split relative deltas to signed 8-bit reports without losing motion.
    // Bound work for arbitrary callers.
    let mut x = event.x.clamp(-32767, 32767);
    let mut y = event.y.clamp(-32767, 32767);
    loop {
        let dx = x.clamp(-127, 127);
        let dy = y.clamp(-127, 127);
        backend
            .send_mouse(MouseEvent {
                x: dx,
                y: dy,
                ..event.clone()
            })
            .await?;
        x -= dx;
        y -= dy;
        if x == 0 && y == 0 {
            return Ok(());
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

#[cfg(test)]
mod queue_tests {
    use super::*;

    #[tokio::test]
    async fn large_relative_motion_is_split_at_the_backend() {
        let backend = test_support::RecordingBackend::new();
        send_mouse_to_backend(backend.as_ref(), MouseEvent::move_rel(300, -260))
            .await
            .unwrap();
        let events = backend.events.lock();
        let deltas: Vec<_> = events.iter().map(|event| (event.x, event.y)).collect();
        assert_eq!(deltas, [(127, -127), (127, -127), (46, -6)]);
    }

    #[tokio::test]
    async fn backend_split_is_bounded_and_preserves_non_relative_events() {
        let backend = test_support::RecordingBackend::new();
        send_mouse_to_backend(backend.as_ref(), MouseEvent::move_rel(i32::MAX, i32::MIN))
            .await
            .unwrap();
        {
            let events = backend.events.lock();
            assert_eq!(events.len(), 259);
            assert_eq!(events.iter().map(|event| event.x).sum::<i32>(), 32767);
            assert_eq!(events.iter().map(|event| event.y).sum::<i32>(), -32767);
        }
        backend.events.lock().clear();
        send_mouse_to_backend(backend.as_ref(), MouseEvent::move_abs(30000, 20000))
            .await
            .unwrap();
        send_mouse_to_backend(backend.as_ref(), MouseEvent::scroll(-2))
            .await
            .unwrap();
        let events = backend.events.lock();
        assert_eq!(events.len(), 2);
        assert_eq!((events[0].x, events[0].y), (30000, 20000));
        assert_eq!(events[0].event_type, MouseEventType::MoveAbs);
        assert_eq!(events[1].scroll, -2);
    }

    #[tokio::test]
    async fn congested_queue_rejects_motion_until_recovery() {
        #[cfg(unix)]
        let controller = HidController::new(HidBackendType::None, None);
        #[cfg(not(unix))]
        let controller = HidController::new(HidBackendType::None);
        let backend = test_support::RecordingBackend::new();
        *controller.backend.write().await = Some(backend);
        controller.sync_runtime_state_from_backend().await;
        for _ in 0..HID_EVENT_QUEUE_CAPACITY {
            controller
                .send_mouse(MouseEvent::move_rel(0, 0))
                .await
                .unwrap();
        }
        assert!(controller
            .send_mouse(MouseEvent::move_rel(100, -20))
            .await
            .is_err());
        assert!(controller
            .send_mouse(MouseEvent::move_rel(80, 30))
            .await
            .is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn composite_otg_updates_and_rollback_preserve_hid_instance() {
        let mut old = crate::config::AppConfig::default();
        old.hid.backend = crate::config::HidBackend::Ch9329;
        old.enforce_invariants();
        let otg = Arc::new(OtgService::new());
        let controller = Arc::new(HidController::new(
            HidBackendType::from(&old.hid),
            Some(otg.clone()),
        ));
        let pressed = Arc::new(AtomicBool::new(true));
        let (runtime, _) = tokio::sync::watch::channel(());
        let backend: Arc<dyn HidBackend> = Arc::new(TestBackend {
            pressed: pressed.clone(),
            reset_done: Arc::new(tokio::sync::Notify::new()),
            runtime,
        });
        *controller.backend.write().await = Some(backend.clone());
        let data = tempfile::tempdir().unwrap();
        let coordinator = crate::runtime::UsbCoordinator::new(
            controller.clone(),
            otg,
            Arc::new(RwLock::new(None)),
            Arc::new(RwLock::new(None)),
            Arc::new(EventBus::new()),
            data.path().into(),
        );
        let mut new = old.clone();
        new.hid.backend = crate::config::HidBackend::Ch9329Otg;
        new.hid.otg_udc = Some("test.udc".into());
        new.hid.otg_descriptor.product = "Advanced USB".into();
        new.hid.otg_keyboard_leds = true;
        coordinator.apply_config(&old, &new).await.unwrap();
        assert!(Arc::ptr_eq(
            controller.backend.read().await.as_ref().unwrap(),
            &backend
        ));
        coordinator.apply_config(&new, &old).await.unwrap();
        assert!(Arc::ptr_eq(
            controller.backend.read().await.as_ref().unwrap(),
            &backend
        ));
        assert!(pressed.load(Ordering::Acquire));
        // Invalid OTG configuration must not touch the healthy input backend.
        new.uac.enabled = true;
        new.uac.sample_rate = 8000;
        assert!(coordinator.apply_config(&old, &new).await.is_err());
        assert!(Arc::ptr_eq(
            controller.backend.read().await.as_ref().unwrap(),
            &backend
        ));
    }

    #[tokio::test]
    async fn screen_resolution_updates_backend_and_retains_valid_dimensions() {
        #[cfg(unix)]
        let controller = HidController::new(HidBackendType::None, None);
        #[cfg(not(unix))]
        let controller = HidController::new(HidBackendType::None);
        let backend = Arc::new(ch9329::Ch9329Backend::new("/dev/null").unwrap());
        *controller.backend.write().await = Some(backend.clone());
        controller.set_screen_resolution(3840, 2160).await;
        assert_eq!(
            backend.runtime_snapshot().screen_resolution,
            Some((3840, 2160))
        );
        controller.set_screen_resolution(0, 0).await;
        assert_eq!(*controller.screen_resolution.read(), (3840, 2160));
        controller.set_screen_resolution(1280, 720).await;
        assert_eq!(
            backend.runtime_snapshot().screen_resolution,
            Some((1280, 720))
        );
    }
    struct TestBackend {
        pressed: Arc<AtomicBool>,
        reset_done: Arc<tokio::sync::Notify>,
        runtime: tokio::sync::watch::Sender<()>,
    }
    #[async_trait::async_trait]
    impl HidBackend for TestBackend {
        async fn init(&self) -> Result<()> {
            Ok(())
        }
        async fn send_keyboard(&self, _: KeyboardEvent) -> Result<()> {
            self.pressed.store(true, Ordering::Release);
            Ok(())
        }
        async fn send_mouse(&self, _: MouseEvent) -> Result<()> {
            Ok(())
        }
        async fn reset(&self) -> Result<()> {
            self.pressed.store(false, Ordering::Release);
            self.reset_done.notify_one();
            Ok(())
        }
        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
        fn runtime_snapshot(&self) -> HidBackendRuntimeSnapshot {
            HidBackendRuntimeSnapshot::default()
        }
        fn subscribe_runtime(&self) -> tokio::sync::watch::Receiver<()> {
            self.runtime.subscribe()
        }
    }
    #[tokio::test]
    async fn congested_queue_releases_host_instead_of_replaying_keydowns() {
        #[cfg(unix)]
        let controller = HidController::new(HidBackendType::None, None);
        #[cfg(not(unix))]
        let controller = HidController::new(HidBackendType::None);
        let pressed = Arc::new(AtomicBool::new(true));
        let done = Arc::new(tokio::sync::Notify::new());
        let (runtime, _) = tokio::sync::watch::channel(());
        *controller.backend.write().await = Some(Arc::new(TestBackend {
            pressed: pressed.clone(),
            reset_done: done.clone(),
            runtime,
        }));
        for _ in 0..HID_EVENT_QUEUE_CAPACITY {
            controller
                .enqueue_event(QueuedHidEvent::Keyboard(KeyboardEvent::key_down(
                    CanonicalKey::KeyA,
                    KeyboardModifiers::default(),
                )))
                .await
                .unwrap();
        }
        assert!(controller
            .enqueue_event(QueuedHidEvent::Reset)
            .await
            .is_err());
        controller.start_event_worker().await;
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
        assert!(!pressed.load(Ordering::Acquire));
    }
}
