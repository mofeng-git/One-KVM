use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::sync::Arc;
use tracing::warn;

use crate::state::AppState;

/// WebSocket endpoint for UAC microphone passthrough audio input.
///
/// Accepts Opus-encoded audio frames (same binary protocol as audio
/// output, message type 0x03) and routes decoded PCM to the UAC
/// playback device on the USB gadget side.
pub async fn uac_audio_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let playback = {
        let guard = state.uac_playback.read().await;
        match guard.as_ref() {
            Some(p) => Arc::new(p.clone()),
            None => {
                warn!("UAC audio WS rejected: playback not initialized");
                return (StatusCode::SERVICE_UNAVAILABLE, "UAC playback not initialized").into_response();
            }
        }
    };

    ws.on_upgrade(move |socket| {
        crate::audio::uac_websocket::handle_uac_audio_ws(socket, playback)
    })
}
