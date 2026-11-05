use super::*;
use crate::auth::server_time_unix_ms;
use crate::state::ShutdownAction;

/// Change password request
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub server_time_unix_ms: u64,
}

#[derive(Deserialize)]
pub struct BeginTotpEnrollmentRequest {
    pub current_password: String,
}

#[derive(Serialize)]
pub struct TotpEnrollmentResponse {
    pub enrollment_id: String,
    pub secret: String,
    pub otpauth_uri: String,
    pub expires_at_unix_ms: u64,
    pub server_time_unix_ms: u64,
}

#[derive(Deserialize)]
pub struct ConfirmTotpEnrollmentRequest {
    pub enrollment_id: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct DisableTotpRequest {
    pub current_password: String,
    pub code: String,
}

pub async fn totp_status(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> Result<Json<TotpStatusResponse>> {
    Ok(Json(TotpStatusResponse {
        enabled: state.two_factor.is_enabled(&session.user_id).await?,
        server_time_unix_ms: server_time_unix_ms(),
    }))
}

pub async fn begin_totp_enrollment(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<BeginTotpEnrollmentRequest>,
) -> Result<Json<TotpEnrollmentResponse>> {
    let user = authenticated_user(&state, &session).await?;
    verify_current_password(&state, &user, &req.current_password).await?;
    let enrollment = state
        .two_factor
        .begin_enrollment(&session.id, &user.id, &user.username)
        .await?;
    Ok(Json(TotpEnrollmentResponse {
        enrollment_id: enrollment.id,
        secret: enrollment.secret,
        otpauth_uri: enrollment.otpauth_uri,
        expires_at_unix_ms: enrollment.expires_at_unix_ms,
        server_time_unix_ms: server_time_unix_ms(),
    }))
}

pub async fn confirm_totp_enrollment(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ConfirmTotpEnrollmentRequest>,
) -> Result<Json<LoginResponse>> {
    authenticated_user(&state, &session).await?;
    state
        .two_factor
        .confirm_enrollment(&session.id, &session.user_id, &req.enrollment_id, &req.code)
        .await?;
    revoke_other_sessions(&state, &session.id).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: None,
    }))
}

pub async fn disable_totp(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<DisableTotpRequest>,
) -> Result<Json<LoginResponse>> {
    let user = authenticated_user(&state, &session).await?;
    verify_current_password(&state, &user, &req.current_password).await?;
    state.two_factor.disable(&user.id, &req.code).await?;
    revoke_other_sessions(&state, &session.id).await?;
    Ok(Json(LoginResponse {
        success: true,
        message: None,
    }))
}

async fn authenticated_user(state: &Arc<AppState>, session: &Session) -> Result<crate::auth::User> {
    state
        .users
        .single_user()
        .await?
        .filter(|user| user.id == session.user_id)
        .ok_or_else(|| AppError::AuthError("Invalid session".to_string()))
}

async fn verify_current_password(
    state: &Arc<AppState>,
    user: &crate::auth::User,
    password: &str,
) -> Result<()> {
    if state
        .users
        .verify(&user.username, password)
        .await?
        .is_none()
    {
        return Err(AppError::AuthError(
            "Current password is incorrect".to_string(),
        ));
    }
    Ok(())
}

async fn revoke_other_sessions(state: &Arc<AppState>, current_session_id: &str) -> Result<()> {
    let revoked = state.sessions.delete_all_except(current_session_id).await?;
    state.remember_revoked_sessions(revoked).await;
    Ok(())
}

/// Change current user's password
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<LoginResponse>> {
    let current_user = state
        .users
        .single_user()
        .await?
        .ok_or_else(|| AppError::AuthError("User not found".to_string()))?;

    if current_user.id != session.user_id {
        return Err(AppError::AuthError("Invalid session".to_string()));
    }

    if req.new_password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 4 characters".to_string(),
        ));
    }

    let verified = state
        .users
        .verify(&current_user.username, &req.current_password)
        .await?;
    if verified.is_none() {
        return Err(AppError::AuthError(
            "Current password is incorrect".to_string(),
        ));
    }

    state
        .users
        .update_password(&session.user_id, &req.new_password)
        .await?;
    info!("Password changed for user ID: {}", session.user_id);

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Password changed successfully".to_string()),
    }))
}

/// Change username request
#[derive(Deserialize)]
pub struct ChangeUsernameRequest {
    pub username: String,
    pub current_password: String,
}

/// Change current user's username
pub async fn change_username(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ChangeUsernameRequest>,
) -> Result<Json<LoginResponse>> {
    let current_user = state
        .users
        .single_user()
        .await?
        .ok_or_else(|| AppError::AuthError("User not found".to_string()))?;

    if current_user.id != session.user_id {
        return Err(AppError::AuthError("Invalid session".to_string()));
    }

    if req.username.len() < 2 {
        return Err(AppError::BadRequest(
            "Username must be at least 2 characters".to_string(),
        ));
    }

    let verified = state
        .users
        .verify(&current_user.username, &req.current_password)
        .await?;
    if verified.is_none() {
        return Err(AppError::AuthError(
            "Current password is incorrect".to_string(),
        ));
    }

    if current_user.username != req.username {
        state
            .users
            .update_username(&session.user_id, &req.username)
            .await?;
    }
    info!("Username changed for user ID: {}", session.user_id);

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Username changed successfully".to_string()),
    }))
}

/// Restart the application
pub async fn system_restart(State(state): State<Arc<AppState>>) -> Json<LoginResponse> {
    info!("System restart requested via API");

    let _ = state
        .shutdown_tx
        .send(ShutdownAction::Restart { exe_path: None });

    Json(LoginResponse {
        success: true,
        message: Some("Restarting...".to_string()),
    })
}
