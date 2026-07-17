use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

use super::actions::{
    ComputerUseAction, ComputerUseButton, ComputerUseConversationMessage, ComputerUsePoint,
    ComputerUseScreenshot,
};
use crate::error::{AppError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const REASONING_FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const ERROR_SNIPPET_LIMIT: usize = 800;

const COMPUTER_USE_SYSTEM_PROMPT: &str = r#"You control a physical remote computer through One-KVM, an IP-KVM system.
You have no DOM access, clipboard access, shell, or direct system APIs. You can only observe the remote host through screenshots and interact through these mouse and HID keyboard actions: click, double_click, move, drag, scroll, type, keypress, wait, and screenshot. click supports left, right, and middle buttons.

Return exactly one JSON object using this shape:
{"done":boolean,"message":string|null,"actions":[{"type":"click","x":0,"y":0,"button":"left"},{"type":"double_click","x":0,"y":0,"button":"left"},{"type":"move","x":0,"y":0},{"type":"drag","path":[{"x":0,"y":0}],"button":"left"},{"type":"scroll","x":0,"y":0,"dx":0,"dy":0},{"type":"type","text":"ASCII text"},{"type":"keypress","keys":["ctrl","l"]},{"type":"wait","ms":500},{"type":"screenshot"}]}

On the first turn there is no screenshot. Request it with done=false and actions=[{"type":"screenshot"}]. Coordinates must be based on the latest screenshot. When the interface may have changed, explicitly request a new screenshot. Every done=false action batch must contain exactly one screenshot action, and it must be last. Actions before it are executed in order, then One-KVM captures the new screenshot. When the task is complete, return done=true with no actions and put the final response in message.

Keyboard type sends printable US-keyboard ASCII only. Never put Chinese or other non-ASCII characters in type. Do not assume the remote input method state: inspect focus and language state before typing, switch it manually when needed, and request a screenshot to confirm. To enter Chinese, switch the remote host to a Chinese input method, type pinyin as ASCII, and choose candidates from visible feedback.

One-KVM does not add approval gates for reboot, shutdown, firmware, credentials, payments, or other sensitive operations. Follow the user's task directly; the model provider may still enforce its own policies."#;

pub struct OpenAiComputerProvider {
    client: reqwest::Client,
    api_key: String,
    endpoint_url: String,
    model: String,
}

#[derive(Debug)]
pub struct OpenAiComputerResponse {
    pub done: bool,
    pub actions: Vec<ComputerUseAction>,
    pub message: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndpointKind {
    Responses,
    ChatCompletions,
}

impl EndpointKind {
    fn label(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat/completions",
        }
    }
}

impl OpenAiComputerProvider {
    pub fn new(api_key: String, endpoint_url: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build Computer Use HTTP client");
        Self {
            client,
            api_key,
            endpoint_url,
            model,
        }
    }

    pub async fn next_actions<F>(
        &self,
        prompt: &str,
        conversation: &[ComputerUseConversationMessage],
        action_history: &[String],
        screenshot: Option<&ComputerUseScreenshot>,
        mut on_reasoning: F,
    ) -> Result<OpenAiComputerResponse>
    where
        F: FnMut(&str),
    {
        let kind = endpoint_kind(&self.endpoint_url)?;
        let request_text = request_context(prompt, conversation, action_history, screenshot);
        let body = match kind {
            EndpointKind::ChatCompletions => chat_body(&self.model, &request_text, screenshot),
            EndpointKind::Responses => responses_body(&self.model, &request_text, screenshot),
        };

        let response = self
            .client
            .post(self.endpoint_url.trim())
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|err| self.request_error(kind, &err.to_string()))?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        if !status.is_success() {
            let raw = response.bytes().await.map_err(|err| {
                self.response_error(
                    kind,
                    status.as_u16(),
                    &content_type,
                    None,
                    &err.to_string(),
                    "",
                )
            })?;
            let text = String::from_utf8_lossy(&raw);
            let provider_message = serde_json::from_slice::<Value>(&raw)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "provider request failed".to_string());
            return Err(self.response_error(
                kind,
                status.as_u16(),
                &content_type,
                None,
                &provider_message,
                &text,
            ));
        }

        let mut result = if content_type
            .to_ascii_lowercase()
            .contains("text/event-stream")
        {
            self.parse_stream(kind, response, &content_type, &mut on_reasoning)
                .await?
        } else {
            let raw = response.bytes().await.map_err(|err| {
                self.response_error(
                    kind,
                    status.as_u16(),
                    &content_type,
                    None,
                    &err.to_string(),
                    "",
                )
            })?;
            self.parse_json_response(kind, &raw, &content_type, &mut on_reasoning)?
        };

        validate_protocol(&result, screenshot.is_some())
            .map_err(|message| self.response_error(kind, 200, &content_type, None, &message, ""))?;
        if result.reasoning.as_deref() == Some("") {
            result.reasoning = None;
        }
        Ok(result)
    }

    async fn parse_stream<F>(
        &self,
        kind: EndpointKind,
        response: reqwest::Response,
        content_type: &str,
        on_reasoning: &mut F,
    ) -> Result<OpenAiComputerResponse>
    where
        F: FnMut(&str),
    {
        let mut stream = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut output = String::new();
        let mut reasoning = ReasoningCollector::new(on_reasoning);
        let mut saw_event = false;
        let mut last_event_type: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| {
                reasoning.flush();
                self.response_error(
                    kind,
                    200,
                    content_type,
                    last_event_type.as_deref(),
                    &format!("stream interrupted: {err}"),
                    "",
                )
            })?;
            for event in decoder.push(&chunk) {
                saw_event = true;
                if event.data.trim().is_empty() || event.data.trim() == "[DONE]" {
                    continue;
                }
                let parsed_event_type = serde_json::from_str::<Value>(&event.data)
                    .ok()
                    .and_then(|value| value.get("type")?.as_str().map(str::to_string));
                let event_type = event.event.clone().or(parsed_event_type);
                last_event_type = event_type.clone();
                if let Err(err) = self.consume_stream_event(
                    kind,
                    &event,
                    event_type.as_deref(),
                    &mut output,
                    &mut reasoning,
                    content_type,
                ) {
                    reasoning.flush();
                    return Err(err);
                }
            }
        }
        for event in decoder.finish() {
            if event.data.trim().is_empty() || event.data.trim() == "[DONE]" {
                continue;
            }
            saw_event = true;
            let parsed_event_type = serde_json::from_str::<Value>(&event.data)
                .ok()
                .and_then(|value| value.get("type")?.as_str().map(str::to_string));
            let event_type = event.event.clone().or(parsed_event_type);
            last_event_type = event_type.clone();
            if let Err(err) = self.consume_stream_event(
                kind,
                &event,
                event_type.as_deref(),
                &mut output,
                &mut reasoning,
                content_type,
            ) {
                reasoning.flush();
                return Err(err);
            }
        }
        reasoning.flush();

        if !saw_event {
            return Err(self.response_error(
                kind,
                200,
                content_type,
                last_event_type.as_deref(),
                "stream ended without SSE events",
                "",
            ));
        }
        let reasoning_text = reasoning.into_text();
        self.parse_protocol_output(
            kind,
            &output,
            reasoning_text,
            content_type,
            last_event_type.as_deref(),
        )
    }

    fn consume_stream_event<F>(
        &self,
        kind: EndpointKind,
        event: &SseEvent,
        event_type: Option<&str>,
        output: &mut String,
        reasoning: &mut ReasoningCollector<'_, F>,
        content_type: &str,
    ) -> Result<()>
    where
        F: FnMut(&str),
    {
        let value: Value = serde_json::from_str(&event.data).map_err(|err| {
            self.response_error(
                kind,
                200,
                content_type,
                event_type,
                &format!(
                    "SSE JSON was invalid at line {}, column {}",
                    err.line(),
                    err.column()
                ),
                &event.data,
            )
        })?;
        if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
            return Err(self.response_error(
                kind,
                200,
                content_type,
                event_type,
                message,
                &event.data,
            ));
        }

        match kind {
            EndpointKind::ChatCompletions => {
                if let Some(delta) = value
                    .pointer("/choices/0/delta/reasoning_content")
                    .and_then(Value::as_str)
                {
                    reasoning.push(delta);
                }
                if let Some(delta) = value
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                {
                    output.push_str(delta);
                }
            }
            EndpointKind::Responses => match event_type
                .or_else(|| value.get("type").and_then(Value::as_str))
                .unwrap_or_default()
            {
                "response.output_text.delta" => {
                    if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                        output.push_str(delta);
                    }
                }
                "response.reasoning_summary_text.delta" | "response.reasoning_summary.delta" => {
                    if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                        reasoning.push(delta);
                    }
                }
                "error" | "response.failed" => {
                    let message = value
                        .pointer("/response/error/message")
                        .or_else(|| value.pointer("/error/message"))
                        .and_then(Value::as_str)
                        .unwrap_or("Responses stream failed");
                    return Err(self.response_error(
                        kind,
                        200,
                        content_type,
                        event_type,
                        message,
                        &event.data,
                    ));
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn parse_json_response<F>(
        &self,
        kind: EndpointKind,
        raw: &[u8],
        content_type: &str,
        on_reasoning: &mut F,
    ) -> Result<OpenAiComputerResponse>
    where
        F: FnMut(&str),
    {
        let value: Value = serde_json::from_slice(raw).map_err(|err| {
            self.response_error(
                kind,
                200,
                content_type,
                None,
                &format!(
                    "response JSON was invalid at line {}, column {}",
                    err.line(),
                    err.column()
                ),
                &String::from_utf8_lossy(raw),
            )
        })?;
        let (output, reasoning) = match kind {
            EndpointKind::ChatCompletions => {
                let output = value
                    .pointer("/choices/0/message/content")
                    .and_then(chat_content_text)
                    .ok_or_else(|| {
                        self.response_error(
                            kind,
                            200,
                            content_type,
                            None,
                            "response had no message content",
                            &String::from_utf8_lossy(raw),
                        )
                    })?;
                let reasoning = value
                    .pointer("/choices/0/message/reasoning_content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                (output, reasoning)
            }
            EndpointKind::Responses => collect_responses_json(&value),
        };
        if !reasoning.is_empty() {
            on_reasoning(&reasoning);
        }
        self.parse_protocol_output(kind, &output, reasoning, content_type, None)
    }

    fn parse_protocol_output(
        &self,
        kind: EndpointKind,
        output: &str,
        reasoning: String,
        content_type: &str,
        event_type: Option<&str>,
    ) -> Result<OpenAiComputerResponse> {
        let parsed = parse_json_object_text(output).map_err(|message| {
            self.response_error(kind, 200, content_type, event_type, &message, output)
        })?;
        let done = parsed.get("done").and_then(Value::as_bool).ok_or_else(|| {
            self.response_error(
                kind,
                200,
                content_type,
                event_type,
                "action response field done was not a boolean",
                output,
            )
        })?;
        let actions = parse_actions_array(&parsed).map_err(|message| {
            self.response_error(kind, 200, content_type, event_type, &message, output)
        })?;
        let message = parsed
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .map(str::to_string);
        Ok(OpenAiComputerResponse {
            done,
            actions,
            message,
            reasoning: (!reasoning.is_empty()).then_some(reasoning),
        })
    }

    fn request_error(&self, kind: EndpointKind, detail: &str) -> AppError {
        AppError::ServiceUnavailable(format!(
            "Computer Use provider request failed: interface={}, model={}, endpoint={}, detail={}",
            kind.label(),
            self.model,
            redact_endpoint(&self.endpoint_url),
            sanitize(detail, &self.api_key),
        ))
    }

    fn response_error(
        &self,
        kind: EndpointKind,
        status: u16,
        content_type: &str,
        event_type: Option<&str>,
        detail: &str,
        snippet: &str,
    ) -> AppError {
        let snippet = sanitize(snippet, &self.api_key);
        AppError::ServiceUnavailable(format!(
            "Computer Use provider error: interface={}, model={}, endpoint={}, HTTP {}, content-type={}, event={}, detail={}, response={}",
            kind.label(),
            self.model,
            redact_endpoint(&self.endpoint_url),
            status,
            content_type,
            event_type.unwrap_or("none"),
            sanitize(detail, &self.api_key),
            if snippet.is_empty() { "(empty)" } else { &snippet },
        ))
    }
}

fn chat_body(model: &str, request_text: &str, screenshot: Option<&ComputerUseScreenshot>) -> Value {
    let user_content = if let Some(screenshot) = screenshot {
        json!([
            {"type": "text", "text": request_text},
            {"type": "image_url", "image_url": {"url": screenshot.data_url}}
        ])
    } else {
        json!(request_text)
    };
    json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": COMPUTER_USE_SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ]
    })
}

