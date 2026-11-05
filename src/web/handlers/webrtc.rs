use super::*;

use crate::webrtc::signaling::{AnswerResponse, IceCandidateRequest, OfferRequest};

/// Create WebRTC session
#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
}

pub async fn webrtc_create_session(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CreateSessionResponse>> {
    // Check if WebRTC mode is active
    if !state.stream_manager.is_webrtc_enabled().await {
        return Err(AppError::ServiceUnavailable(
            "WebRTC mode not active. Current mode is MJPEG.".to_string(),
        ));
    }

    let session_id = state.webrtc.create_session().await?;
    Ok(Json(CreateSessionResponse { session_id }))
}

/// Handle WebRTC offer
pub async fn webrtc_offer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OfferRequest>,
) -> Result<Json<AnswerResponse>> {
    // Check if WebRTC mode is active
    if !state.stream_manager.is_webrtc_enabled().await {
        return Err(AppError::ServiceUnavailable(
            "WebRTC mode not active. Current mode is MJPEG.".to_string(),
        ));
    }

    // Backward compatibility: `client_id` is treated as an existing session_id hint.
    // New clients should not pass it; each offer creates a fresh session.
    let webrtc = &state.webrtc;
    let session_id = if let Some(client_id) = &req.client_id {
        // Reuse only when it matches an active session ID.
        if webrtc.get_session(client_id).await.is_some() {
            client_id.clone()
        } else {
            webrtc.create_session().await?
        }
    } else {
        webrtc.create_session().await?
    };

    // Handle offer
    let offer = crate::webrtc::SdpOffer::new(req.sdp);
    let answer = webrtc.handle_offer(&session_id, offer).await?;

    Ok(Json(AnswerResponse::new(
        answer.sdp,
        session_id,
        answer.ice_candidates.unwrap_or_default(),
    )))
}

/// Add ICE candidate
pub async fn webrtc_ice_candidate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<Json<LoginResponse>> {
    state
        .webrtc
        .add_ice_candidate(&req.session_id, req.candidate)
        .await?;

    Ok(Json(LoginResponse {
        success: true,
        message: None,
    }))
}

/// Get WebRTC session info
#[derive(Serialize)]
pub struct WebRtcSessionInfo {
    pub session_id: String,
    pub state: String,
}

#[derive(Serialize)]
pub struct WebRtcStatus {
    pub session_count: usize,
    pub sessions: Vec<WebRtcSessionInfo>,
}

pub async fn webrtc_status(State(state): State<Arc<AppState>>) -> Json<WebRtcStatus> {
    let sessions = state.webrtc.list_sessions().await;
    Json(WebRtcStatus {
        session_count: sessions.len(),
        sessions: sessions
            .into_iter()
            .map(|s| WebRtcSessionInfo {
                session_id: s.session_id,
                state: s.state,
            })
            .collect(),
    })
}

/// Close WebRTC session
#[derive(Deserialize)]
pub struct CloseSessionRequest {
    pub session_id: String,
}

pub async fn webrtc_close_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CloseSessionRequest>,
) -> Result<Json<LoginResponse>> {
    state.webrtc.close_session(&req.session_id).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Session closed".to_string()),
    }))
}

/// ICE servers configuration for WebRTC
#[derive(Serialize)]
pub struct IceServersResponse {
    pub ice_servers: Vec<IceServerInfo>,
    pub mdns_mode: String,
}

#[derive(Serialize)]
pub struct IceServerInfo {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

fn non_empty_config_value(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|value| !value.is_empty())
}

/// Get ICE servers configuration for client-side WebRTC
/// Returns user-configured servers, or Google STUN as fallback if none configured
pub async fn webrtc_ice_servers(State(state): State<Arc<AppState>>) -> Json<IceServersResponse> {
    use crate::webrtc::config::public_ice;
    use crate::webrtc::mdns::{mdns_mode, mdns_mode_label};

    let config = state.config.get();
    let mut ice_servers = Vec::new();

    // Check if user has configured custom ICE servers
    let stun_server = non_empty_config_value(&config.stream.stun_server);
    let turn_server = non_empty_config_value(&config.stream.turn_server);

    if stun_server.is_some() || turn_server.is_some() {
        // Use user-configured ICE servers
        if let Some(stun) = stun_server {
            ice_servers.push(IceServerInfo {
                urls: vec![stun.to_string()],
                username: None,
                credential: None,
            });
        }

        if let Some(turn) = turn_server {
            let username = config.stream.turn_username.clone();
            let credential = config.stream.turn_password.clone();
            if username.is_some() && credential.is_some() {
                ice_servers.push(IceServerInfo {
                    urls: vec![turn.to_string()],
                    username,
                    credential,
                });
            }
        }
    } else {
        // No custom servers — baked-in public STUN
        ice_servers.push(IceServerInfo {
            urls: vec![public_ice::stun_server().to_string()],
            username: None,
            credential: None,
        });
        // Note: TURN servers are not provided - users must configure their own
    }

    let mdns_mode = mdns_mode();
    let mdns_mode = mdns_mode_label(mdns_mode).to_string();

    Json(IceServersResponse {
        ice_servers,
        mdns_mode,
    })
}
