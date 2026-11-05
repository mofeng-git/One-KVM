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

#[derive(Serialize)]
pub struct AuthLoginResponse {
    pub next: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Deserialize)]
pub struct TotpLoginRequest {
    pub challenge_id: String,
    pub code: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<AuthLoginResponse>)> {
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

    if let Some(challenge) = state.two_factor.begin_login(&user.id).await? {
        return Ok((
            cookies,
            Json(AuthLoginResponse {
                next: "totp",
                challenge_id: Some(challenge.id),
                expires_at_unix_ms: Some(challenge.expires_at_unix_ms),
            }),
        ));
    }

    create_authenticated_session(&state, cookies, &user.id).await
}

pub async fn login_totp(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    Json(req): Json<TotpLoginRequest>,
) -> Result<(CookieJar, Json<AuthLoginResponse>)> {
    let user_id = state
        .two_factor
        .complete_login(&req.challenge_id, &req.code)
        .await?;
    create_authenticated_session(&state, cookies, &user_id).await
}

async fn create_authenticated_session(
    state: &Arc<AppState>,
    cookies: CookieJar,
    user_id: &str,
) -> Result<(CookieJar, Json<AuthLoginResponse>)> {
    let config = state.config.get();
    let (session, revoked_ids) = state
        .sessions
        .create_for_login(user_id, config.auth.single_user_allow_multiple_sessions)
        .await?;
    state.remember_revoked_sessions(revoked_ids).await;

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
        Json(AuthLoginResponse {
            next: "authenticated",
            challenge_id: None,
            expires_at_unix_ms: None,
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
