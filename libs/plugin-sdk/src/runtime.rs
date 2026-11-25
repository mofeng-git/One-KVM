pub use crate::runtime_error::ServiceError;
use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::{Buf, BytesMut};
use futures::{Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::mpsc};

use crate::{
    types::{KeyboardEvent, MouseEvent},
    Bitrate, Codec, Frame, Status, VideoInfo, MAX_FRAME,
};

#[derive(Clone)]
pub struct Client {
    url: String,
    token: String,
    http: reqwest::Client,
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Client {
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("ONE_KVM_IPC")?;
        if !url.starts_with("http://127.0.0.1:") {
            bail!("Expected a local One-KVM capability server");
        }
        Ok(Self {
            url,
            token: std::env::var("ONE_KVM_TOKEN")?,
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .build()?,
            config: std::env::var_os("ONE_KVM_CONFIG")
                .context("ONE_KVM_CONFIG is missing")?
                .into(),
            data: std::env::var_os("ONE_KVM_DATA")
                .context("ONE_KVM_DATA is missing")?
                .into(),
        })
    }

    pub async fn read_config<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(
            &tokio::fs::read(&self.config).await?,
        )?)
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        checked(
            self.http
                .get(format!("{}{path}", self.url))
                .bearer_auth(&self.token)
                .timeout(Duration::from_secs(8))
                .send()
                .await?,
        )
        .await
    }

    async fn post(&self, path: &str, value: &impl Serialize) -> Result<()> {
        checked(
            self.http
                .post(format!("{}{path}", self.url))
                .bearer_auth(&self.token)
                .timeout(Duration::from_secs(20))
                .json(value)
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }

    pub async fn claim_video(&self, codec: Codec) -> Result<()> {
        self.post("/v1/video/claim", &codec).await
    }

    pub async fn video_info(&self) -> Result<VideoInfo> {
        Ok(self.get("/v1/video/info").await?.json().await?)
    }

    pub async fn request_keyframe(&self) -> Result<()> {
        self.post("/v1/video/keyframe", &()).await
    }

    pub async fn set_bitrate(&self, preset: Bitrate) -> Result<()> {
        self.post("/v1/video/bitrate", &preset).await
    }

    pub async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> {
        self.post("/v1/hid/keyboard", &event).await
    }

    pub async fn send_mouse(&self, event: MouseEvent) -> Result<()> {
        self.post("/v1/hid/mouse", &event).await
    }

    pub async fn reset_hid(&self) -> Result<()> {
        self.post("/v1/hid/reset", &()).await
    }

    pub async fn images_dir(&self) -> Result<PathBuf> {
        Ok(self.get("/v1/images").await?.json().await?)
    }

    pub async fn put_image(&self, name: &str, file: tokio::fs::File) -> Result<()> {
        let mut url = reqwest::Url::parse(&format!("{}/v1/images/", self.url))?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid IPC URL"))?
            .pop_if_empty()
            .push(name);
        let stream = futures::stream::try_unfold(file, |mut file| async move {
            let mut bytes = BytesMut::with_capacity(65536);
            let len = file.read_buf(&mut bytes).await?;
            Ok::<_, std::io::Error>((len > 0).then(|| (bytes.freeze(), file)))
        });
        checked(
            self.http
                .put(url)
                .bearer_auth(&self.token)
                .timeout(Duration::from_secs(60))
                .body(reqwest::Body::wrap_stream(stream))
                .send()
                .await?,
        )
        .await?;
        Ok(())
    }

    pub async fn subscribe_video(&self, codec: Codec) -> Result<mpsc::Receiver<Frame>> {
        let path = match codec {
            Codec::Mjpeg => "/v1/video/mjpeg",
            Codec::H264 | Codec::H265 | Codec::Vp8 | Codec::Vp9 => "/v1/video/encoded",
            Codec::Opus => bail!("Use subscribe_audio for Opus"),
        };
        self.frames(path, codec).await
    }

    pub async fn subscribe_audio(&self) -> Result<mpsc::Receiver<Frame>> {
        self.frames("/v1/audio/opus", Codec::Opus).await
    }

    pub async fn subscribe_events(&self) -> Result<impl Stream<Item = Result<Value>>> {
        let response = self.open_stream("/v1/events").await?;
        Ok(response
            .bytes_stream()
            .map(|result| result.map_err(anyhow::Error::from))
            .scan(BytesMut::new(), |buffer, chunk| {
                let values = match chunk {
                    Err(error) => vec![Err(error)],
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);
                        let mut values = Vec::new();
                        while let Some(end) = buffer.iter().position(|&byte| byte == b'\n') {
                            let record = buffer.split_to(end + 1);
                            values.push(serde_json::from_slice(&record).map_err(Into::into));
                        }
                        if buffer.len() > crate::MAX_MANIFEST {
                            buffer.clear();
                            values.push(Err(anyhow::anyhow!("Event exceeds the record limit")));
                        }
                        values
                    }
                };
                futures::future::ready(Some(futures::stream::iter(values)))
            })
            .flatten())
    }

    async fn open_stream(&self, path: &str) -> Result<reqwest::Response> {
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.http
                .get(format!("{}{path}", self.url))
                .bearer_auth(&self.token)
                .send(),
        )
        .await??;
        checked(response).await
    }

    async fn frames(&self, path: &str, codec: Codec) -> Result<mpsc::Receiver<Frame>> {
        let response = self.open_stream(path).await?;
        let (tx, rx) = mpsc::channel(8);
        let mut stream = response.bytes_stream();
        tokio::spawn(async move {
            let mut buffer = BytesMut::new();
            loop {
                let chunk = tokio::select! {
                    _ = tx.closed() => return,
                    chunk = stream.next() => chunk,
                };
                let Some(Ok(chunk)) = chunk else { return };
                buffer.extend_from_slice(&chunk);
                while buffer.len() >= 8 {
                    let metadata_len = u32::from_be_bytes(buffer[..4].try_into().unwrap()) as usize;
                    let data_len = u32::from_be_bytes(buffer[4..8].try_into().unwrap()) as usize;
                    if metadata_len > 4096 || data_len > MAX_FRAME {
                        return;
                    }
                    if buffer.len() < 8 + metadata_len + data_len {
                        break;
                    }
                    buffer.advance(8);
                    let Ok(header) = serde_json::from_slice::<crate::FrameHeader>(
                        &buffer.split_to(metadata_len),
                    ) else {
                        return;
                    };
                    let data = buffer.split_to(data_len).freeze();
                    if header.codec == codec
                        && tx.try_send(Frame { header, data }).is_err()
                        && tx.is_closed()
                    {
                        return;
                    }
                }
            }
        });
        Ok(rx)
    }
}

