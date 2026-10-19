use super::actions::{self, PreparedActions};
use super::hardware::{HardwareCommand, McpHardware};
use crate::{
    config::ConfigStore,
    hid::HidController,
    video::{snapshot::Snapshot, VideoStreamManager},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use parking_lot::Mutex;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

pub(super) enum Command {
    Status,
    Screenshot(Option<u8>),
    Actions(PreparedActions, bool),
    Hardware(HardwareCommand),
}
struct Job {
    command: Command,
    enqueued: Instant,
    reply: oneshot::Sender<CallToolResult>,
}

struct Bucket {
    tokens: f64,
    updated: Instant,
    rate: u32,
}
impl Bucket {
    fn take(&mut self, rate: u32, now: Instant) -> bool {
        let rate = rate.clamp(1, 100);
        self.tokens = (self.tokens
            + now.duration_since(self.updated).as_secs_f64() * self.rate as f64)
            .min(rate as f64);
        self.updated = now;
        self.rate = rate;
        if self.tokens < 1.0 {
            false
        } else {
            self.tokens -= 1.0;
            true
        }
    }
}

pub struct McpService {
    config: ConfigStore,
    tx: mpsc::Sender<Job>,
    bucket: Mutex<Bucket>,
}
impl McpService {
    pub(crate) fn new(
        config: ConfigStore,
        hid: Arc<HidController>,
        video: Arc<VideoStreamManager>,
        hardware: McpHardware,
    ) -> Arc<Self> {
        let (tx, mut rx) = mpsc::channel::<Job>(8);
        let rate = config.get().mcp.requests_per_second.clamp(1, 100);
        let service = Arc::new(Self {
            config,
            tx,
            bucket: Mutex::new(Bucket {
                tokens: rate as f64,
                updated: Instant::now(),
                rate,
            }),
        });
        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                // Accepted jobs are independent of the caller's transport lifetime.
                let result = if job.enqueued.elapsed() > Duration::from_secs(30) {
                    failure(
                        "queue_timeout",
                        "Request expired while waiting; no action executed",
                        0,
                    )
                } else {
                    match job.command {
                        Command::Status => {
                            let h = hid.snapshot().await;
                            let v = video.stats().await;
                            let devices = hardware.status().await;
                            output(
                                json!({ "version": env!("CARGO_PKG_VERSION"), "video": {"state": v.state, "resolution": v.resolution, "format": v.format}, "hid": {"initialized": h.initialized, "online": h.online, "backend": h.backend, "supports_absolute_mouse": h.supports_absolute_mouse}, "atx": devices["atx"], "media": devices["media"], "queue_length": rx.len() }),
                                false,
                            )
                        }
                        Command::Screenshot(quality) => match video.capture_snapshot(quality).await
                        {
                            Ok(snapshot) => {
                                with_snapshot(output(json!({"success": true}), false), snapshot)
                            }
                            Err(error) => failure("screenshot_failed", &error.to_string(), 0),
                        },
                        Command::Actions(prepared, screenshot) => {
                            run_actions(&hid, &video, prepared, screenshot).await
                        }
                        Command::Hardware(command) => hardware.execute(command).await,
                    }
                };
                let _ = job.reply.send(result);
            }
        });
        service
    }
    pub(super) async fn call(&self, command: Command) -> CallToolResult {
        let (reply, rx) = oneshot::channel();
        {
            // One lock defines admission order and a single service-wide quota.
            let mut bucket = self.bucket.lock();
            if !bucket.take(self.config.get().mcp.requests_per_second, Instant::now()) {
                return failure(
                    "rate_limited",
                    "Global MCP rate limit exceeded; retry after 1 second",
                    0,
                );
            }
            if self
                .tx
                .try_send(Job {
                    command,
                    enqueued: Instant::now(),
                    reply,
                })
                .is_err()
            {
                return failure("busy", "MCP queue is full; retry later", 0);
            }
        }
        rx.await
            .unwrap_or_else(|_| failure("unavailable", "MCP worker unavailable", 0))
    }
}

