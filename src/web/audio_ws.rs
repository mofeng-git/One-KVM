//! Audio WebSocket handler for MJPEG mode
//!
//! Provides a dedicated WebSocket endpoint (`/api/ws/audio`) for streaming
//! Opus-encoded audio data in binary format.
//!
//! ## Binary Protocol
//!
//! Each audio packet is sent as a binary WebSocket message with the following format:
//!
//! ```text
//! Byte 0:      Type (0x02 = audio)
//! Bytes 1-4:   Timestamp (u32 LE, milliseconds since stream start)
//! Bytes 5-6:   Duration (u16 LE, milliseconds)
//! Bytes 7-10:  Sequence (u32 LE)
//! Bytes 11-14: Data length (u32 LE)
//! Bytes 15+:   Opus encoded data
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::audio::OpusFrame;
use crate::state::AppState;

/// Audio packet type identifier
const AUDIO_PACKET_TYPE: u8 = 0x02;

/// Audio WebSocket upgrade handler
///
/// Upgrades HTTP connection to WebSocket for audio streaming.
pub async fn audio_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_audio_socket(socket, state))
}

/// Handle audio WebSocket connection
async fn handle_audio_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Try to get Opus frame subscription
    let opus_rx = match state.audio.subscribe_opus_async().await {
        Some(rx) => rx,
        None => {
            warn!("Audio not streaming, rejecting WebSocket connection");
            // Send error message before closing
            let _ = sender
                .send(Message::Text(
                    r#"{"error": "Audio not streaming"}"#.to_string().into(),
                ))
                .await;
            return;
        }
    };

    let mut opus_rx = opus_rx;
    let stream_start = Instant::now();

    info!("Audio WebSocket client connected");

    // Track connection for cleanup
    let mut closed = false;

    // Use interval instead of sleep for more efficient keepalive
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            // Receive Opus frames and send to client
            opus_result = opus_rx.changed() => {
                if opus_result.is_err() {
                    info!("Audio stream closed");
                    break;
                }

                let frame = match opus_rx.borrow().clone() {
                    Some(frame) => frame,
                    None => continue,
                };

                let binary = encode_audio_packet(&frame, stream_start);
                if sender.send(Message::Binary(binary.into())).await.is_err() {
                    debug!("Failed to send audio frame, client disconnected");
                    break;
                }
            }

            // Handle client messages (ping/close)
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => {
                        debug!("Audio WebSocket client requested close");
                        closed = true;
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    Some(Ok(Message::Text(text))) => {
                        // Handle potential control messages
                        debug!("Received text message on audio WS: {}", text);
                    }
                    Some(Err(e)) => {
                        warn!("Audio WebSocket receive error: {}", e);
                        break;
                    }
                    None => {
                        // Connection closed
                        break;
                    }
                    _ => {}
                }
            }

            // Periodic ping to keep connection alive (using interval)
            _ = ping_interval.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    warn!("Failed to send ping, disconnecting");
                    break;
                }
            }
        }
    }

    if !closed {
        // Try to send close message
        let _ = sender.send(Message::Close(None)).await;
    }

    info!("Audio WebSocket client disconnected");
}

/// Encode Opus frame to binary packet format
///
/// ## Format
///
/// | Offset | Size | Description |
/// |--------|------|-------------|
/// | 0 | 1 | Packet type (0x02 for audio) |
/// | 1 | 4 | Timestamp (u32 LE, ms since start) |
/// | 5 | 2 | Duration (u16 LE, ms) |
/// | 7 | 4 | Sequence number (u32 LE) |
/// | 11 | 4 | Data length (u32 LE) |
/// | 15 | N | Opus encoded data |
fn encode_audio_packet(frame: &OpusFrame, stream_start: Instant) -> Vec<u8> {
    let timestamp_ms = stream_start.elapsed().as_millis() as u32;
    let data_len = frame.data.len() as u32;

    let mut buf = Vec::with_capacity(15 + frame.data.len());

    // Header
    buf.push(AUDIO_PACKET_TYPE);
    buf.extend_from_slice(&timestamp_ms.to_le_bytes());
    buf.extend_from_slice(&(frame.duration_ms as u16).to_le_bytes());
    buf.extend_from_slice(&(frame.sequence as u32).to_le_bytes());
    buf.extend_from_slice(&data_len.to_le_bytes());

    // Opus data
    buf.extend_from_slice(&frame.data);

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_encode_decode_packet() {
        let frame = OpusFrame {
            data: Bytes::from(vec![1, 2, 3, 4, 5]),
            duration_ms: 20,
            sequence: 42,
            timestamp: Instant::now(),
            rtp_timestamp: 0,
        };

        let stream_start = Instant::now();
        let encoded = encode_audio_packet(&frame, stream_start);

        assert!(encoded.len() >= 15);
        assert_eq!(encoded[0], AUDIO_PACKET_TYPE);
        // decode_audio_packet function was removed, skip decode test
    }

    #[test]
    fn test_decode_invalid_packet() {
        // decode_audio_packet function was removed, skip this test
    }
}
