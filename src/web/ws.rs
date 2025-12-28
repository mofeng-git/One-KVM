//! WebSocket handler for real-time event streaming
//!
//! This module provides a WebSocket endpoint at `/api/ws` that:
//! - Broadcasts system events to connected clients
//! - Supports topic-based event filtering
//! - Handles client subscription management
//! - Includes heartbeat (ping/pong) mechanism

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::events::SystemEvent;
use crate::state::AppState;

/// Client-to-server message
#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload")]
enum ClientMessage {
    /// Subscribe to event topics
    #[serde(rename = "subscribe")]
    Subscribe { topics: Vec<String> },

    /// Unsubscribe from event topics
    #[serde(rename = "unsubscribe")]
    Unsubscribe { topics: Vec<String> },

    /// Ping (keep-alive)
    #[serde(rename = "ping")]
    Ping,
}

/// WebSocket upgrade handler
///
/// This is the entry point for WebSocket connections at `/api/ws`.
/// Authentication is handled by the middleware.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to event bus
    let mut event_rx = state.events.subscribe();

    // Track subscribed topics (default: none until client subscribes)
    let mut subscribed_topics: Vec<String> = vec![];

    // Flag to send device info after first subscribe
    let mut device_info_sent = false;

    info!("WebSocket client connected");

    // Heartbeat interval (30 seconds)
    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

    loop {
        tokio::select! {
            // Receive message from client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle_client_message(&text, &mut subscribed_topics).await {
                            warn!("Failed to handle client message: {}", e);
                        }

                        // Send device info after first subscribe
                        if !device_info_sent && !subscribed_topics.is_empty() {
                            let device_info = state.get_device_info().await;
                            if let Ok(json) = serialize_event(&device_info) {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    warn!("Failed to send device info to client");
                                    break;
                                }
                            }
                            device_info_sent = true;
                        }
                    }
                    Some(Ok(Message::Ping(_))) => {
                        // WebSocket automatically handles ping/pong
                        debug!("Received ping from client");
                    }
                    Some(Ok(Message::Pong(_))) => {
                        debug!("Received pong from client");
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("WebSocket client disconnected");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            // Receive event from event bus
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        // Filter event based on subscribed topics
                        if should_send_event(&event, &subscribed_topics) {
                            if let Ok(json) = serialize_event(&event) {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    warn!("Failed to send event to client, disconnecting");
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket client lagged by {} events", n);
                        // Send error notification to client using SystemEvent::Error
                        let error_event = SystemEvent::Error {
                            message: format!("Lagged by {} events", n),
                        };
                        if let Ok(json) = serialize_event(&error_event) {
                            let _ = sender.send(Message::Text(json)).await;
                        }
                    }
                    Err(_) => {
                        warn!("Event bus closed");
                        break;
                    }
                }
            }

            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if sender.send(Message::Ping(vec![])).await.is_err() {
                    warn!("Failed to send ping, disconnecting");
                    break;
                }
            }
        }
    }

    info!("WebSocket handler exiting");
}

/// Handle message from client
async fn handle_client_message(
    text: &str,
    topics: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let msg: ClientMessage = serde_json::from_str(text)?;

    match msg {
        ClientMessage::Subscribe { topics: new_topics } => {
            *topics = new_topics.clone();
            info!("Client subscribed to topics: {:?}", new_topics);
        }
        ClientMessage::Unsubscribe {
            topics: remove_topics,
        } => {
            topics.retain(|t| !remove_topics.contains(t));
            info!("Client unsubscribed from topics: {:?}", remove_topics);
        }
        ClientMessage::Ping => {
            debug!("Received ping from client");
        }
    }

    Ok(())
}

/// Check if an event should be sent based on subscribed topics
fn should_send_event(event: &SystemEvent, topics: &[String]) -> bool {
    if topics.is_empty() {
        return false;
    }

    // Fast path: check for wildcard subscription (avoid String allocation)
    if topics.iter().any(|t| t == "*") {
        return true;
    }

    // Check if event matches any subscribed topic
    topics.iter().any(|topic| event.matches_topic(topic))
}

/// Serialize event to JSON string
fn serialize_event(event: &SystemEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::SystemEvent;

    #[test]
    fn test_should_send_event_wildcard() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: None,
        };

        assert!(should_send_event(&event, &["*".to_string()]));
    }

    #[test]
    fn test_should_send_event_prefix() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: None,
        };

        assert!(should_send_event(&event, &["stream.*".to_string()]));
        assert!(!should_send_event(&event, &["msd.*".to_string()]));
    }

    #[test]
    fn test_should_send_event_exact() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: None,
        };

        assert!(should_send_event(
            &event,
            &["stream.state_changed".to_string()]
        ));
        assert!(!should_send_event(
            &event,
            &["stream.config_changed".to_string()]
        ));
    }

    #[test]
    fn test_should_send_event_empty_topics() {
        let event = SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: None,
        };

        assert!(!should_send_event(&event, &[]));
    }
}