fn responses_body(
    model: &str,
    request_text: &str,
    screenshot: Option<&ComputerUseScreenshot>,
) -> Value {
    let mut content = vec![json!({"type": "input_text", "text": request_text})];
    if let Some(screenshot) = screenshot {
        content.push(json!({
            "type": "input_image",
            "image_url": screenshot.data_url,
            "detail": "original"
        }));
    }
    json!({
        "model": model,
        "stream": true,
        "instructions": COMPUTER_USE_SYSTEM_PROMPT,
        "reasoning": {"summary": "auto"},
        "input": [{"role": "user", "content": content}]
    })
}

fn request_context(
    prompt: &str,
    conversation: &[ComputerUseConversationMessage],
    action_history: &[String],
    screenshot: Option<&ComputerUseScreenshot>,
) -> String {
    let conversation = conversation_history_text(conversation);
    let actions = compact_action_history(action_history);
    let screen = screenshot
        .map(|image| {
            format!(
                "{}x{} (latest screenshot attached)",
                image.width, image.height
            )
        })
        .unwrap_or_else(|| "unavailable (request screenshot first)".to_string());
    format!(
        "Previous conversation:\n{}\n\nCurrent task:\n{}\n\nAction history:\n{}\n\nVisual context: {}\nReturn only the action JSON object.",
        if conversation.is_empty() { "(none)" } else { &conversation },
        prompt,
        if actions.is_empty() { "(none)" } else { &actions },
        screen,
    )
}

