use crate::{
    error::{AppError, Result},
    state::AppState,
};
use axum::{extract::State, http::header, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
pub struct McpConfigResponse {
    pub enabled: bool,
    pub requests_per_second: u32,
    pub api_key_configured: bool,
    pub endpoint: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfigUpdate {
    pub enabled: Option<bool>,
    pub requests_per_second: Option<u32>,
    #[serde(default)]
    pub regenerate_api_key: bool,
}
fn response(state: &AppState) -> McpConfigResponse {
    let config = state.config.get();
    McpConfigResponse {
        enabled: config.mcp.enabled,
        requests_per_second: config.mcp.requests_per_second,
        api_key_configured: config.mcp.api_key_hash.is_some(),
        endpoint: "/mcp",
        api_key: config.mcp.api_key.clone(),
    }
}
pub async fn mcp_config(
    State(state): State<Arc<AppState>>,
) -> (
    [(header::HeaderName, &'static str); 1],
    Json<McpConfigResponse>,
) {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(response(&state)),
    )
}
pub async fn mcp_update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<McpConfigUpdate>,
) -> Result<(
    [(header::HeaderName, &'static str); 1],
    Json<McpConfigResponse>,
)> {
    if req
        .requests_per_second
        .is_some_and(|rate| !(1..=100).contains(&rate))
    {
        return Err(AppError::BadRequest(
            "requests_per_second must be between 1 and 100".into(),
        ));
    }
    state
        .config
        .update(|config| {
            if let Some(enabled) = req.enabled {
                config.mcp.enabled = enabled;
            }
            if let Some(rate) = req.requests_per_second {
                config.mcp.requests_per_second = rate;
            }
            if req.regenerate_api_key || (config.mcp.enabled && config.mcp.api_key_hash.is_none()) {
                let key = crate::mcp::generate_key();
                config.mcp.api_key_hash = Some(crate::mcp::hash_key(&key));
                config.mcp.api_key = Some(key);
            }
        })
        .await?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(response(&state)),
    ))
}
