use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use std::sync::Arc;

use crate::state::AppState;
use crate::web::ErrorResponse;

pub const SESSION_COOKIE: &str = "one_kvm_session";

pub fn extract_session_id(cookies: &CookieJar, headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        return Some(cookie.value().to_string());
    }

    if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    None
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let raw_path = request.uri().path();
    // Mounted under /api: inner path may lack prefix; normalize for whitelist checks.
    let path = raw_path.strip_prefix("/api").unwrap_or(raw_path);

    if !state.config.is_initialized() {
        if is_setup_public_endpoint(path) {
            return Ok(next.run(request).await);
        }
    }

    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    let session_id = extract_session_id(&cookies, request.headers());

    if let Some(session_id) = session_id {
        if let Ok(Some(session)) = state.sessions.get(&session_id).await {
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
        code: None,
        message: message.to_string(),
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

fn is_public_endpoint(path: &str) -> bool {
    matches!(
        path,
        "/" | "/auth/login" | "/auth/login/totp" | "/health" | "/setup" | "/setup/init"
    )
}

fn is_setup_public_endpoint(path: &str) -> bool {
    matches!(
        path,
        "/setup"
            | "/setup/init"
            | "/devices"
            | "/hid/bluetooth/adapters"
            | "/video/input-status"
            | "/stream/codecs"
            | "/video/codecs"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_extensions_do_not_bypass_api_authentication() {
        for extension in ["js", "css", "ico", "png", "svg", "bin"] {
            for prefix in ["/plugins/test/files/", "/msd/drive/files/"] {
                let path = format!("{prefix}0123456789abcdef0123456789abcdef-file.{extension}");
                assert!(!is_public_endpoint(&path), "{path}");
                assert!(!is_setup_public_endpoint(&path), "{path}");
            }
        }
        assert!(is_public_endpoint("/auth/login"));
        assert!(is_public_endpoint("/health"));
    }
}