fn conversation_history_text(conversation: &[ComputerUseConversationMessage]) -> String {
    conversation
        .iter()
        .map(|message| match message {
            ComputerUseConversationMessage::User { text } => format!("User: {text}"),
            ComputerUseConversationMessage::Assistant { text } => format!("Assistant: {text}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compact_action_history(history: &[String]) -> String {
    const MAX_CHARS: usize = 12_000;
    let mut selected = Vec::new();
    let mut chars = 0;
    for item in history.iter().rev() {
        let size = item.chars().count() + 1;
        if chars + size > MAX_CHARS && !selected.is_empty() {
            break;
        }
        selected.push(item.as_str());
        chars += size;
    }
    selected.reverse();
    selected.join("\n")
}

fn endpoint_kind(url: &str) -> Result<EndpointKind> {
    let url = url.trim().to_ascii_lowercase();
    if url.contains("/chat/completions") {
        Ok(EndpointKind::ChatCompletions)
    } else if url.contains("/responses") {
        Ok(EndpointKind::Responses)
    } else {
        Err(AppError::BadRequest(
            "API URL must include /responses or /chat/completions".to_string(),
        ))
    }
}

fn chat_content_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    value.as_array().map(|parts| {
        parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn collect_responses_json(value: &Value) -> (String, String) {
    let mut output = String::new();
    let mut reasoning = String::new();
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        output.push_str(text);
    }
    for item in value
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("type").and_then(Value::as_str).unwrap_or_default() {
            "message" => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        output.push_str(text);
                    }
                }
            }
            "reasoning" => {
                for part in item
                    .get("summary")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        reasoning.push_str(text);
                    }
                }
            }
            _ => {}
        }
    }
    (output, reasoning)
}

