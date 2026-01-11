//! Terminal proxy handler - reverse proxy to ttyd via Unix socket

use axum::{
    body::Body,
    extract::{
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
        OriginalUri, Path, State,
    },
    http::{Request, StatusCode},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest, http::HeaderValue, Message as TungsteniteMessage,
};

use crate::error::AppError;
use crate::extensions::TTYD_SOCKET_PATH;
use crate::state::AppState;

/// Handle WebSocket upgrade for terminal
pub async fn terminal_ws(
    State(_state): State<Arc<AppState>>,
    OriginalUri(original_uri): OriginalUri,
    ws: WebSocketUpgrade,
) -> Response {
    let query_string = original_uri
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    // Use the tty subprotocol that ttyd expects
    ws.protocols(["tty"])
        .on_upgrade(move |socket| handle_terminal_websocket(socket, query_string))
}

/// Handle terminal WebSocket connection - bridge browser and ttyd
async fn handle_terminal_websocket(client_ws: WebSocket, query_string: String) {
    // Connect to ttyd Unix socket
    let unix_stream = match UnixStream::connect(TTYD_SOCKET_PATH).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to connect to ttyd socket: {}", e);
            return;
        }
    };

    // Build WebSocket request for ttyd with tty subprotocol
    let uri_str = format!("ws://localhost/api/terminal/ws{}", query_string);
    let mut request = match uri_str.into_client_request() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to create WebSocket request: {}", e);
            return;
        }
    };

    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", HeaderValue::from_static("tty"));

    // Create WebSocket connection to ttyd
    let ws_stream = match tokio_tungstenite::client_async(request, unix_stream).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            tracing::error!("Failed to establish WebSocket with ttyd: {}", e);
            return;
        }
    };

    // Split both WebSocket connections
    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut ttyd_tx, mut ttyd_rx) = ws_stream.split();

    // Forward messages from browser to ttyd
    let client_to_ttyd = tokio::spawn(async move {
        while let Some(msg) = client_rx.next().await {
            let ttyd_msg = match msg {
                Ok(AxumMessage::Text(text)) => TungsteniteMessage::Text(text.to_string().into()),
                Ok(AxumMessage::Binary(data)) => TungsteniteMessage::Binary(data),
                Ok(AxumMessage::Ping(data)) => TungsteniteMessage::Ping(data),
                Ok(AxumMessage::Pong(data)) => TungsteniteMessage::Pong(data),
                Ok(AxumMessage::Close(_)) => {
                    let _ = ttyd_tx.send(TungsteniteMessage::Close(None)).await;
                    break;
                }
                Err(_) => break,
            };

            if ttyd_tx.send(ttyd_msg).await.is_err() {
                break;
            }
        }
    });

    // Forward messages from ttyd to browser
    let ttyd_to_client = tokio::spawn(async move {
        while let Some(msg) = ttyd_rx.next().await {
            let client_msg = match msg {
                Ok(TungsteniteMessage::Text(text)) => AxumMessage::Text(text.to_string().into()),
                Ok(TungsteniteMessage::Binary(data)) => AxumMessage::Binary(data),
                Ok(TungsteniteMessage::Ping(data)) => AxumMessage::Ping(data),
                Ok(TungsteniteMessage::Pong(data)) => AxumMessage::Pong(data),
                Ok(TungsteniteMessage::Close(_)) => {
                    let _ = client_tx.send(AxumMessage::Close(None)).await;
                    break;
                }
                Ok(TungsteniteMessage::Frame(_)) => continue,
                Err(_) => break,
            };

            if client_tx.send(client_msg).await.is_err() {
                break;
            }
        }
    });

    // Wait for either direction to complete
    tokio::select! {
        _ = client_to_ttyd => {}
        _ = ttyd_to_client => {}
    }
}

/// Proxy HTTP requests to ttyd
pub async fn terminal_proxy(
    State(_state): State<Arc<AppState>>,
    path: Option<Path<String>>,
    req: Request<Body>,
) -> Result<Response, AppError> {
    let path_str = path.map(|p| p.0).unwrap_or_default();

    // Connect to ttyd Unix socket
    let mut unix_stream = UnixStream::connect(TTYD_SOCKET_PATH)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("ttyd not running: {}", e)))?;

    // Build HTTP request to forward
    let method = req.method().as_str();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let uri_path = if path_str.is_empty() {
        format!("/api/terminal/{}", query)
    } else {
        format!("/api/terminal/{}{}", path_str, query)
    };

    // Forward relevant headers
    let mut headers_str = String::new();
    for (name, value) in req.headers() {
        if let Ok(v) = value.to_str() {
            let name_lower = name.as_str().to_lowercase();
            if !matches!(
                name_lower.as_str(),
                "connection" | "keep-alive" | "transfer-encoding" | "upgrade"
            ) {
                headers_str.push_str(&format!("{}: {}\r\n", name, v));
            }
        }
    }

    let http_request = format!(
        "{} {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{}\r\n",
        method, uri_path, headers_str
    );

    // Send request
    unix_stream
        .write_all(http_request.as_bytes())
        .await
        .map_err(|e| AppError::Internal(format!("Failed to send request: {}", e)))?;

    // Read response
    let mut response_buf = Vec::new();
    unix_stream
        .read_to_end(&mut response_buf)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read response: {}", e)))?;

    // Parse HTTP response
    let response_str = String::from_utf8_lossy(&response_buf);
    let header_end = response_str
        .find("\r\n\r\n")
        .ok_or_else(|| AppError::Internal("Invalid HTTP response".to_string()))?;

    let headers_part = &response_str[..header_end];
    let body_start = header_end + 4;

    // Parse status line
    let status_line = headers_part
        .lines()
        .next()
        .ok_or_else(|| AppError::Internal("Missing status line".to_string()))?;
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    // Build response
    let mut builder =
        Response::builder().status(StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK));

    // Forward response headers
    for line in headers_part.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if !matches!(
                name.to_lowercase().as_str(),
                "connection" | "keep-alive" | "transfer-encoding"
            ) {
                builder = builder.header(name, value);
            }
        }
    }

    let body = if body_start < response_buf.len() {
        Body::from(response_buf[body_start..].to_vec())
    } else {
        Body::empty()
    };

    builder
        .body(body)
        .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))
}

/// Terminal index page
pub async fn terminal_index(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response, AppError> {
    terminal_proxy(State(state), None, req).await
}
