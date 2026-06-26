use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ComputerUseConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    #[typeshare(skip)]
    pub openai_api_key: Option<String>,
    pub max_steps: u32,
    pub timeout_seconds: u32,
}

impl Default for ComputerUseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai".to_string(),
            base_url: "https://api.openai.com/v1/responses".to_string(),
            model: "gpt-5.5".to_string(),
            openai_api_key: None,
            max_steps: 30,
            timeout_seconds: 600,
        }
    }
}
