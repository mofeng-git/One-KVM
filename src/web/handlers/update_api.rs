use super::*;

#[derive(Deserialize)]
pub struct UpdateOverviewQuery {
    pub channel: Option<UpdateChannel>,
}

pub async fn update_overview(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<UpdateOverviewQuery>,
) -> Result<Json<UpdateOverviewResponse>> {
    if cfg!(feature = "android") {
        return Err(AppError::BadRequest(
            "Online upgrade is disabled on Android".to_string(),
        ));
    }

    let channel = query.channel.unwrap_or(UpdateChannel::Stable);
    let response = state.update.overview(channel).await?;
    Ok(Json(response))
}

pub async fn update_upgrade(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpgradeRequest>,
) -> Result<Json<LoginResponse>> {
    if cfg!(feature = "android") {
        return Err(AppError::BadRequest(
            "Online upgrade is disabled on Android".to_string(),
        ));
    }

    state.update.start_upgrade(req, state.shutdown_tx.clone())?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some("Upgrade started".to_string()),
    }))
}

pub async fn update_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<UpdateStatusResponse>> {
    if cfg!(feature = "android") {
        return Err(AppError::BadRequest(
            "Online upgrade is disabled on Android".to_string(),
        ));
    }

    Ok(Json(state.update.status().await))
}
