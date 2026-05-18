use super::*;

/// Change password request
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
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

    // Send shutdown signal
    let _ = state.shutdown_tx.send(());

    // Spawn restart task in background
    tokio::spawn(async {
        // Wait for resources to be released (OTG, video, etc.)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Get current executable and args
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Failed to get current exe: {}", e);
                std::process::exit(1);
            }
        };
        let args: Vec<String> = std::env::args().skip(1).collect();

        info!("Restarting: {:?} {:?}", exe, args);

        // Use exec to replace current process (Unix)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let err = std::process::Command::new(&exe).args(&args).exec();
            tracing::error!("Failed to restart: {}", err);
            std::process::exit(1);
        }

        #[cfg(not(unix))]
        {
            let _ = std::process::Command::new(&exe).args(&args).spawn();
            std::process::exit(0);
        }
    });

    Json(LoginResponse {
        success: true,
        message: Some("Restarting...".to_string()),
    })
}
