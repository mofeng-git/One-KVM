use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseSessionStatus {
    Idle,
    WaitingScreenshot,
    Thinking,
    Executing,
    Completed,
    Failed,
    Stopped,
}

#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseButton {
    Left,
    Middle,
    Right,
}

impl Default for ComputerUseButton {
    fn default() -> Self {
        Self::Left
    }
}

// Kept out of typeshare because these internally tagged enums are consumed by
// manually maintained frontend types and changing serde shape would break WS/API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComputerUseAction {
    Click {
        x: u32,
        y: u32,
        #[serde(default)]
        button: ComputerUseButton,
    },
    DoubleClick {
        x: u32,
        y: u32,
        #[serde(default)]
        button: ComputerUseButton,
    },
    Move {
        x: u32,
        y: u32,
    },
    Drag {
        path: Vec<ComputerUsePoint>,
        #[serde(default)]
        button: ComputerUseButton,
    },
    Scroll {
        x: u32,
        y: u32,
        #[serde(default)]
        dx: i32,
        #[serde(default)]
        dy: i32,
    },
    Type {
        text: String,
    },
    Keypress {
        keys: Vec<String>,
    },
    Wait {
        ms: u64,
    },
    Screenshot,
}

#[typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ComputerUsePoint {
    pub x: u32,
    pub y: u32,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseScreenshot {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ComputerUseConversationMessage {
    User { text: String },
    Assistant { text: String },
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseStartRequest {
    pub prompt: String,
    #[serde(default)]
    pub continue_conversation: bool,
    pub client_id: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseConfigResponse {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_configured: bool,
    pub api_key_source: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseConfigUpdate {
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    #[serde(alias = "openai_api_key")]
    pub api_key: Option<String>,
    #[serde(alias = "clear_openai_api_key")]
    pub clear_api_key: Option<bool>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseSessionSummary {
    pub id: Option<String>,
    pub status: ComputerUseSessionStatus,
    pub prompt: Option<String>,
    pub step: u32,
    pub last_error: Option<String>,
    pub final_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComputerUseWsClientMessage {
    ScreenshotResult {
        request_id: String,
        screenshot: ComputerUseScreenshot,
    },
    ScreenshotError {
        request_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComputerUseWsServerMessage {
    SessionUpdated { session: ComputerUseSessionSummary },
    ScreenshotRequested { request_id: String },
    ScreenshotCaptured { screenshot: ComputerUseScreenshot },
    StepStarted { step: u32 },
    ReasoningDelta { delta: String },
    ReasoningCompleted { failed: bool },
    ActionsExecuted { actions: Vec<ComputerUseAction> },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn computer_use_action_keeps_flat_tagged_json_shape() {
        let action = ComputerUseAction::Click {
            x: 10,
            y: 20,
            button: ComputerUseButton::Left,
        };

        assert_eq!(
            serde_json::to_value(action).unwrap(),
            json!({
                "type": "click",
                "x": 10,
                "y": 20,
                "button": "left"
            })
        );
    }

    #[test]
    fn computer_use_ws_message_keeps_flat_tagged_json_shape() {
        let message = ComputerUseWsServerMessage::ScreenshotRequested {
            request_id: "req-1".to_string(),
        };

        assert_eq!(
            serde_json::to_value(message).unwrap(),
            json!({
                "type": "screenshot_requested",
                "request_id": "req-1"
            })
        );
    }

    #[test]
    fn config_update_accepts_legacy_api_key_names() {
        let update: ComputerUseConfigUpdate = serde_json::from_value(json!({
            "openai_api_key": "legacy-key",
            "clear_openai_api_key": true
        }))
        .unwrap();

        assert_eq!(update.api_key.as_deref(), Some("legacy-key"));
        assert_eq!(update.clear_api_key, Some(true));
    }
}
