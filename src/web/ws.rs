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
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{debug, info, warn};

use crate::events::SystemEvent;
use crate::state::AppState;

enum BusMessage {
    Event(SystemEvent),
    Lagged { topic: String, count: u64 },
}

fn normalize_topics(topics: &[String]) -> Vec<String> {
    let mut normalized = topics.to_vec();
    normalized.sort();
    normalized.dedup();

    if normalized.iter().any(|topic| topic == "*") {
        return vec!["*".to_string()];
    }

    normalized
        .into_iter()
        .filter(|topic| {
            if topic.ends_with(".*") {
                return true;
            }

            let Some((prefix, _)) = topic.split_once('.') else {
                return true;
            };

            let wildcard = format!("{}.*", prefix);
            !topics.iter().any(|candidate| candidate == &wildcard)
        })
        .collect()
}

fn is_device_info_topic(topic: &str) -> bool {
    matches!(topic, "*" | "system.*" | "system.device_info")
}

fn rebuild_event_tasks(
    state: &Arc<AppState>,
    topics: &[String],
    event_tx: &mpsc::UnboundedSender<BusMessage>,
    event_tasks: &mut Vec<JoinHandle<()>>,
) {
    for task in event_tasks.drain(..) {
        task.abort();
    }

    let topics = normalize_topics(topics);
    let mut device_info_task_added = false;
    for topic in topics {
        if is_device_info_topic(&topic) && !device_info_task_added {
            let mut rx = state.subscribe_device_info();
            let event_tx = event_tx.clone();
            event_tasks.push(tokio::spawn(async move {
                if let Some(snapshot) = rx.borrow().clone() {
                    if event_tx.send(BusMessage::Event(snapshot)).is_err() {
                        return;
                    }
                }

                loop {
                    if rx.changed().await.is_err() {
                        break;
                    }

                    if let Some(snapshot) = rx.borrow().clone() {
                        if event_tx.send(BusMessage::Event(snapshot)).is_err() {
                            break;
                        }
                    }
                }
            }));
            device_info_task_added = true;
        }

        if is_device_info_topic(&topic) && topic != "*" {
            continue;
        }

        let Some(mut rx) = state.events.subscribe_topic(&topic) else {
            warn!("Client subscribed to unknown topic: {}", topic);
            continue;
        };

        let event_tx = event_tx.clone();
        let topic_name = topic.clone();
        event_tasks.push(tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event_tx.send(BusMessage::Event(event)).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        if event_tx
                            .send(BusMessage::Lagged {
                                topic: topic_name.clone(),
                                count,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }));
    }
}

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
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut event_tasks: Vec<JoinHandle<()>> = Vec::new();

    // Track subscribed topics (default: none until client subscribes)
    let mut subscribed_topics: Vec<String> = vec![];

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
                        } else {
                            rebuild_event_tasks(
                                &state,
                                &subscribed_topics,
                                &event_tx,
                                &mut event_tasks,
                            );
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
                    Some(BusMessage::Event(event)) => {
                        // Filter event based on subscribed topics
                        if let Ok(json) = serialize_event(&event) {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                warn!("Failed to send event to client, disconnecting");
                                break;
                            }
                        }
                    }
                    Some(BusMessage::Lagged { topic, count }) => {
                        warn!(
                            "WebSocket client lagged by {} events on topic {}",
                            count, topic
                        );
                        // Send error notification to client using SystemEvent::Error
                        let error_event = SystemEvent::Error {
                            message: format!("Lagged by {} events", count),
                        };
                        if let Ok(json) = serialize_event(&error_event) {
                            let _ = sender.send(Message::Text(json.into())).await;
                        }
                    }
                    None => {
                        warn!("Event bus closed");
                        break;
                    }
                }
            }

            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    warn!("Failed to send ping, disconnecting");
                    break;
                }
            }
        }
    }

    for task in event_tasks {
        task.abort();
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

/// Serialize event to JSON string
fn serialize_event(event: &SystemEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_topics_dedupes_and_sorts() {
        let topics = vec![
            "stream.state_changed".to_string(),
            "stream.state_changed".to_string(),
            "system.device_info".to_string(),
        ];

        assert_eq!(
            normalize_topics(&topics),
            vec![
                "stream.state_changed".to_string(),
                "system.device_info".to_string()
            ]
        );
    }

    #[test]
    fn test_normalize_topics_wildcard_wins() {
        let topics = vec!["*".to_string(), "stream.state_changed".to_string()];
        assert_eq!(normalize_topics(&topics), vec!["*".to_string()]);
    }

    #[test]
    fn test_normalize_topics_drops_exact_when_prefix_exists() {
        let topics = vec![
            "stream.*".to_string(),
            "stream.state_changed".to_string(),
            "system.device_info".to_string(),
        ];

        assert_eq!(
            normalize_topics(&topics),
            vec!["stream.*".to_string(), "system.device_info".to_string()]
        );
    }

    #[test]
    fn test_is_device_info_topic_matches_expected_topics() {
        assert!(is_device_info_topic("system.device_info"));
        assert!(is_device_info_topic("system.*"));
        assert!(is_device_info_topic("*"));
        assert!(!is_device_info_topic("stream.*"));
    }
}
