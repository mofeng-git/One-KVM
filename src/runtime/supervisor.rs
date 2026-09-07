use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::ConfigStore;
use crate::events::EventBus;
use crate::extensions::ExtensionManager;
use crate::state::AppState;

pub(super) struct RuntimeSupervisor {
    tasks: Vec<JoinHandle<()>>,
}

impl RuntimeSupervisor {
    pub(super) fn start(
        state: Arc<AppState>,
        events: Arc<EventBus>,
        extensions: Arc<ExtensionManager>,
        config: ConfigStore,
    ) -> Self {
        let mut tasks = spawn_device_info_broadcaster(state, events);
        tasks.push(spawn_extension_health_check(extensions, config));
        Self { tasks }
    }

    pub(super) async fn shutdown(&mut self, state: &Arc<AppState>) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        cleanup(state).await;
    }
}

fn spawn_extension_health_check(
    extensions: Arc<ExtensionManager>,
    config: ConfigStore,
) -> JoinHandle<()> {
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let config = config.get();
            extensions.health_check(&config.extensions).await;
        }
    });
    tracing::info!("Extension health check task started");
    task
}

fn spawn_device_info_broadcaster(
    state: Arc<AppState>,
    events: Arc<EventBus>,
) -> Vec<JoinHandle<()>> {
    enum DeviceInfoTrigger {
        Event,
        Lagged { topic: &'static str, count: u64 },
    }

    const DEVICE_INFO_TOPICS: &[&str] = &[
        "stream.state_changed",
        "stream.config_applied",
        "stream.mode_ready",
    ];
    const DEBOUNCE_MS: u64 = 100;

    let (trigger_tx, mut trigger_rx) = mpsc::unbounded_channel();
    let mut tasks = Vec::new();

    for topic in DEVICE_INFO_TOPICS {
        let Some(mut rx) = events.subscribe_topic(topic) else {
            tracing::warn!(
                "DeviceInfo broadcaster missing topic subscription: {}",
                topic
            );
            continue;
        };

        let trigger_tx = trigger_tx.clone();
        let topic_name = *topic;
        tasks.push(tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(_) => {
                        if trigger_tx.send(DeviceInfoTrigger::Event).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        if trigger_tx
                            .send(DeviceInfoTrigger::Lagged {
                                topic: topic_name,
                                count,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }));
    }

    {
        let mut dirty_rx = events.subscribe_device_info_dirty();
        let trigger_tx = trigger_tx.clone();
        tasks.push(tokio::spawn(async move {
            loop {
                match dirty_rx.recv().await {
                    Ok(()) => {
                        if trigger_tx.send(DeviceInfoTrigger::Event).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        if trigger_tx
                            .send(DeviceInfoTrigger::Lagged {
                                topic: "device_info_dirty",
                                count,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }));
    }

    tasks.push(tokio::spawn(async move {
        let mut last_broadcast = Instant::now() - Duration::from_millis(DEBOUNCE_MS);
        let mut pending_broadcast = false;

        loop {
            let recv_result = if pending_broadcast {
                let remaining =
                    DEBOUNCE_MS.saturating_sub(last_broadcast.elapsed().as_millis() as u64);
                tokio::time::timeout(Duration::from_millis(remaining), trigger_rx.recv()).await
            } else {
                Ok(trigger_rx.recv().await)
            };

            match recv_result {
                Ok(Some(DeviceInfoTrigger::Event)) => {
                    pending_broadcast = true;
                }
                Ok(Some(DeviceInfoTrigger::Lagged { topic, count })) => {
                    tracing::warn!(
                        "DeviceInfo broadcaster lagged by {} events on topic {}",
                        count,
                        topic
                    );
                    pending_broadcast = true;
                }
                Ok(None) => {
                    tracing::info!("Event bus closed, stopping DeviceInfo broadcaster");
                    break;
                }
                Err(_timeout) => {}
            }

            if pending_broadcast && last_broadcast.elapsed() >= Duration::from_millis(DEBOUNCE_MS) {
                state.publish_device_info().await;
                tracing::trace!("Broadcasted DeviceInfo (debounced)");
                last_broadcast = Instant::now();
                pending_broadcast = false;
            }
        }
    }));

    tracing::info!(
        "DeviceInfo broadcaster task started (debounce: {}ms)",
        DEBOUNCE_MS
    );
    tasks
}

async fn cleanup(state: &Arc<AppState>) {
    state.extensions.stop_all().await;
    tracing::info!("Extensions stopped");

    state.remote_access.shutdown().await;

    if let Err(error) = state.stream_manager.stop().await {
        tracing::warn!("Failed to stop streamer: {}", error);
    }

    if let Err(error) = state.hid.shutdown().await {
        tracing::warn!("Failed to shutdown HID: {}", error);
    }

    #[cfg(unix)]
    {
        let msd = state.msd.write().await.take();
        if let Some(msd) = msd {
            if let Err(error) = msd.shutdown().await {
                tracing::warn!("Failed to shutdown MSD: {}", error);
            }
        }

        if let Err(error) = state.otg_service.shutdown().await {
            tracing::warn!("Failed to shutdown OTG: {}", error);
        }
    }

    let atx = state.atx.write().await.take();
    if let Some(atx) = atx {
        if let Err(error) = atx.shutdown().await {
            tracing::warn!("Failed to shutdown ATX: {}", error);
        }
    }

    if let Err(error) = state.audio.shutdown().await {
        tracing::warn!("Failed to shutdown audio: {}", error);
    }

    if let Err(error) = state.watchdog.disable().await {
        tracing::error!(
            "CRITICAL: failed to disable hardware watchdog during shutdown: {}",
            error
        );
    }
}