async fn run_actions(
    hid: &HidController,
    video: &VideoStreamManager,
    prepared: PreparedActions,
    screenshot: bool,
) -> CallToolResult {
    let state = hid.snapshot().await;
    if !state.initialized || (prepared.needs_absolute_mouse && !state.supports_absolute_mouse) {
        return failure("hid_unavailable", "Actions require an initialized HID backend; pointer actions require absolute mouse support", 0);
    }
    let mut completed = 0;
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        for action in prepared.actions {
            actions::execute(hid, action).await?;
            completed += 1;
        }
        Ok::<(), crate::AppError>(())
    })
    .await;
    // Successful actions already release their keys/buttons. A backend reset
    // also moves an absolute pointer to (0,0), so reserve it for failed or
    // timed-out batches that may have left input held.
    let cleanup = if matches!(&result, Ok(Ok(()))) {
        Ok(())
    } else {
        hid.reset_confirmed().await
    };
    let mut response = match result {
        Ok(Ok(())) => output(
            json!({"success": true, "completed_actions": completed}),
            false,
        ),
        Ok(Err(error)) => failure("execution_failed", &error.to_string(), completed),
        Err(_) => failure(
            "execution_timeout",
            "Execution time limit exceeded; last action may be partially executed",
            completed,
        ),
    };
    if let Err(error) = cleanup {
        response.is_error = Some(true);
        if let Some(Value::Object(value)) = response.structured_content.as_mut() {
            value.insert("success".into(), json!(false));
            value.insert("cleanup_error".into(), json!(error.to_string()));
        }
        refresh_text(&mut response);
        return response;
    }
    if screenshot {
        match video.capture_snapshot(None).await {
            Ok(snapshot) => response = with_snapshot(response, snapshot),
            Err(error) => {
                if let Some(Value::Object(value)) = response.structured_content.as_mut() {
                    value.insert("screenshot_error".into(), json!(error.to_string()));
                }
                refresh_text(&mut response);
            }
        }
    }
    response
}

