use super::*;
use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::Response,
};
use tokio_util::io::ReaderStream;

use crate::airferry::eligibility;

pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    let (eligible, reason) = eligibility(&state.stream_manager).await;
    Ok(Json(state.airferry.status(eligible, reason).await?))
}

pub async fn start(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    let (eligible, reason) = eligibility(&state.stream_manager).await;
    if !eligible {
        return Err(AppError::BadRequest(reason.into()));
    }
    state.airferry.start(state.stream_manager.clone()).await?;
    Ok(Json(state.airferry.status(eligible, reason).await?))
}

pub async fn stop(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    state.airferry.stop().await?;
    let (eligible, reason) = eligibility(&state.stream_manager).await;
    Ok(Json(state.airferry.status(eligible, reason).await?))
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Response> {
    let path = state.airferry.file_path(&name).await?;
    let file = tokio::fs::File::open(path).await?;
    let length = file.metadata().await?.len();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, length)
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                urlencoding::encode(&crate::airferry::display_name(&name))
            ),
        )
        .body(Body::from_stream(ReaderStream::with_capacity(file, 65536)))
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>> {
    #[cfg(unix)]
    {
        let path = state.airferry.file_path(&name).await?;
        let guard = state.msd.read().await;
        if let Some(controller) = guard.as_ref() {
            let manager = crate::msd::ImageManager::new(path.parent().unwrap().to_path_buf());
            let image = manager.get_by_name(&name)?;
            // Share MSD's operation lock and mounted-media protection.
            controller.delete_image(&image.id).await?;
            return Ok(Json(serde_json::json!({"deleted":true})));
        }
    }
    state.airferry.delete_file(&name).await?;
    Ok(Json(serde_json::json!({"deleted":true})))
}
