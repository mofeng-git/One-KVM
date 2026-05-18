use super::*;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: Option<String>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>)> {
    let config = state.config.get();

    // Check if system is initialized
    if !config.initialized {
        return Err(AppError::BadRequest("System not initialized".to_string()));
    }

    // Verify user credentials
    let user = state
        .users
        .verify(&req.username, &req.password)
        .await?
        .ok_or_else(|| AppError::AuthError("Invalid username or password".to_string()))?;

    if !config.auth.single_user_allow_multiple_sessions {
        // Kick existing sessions before creating a new one.
        let revoked_ids = state.sessions.list_ids().await?;
        state.sessions.delete_all().await?;
        state.remember_revoked_sessions(revoked_ids).await;
    }

    // Create session
    let session = state.sessions.create(&user.id).await?;

    // Set session cookie
    let cookie = Cookie::build((SESSION_COOKIE, session.id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(
            config.auth.session_timeout_secs as i64,
        ))
        .build();

    Ok((
        cookies.add(cookie),
        Json(LoginResponse {
            success: true,
            message: None,
        }),
    ))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
) -> Result<(CookieJar, Json<LoginResponse>)> {
    // Get session ID from cookie
    if let Some(cookie) = cookies.get(SESSION_COOKIE) {
        state.sessions.delete(cookie.value()).await?;
    }

    // Remove cookie
    let cookie = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();

    Ok((
        cookies.remove(cookie),
        Json(LoginResponse {
            success: true,
            message: Some("Logged out".to_string()),
        }),
    ))
}

#[derive(Serialize)]
pub struct AuthCheckResponse {
    pub authenticated: bool,
    pub user: Option<String>,
}

pub async fn auth_check(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Json<AuthCheckResponse> {
    // Get user info from user_id
    let username = match state.users.single_user().await {
        Ok(Some(user)) if user.id == session.user_id => Some(user.username),
        _ => None,
    };

    Json(AuthCheckResponse {
        authenticated: true,
        user: username,
    })
}
