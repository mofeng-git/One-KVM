use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComputerUseConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    #[typeshare(skip)]
    #[serde(alias = "openai_api_key")]
    pub api_key: Option<String>,
}

impl Default for ComputerUseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.openai.com/v1/responses".to_string(),
            model: "gpt-5.5".to_string(),
            api_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_openai_api_key_migrates_to_generic_key() {
        let config: ComputerUseConfig = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "provider": "openai",
            "base_url": "https://example.test/v1/chat/completions",
            "model": "vision-model",
            "openai_api_key": "legacy-key",
            "max_steps": 30,
            "timeout_seconds": 600
        }))
        .unwrap();

        assert_eq!(config.api_key.as_deref(), Some("legacy-key"));
        assert_eq!(config.model, "vision-model");
    }
}