pub(super) fn output(value: Value, failed: bool) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    result.is_error = Some(failed);
    result
}
pub(super) fn failure(code: &str, message: &str, completed: usize) -> CallToolResult {
    output(
        json!({"success": false, "error": code, "message": message, "completed_actions": completed, "failed_action_index": completed}),
        true,
    )
}
fn refresh_text(result: &mut CallToolResult) {
    if let Some(value) = &result.structured_content {
        result.content[0] = ContentBlock::text(value.to_string());
    }
}
fn with_snapshot(mut result: CallToolResult, snapshot: Snapshot) -> CallToolResult {
    if let Some(Value::Object(value)) = result.structured_content.as_mut() {
        value.insert(
            "screenshot".into(),
            json!({"width": snapshot.width, "height": snapshot.height, "age_ms": snapshot.age_ms}),
        );
    }
    refresh_text(&mut result);
    result.content.push(ContentBlock::image(
        STANDARD.encode(snapshot.jpeg),
        "image/jpeg",
    ));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_bucket_has_bounded_burst_and_recovers() {
        let now = Instant::now();
        let mut bucket = Bucket {
            tokens: 2.0,
            updated: now,
            rate: 2,
        };
        assert!(bucket.take(2, now));
        assert!(bucket.take(2, now));
        assert!(!bucket.take(2, now));
        assert!(bucket.take(2, now + Duration::from_millis(500)));
        assert!(!bucket.take(2, now + Duration::from_millis(500)));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::hid::{
        backend::{HidBackend, HidBackendRuntimeSnapshot},
        CanonicalKey, KeyEventType, KeyboardEvent, MouseEvent,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::{watch, Notify};
    struct Backend {
        events: Mutex<Vec<String>>,
        entered: Notify,
        proceed: Notify,
        pressed: AtomicBool,
        runtime: watch::Sender<()>,
    }
    #[async_trait::async_trait]
    impl HidBackend for Backend {
        async fn init(&self) -> crate::Result<()> {
            Ok(())
        }
        async fn send_keyboard(&self, event: KeyboardEvent) -> crate::Result<()> {
            self.events
                .lock()
                .push(format!("{:?}:{:?}", event.key, event.event_type));
            self.pressed.store(
                matches!(event.event_type, KeyEventType::Down),
                Ordering::SeqCst,
            );
            if event.key == CanonicalKey::KeyA && matches!(event.event_type, KeyEventType::Down) {
                self.entered.notify_one();
                self.proceed.notified().await;
            }
            if event.key == CanonicalKey::KeyC && matches!(event.event_type, KeyEventType::Down) {
                return Err(crate::AppError::ServiceUnavailable(
                    "Simulated hardware failure".into(),
                ));
            }
            Ok(())
        }
        async fn send_mouse(&self, _: MouseEvent) -> crate::Result<()> {
            Ok(())
        }
        async fn reset(&self) -> crate::Result<()> {
            self.events.lock().push("reset".into());
            self.pressed.store(false, Ordering::SeqCst);
            Ok(())
        }
        async fn shutdown(&self) -> crate::Result<()> {
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
        fn subscribe_runtime(&self) -> watch::Receiver<()> {
            self.runtime.subscribe()
        }
    }
    #[tokio::test]
    async fn fifo_batches_survive_caller_disconnect_and_release_input() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::DatabasePool::new(&dir.path().join("mcp.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let config = ConfigStore::new(db.clone_pool());
        config.load().await.unwrap();
        config
            .update(|c| c.mcp.requests_per_second = 100)
            .await
            .unwrap();
        let backend = Arc::new(Backend {
            events: Mutex::new(Vec::new()),
            entered: Notify::new(),
            proceed: Notify::new(),
            pressed: AtomicBool::new(false),
            runtime: watch::channel(()).0,
        });
        let hid = crate::hid::test_controller(backend.clone()).await;
        let video = VideoStreamManager::with_webrtc_streamer(
            crate::video::streamer::Streamer::new(),
            crate::webrtc::WebRtcStreamer::with_config(Default::default()),
        );
        let hardware = McpHardware::for_test(config.clone(), &db);
        let service = McpService::new(config, hid.clone(), video.clone(), hardware);
        let call = |text: &str| {
            Command::Actions(
                actions::prepare(vec![actions::Action::Type { text: text.into() }]).unwrap(),
                false,
            )
        };
        let s = service.clone();
        let first = call("A");
        let caller = tokio::spawn(async move { s.call(first).await });
        tokio::time::timeout(Duration::from_secs(2), backend.entered.notified())
            .await
            .unwrap();
        assert!(backend.pressed.load(Ordering::SeqCst));
        caller.abort();
        let s = service.clone();
        let second = call("B");
        let second = tokio::spawn(async move { s.call(second).await });
        // Wait for admission, then fill the remaining bounded queue with status reads.
        tokio::time::timeout(Duration::from_secs(2), async {
            while service.tx.capacity() != 7 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut pending = Vec::new();
        for _ in 0..7 {
            let s = service.clone();
            pending.push(tokio::spawn(async move { s.call(Command::Status).await }));
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while service.tx.capacity() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let busy = service.call(Command::Status).await;
        assert_eq!(busy.structured_content.unwrap()["error"], "busy");
        backend.proceed.notify_one();
        let result = tokio::time::timeout(Duration::from_secs(3), second)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.structured_content.unwrap()["completed_actions"], 1);
        for job in pending {
            assert_eq!(job.await.unwrap().is_error, Some(false));
        }
        assert!(!backend.pressed.load(Ordering::SeqCst));
        assert_eq!(
            *backend.events.lock(),
            ["KeyA:Down", "KeyA:Up", "KeyB:Down", "KeyB:Up"]
        );
        let partial = service
            .call(Command::Actions(
                actions::prepare(vec![
                    actions::Action::Type { text: "B".into() },
                    actions::Action::Type { text: "C".into() },
                ])
                .unwrap(),
                false,
            ))
            .await;
        assert_eq!(partial.is_error, Some(true));
        let partial = partial.structured_content.unwrap();
        assert_eq!(partial["completed_actions"], 1);
        assert_eq!(partial["failed_action_index"], 1);
        assert!(!backend.pressed.load(Ordering::SeqCst));
        assert_eq!(backend.events.lock().last().unwrap(), "reset");
        // Hold a snapshot request so capture deterministically fails as busy,
        // independently of video hardware present on the build machine.
        let pending_snapshot = video.streamer().snapshot_slot.request().unwrap();
        let screenshot_failure = service
            .call(Command::Actions(
                actions::prepare(vec![actions::Action::Type { text: "B".into() }]).unwrap(),
                true,
            ))
            .await;
        assert_eq!(screenshot_failure.is_error, Some(false));
        let value = screenshot_failure.structured_content.unwrap();
        assert_eq!(value["success"], true);
        assert_eq!(value["completed_actions"], 1);
        assert!(value.get("screenshot_error").is_some());
        assert_eq!(backend.events.lock().last().unwrap(), "KeyB:Up");
        drop(pending_snapshot);
        hid.shutdown().await.unwrap();
    }
}
