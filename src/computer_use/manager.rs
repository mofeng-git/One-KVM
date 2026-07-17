use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::{broadcast, oneshot, watch, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::actions::*;
use super::openai::{normalize_data_url, OpenAiComputerProvider};
use crate::config::ConfigStore;
use crate::error::{AppError, Result};
use crate::hid::{
    CanonicalKey, HidController, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton,
    MouseEvent,
};

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(10);
const KEY_DELAY: Duration = Duration::from_millis(35);
const ACTION_DELAY: Duration = Duration::from_millis(120);
const STOPPED_MESSAGE: &str = "Computer use task was stopped";

#[derive(Clone)]
pub struct ComputerUseManager {
    config: ConfigStore,
    hid: Arc<HidController>,
    state: Arc<Mutex<ManagerState>>,
    event_tx: broadcast::Sender<ComputerUseWsServerMessage>,
    screenshot_tx: broadcast::Sender<ScreenshotRequest>,
}

struct ManagerState {
    session: ComputerUseSessionSummary,
    conversation: Vec<ComputerUseConversationMessage>,
    screenshot_waiter: Option<ScreenshotWaiter>,
    stop_tx: Option<oneshot::Sender<()>>,
    cancel_tx: Option<watch::Sender<bool>>,
    task: Option<JoinHandle<()>>,
}

struct ScreenshotWaiter {
    request_id: String,
    client_id: String,
    tx: oneshot::Sender<Result<ComputerUseScreenshot>>,
}

#[derive(Debug, Clone)]
struct ScreenshotRequest {
    request_id: String,
    client_id: String,
}

impl ComputerUseManager {
    pub fn new(config: ConfigStore, hid: Arc<HidController>) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(128);
        let (screenshot_tx, _) = broadcast::channel(8);
        Arc::new(Self {
            config,
            hid,
            state: Arc::new(Mutex::new(ManagerState {
                session: empty_session(),
                conversation: Vec::new(),
                screenshot_waiter: None,
                stop_tx: None,
                cancel_tx: None,
                task: None,
            })),
            event_tx,
            screenshot_tx,
        })
    }

    pub fn config_response(&self) -> ComputerUseConfigResponse {
        let config = self.config.get();
        let key_env = cua_api_key_env();
        let key_db = config
            .computer_use
            .api_key
            .as_ref()
            .filter(|key| !key.is_empty());
        ComputerUseConfigResponse {
            enabled: config.computer_use.enabled,
            base_url: cua_base_url_env().unwrap_or_else(|| config.computer_use.base_url.clone()),
            model: config.computer_use.model.clone(),
            api_key_configured: key_env.is_some() || key_db.is_some(),
            api_key_source: if key_env.is_some() {
                "env".to_string()
            } else if key_db.is_some() {
                "config".to_string()
            } else {
                "none".to_string()
            },
        }
    }

    pub async fn update_config(
        &self,
        req: ComputerUseConfigUpdate,
    ) -> Result<ComputerUseConfigResponse> {
        if let Some(base_url) = req
            .base_url
            .as_ref()
            .filter(|base_url| !base_url.trim().is_empty())
        {
            validate_endpoint_url(base_url)?;
        }

        self.config
            .update(|config| {
                if let Some(enabled) = req.enabled {
                    config.computer_use.enabled = enabled;
                }
                if let Some(model) = req.model.as_ref().filter(|model| !model.trim().is_empty()) {
                    config.computer_use.model = model.trim().to_string();
                }
                if let Some(base_url) = req
                    .base_url
                    .as_ref()
                    .filter(|base_url| !base_url.trim().is_empty())
                {
                    config.computer_use.base_url = base_url.trim().to_string();
                }
                if req.clear_api_key.unwrap_or(false) {
                    config.computer_use.api_key = None;
                }
                if let Some(key) = req.api_key.as_ref() {
                    config.computer_use.api_key = if key.trim().is_empty() {
                        None
                    } else {
                        Some(key.trim().to_string())
                    };
                }
            })
            .await?;

        Ok(self.config_response())
    }

    pub async fn summary(&self) -> ComputerUseSessionSummary {
        self.state.lock().await.session.clone()
    }

    pub async fn start(
        self: &Arc<Self>,
        req: ComputerUseStartRequest,
    ) -> Result<ComputerUseSessionSummary> {
        let app_config = self.config.get();
        let config = app_config.computer_use.clone();
        if !config.enabled {
            return Err(AppError::BadRequest("Computer use is disabled".to_string()));
        }
        if req.prompt.trim().is_empty() {
            return Err(AppError::BadRequest("Task prompt is required".to_string()));
        }
        let client_id = req.client_id.trim();
        if client_id.is_empty() {
            return Err(AppError::BadRequest(
                "Computer use client_id is required".to_string(),
            ));
        }
        let client_id = client_id.to_string();
        let hid = self.hid.snapshot().await;
        if !hid.initialized || !hid.supports_absolute_mouse {
            return Err(AppError::BadRequest(
                "Computer use requires an initialized absolute mouse HID backend".to_string(),
            ));
        }

        let api_key = cua_api_key_env()
            .or(config.api_key.clone())
            .ok_or_else(|| {
                AppError::BadRequest("Computer Use API key is not configured".to_string())
            })?;
        let base_url = cua_base_url_env().unwrap_or_else(|| config.base_url.clone());
        validate_endpoint_url(&base_url)?;

        let mut state = self.state.lock().await;
        if matches!(
            state.session.status,
            ComputerUseSessionStatus::WaitingScreenshot
                | ComputerUseSessionStatus::Thinking
                | ComputerUseSessionStatus::Executing
        ) {
            return Err(AppError::BadRequest(
                "A computer use session is already running".to_string(),
            ));
        }

        if let Some(handle) = state.task.take() {
            handle.abort();
        }
        if !req.continue_conversation {
            state.conversation.clear();
        }
        let conversation = state.conversation.clone();
        state
            .conversation
            .push(ComputerUseConversationMessage::User {
                text: req.prompt.trim().to_string(),
            });

        let (stop_tx, stop_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let session_id = Uuid::new_v4().to_string();
        state.session = ComputerUseSessionSummary {
            id: Some(session_id),
            status: ComputerUseSessionStatus::Thinking,
            prompt: Some(req.prompt.trim().to_string()),
            step: 0,
            last_error: None,
            final_message: None,
        };
        state.stop_tx = Some(stop_tx);
        state.cancel_tx = Some(cancel_tx);
        let summary = state.session.clone();
        drop(state);

        self.publish_session().await;
        let manager = self.clone();
        let prompt = req.prompt.trim().to_string();
        let model = config.model.clone();
        let handle = tokio::spawn(async move {
            manager
                .run_loop(
                    prompt,
                    api_key,
                    base_url,
                    model,
                    conversation,
                    client_id,
                    cancel_rx,
                    stop_rx,
                )
                .await;
        });

        self.state.lock().await.task = Some(handle);
        Ok(summary)
    }

    pub async fn stop(&self) -> Result<ComputerUseSessionSummary> {
        let mut state = self.state.lock().await;
        if let Some(tx) = state.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = state.cancel_tx.take() {
            let _ = tx.send(true);
        }
        if let Some(waiter) = state.screenshot_waiter.take() {
            drop(waiter.tx);
        }
        state.session.status = ComputerUseSessionStatus::Stopped;
        drop(state);
        let _ = self.hid.reset().await;
        self.publish_session().await;
        Ok(self.summary().await)
    }

    pub async fn submit_screenshot(
        &self,
        client_id: &str,
        request_id: String,
        mut screenshot: ComputerUseScreenshot,
    ) -> Result<()> {
        if screenshot.width == 0 || screenshot.height == 0 {
            return Err(AppError::BadRequest(
                "Screenshot dimensions are invalid".to_string(),
            ));
        }
        screenshot.data_url = normalize_data_url(&screenshot.data_url)?;

        let mut state = self.state.lock().await;
        let Some(waiter) = state.screenshot_waiter.take() else {
            return Ok(());
        };
        if waiter.request_id != request_id || waiter.client_id != client_id {
            state.screenshot_waiter = Some(waiter);
            return Ok(());
        }
        let _ = waiter.tx.send(Ok(screenshot));
        Ok(())
    }

    async fn submit_screenshot_error(&self, client_id: &str, request_id: String, message: String) {
        let mut state = self.state.lock().await;
        let Some(waiter) = state.screenshot_waiter.take() else {
            return;
        };
        if waiter.request_id != request_id || waiter.client_id != client_id {
            state.screenshot_waiter = Some(waiter);
            return;
        }
        let message: String = message.chars().take(300).collect();
        let _ = waiter.tx.send(Err(AppError::ServiceUnavailable(format!(
            "Screenshot capture failed: {}",
            if message.trim().is_empty() {
                "client did not provide an error"
            } else {
                message.trim()
            }
        ))));
    }

    pub async fn handle_socket(self: Arc<Self>, socket: WebSocket, client_id: Option<String>) {
        let (mut sender, mut receiver) = socket.split();
        let mut event_rx = self.event_tx.subscribe();
        let client_id = client_id
            .as_deref()
            .map(str::trim)
            .filter(|client_id| !client_id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let mut screenshot_rx = self.screenshot_tx.subscribe();

        let _ = sender
            .send(Message::Text(
                serde_json::to_string(&ComputerUseWsServerMessage::SessionUpdated {
                    session: self.summary().await,
                })
                .unwrap_or_default()
                .into(),
            ))
            .await;

        loop {
            tokio::select! {
                Ok(event) = event_rx.recv() => {
                    if let Ok(text) = serde_json::to_string(&event) {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Ok(req) = screenshot_rx.recv() => {
                    if req.client_id != client_id {
                        continue;
                    }
                    let event = ComputerUseWsServerMessage::ScreenshotRequested { request_id: req.request_id };
                    if let Ok(text) = serde_json::to_string(&event) {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                }
                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<ComputerUseWsClientMessage>(&text) {
                                Ok(ComputerUseWsClientMessage::ScreenshotResult { request_id, screenshot }) => {
                                    let _ = self.submit_screenshot(&client_id, request_id, screenshot).await;
                                }
                                Ok(ComputerUseWsClientMessage::ScreenshotError { request_id, message }) => {
                                    self.submit_screenshot_error(&client_id, request_id, message).await;
                                }
                                Err(_) => {}
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(_)) => break,
                        _ => {}
                    }
                }
            }
        }
    }

    async fn run_loop(
        &self,
        prompt: String,
        api_key: String,
        base_url: String,
        model: String,
        conversation: Vec<ComputerUseConversationMessage>,
        client_id: String,
        cancel_rx: watch::Receiver<bool>,
        mut stop_rx: oneshot::Receiver<()>,
    ) {
        let provider = OpenAiComputerProvider::new(api_key, base_url, model);
        let mut latest_screenshot: Option<ComputerUseScreenshot> = None;
        let mut action_history: Vec<String> = Vec::new();
        let mut step = 0_u32;

        loop {
            step = step.saturating_add(1);
            self.set_status(ComputerUseSessionStatus::Thinking, step, None)
                .await;
            let response = tokio::select! {
                _ = &mut stop_rx => {
                    let _ = self.event_tx.send(ComputerUseWsServerMessage::ReasoningCompleted {
                        failed: true,
                    });
                    self.set_stopped().await;
                    return;
                }
                response = provider.next_actions(
                    &prompt,
                    &conversation,
                    &action_history,
                    latest_screenshot.as_ref(),
                    |delta| {
                        let _ = self.event_tx.send(ComputerUseWsServerMessage::ReasoningDelta {
                            delta: delta.to_string(),
                        });
                    },
                ) => response,
            };

            let response = match response {
                Ok(response) => {
                    let _ = self
                        .event_tx
                        .send(ComputerUseWsServerMessage::ReasoningCompleted { failed: false });
                    response
                }
                Err(err) => {
                    let _ = self
                        .event_tx
                        .send(ComputerUseWsServerMessage::ReasoningCompleted { failed: true });
                    self.fail(&err.to_string()).await;
                    return;
                }
            };

            if *cancel_rx.borrow() {
                self.set_stopped().await;
                return;
            }

            if response.done {
                self.complete(response.message).await;
                return;
            }

            let executable = &response.actions[..response.actions.len().saturating_sub(1)];
            action_history.push(format!(
                "Step {step}: {}",
                serde_json::to_string(&response.actions).unwrap_or_else(|_| "[]".to_string())
            ));
            if !executable.is_empty() {
                let Some(screenshot) = latest_screenshot.as_ref() else {
                    self.fail("Computer Use protocol error: actions require a screenshot")
                        .await;
                    return;
                };
                self.set_status(ComputerUseSessionStatus::Executing, step, None)
                    .await;
                if let Err(err) = self
                    .execute_actions(
                        executable,
                        screenshot.width,
                        screenshot.height,
                        cancel_rx.clone(),
                    )
                    .await
                {
                    if *cancel_rx.borrow() {
                        self.set_stopped().await;
                    } else {
                        self.fail(&err.to_string()).await;
                    }
                    return;
                }
                let _ = self
                    .event_tx
                    .send(ComputerUseWsServerMessage::ActionsExecuted {
                        actions: executable.to_vec(),
                    });
            }

            self.set_status(ComputerUseSessionStatus::WaitingScreenshot, step, None)
                .await;
            let screenshot = tokio::select! {
                _ = &mut stop_rx => {
                    self.set_stopped().await;
                    return;
                }
                screenshot = self.request_screenshot(&client_id) => screenshot,
            };
            let screenshot = match screenshot {
                Ok(screenshot) => screenshot,
                Err(err) => {
                    self.fail(&err.to_string()).await;
                    return;
                }
            };
            let _ = self
                .event_tx
                .send(ComputerUseWsServerMessage::ScreenshotCaptured {
                    screenshot: screenshot.clone(),
                });
            latest_screenshot = Some(screenshot);
        }
    }

    async fn request_screenshot(&self, client_id: &str) -> Result<ComputerUseScreenshot> {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut state = self.state.lock().await;
            state.screenshot_waiter = Some(ScreenshotWaiter {
                request_id: request_id.clone(),
                client_id: client_id.to_string(),
                tx,
            });
        }
        let _ = self.screenshot_tx.send(ScreenshotRequest {
            request_id,
            client_id: client_id.to_string(),
        });
        let reply = tokio::time::timeout(SCREENSHOT_TIMEOUT, rx)
            .await
            .map_err(|_| {
                AppError::ServiceUnavailable("Timed out waiting for screenshot".to_string())
            })?
            .map_err(|_| {
                AppError::ServiceUnavailable("Screenshot request was cancelled".to_string())
            })?;
        reply
    }

    async fn execute_actions(
        &self,
        actions: &[ComputerUseAction],
        width: u32,
        height: u32,
        mut cancel_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        for action in actions {
            if *cancel_rx.borrow() {
                return Err(stopped_error());
            }
            match action {
                ComputerUseAction::Click { x, y, button } => {
                    self.move_abs(*x, *y, width, height).await?;
                    self.mouse_button(*button, true).await?;
                    let click_result = sleep_or_cancel(KEY_DELAY, &mut cancel_rx).await;
                    self.mouse_button(*button, false).await?;
                    click_result?;
                }
                ComputerUseAction::DoubleClick { x, y, button } => {
                    for _ in 0..2 {
                        self.move_abs(*x, *y, width, height).await?;
                        self.mouse_button(*button, true).await?;
                        let click_result = sleep_or_cancel(KEY_DELAY, &mut cancel_rx).await;
                        self.mouse_button(*button, false).await?;
                        click_result?;
                        sleep_or_cancel(KEY_DELAY, &mut cancel_rx).await?;
                    }
                }
                ComputerUseAction::Move { x, y } => self.move_abs(*x, *y, width, height).await?,
                ComputerUseAction::Drag { path, button } => {
                    if let Some(first) = path.first() {
                        self.move_abs(first.x, first.y, width, height).await?;
                        self.mouse_button(*button, true).await?;
                        let drag_result = async {
                            for point in path.iter().skip(1) {
                                sleep_or_cancel(KEY_DELAY, &mut cancel_rx).await?;
                                self.move_abs(point.x, point.y, width, height).await?;
                            }
                            Result::<()>::Ok(())
                        }
                        .await;
                        self.mouse_button(*button, false).await?;
                        drag_result?;
                    }
                }
                ComputerUseAction::Scroll { x, y, dy, .. } => {
                    self.move_abs(*x, *y, width, height).await?;
                    let ticks = ((*dy).clamp(-1200, 1200) / 120).clamp(-10, 10);
                    let ticks = if ticks == 0 { dy.signum() } else { ticks };
                    for _ in 0..ticks.abs() {
                        if *cancel_rx.borrow() {
                            return Err(stopped_error());
                        }
                        self.hid
                            .send_mouse(MouseEvent::scroll(if ticks > 0 { 1 } else { -1 }))
                            .await?;
                    }
                }
                ComputerUseAction::Type { text } => self.type_text(text, &mut cancel_rx).await?,
                ComputerUseAction::Keypress { keys } => self.keypress(keys, &mut cancel_rx).await?,
                ComputerUseAction::Wait { ms } => {
                    sleep_or_cancel(Duration::from_millis((*ms).min(5000)), &mut cancel_rx).await?
                }
                ComputerUseAction::Screenshot => {}
            }
            sleep_or_cancel(ACTION_DELAY, &mut cancel_rx).await?;
        }
        Ok(())
    }

    async fn move_abs(&self, x: u32, y: u32, width: u32, height: u32) -> Result<()> {
        let hid_x = ((x.min(width.saturating_sub(1)) as f64 / width.max(1) as f64) * 32767.0)
            .round() as i32;
        let hid_y = ((y.min(height.saturating_sub(1)) as f64 / height.max(1) as f64) * 32767.0)
            .round() as i32;
        self.hid
            .send_mouse(MouseEvent::move_abs(hid_x, hid_y))
            .await
    }

    async fn mouse_button(&self, button: ComputerUseButton, down: bool) -> Result<()> {
        let button = match button {
            ComputerUseButton::Left => MouseButton::Left,
            ComputerUseButton::Middle => MouseButton::Middle,
            ComputerUseButton::Right => MouseButton::Right,
        };
        let event = if down {
            MouseEvent::button_down(button)
        } else {
            MouseEvent::button_up(button)
        };
        self.hid.send_mouse(event).await
    }

    async fn type_text(&self, text: &str, cancel_rx: &mut watch::Receiver<bool>) -> Result<()> {
        for ch in text.chars() {
            if *cancel_rx.borrow() {
                return Err(stopped_error());
            }
            let (key, mods) = char_to_key(ch).ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Cannot type unsupported character {ch:?} through HID keyboard mapping"
                ))
            })?;
            self.key_down_up(key, mods, cancel_rx).await?;
        }
        Ok(())
    }

    async fn keypress(&self, keys: &[String], cancel_rx: &mut watch::Receiver<bool>) -> Result<()> {
        let mut mods = KeyboardModifiers::default();
        let mut key = None;
        for item in keys {
            match item.to_lowercase().as_str() {
                "ctrl" | "control" | "controlleft" => mods.left_ctrl = true,
                "shift" | "shiftleft" => mods.left_shift = true,
                "alt" | "altleft" => mods.left_alt = true,
                "meta" | "win" | "cmd" | "super" => mods.left_meta = true,
                other => key = key_name_to_canonical(other),
            }
        }
        if let Some(key) = key {
            self.key_down_up(key, mods, cancel_rx).await?;
        }
        Ok(())
    }

    async fn key_down_up(
        &self,
        key: CanonicalKey,
        mods: KeyboardModifiers,
        cancel_rx: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        self.hid
            .send_keyboard(KeyboardEvent {
                event_type: KeyEventType::Down,
                key,
                modifiers: mods,
            })
            .await?;
        let key_result = sleep_or_cancel(KEY_DELAY, cancel_rx).await;
        self.hid
            .send_keyboard(KeyboardEvent {
                event_type: KeyEventType::Up,
                key,
                modifiers: KeyboardModifiers::default(),
            })
            .await?;
        key_result
    }

    async fn publish_session(&self) {
        let _ = self
            .event_tx
            .send(ComputerUseWsServerMessage::SessionUpdated {
                session: self.summary().await,
            });
    }

    async fn set_status(&self, status: ComputerUseSessionStatus, step: u32, error: Option<String>) {
        {
            let mut state = self.state.lock().await;
            state.session.status = status;
            state.session.step = step;
            state.session.last_error = error;
        }
        if matches!(status, ComputerUseSessionStatus::Thinking) {
            let _ = self
                .event_tx
                .send(ComputerUseWsServerMessage::StepStarted { step });
        }
        self.publish_session().await;
    }

    async fn complete(&self, message: Option<String>) {
        {
            let mut state = self.state.lock().await;
            if let Some(message) = message.as_ref().filter(|message| !message.is_empty()) {
                state
                    .conversation
                    .push(ComputerUseConversationMessage::Assistant {
                        text: message.clone(),
                    });
            }
            state.session.status = ComputerUseSessionStatus::Completed;
            state.session.final_message = message;
            state.stop_tx = None;
        }
        self.publish_session().await;
        let _ = self.hid.reset().await;
    }

    async fn fail(&self, message: &str) {
        {
            let mut state = self.state.lock().await;
            state.session.status = ComputerUseSessionStatus::Failed;
            state.session.last_error = Some(message.to_string());
            state.stop_tx = None;
        }
        let _ = self.event_tx.send(ComputerUseWsServerMessage::Error {
            message: message.to_string(),
        });
        self.publish_session().await;
        let _ = self.hid.reset().await;
    }

    async fn set_stopped(&self) {
        {
            let mut state = self.state.lock().await;
            state.session.status = ComputerUseSessionStatus::Stopped;
            state.stop_tx = None;
        }
        self.publish_session().await;
        let _ = self.hid.reset().await;
    }
}

