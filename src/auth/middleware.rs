use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use std::sync::Arc;

use crate::state::AppState;

/// Session cookie name
pub const SESSION_COOKIE: &str = "one_kvm_session";

/// Auth layer for extracting session from request
#[derive(Clone)]
pub struct AuthLayer;

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
    }

    Err(StatusCode::UNAUTHORIZED)
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

/// Require authentication - returns 401 if not authenticated
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let session_id = extract_session_id(&cookies, request.headers());

    if let Some(session_id) = session_id {
        if let Ok(Some(_session)) = state.sessions.get(&session_id).await {
            return Ok(next.run(request).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Require admin privileges - returns 403 if not admin
pub async fn require_admin(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let session_id = extract_session_id(&cookies, request.headers());

    if let Some(session_id) = session_id {
        if let Ok(Some(session)) = state.sessions.get(&session_id).await {
            // Get user and check admin status
            if let Ok(Some(user)) = state.users.get(&session.user_id).await {
                if user.is_admin {
                    return Ok(next.run(request).await);
                }
                // User is authenticated but not admin
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    // Not authenticated at all
    Err(StatusCode::UNAUTHORIZED)
}
