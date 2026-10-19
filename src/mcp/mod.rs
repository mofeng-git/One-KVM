//! Native MCP service: one FIFO and one shared API key.
mod actions;
mod executor;
mod hardware;
mod keyboard;
mod tools;
pub use executor::McpService;
pub(crate) use hardware::McpHardware;

use crate::config::ConfigStore;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) fn hash_key(key: &str) -> String {
    Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub(crate) fn generate_key() -> String {
    format!(
        "okvm_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn key_matches(key: &str, expected: Option<&str>) -> bool {
    expected.is_some_and(|expected| {
        expected.len() == 64 && openssl::memcmp::eq(hash_key(key).as_bytes(), expected.as_bytes())
    })
}
fn valid_origin(request: &Request) -> bool {
    let Some(origin) = request.headers().get(header::ORIGIN) else {
        return true;
    };
    let Some(origin) = origin.to_str().ok().and_then(|s| s.parse::<Uri>().ok()) else {
        return false;
    };
    matches!(origin.scheme_str(), Some("http" | "https"))
        && origin.authority().is_some_and(|authority| {
            request
                .headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|host| host.eq_ignore_ascii_case(authority.as_str()))
        })
        && origin.path_and_query().is_none_or(|p| p.as_str() == "/")
}
async fn authorize(State(config): State<ConfigStore>, request: Request, next: Next) -> Response {
    if !valid_origin(&request) {
        return (StatusCode::FORBIDDEN, "Origin not allowed").into_response();
    }
    let config = config.get();
    if !config.mcp.enabled {
        return (StatusCode::SERVICE_UNAVAILABLE, "MCP is disabled").into_response();
    }
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    if !supplied.is_some_and(|key| key_matches(key, config.mcp.api_key_hash.as_deref())) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "Invalid MCP API key",
        )
            .into_response();
    }
    drop(config);
    next.run(request).await
}
pub fn router<S: Clone + Send + Sync + 'static>(
    service: Arc<McpService>,
    config: ConfigStore,
) -> Router<S> {
    let http = StreamableHttpService::new(
        move || Ok(tools::McpTools::new(service.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            // One-KVM is served on device IPs and user-provided hostnames.
            // Shared-key authentication and same-origin checks run above the SDK.
            .disable_allowed_hosts()
            .with_legacy_session_mode(false)
            .with_json_response(true),
    );
    Router::new()
        .route_service("/mcp", http)
        .layer(middleware::from_fn_with_state(config, authorize))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(64 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_key_and_origin_validation() {
        let key = generate_key();
        let hash = hash_key(&key);
        assert!(key_matches(&key, Some(&hash)));
        assert!(!key_matches("wrong", Some(&hash)));
        assert!(!key_matches(&key, None));
        assert!(!key_matches(&key, Some("malformed")));
        let build = |origin: &str| {
            Request::builder()
                .header("host", "kvm.local:8080")
                .header("origin", origin)
                .body(axum::body::Body::empty())
                .unwrap()
        };
        assert!(valid_origin(&build("http://kvm.local:8080")));
        assert!(!valid_origin(&build("https://attacker.example")));
        assert!(!valid_origin(&build("null")));
        assert!(!valid_origin(&build("http://kvm.local:8080/extra")));
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::{
        db::DatabasePool,
        hid::{HidBackendType, HidController},
        video::{streamer::Streamer, VideoStreamManager},
        webrtc::WebRtcStreamer,
    };
    use serde_json::{json, Value};

    #[tokio::test]
    async fn http_auth_discovery_native_schema_and_shared_rate_limit() {
        let dir = tempfile::tempdir().unwrap();
        let db = DatabasePool::new(&dir.path().join("mcp-http.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let config = ConfigStore::new(db.clone_pool());
        config.load().await.unwrap();
        #[cfg(unix)]
        let hid = Arc::new(HidController::new(HidBackendType::None, None));
        #[cfg(not(unix))]
        let hid = Arc::new(HidController::new(HidBackendType::None));
        let video = VideoStreamManager::with_webrtc_streamer(
            Streamer::new(),
            WebRtcStreamer::with_config(Default::default()),
        );
        config
            .update(|c| c.msd.msd_dir = dir.path().join("msd").to_string_lossy().into())
            .await
            .unwrap();
        let hardware = McpHardware::for_test(config.clone(), &db);
        let atx = hardware.atx.clone();
        let service = McpService::new(config.clone(), hid, video, hardware);
        let app = router::<()>(service, config.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("http://{address}/mcp");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        assert_eq!(
            client.post(&url).send().await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let key = generate_key();
        config
            .update(|c| {
                c.mcp.enabled = true;
                c.mcp.api_key_hash = Some(hash_key(&key));
                c.mcp.requests_per_second = 100;
            })
            .await
            .unwrap();
        assert_eq!(
            client.post(&url).send().await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(&key)
                .header("Origin", "https://other.invalid")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        let rpc = |method: &str, params: Value| {
            client
                .post(&url)
                .bearer_auth(&key)
                .header("Host", "192.168.1.15:8080")
                .header("Origin", "http://192.168.1.15:8080")
                .header("Accept", "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2025-11-25")
                .json(&json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params}))
        };
        let init: Value = rpc("initialize", json!({"protocolVersion":"2025-11-25", "clientInfo":{"name":"mcp-test","version":"1"}, "capabilities":{}})).send().await.unwrap().json().await.unwrap();
        assert!(init.get("result").is_some(), "{init}");
        assert_eq!(init["result"]["serverInfo"]["name"], "one-kvm");
        assert_eq!(
            init["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        let list: Value = rpc("tools/list", json!({}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        let names = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&"one_kvm_screenshot"));
        assert!(names.contains(&"one_kvm_execute_actions"));
        assert!(names.contains(&"one_kvm_get_status"));
        for name in [
            "one_kvm_power",
            "one_kvm_wake_on_lan",
            "one_kvm_list_media",
            "one_kvm_mount_media",
            "one_kvm_unmount_media",
        ] {
            assert!(names.contains(&name));
        }
        let screenshot = tools
            .iter()
            .find(|t| t["name"] == "one_kvm_screenshot")
            .unwrap();
        assert!(screenshot["inputSchema"]["properties"]
            .get("max_width")
            .is_none());
        assert!(screenshot["inputSchema"]["properties"]
            .get("max_height")
            .is_none());
        assert_eq!(screenshot["inputSchema"]["additionalProperties"], false);
        let status: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_get_status", "arguments":{}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(status["result"]["isError"], false, "{status}");
        assert!(status["result"]["structuredContent"]["hid"]["initialized"] == false);
        assert_eq!(
            status["result"]["structuredContent"]["atx"]["power_status"],
            "unknown"
        );
        let power: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_power", "arguments":{"action":"press_short"}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(power["result"]["isError"], true);
        let media: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_list_media", "arguments":{}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        #[cfg(unix)]
        assert_eq!(media["result"]["structuredContent"]["images"], json!([]));
        #[cfg(not(unix))]
        assert_eq!(media["result"]["isError"], true);
        let invalid_id: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_mount_media", "arguments":{"image_id":"/etc/passwd"}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(
            invalid_id["result"]["structuredContent"]["error"],
            "invalid_arguments"
        );
        // A slow controller must retain its HTTP response while waiting in the FIFO.
        for _ in 0..6 {
            let guard = atx.write().await;
            let request = rpc(
                "tools/call",
                json!({"name":"one_kvm_get_status", "arguments":{}}),
            );
            let response = tokio::spawn(async move {
                request.send().await.unwrap().json::<Value>().await.unwrap()
            });
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            assert!(!response.is_finished());
            drop(guard);
            assert_eq!(response.await.unwrap()["result"]["isError"], false);
        }
        // Current MCP revision: per-request metadata, without a transport session.
        let modern: Value = client
            .post(&url)
            .bearer_auth(&key)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", "tools/call")
            .header("Mcp-Name", "one_kvm_get_status")
            .json(
                &json!({"jsonrpc":"2.0", "id":2, "method":"tools/call", "params":{
                    "name":"one_kvm_get_status", "arguments":{},
                    "_meta":{
                        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                        "io.modelcontextprotocol/clientInfo":{"name":"mcp-test", "version":"1"},
                        "io.modelcontextprotocol/clientCapabilities":{}
                    }
                }}),
            )
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(modern["result"]["isError"], false, "{modern}");
        let invalid: Value = rpc("tools/call", json!({"name":"one_kvm_execute_actions", "arguments":{"actions":[{"type":"type", "text":"中文"}]}})).send().await.unwrap().json().await.unwrap();
        assert_eq!(invalid["result"]["isError"], true, "{invalid}");
        let oversize = rpc("tools/list", json!({"padding":"x".repeat(70_000)}))
            .send()
            .await
            .unwrap();
        assert_eq!(oversize.status(), StatusCode::PAYLOAD_TOO_LARGE);
        // All independent HTTP connections use the same service bucket.
        config
            .update(|c| c.mcp.requests_per_second = 1)
            .await
            .unwrap();
        let a: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_get_status", "arguments":{}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        let b: Value = rpc(
            "tools/call",
            json!({"name":"one_kvm_list_media", "arguments":{}}),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
        assert!(
            a["result"]["structuredContent"]["error"] == "rate_limited"
                || b["result"]["structuredContent"]["error"] == "rate_limited"
        );
        let rotated = generate_key();
        config
            .update(|c| c.mcp.api_key_hash = Some(hash_key(&rotated)))
            .await
            .unwrap();
        assert_eq!(
            rpc("tools/list", json!({})).send().await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        server.abort();
    }
}