async fn sleep_or_cancel(duration: Duration, cancel_rx: &mut watch::Receiver<bool>) -> Result<()> {
    if *cancel_rx.borrow() {
        return Err(stopped_error());
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => Ok(()),
        changed = cancel_rx.changed() => {
            match changed {
                Ok(()) if *cancel_rx.borrow() => {
                    Err(stopped_error())
                }
                Ok(()) => Ok(()),
                Err(_) => Err(stopped_error()),
            }
        }
    }
}

fn stopped_error() -> AppError {
    AppError::BadRequest(STOPPED_MESSAGE.to_string())
}

fn empty_session() -> ComputerUseSessionSummary {
    ComputerUseSessionSummary {
        id: None,
        status: ComputerUseSessionStatus::Idle,
        prompt: None,
        step: 0,
        last_error: None,
        final_message: None,
    }
}

fn cua_api_key_env() -> Option<String> {
    std::env::var("ONE_KVM_CUA_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| {
            std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|key| !key.trim().is_empty())
        })
}

fn cua_base_url_env() -> Option<String> {
    std::env::var("ONE_KVM_CUA_BASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .or_else(|| {
            std::env::var("ONE_KVM_OPENAI_BASE_URL")
                .ok()
                .filter(|url| !url.trim().is_empty())
        })
}

