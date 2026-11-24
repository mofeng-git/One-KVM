use std::sync::Arc;

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::header,
    response::Response,
    routing::get,
    Json, Router,
};
use futures::{Stream, StreamExt};
use one_kvm_plugin_sdk::MAX_PACKAGE;
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use super::manager::PluginView;
use crate::{state::AppState, AppError, Result};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/plugins",
            get(list).post(upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/plugins/install-url", axum::routing::post(install_url))
        .route(
            "/plugins/{id}",
            axum::routing::patch(configure).delete(uninstall),
        )
        .route("/plugins/{id}/logs", get(logs))
        .route(
            "/plugins/{id}/actions/{action}",
            axum::routing::post(action),
        )
        .route("/plugins/{id}/files/{name}", get(file).delete(remove_file))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        ))
}

// Explicit header on requests that can install or execute local code. HTML forms
// cannot set it, and cross-origin requests must still carry a valid session.
fn check_mutation(headers: &axum::http::HeaderMap) -> Result<()> {
    if headers
        .get("x-one-kvm-plugin-management")
        .is_some_and(|value| value == "1")
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Missing plugin management request header".into(),
        ))
    }
}

async fn list(State(state): State<Arc<AppState>>) -> Json<Vec<PluginView>> {
    Json(state.plugins.list().await)
}

async fn upload(State(state): State<Arc<AppState>>, request: Request) -> Result<Json<PluginView>> {
    check_mutation(request.headers())?;
    install(&state, request.into_body().into_data_stream()).await
}

#[derive(Deserialize)]
struct UrlInstall {
    url: String,
}

async fn install_url(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<UrlInstall>,
) -> Result<Json<PluginView>> {
    check_mutation(&headers)?;
    let url = reqwest::Url::parse(&request.url)
        .map_err(|_| AppError::BadRequest("Invalid plugin package URL".into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::BadRequest(
            "Expected an HTTP(S) package URL without credentials".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5
                || !matches!(attempt.url().scheme(), "http" | "https")
                || !attempt.url().username().is_empty()
                || attempt.url().password().is_some()
            {
                attempt.error("Invalid package URL redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(download_error)?;
    let response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(download_error)?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_PACKAGE as u64)
    {
        return Err(too_large());
    }
    install(&state, response.bytes_stream()).await
}

async fn install<E: std::fmt::Display>(
    state: &AppState,
    input: impl Stream<Item = std::result::Result<bytes::Bytes, E>>,
) -> Result<Json<PluginView>> {
    let file = tempfile::NamedTempFile::new_in(state.plugins.root())?;
    let mut output = tokio::fs::File::from_std(file.reopen()?);
    let mut length = 0;
    tokio::pin!(input);
    while let Some(chunk) = input.next().await {
        let chunk = chunk.map_err(download_error)?;
        length += chunk.len();
        if length > MAX_PACKAGE {
            return Err(too_large());
        }
        output.write_all(&chunk).await?;
    }
    output.flush().await?;
    drop(output);
    // Once staging succeeds, finish disk and process state changes even if the client disconnects.
    let state_plugins = state.plugins.clone();
    tokio::spawn(async move { state_plugins.install(file.path().to_owned()).await })
        .await
        .map_err(task_error)?
        .map(Json)
}

#[derive(Deserialize)]
struct Configure {
    enabled: Option<bool>,
    autostart: Option<bool>,
    config: Option<Map<String, Value>>,
}

async fn configure(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(request): Json<Configure>,
) -> Result<Json<PluginView>> {
    check_mutation(&headers)?;
    tokio::spawn(async move {
        state
            .plugins
            .configure(&id, request.enabled, request.autostart, request.config)
            .await
    })
    .await
    .map_err(task_error)?
    .map(Json)
}

async fn uninstall(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>> {
    check_mutation(&headers)?;
    tokio::spawn(async move { state.plugins.uninstall(&id).await })
        .await
        .map_err(task_error)??;
    Ok(Json(serde_json::json!({"success":true})))
}

async fn logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>> {
    Ok(Json(state.plugins.logs(&id).await?))
}

async fn action(
    State(state): State<Arc<AppState>>,
    Path((id, action)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>> {
    check_mutation(&headers)?;
    let result = state.plugins.action(&id, &action).await?;
    Ok(Json(serde_json::json!({"result":result})))
}

async fn file(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Response> {
    let file = state.plugins.open_file(&id, &name).await?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file.metadata().await?.len())
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                urlencoding::encode(&name)
            ),
        )
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|error| AppError::Internal(error.to_string()))
}

async fn remove_file(
    State(state): State<Arc<AppState>>,
    Path((id, name)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>> {
    check_mutation(&headers)?;
    state.plugins.remove_file(&id, &name).await?;
    Ok(Json(serde_json::json!({"success":true})))
}

fn too_large() -> AppError {
    AppError::BadRequest("Plugin package exceeds the size limit".into())
}
fn download_error(error: impl std::fmt::Display) -> AppError {
    AppError::BadRequest(format!("Plugin download failed: {error}"))
}
fn task_error(error: tokio::task::JoinError) -> AppError {
    AppError::Internal(error.to_string())
}
