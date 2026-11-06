//! WebSocket endpoint streaming runtime log lines to the settings page.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use serde::Serialize;
use tokio::{
    sync::broadcast::error::RecvError,
    time::{interval, MissedTickBehavior},
};
use tracing::debug;

use crate::logging::{self, LOG_BUFFER_CAPACITY};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LogSocketMessage<'a> {
    /// Initial backlog sent right after the client connects.
    Snapshot { lines: &'a [String] },
    /// A single line emitted after the snapshot.
    Line { line: &'a str },
    /// The client could not keep up; `count` lines were dropped.
    Lagged { count: u64 },
}

pub async fn logs_ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe before taking the snapshot so lines emitted in between are
    // not lost. The overlap can produce a duplicate, which is harmless.
    let hub = logging::hub();
    let mut lines_rx = hub.subscribe();
    let snapshot = hub.snapshot(LOG_BUFFER_CAPACITY);

    if !send_message(&mut sender, &LogSocketMessage::Snapshot { lines: &snapshot }).await {
        return;
    }

    debug!("Runtime log WebSocket client connected");

    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            received = lines_rx.recv() => match received {
                Ok(line) => {
                    if !send_message(&mut sender, &LogSocketMessage::Line { line: &line }).await {
                        break;
                    }
                }
                Err(RecvError::Lagged(count)) => {
                    if !send_message(&mut sender, &LogSocketMessage::Lagged { count }).await {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            incoming = receiver.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    if sender.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                Some(Err(_)) => break,
                _ => {}
            },
            _ = heartbeat.tick() => {
                if sender.send(Message::Ping(Vec::<u8>::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }

    debug!("Runtime log WebSocket client disconnected");
}

async fn send_message(
    sender: &mut SplitSink<WebSocket, Message>,
    message: &LogSocketMessage<'_>,
) -> bool {
    match serde_json::to_string(message) {
        Ok(text) => sender.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => false,
    }
}