fn validate_endpoint_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(AppError::BadRequest(
            "API URL must be a complete http(s) endpoint".to_string(),
        ));
    }
    if trimmed.ends_with('/') {
        return Err(AppError::BadRequest(
            "API URL must include the full endpoint path without a trailing slash".to_string(),
        ));
    }
    if !trimmed.contains("/responses") && !trimmed.contains("/chat/completions") {
        return Err(AppError::BadRequest(
            "API URL must include /responses or /chat/completions".to_string(),
        ));
    }
    Ok(())
}

fn char_to_key(ch: char) -> Option<(CanonicalKey, KeyboardModifiers)> {
    let mut mods = KeyboardModifiers::default();
    let key = match ch {
        'a'..='z' => key_name_to_canonical(&ch.to_string())?,
        'A'..='Z' => {
            mods.left_shift = true;
            key_name_to_canonical(&ch.to_ascii_lowercase().to_string())?
        }
        '0' => CanonicalKey::Digit0,
        '1' => CanonicalKey::Digit1,
        '2' => CanonicalKey::Digit2,
        '3' => CanonicalKey::Digit3,
        '4' => CanonicalKey::Digit4,
        '5' => CanonicalKey::Digit5,
        '6' => CanonicalKey::Digit6,
        '7' => CanonicalKey::Digit7,
        '8' => CanonicalKey::Digit8,
        '9' => CanonicalKey::Digit9,
        ' ' => CanonicalKey::Space,
        '\n' => CanonicalKey::Enter,
        '-' => CanonicalKey::Minus,
        '_' => {
            mods.left_shift = true;
            CanonicalKey::Minus
        }
        '=' => CanonicalKey::Equal,
        '+' => {
            mods.left_shift = true;
            CanonicalKey::Equal
        }
        '.' => CanonicalKey::Period,
        ',' => CanonicalKey::Comma,
        '/' => CanonicalKey::Slash,
        '?' => {
            mods.left_shift = true;
            CanonicalKey::Slash
        }
        ';' => CanonicalKey::Semicolon,
        ':' => {
            mods.left_shift = true;
            CanonicalKey::Semicolon
        }
        '\'' => CanonicalKey::Quote,
        '"' => {
            mods.left_shift = true;
            CanonicalKey::Quote
        }
        '[' => CanonicalKey::BracketLeft,
        '{' => {
            mods.left_shift = true;
            CanonicalKey::BracketLeft
        }
        ']' => CanonicalKey::BracketRight,
        '}' => {
            mods.left_shift = true;
            CanonicalKey::BracketRight
        }
        '\\' => CanonicalKey::Backslash,
        '|' => {
            mods.left_shift = true;
            CanonicalKey::Backslash
        }
        '`' => CanonicalKey::Backquote,
        '~' => {
            mods.left_shift = true;
            CanonicalKey::Backquote
        }
        '!' => {
            mods.left_shift = true;
            CanonicalKey::Digit1
        }
        '@' => {
            mods.left_shift = true;
            CanonicalKey::Digit2
        }
        '#' => {
            mods.left_shift = true;
            CanonicalKey::Digit3
        }
        '$' => {
            mods.left_shift = true;
            CanonicalKey::Digit4
        }
        '%' => {
            mods.left_shift = true;
            CanonicalKey::Digit5
        }
        '^' => {
            mods.left_shift = true;
            CanonicalKey::Digit6
        }
        '&' => {
            mods.left_shift = true;
            CanonicalKey::Digit7
        }
        '*' => {
            mods.left_shift = true;
            CanonicalKey::Digit8
        }
        '(' => {
            mods.left_shift = true;
            CanonicalKey::Digit9
        }
        ')' => {
            mods.left_shift = true;
            CanonicalKey::Digit0
        }
        _ => return None,
    };
    Some((key, mods))
}

