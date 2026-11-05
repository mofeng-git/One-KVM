use std::borrow::Cow;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::{debug, info, warn};

use crate::audio::uac::{
    parse_audio_packet, UacAudioPacket, UacOpusDecoder, UacPlaybackState, UacSession,
};
use crate::state::AppState;

pub async fn uac_audio_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let session = {
        let playback = state.uac_playback.read().await;
        let Some(playback) = playback.as_ref() else {
            return (StatusCode::SERVICE_UNAVAILABLE, "UAC playback is disabled").into_response();
        };
        match playback.acquire_session() {
            Ok(session) => session,
            Err(error) => {
                return (StatusCode::CONFLICT, error.to_string()).into_response();
            }
        }
    };

    ws.on_upgrade(move |socket| handle_uac_audio(socket, session))
}

async fn handle_uac_audio(mut socket: WebSocket, session: UacSession) {
    let mut decoder = match UacOpusDecoder::new() {
        Ok(decoder) => decoder,
        Err(error) => {
            warn!("Unable to initialize UAC Opus decoder: {error}");
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
    let mut dropped_frames = 0u64;
    info!("UAC microphone WebSocket connected");

    let mut playback_state = session.state();
    if socket
        .send(playback_state_message(playback_state))
        .await
        .is_err()
    {
        return;
    }

    while let Some(message) = socket.recv().await {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                warn!("UAC microphone WebSocket failed: {error}");
                break;
            }
        };

        match message {
            Message::Binary(data) => {
                let packet = match parse_audio_packet(&data) {
                    Ok(packet) => packet,
                    Err(error) => {
                        warn!("Rejected UAC audio packet: {error}");
                        continue;
                    }
                };
                let pcm: Cow<'_, [i16]> = match packet {
                    UacAudioPacket::Opus(payload) => match decoder.decode(payload) {
                        Ok(pcm) => Cow::Borrowed(pcm),
                        Err(error) => {
                            warn!("Rejected UAC Opus packet: {error}");
                            continue;
                        }
                    },
                    packet @ UacAudioPacket::Pcm(_) => match packet.pcm_samples() {
                        Ok(pcm) => Cow::Owned(pcm),
                        Err(error) => {
                            warn!("Rejected UAC PCM packet: {error}");
                            continue;
                        }
                    },
                };

                let (accepted, current_state) = match session.try_write(pcm.as_ref()) {
                    Ok(result) => result,
                    Err(error) => {
                        warn!("UAC playback stopped: {error}");
                        break;
                    }
                };
                if !accepted {
                    dropped_frames += 1;
                    if dropped_frames == 1 || dropped_frames.is_multiple_of(250) {
                        debug!(
                            "Dropped {dropped_frames} UAC audio frames while the target was unavailable"
                        );
                    }
                }

                if current_state != playback_state
                    && socket
                        .send(playback_state_message(current_state))
                        .await
                        .is_err()
                {
                    break;
                }
                playback_state = current_state;
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Text(_) => debug!("Ignoring text message on UAC audio WebSocket"),
        }
    }

    info!("UAC microphone WebSocket disconnected; dropped_frames={dropped_frames}");
}

fn playback_state_message(state: UacPlaybackState) -> Message {
    Message::Text(
        serde_json::json!({
            "type": "uac_status",
            "state": state.as_str(),
        })
        .to_string()
        .into(),
    )
}