fn parse_json_object_text(text: &str) -> std::result::Result<Value, String> {
    let trimmed = text.trim();
    let unwrapped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|text| text.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    let json_text = if unwrapped.starts_with('{') {
        unwrapped
    } else {
        let start = unwrapped
            .find('{')
            .ok_or_else(|| "action response was not a JSON object".to_string())?;
        let end = unwrapped
            .rfind('}')
            .ok_or_else(|| "action response was not a JSON object".to_string())?;
        &unwrapped[start..=end]
    };
    serde_json::from_str(json_text).map_err(|err| {
        format!(
            "action JSON was invalid at line {}, column {}: {}",
            err.line(),
            err.column(),
            err
        )
    })
}

fn validate_protocol(
    response: &OpenAiComputerResponse,
    has_screenshot: bool,
) -> std::result::Result<(), String> {
    if response.done {
        if !response.actions.is_empty() {
            return Err("done=true requires actions=[]".to_string());
        }
        return Ok(());
    }
    let screenshot_positions = response
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            matches!(action, ComputerUseAction::Screenshot).then_some(index)
        })
        .collect::<Vec<_>>();
    if screenshot_positions.len() != 1 || screenshot_positions[0] + 1 != response.actions.len() {
        return Err(
            "done=false requires exactly one screenshot action in the final position".to_string(),
        );
    }
    if !has_screenshot && response.actions.len() != 1 {
        return Err(
            "coordinate or keyboard actions are not allowed before the first screenshot"
                .to_string(),
        );
    }
    Ok(())
}