fn key_name_to_canonical(name: &str) -> Option<CanonicalKey> {
    match name.trim().to_lowercase().as_str() {
        "a" => Some(CanonicalKey::KeyA),
        "b" => Some(CanonicalKey::KeyB),
        "c" => Some(CanonicalKey::KeyC),
        "d" => Some(CanonicalKey::KeyD),
        "e" => Some(CanonicalKey::KeyE),
        "f" => Some(CanonicalKey::KeyF),
        "g" => Some(CanonicalKey::KeyG),
        "h" => Some(CanonicalKey::KeyH),
        "i" => Some(CanonicalKey::KeyI),
        "j" => Some(CanonicalKey::KeyJ),
        "k" => Some(CanonicalKey::KeyK),
        "l" => Some(CanonicalKey::KeyL),
        "m" => Some(CanonicalKey::KeyM),
        "n" => Some(CanonicalKey::KeyN),
        "o" => Some(CanonicalKey::KeyO),
        "p" => Some(CanonicalKey::KeyP),
        "q" => Some(CanonicalKey::KeyQ),
        "r" => Some(CanonicalKey::KeyR),
        "s" => Some(CanonicalKey::KeyS),
        "t" => Some(CanonicalKey::KeyT),
        "u" => Some(CanonicalKey::KeyU),
        "v" => Some(CanonicalKey::KeyV),
        "w" => Some(CanonicalKey::KeyW),
        "x" => Some(CanonicalKey::KeyX),
        "y" => Some(CanonicalKey::KeyY),
        "z" => Some(CanonicalKey::KeyZ),
        "enter" | "return" => Some(CanonicalKey::Enter),
        "escape" | "esc" => Some(CanonicalKey::Escape),
        "backspace" => Some(CanonicalKey::Backspace),
        "tab" => Some(CanonicalKey::Tab),
        "space" => Some(CanonicalKey::Space),
        "delete" | "del" => Some(CanonicalKey::Delete),
        "arrowup" | "up" => Some(CanonicalKey::ArrowUp),
        "arrowdown" | "down" => Some(CanonicalKey::ArrowDown),
        "arrowleft" | "left" => Some(CanonicalKey::ArrowLeft),
        "arrowright" | "right" => Some(CanonicalKey::ArrowRight),
        "home" => Some(CanonicalKey::Home),
        "end" => Some(CanonicalKey::End),
        "pageup" => Some(CanonicalKey::PageUp),
        "pagedown" => Some(CanonicalKey::PageDown),
        "f1" => Some(CanonicalKey::F1),
        "f2" => Some(CanonicalKey::F2),
        "f3" => Some(CanonicalKey::F3),
        "f4" => Some(CanonicalKey::F4),
        "f5" => Some(CanonicalKey::F5),
        "f6" => Some(CanonicalKey::F6),
        "f7" => Some(CanonicalKey::F7),
        "f8" => Some(CanonicalKey::F8),
        "f9" => Some(CanonicalKey::F9),
        "f10" => Some(CanonicalKey::F10),
        "f11" => Some(CanonicalKey::F11),
        "f12" => Some(CanonicalKey::F12),
        _ => None,
    }
}
