use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use std::sync::Arc;

use super::schema::RedfishError;
use crate::state::AppState;

pub async fn redfish_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if !state.config.is_initialized() {
        let body = RedfishError::service_unavailable("System not initialized");
        return (StatusCode::SERVICE_UNAVAILABLE, axum::Json(body)).into_response();
    }

    let path = request.uri().path();
    if is_redfish_public_endpoint(path, request.method()) {
        return next.run(request).await;
    }

    if let Some(token) = request.headers().get("X-Auth-Token") {
        if let Ok(token_str) = token.to_str() {
            if state.is_session_revoked(token_str).await {
                let body = RedfishError::invalid_credentials();
                return (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response();
            }
            if let Ok(Some(session)) = state.sessions.get(token_str).await {
                request.extensions_mut().insert(session);
                return next.run(request).await;
            }
        }
    }

    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                if let Some((username, password)) = decode_basic_auth(credentials) {
                    match state.users.verify(&username, &password).await {
                        Ok(Some(user)) => {
                            request.extensions_mut().insert(user);
                            return next.run(request).await;
                        }
                        _ => {
                            let body = RedfishError::invalid_credentials();
                            return (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response();
                        }
                    }
                }
            }
        }
    }

    let body = RedfishError::authentication_required();
    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}

fn is_redfish_public_endpoint(path: &str, method: &Method) -> bool {
    matches!(
        path,
        "/" | "/v1" | "/v1/" | "/v1/odata"
    ) || path.starts_with("/v1/$metadata")
        || (path == "/v1/SessionService/Sessions" && *method == Method::POST)
}

fn decode_basic_auth(encoded: &str) -> Option<(String, String)> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let mut parts = credentials.splitn(2, ':');
    let username = parts.next()?.to_string();
    let password = parts.next()?.to_string();
    if username.is_empty() {
        return None;
    }
    Some((username, password))
}