fn parse_actions_array(value: &Value) -> std::result::Result<Vec<ComputerUseAction>, String> {
    let actions = value
        .get("actions")
        .ok_or_else(|| "action response was missing actions".to_string())?
        .as_array()
        .ok_or_else(|| "action response field actions was not an array".to_string())?;
    actions.iter().map(parse_action).collect()
}

fn parse_action(value: &Value) -> std::result::Result<ComputerUseAction, String> {
    let action_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "action was missing type".to_string())?;
    let click_alias = match action_type {
        "left_click" => Some(ComputerUseButton::Left),
        "right_click" => Some(ComputerUseButton::Right),
        "middle_click" => Some(ComputerUseButton::Middle),
        _ => None,
    };
    if let Some(button) = click_alias {
        return Ok(ComputerUseAction::Click {
            x: required_u32(value, "x", action_type)?,
            y: required_u32(value, "y", action_type)?,
            button,
        });
    }
    match action_type {
        "click" => Ok(ComputerUseAction::Click {
            x: required_u32(value, "x", action_type)?,
            y: required_u32(value, "y", action_type)?,
            button: parse_button(value.get("button")),
        }),
        "double_click" | "doubleClick" => Ok(ComputerUseAction::DoubleClick {
            x: required_u32(value, "x", action_type)?,
            y: required_u32(value, "y", action_type)?,
            button: parse_button(value.get("button")),
        }),
        "move" | "move_mouse" => Ok(ComputerUseAction::Move {
            x: required_u32(value, "x", action_type)?,
            y: required_u32(value, "y", action_type)?,
        }),
        "drag" => {
            let path = value
                .get("path")
                .and_then(Value::as_array)
                .ok_or_else(|| "drag action was missing path array".to_string())?;
            let path = path
                .iter()
                .map(|point| {
                    Ok(ComputerUsePoint {
                        x: required_u32(point, "x", action_type)?,
                        y: required_u32(point, "y", action_type)?,
                    })
                })
                .collect::<std::result::Result<Vec<_>, String>>()?;
            if path.is_empty() {
                return Err("drag action had an empty path".to_string());
            }
            Ok(ComputerUseAction::Drag {
                path,
                button: parse_button(value.get("button")),
            })
        }
        "scroll" => Ok(ComputerUseAction::Scroll {
            x: required_u32(value, "x", action_type)?,
            y: required_u32(value, "y", action_type)?,
            dx: value_i32(value, "dx")
                .or_else(|| value_i32(value, "scroll_x"))
                .unwrap_or(0),
            dy: value_i32(value, "dy")
                .or_else(|| value_i32(value, "scroll_y"))
                .unwrap_or(0),
        }),
        "type" => {
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !text.is_ascii() {
                return Err(
                    "type action contains non-ASCII text; use the remote input method".to_string(),
                );
            }
            Ok(ComputerUseAction::Type {
                text: text.to_string(),
            })
        }
        "keypress" | "key_press" => Ok(ComputerUseAction::Keypress {
            keys: value
                .get("keys")
                .and_then(Value::as_array)
                .map(|keys| {
                    keys.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .or_else(|| {
                    value
                        .get("key")
                        .and_then(Value::as_str)
                        .map(|key| vec![key.to_string()])
                })
                .unwrap_or_default(),
        }),
        "wait" => Ok(ComputerUseAction::Wait {
            ms: value
                .get("ms")
                .or_else(|| value.get("duration"))
                .and_then(Value::as_u64)
                .unwrap_or(500),
        }),
        "screenshot" => Ok(ComputerUseAction::Screenshot),
        _ => Err(format!("unsupported computer action type: {action_type}")),
    }
}

fn parse_button(value: Option<&Value>) -> ComputerUseButton {
    match value.and_then(Value::as_str).unwrap_or("left") {
        "right" => ComputerUseButton::Right,
        "middle" => ComputerUseButton::Middle,
        _ => ComputerUseButton::Left,
    }
}

fn required_u32(value: &Value, key: &str, action_type: &str) -> std::result::Result<u32, String> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{action_type} action was missing numeric {key}"))?;
    u32::try_from(raw).map_err(|_| format!("{action_type} action field {key} was out of range"))
}

