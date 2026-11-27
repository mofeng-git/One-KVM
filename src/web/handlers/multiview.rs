use super::*;

use axum::body::Body;
use axum::extract::Path;
use axum::http::header;
use axum::response::{IntoResponse, Response};

#[derive(Serialize)]
pub struct SourcesResponse {
    sources: Vec<crate::video::multiview::SourceInfo>,
    primary_source_id: String,
}

pub async fn video_sources(State(state): State<Arc<AppState>>) -> Json<SourcesResponse> {
    let sources = state.stream_manager.sources().await;
    let primary_source_id = sources
        .iter()
        .find(|s| s.primary)
        .map(|s| s.id.clone())
        .unwrap_or_else(|| "primary".into());
    Json(SourcesResponse {
        sources,
        primary_source_id,
    })
}

pub async fn video_source_stream(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MjpegStreamQuery>,
) -> Response {
    let client_id = query
        .client_id
        .filter(|id| !id.is_empty() && id.len() <= 64)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let (handler, guard) = match state.stream_manager.source_mjpeg(&id, client_id).await {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let body = crate::stream::mjpeg_http::body_stream(handler, guard);
    (
        [
            (
                header::CONTENT_TYPE,
                "multipart/x-mixed-replace; boundary=frame",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(body),
    )
        .into_response()
}

pub async fn video_source_snapshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state
        .stream_manager
        .capture_source_snapshot(Some(&id), None)
        .await
    {
        Ok(snapshot) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Body::from(snapshot.jpeg),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn video_source_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MjpegStreamQuery>,
) -> Response {
    match state
        .stream_manager
        .source_status(&id, query.client_id.as_deref())
        .await
    {
        Ok(status) => ([(header::CACHE_CONTROL, "no-store")], Json(status)).into_response(),
        Err(error) => error.into_response(),
    }
}
