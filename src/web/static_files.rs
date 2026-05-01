use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    routing::get,
    Router,
};
#[cfg(debug_assertions)]
use std::path::PathBuf;
#[cfg(debug_assertions)]
use std::sync::OnceLock;

#[cfg(not(debug_assertions))]
use rust_embed::Embed;

#[cfg(not(debug_assertions))]
#[derive(Embed)]
#[folder = "web/dist"]
#[prefix = ""]
pub struct StaticAssets;

#[cfg(debug_assertions)]
fn get_static_base_dir() -> PathBuf {
    static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();
    BASE_DIR
        .get_or_init(|| {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    return exe_dir.join("web").join("dist");
                }
            }
            PathBuf::from("web/dist")
        })
        .clone()
}

pub fn static_file_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(index_handler))
        .route("/{*path}", get(static_handler))
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
        if let Some(response) = try_serve_file("index.html") {
            return response;
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(placeholder_html()))
        .unwrap()
}

fn serve_file(path: &str) -> Response<Body> {
    try_serve_file(path).unwrap_or_else(|| {
        if path == "index.html" {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(placeholder_html()))
                .unwrap()
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()
        }
    })
}

fn try_serve_file(path: &str) -> Option<Response<Body>> {
    #[cfg(debug_assertions)]
    {
        let base_dir = get_static_base_dir();
        let file_path = base_dir.join(path);

        if !file_path.starts_with(&base_dir) {
            tracing::warn!("Path traversal attempt blocked: {}", path);
            return None;
        }

        if let (Ok(normalized_path), Ok(normalized_base)) =
            (file_path.canonicalize(), base_dir.canonicalize())
        {
            if !normalized_path.starts_with(&normalized_base) {
                tracing::warn!("Path traversal attempt blocked (canonicalized): {}", path);
                return None;
            }
        }

        match std::fs::read(&file_path) {
            Ok(data) => {
                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();

                Some(
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, mime)
                        .header(header::CACHE_CONTROL, "public, max-age=86400")
                        .body(Body::from(data))
                        .unwrap(),
                )
            }
            Err(e) => {
                tracing::debug!(
                    "Failed to read static file '{}' from '{}': {}",
                    path,
                    file_path.display(),
                    e
                );
                None
            }
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let asset = StaticAssets::get(path)?;

        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Some(
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(Body::from(asset.data.to_vec()))
                .unwrap(),
        )
    }
}

pub fn placeholder_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>One-KVM</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            color: #fff;
        }
        .container {
            text-align: center;
            padding: 2rem;
        }
        h1 { font-size: 2.5rem; margin-bottom: 1rem; }
        p { color: #888; font-size: 1.1rem; }
        .version { color: #666; margin-top: 2rem; font-size: 0.9rem; }
    </style>
</head>
<body>
    <div class="container">
        <h1>One-KVM</h1>
        <p>Frontend not built yet.</p>
        <p>Please build the frontend or access the API directly.</p>
        <div class="version">v0.1.9</div>
    </div>
</body>
</html>"#
}