fn value_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

#[derive(Debug, Default)]
struct SseDecoder {
    buffer: Vec<u8>,
    event_name: Option<String>,
    data_lines: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct SseEvent {
    event: Option<String>,
    data: String,
}

impl SseDecoder {
    fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(position) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=position).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.consume_line(&String::from_utf8_lossy(&line), &mut events);
        }
        events
    }

    fn finish(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            self.consume_line(&String::from_utf8_lossy(&line), &mut events);
        }
        self.dispatch(&mut events);
        events
    }

    fn consume_line(&mut self, line: &str, events: &mut Vec<SseEvent>) {
        if line.is_empty() {
            self.dispatch(events);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event_name = Some(value.to_string()),
            "data" => self.data_lines.push(value.to_string()),
            _ => {}
        }
    }

    fn dispatch(&mut self, events: &mut Vec<SseEvent>) {
        if self.event_name.is_none() && self.data_lines.is_empty() {
            return;
        }
        events.push(SseEvent {
            event: self.event_name.take(),
            data: std::mem::take(&mut self.data_lines).join("\n"),
        });
    }
}

struct ReasoningCollector<'a, F: FnMut(&str)> {
    callback: &'a mut F,
    full: String,
    pending: String,
    last_flush: Instant,
}

impl<'a, F: FnMut(&str)> ReasoningCollector<'a, F> {
    fn new(callback: &'a mut F) -> Self {
        Self {
            callback,
            full: String::new(),
            pending: String::new(),
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, delta: &str) {
        self.full.push_str(delta);
        self.pending.push_str(delta);
        if self.last_flush.elapsed() >= REASONING_FLUSH_INTERVAL {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            (self.callback)(&self.pending);
            self.pending.clear();
        }
        self.last_flush = Instant::now();
    }

    fn into_text(self) -> String {
        self.full
    }
}

fn redact_endpoint(endpoint: &str) -> String {
    let endpoint = endpoint.split('?').next().unwrap_or(endpoint).trim();
    if let Ok(mut url) = reqwest::Url::parse(endpoint) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        return url.to_string().trim_end_matches('/').to_string();
    }
    endpoint.to_string()
}

