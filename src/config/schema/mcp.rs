use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub enabled: bool,
    pub requests_per_second: u32,
    #[typeshare(skip)]
    pub api_key: Option<String>,
    #[typeshare(skip)]
    pub api_key_hash: Option<String>,
}
impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_second: 5,
            api_key: None,
            api_key_hash: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mcp_defaults_are_disabled_and_have_no_model_credentials() {
        let config: super::super::AppConfig = serde_json::from_value(
            serde_json::json!({"computer_use":{"enabled":true,"api_key":"old-model-key"}}),
        )
        .unwrap();
        assert!(!config.mcp.enabled);
        assert!(config.mcp.api_key_hash.is_none());
        assert!(config.mcp.api_key.is_none());
        assert_eq!(config.mcp.requests_per_second, 5);
        assert!(serde_json::to_value(&config)
            .unwrap()
            .get("computer_use")
            .is_none());
    }
}
