use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    routing::get,
    Router,
};
use rust_embed::Embed;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FRONTEND_DIR_ENV: &str = "ONE_KVM_FRONTEND_DIR";

#[derive(Embed)]
#[folder = "web/dist"]
#[prefix = ""]
pub struct StaticAssets;

fn frontend_dir_override() -> Option<PathBuf> {
    static FRONTEND_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    FRONTEND_DIR
        .get_or_init(|| {
            let value = std::env::var_os(FRONTEND_DIR_ENV)?;
            let path = PathBuf::from(value);

            if path.as_os_str().is_empty() {
                return None;
            }

            match path.canonicalize() {
                Ok(path) => Some(path),
                Err(e) => {
                    tracing::warn!(
                        "{}='{}' is not accessible: {}",
                        FRONTEND_DIR_ENV,
                        path.display(),
                        e
                    );
                    None
                }
            }
        })
        .clone()
}

pub fn static_file_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(index_handler))
        // GET-only fallback: a `/{*path}` wildcard would shadow fallbacks
        // nested under other routers (e.g. the /api 404 handler) in axum 0.8.
        .fallback_service(get(static_handler))
}

async fn index_handler() -> Response<Body> {
    serve_file("index.html")
}

async fn static_handler(uri: Uri) -> Response<Body> {
    let path = uri.path().trim_start_matches('/');

    if let Some(response) = try_serve_file(path) {
        return response;
    }

    if !path.contains('.') {
        // SPA fallback: extensionless paths are frontend routes.
        if let Some(response) = try_serve_file("index.html") {
            return response;
        }
    }

    not_found_response()
}

fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(not_found_html()))
        .unwrap()
}

fn serve_file(path: &str) -> Response<Body> {
    try_serve_file(path).unwrap_or_else(|| not_found_response())
}

fn try_serve_file(path: &str) -> Option<Response<Body>> {
    if let Some(base_dir) = frontend_dir_override() {
        return try_serve_file_from_dir(&base_dir, path);
    }

    let asset = StaticAssets::get(path)?;
    Some(static_response(path, asset.data.to_vec()))
}

fn try_serve_file_from_dir(base_dir: &Path, path: &str) -> Option<Response<Body>> {
    let file_path = base_dir.join(path);

    let normalized_path = match file_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            tracing::debug!(
                "Failed to resolve static file '{}' from '{}': {}",
                path,
                file_path.display(),
                e
            );
            return None;
        }
    };

    if !normalized_path.starts_with(base_dir) {
        tracing::warn!("Path traversal attempt blocked: {}", path);
        return None;
    }

    match std::fs::read(&normalized_path) {
        Ok(data) => Some(static_response(path, data)),
        Err(e) => {
            tracing::debug!(
                "Failed to read static file '{}' from '{}': {}",
                path,
                normalized_path.display(),
                e
            );
            None
        }
    }
}

fn static_response(path: &str, data: Vec<u8>) -> Response<Body> {
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap()
}

pub fn not_found_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
    <style>
        body {
            width: 35em;
            margin: 0 auto;
            text-align: center;
            font-family: Tahoma, Verdana, Arial, sans-serif;
        }
    </style>
</head>
<body>
    <h1>404 Not Found</h1>
</body>
</html>"#
        .to_string()
}