fn sanitize(value: &str, api_key: &str) -> String {
    let mut output = if api_key.is_empty() {
        value.to_string()
    } else {
        value.replace(api_key, "[REDACTED_API_KEY]")
    };
    loop {
        let Some(start) = output.find("data:image/") else {
            break;
        };
        let end = output[start..]
            .find(|ch: char| ch == '"' || ch == '\'' || ch.is_whitespace())
            .map(|offset| start + offset)
            .unwrap_or(output.len());
        output.replace_range(start..end, "[REDACTED_IMAGE]");
    }
    output.chars().take(ERROR_SNIPPET_LIMIT).collect()
}

pub fn normalize_data_url(data_url: &str) -> Result<String> {
    if !data_url.starts_with("data:image/") {
        return Err(AppError::BadRequest(
            "Screenshot must be an image data URL".to_string(),
        ));
    }
    let Some((_, data)) = data_url.split_once(',') else {
        return Err(AppError::BadRequest(
            "Invalid screenshot data URL".to_string(),
        ));
    };
    STANDARD
        .decode(data)
        .map_err(|_| AppError::BadRequest("Screenshot is not valid base64".to_string()))?;
    Ok(data_url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider(endpoint: &str) -> OpenAiComputerProvider {
        let _ = rustls::crypto::ring::default_provider().install_default();
        OpenAiComputerProvider::new(
            "secret".to_string(),
            endpoint.to_string(),
            "model".to_string(),
        )
    }

    #[test]
    fn sse_decoder_handles_fragmentation_crlf_multiline_and_empty_events() {
        let input = b": keepalive\r\nevent: sample\r\ndata: {\"a\":\r\ndata: 1}\r\n\r\n\r\ndata: [DONE]\n\n";
        let mut decoder = SseDecoder::default();
        let mut events = Vec::new();
        for byte in input {
            events.extend(decoder.push(&[*byte]));
        }
        events.extend(decoder.finish());
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: Some("sample".to_string()),
                    data: "{\"a\":\n1}".to_string(),
                },
                SseEvent {
                    event: None,
                    data: "[DONE]".to_string(),
                }
            ]
        );
    }

    #[test]
    fn click_aliases_reuse_click_action() {
        for (name, expected) in [
            ("left_click", ComputerUseButton::Left),
            ("right_click", ComputerUseButton::Right),
            ("middle_click", ComputerUseButton::Middle),
        ] {
            let action = parse_action(&json!({"type": name, "x": 4, "y": 8})).unwrap();
            assert!(matches!(
                action,
                ComputerUseAction::Click { x: 4, y: 8, button } if button == expected
            ));
        }
    }

    #[test]
    fn protocol_requires_first_screenshot_and_terminal_screenshot() {
        let first = OpenAiComputerResponse {
            done: false,
            actions: vec![ComputerUseAction::Screenshot],
            message: None,
            reasoning: None,
        };
        assert!(validate_protocol(&first, false).is_ok());

        let invalid = OpenAiComputerResponse {
            done: false,
            actions: vec![
                ComputerUseAction::Click {
                    x: 1,
                    y: 2,
                    button: ComputerUseButton::Left,
                },
                ComputerUseAction::Screenshot,
            ],
            message: None,
            reasoning: None,
        };
        assert!(validate_protocol(&invalid, false).is_err());
        assert!(validate_protocol(&invalid, true).is_ok());

        let duplicate_screenshot = OpenAiComputerResponse {
            done: false,
            actions: vec![ComputerUseAction::Screenshot, ComputerUseAction::Screenshot],
            message: None,
            reasoning: None,
        };
        assert!(validate_protocol(&duplicate_screenshot, true).is_err());

        let done_with_actions = OpenAiComputerResponse {
            done: true,
            actions: vec![ComputerUseAction::Screenshot],
            message: Some("done".to_string()),
            reasoning: None,
        };
        assert!(validate_protocol(&done_with_actions, true).is_err());
    }

    #[test]
    fn invalid_json_reports_line_and_column() {
        let error = parse_json_object_text("{\n  \"done\": nope\n}").unwrap_err();
        assert!(error.contains("line 2, column"));
    }

    #[test]
    fn sanitization_removes_api_keys_and_images() {
        let value = sanitize(
            "key=secret image=data:image/png;base64,AAAA and more",
            "secret",
        );
        assert!(!value.contains("secret"));
        assert!(!value.contains("AAAA"));
    }

    #[test]
    fn chat_sse_collects_reasoning_and_action_json() {
        let provider = test_provider("https://example.test/v1/chat/completions");
        let mut output = String::new();
        let mut received_reasoning = String::new();
        let mut callback = |delta: &str| received_reasoning.push_str(delta);
        let mut reasoning = ReasoningCollector::new(&mut callback);
        let event = SseEvent {
            event: None,
            data: json!({
                "choices": [{"delta": {
                    "reasoning_content": "checking",
                    "content": "{\"done\":false,\"message\":null,\"actions\":[{\"type\":\"screenshot\"}]}"
                }}]
            })
            .to_string(),
        };
        provider
            .consume_stream_event(
                EndpointKind::ChatCompletions,
                &event,
                None,
                &mut output,
                &mut reasoning,
                "text/event-stream",
            )
            .unwrap();
        reasoning.flush();
        drop(reasoning);
        assert_eq!(received_reasoning, "checking");
        assert!(output.contains("\"screenshot\""));
    }

    #[test]
    fn responses_sse_collects_output_and_reasoning_summary() {
        let provider = test_provider("https://example.test/v1/responses");
        let mut output = String::new();
        let mut received_reasoning = String::new();
        let mut callback = |delta: &str| received_reasoning.push_str(delta);
        let mut reasoning = ReasoningCollector::new(&mut callback);
        for (event_type, delta) in [
            ("response.reasoning_summary_text.delta", "looking"),
            (
                "response.output_text.delta",
                "{\"done\":true,\"message\":\"done\",\"actions\":[]}",
            ),
        ] {
            let event = SseEvent {
                event: Some(event_type.to_string()),
                data: json!({"type": event_type, "delta": delta}).to_string(),
            };
            provider
                .consume_stream_event(
                    EndpointKind::Responses,
                    &event,
                    Some(event_type),
                    &mut output,
                    &mut reasoning,
                    "text/event-stream",
                )
                .unwrap();
        }
        reasoning.flush();
        drop(reasoning);
        assert_eq!(received_reasoning, "looking");
        assert!(output.contains("\"done\":true"));
    }

    #[test]
    fn ordinary_chat_json_is_a_streaming_fallback() {
        let provider = test_provider("https://example.test/v1/chat/completions");
        let raw = json!({
            "choices": [{"message": {
                "reasoning_content": "summary",
                "content": "{\"done\":true,\"message\":\"ok\",\"actions\":[]}"
            }}]
        })
        .to_string();
        let mut deltas = String::new();
        let response = provider
            .parse_json_response(
                EndpointKind::ChatCompletions,
                raw.as_bytes(),
                "application/json",
                &mut |delta| deltas.push_str(delta),
            )
            .unwrap();
        assert!(response.done);
        assert_eq!(response.message.as_deref(), Some("ok"));
        assert_eq!(deltas, "summary");
    }

    #[test]
    fn request_bodies_use_plain_text_and_optional_images_only() {
        let chat_first = chat_body("model", "task", None).to_string();
        let responses_first = responses_body("model", "task", None).to_string();
        for body in [&chat_first, &responses_first] {
            assert!(!body.contains("data:image/"));
            assert!(!body.contains("computer_call_output"));
            assert!(!body.contains("previous_response_id"));
            assert!(!body.contains("\"tools\""));
        }

        let screenshot = ComputerUseScreenshot {
            data_url: "data:image/png;base64,AAAA".to_string(),
            width: 1280,
            height: 720,
        };
        assert!(chat_body("model", "task", Some(&screenshot))
            .to_string()
            .contains("image_url"));
        assert!(responses_body("model", "task", Some(&screenshot))
            .to_string()
            .contains("input_image"));
    }
}