async fn checked(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    bail!(
        "Capability server returned {status}: {}",
        response.text().await.unwrap_or_default()
    );
}

pub trait Service: Send + Sync + 'static {
    fn status(&self) -> impl Future<Output = Result<Status>> + Send;
    fn action(&self, _id: &str) -> impl Future<Output = Result<Value>> + Send {
        async { bail!("Unknown action") }
    }
    fn shutdown(&self) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// Starts the control plane and ties the process lifetime to the host's stdin pipe.
pub async fn run<S, F, Start>(start: Start) -> Result<()>
where
    S: Service,
    F: Future<Output = Result<S>>,
    Start: FnOnce(Client) -> F,
{
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let client = Client::from_env()?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let (closed, mut stdin) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0]);
        let _ = closed.send(());
    });
    let service = Arc::new(tokio::select! {
        _ = &mut stdin => return Ok(()),
        result = start(client.clone()) => result?,
    });
    let token = client.token.clone();
    let router = Router::new()
        .route("/health", get(health::<S>))
        .route("/actions/{id}", post(action::<S>))
        .layer(middleware::from_fn(move |req: Request, next: Next| {
            let token = token.clone();
            async move {
                let authorized = req
                    .headers()
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.strip_prefix("Bearer "))
                    == Some(token.as_str());
                if authorized {
                    next.run(req).await
                } else {
                    StatusCode::UNAUTHORIZED.into_response()
                }
            }
        }))
        .with_state(service.clone());
    client.post("/v1/ready", &port).await?;
    tokio::select! {
        _ = &mut stdin => {}
        result = axum::serve(listener, router) => result?,
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), service.shutdown()).await;
    Ok(())
}

async fn health<S: Service>(State(service): State<Arc<S>>) -> Response {
    match service.status().await {
        Ok(status) => Json(status).into_response(),
        Err(error) => Json(Status {
            error: Some(error.to_string()),
            ..Status::default()
        })
        .into_response(),
    }
}

async fn action<S: Service>(State(service): State<Arc<S>>, Path(id): Path<String>) -> Response {
    match service.action(&id).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}
