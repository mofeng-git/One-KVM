use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use std::sync::Arc;

use crate::error::ErrorResponse;
use crate::state::AppState;

/// Session cookie name
pub const SESSION_COOKIE: &str = "one_kvm_session";

/// Extract session ID from request
pub fn extract_session_id(cookies: &CookieJar, headers: &axum::http::HeaderMap) -> Option<String> {
    // First try cookie
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        return Some(cookie.value().to_string());
    }

    // Then try Authorization header (Bearer token)
    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    None
}

/// Authentication middleware
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check if system is initialized
    if !state.config.is_initialized() {
        // Allow access to setup endpoints when not initialized
        let path = request.uri().path();
        if path.starts_with("/api/setup")
            || path == "/api/info"
            || path.starts_with("/") && !path.starts_with("/api/")
        {
            return Ok(next.run(request).await);
        }
    }

    // Public endpoints that don't require auth
    let path = request.uri().path();
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Extract session ID
    let session_id = extract_session_id(&cookies, request.headers());

    if let Some(session_id) = session_id {
        if let Ok(Some(session)) = state.sessions.get(&session_id).await {
            // Add session to request extensions
            request.extensions_mut().insert(session);
            return Ok(next.run(request).await);
        }

        let message = if state.is_session_revoked(&session_id).await {
            "Logged in elsewhere"
        } else {
            "Session expired"
        };
        return Ok(unauthorized_response(message));
    }

    Ok(unauthorized_response("Not authenticated"))
}

fn unauthorized_response(message: &str) -> Response {
    let body = ErrorResponse {
        success: false,
        message: message.to_string(),
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

/// Check if endpoint is public (no auth required)
fn is_public_endpoint(path: &str) -> bool {
    // Note: paths here are relative to /api since middleware is applied before nest
    matches!(
        path,
        "/"
        | "/auth/login"
        | "/info"
        | "/health"
        | "/setup"
        | "/setup/init"
        // Also check with /api prefix for direct access
        | "/api/auth/login"
        | "/api/info"
        | "/api/health"
        | "/api/setup"
        | "/api/setup/init"
    ) || path.starts_with("/assets/")
        || path.starts_with("/static/")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".ico")
        || path.ends_with(".png")
        || path.ends_with(".svg")
}
