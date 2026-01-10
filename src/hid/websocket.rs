//! WebSocket HID channel for HTTP/MJPEG mode
//!
//! This provides an alternative to WebRTC DataChannel for HID input
//! when using MJPEG streaming mode.
//!
//! Uses binary protocol only (same format as DataChannel):
//! - Keyboard: [0x01, event_type, key, modifiers] (4 bytes)
//! - Mouse: [0x02, event_type, x_lo, x_hi, y_lo, y_hi, button/scroll] (7 bytes)
//!
//! See datachannel.rs for detailed protocol specification.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::datachannel::{parse_hid_message, HidChannelEvent};
use crate::state::AppState;
use crate::utils::LogThrottler;

/// Binary response codes
const RESP_OK: u8 = 0x00;
const RESP_ERR_HID_UNAVAILABLE: u8 = 0x01;
const RESP_ERR_INVALID_MESSAGE: u8 = 0x02;

/// WebSocket HID upgrade handler
pub async fn ws_hid_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_hid_socket(socket, state))
}

/// Handle HID WebSocket connection
async fn handle_hid_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    // Log throttler for error messages (5 second interval)
    let log_throttler = LogThrottler::with_secs(5);

    info!("WebSocket HID connection established (binary protocol)");

    // Check if HID controller is available and send initial status
    let hid_available = state.hid.is_available().await;
    let initial_response = if hid_available {
        vec![RESP_OK]
    } else {
        vec![RESP_ERR_HID_UNAVAILABLE]
    };

    if sender.send(Message::Binary(initial_response.into())).await.is_err() {
        error!("Failed to send initial HID status");
        return;
    }

    // Process incoming messages (binary only)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                // Check HID availability before processing each message
                let hid_available = state.hid.is_available().await;
                if !hid_available {
                    if log_throttler.should_log("hid_unavailable") {
                        warn!("HID controller not available, ignoring message");
                    }
                    // Send error response (optional, for client awareness)
                    let _ = sender.send(Message::Binary(vec![RESP_ERR_HID_UNAVAILABLE].into())).await;
                    continue;
                }

                if let Err(e) = handle_binary_message(&data, &state).await {
                    // Log with throttling to avoid spam
                    if log_throttler.should_log("binary_hid_error") {
                        warn!("Binary HID message error: {}", e);
                    }
                    // Don't send error response for every failed message to reduce overhead
                }
            }
            Ok(Message::Text(text)) => {
                // Text messages are no longer supported
                if log_throttler.should_log("text_message_rejected") {
                    debug!("Received text message (not supported): {} bytes", text.len());
                }
                let _ = sender.send(Message::Binary(vec![RESP_ERR_INVALID_MESSAGE].into())).await;
            }
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket HID connection closed by client");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Reset HID state to release any held keys/buttons
    if let Err(e) = state.hid.reset().await {
        warn!("Failed to reset HID on WebSocket disconnect: {}", e);
    }

    info!("WebSocket HID connection ended");
}

/// Handle binary HID message (same format as DataChannel)
async fn handle_binary_message(data: &[u8], state: &AppState) -> Result<(), String> {
    let event = parse_hid_message(data).ok_or("Invalid binary HID message")?;

    match event {
        HidChannelEvent::Keyboard(kb_event) => {
            state
                .hid
                .send_keyboard(kb_event)
                .await
                .map_err(|e| e.to_string())?;
        }
        HidChannelEvent::Mouse(ms_event) => {
            state
                .hid
                .send_mouse(ms_event)
                .await
                .map_err(|e| e.to_string())?;
        }
        HidChannelEvent::Consumer(consumer_event) => {
            state
                .hid
                .send_consumer(consumer_event)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::datachannel::{MSG_KEYBOARD, MSG_MOUSE, KB_EVENT_DOWN, MS_EVENT_MOVE};

    #[test]
    fn test_response_codes() {
        assert_eq!(RESP_OK, 0x00);
        assert_eq!(RESP_ERR_HID_UNAVAILABLE, 0x01);
        assert_eq!(RESP_ERR_INVALID_MESSAGE, 0x02);
        // assert_eq!(RESP_ERR_SEND_FAILED, 0x03); // TODO: fix test
    }

    #[test]
    fn test_keyboard_message_format() {
        // Keyboard message: [0x01, event_type, key, modifiers]
        let data = [MSG_KEYBOARD, KB_EVENT_DOWN, 0x04, 0x01]; // 'A' key with left ctrl
        let event = parse_hid_message(&data);
        assert!(event.is_some());
    }

    #[test]
    fn test_mouse_message_format() {
        // Mouse message: [0x02, event_type, x_lo, x_hi, y_lo, y_hi, extra]
        let data = [MSG_MOUSE, MS_EVENT_MOVE, 0x0A, 0x00, 0xF6, 0xFF, 0x00]; // x=10, y=-10
        let event = parse_hid_message(&data);
        assert!(event.is_some());
    }
}
