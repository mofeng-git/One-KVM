use std::{
    collections::HashMap,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Extension, Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use one_kvm_plugin_sdk::{
    types::{KeyboardEvent, MouseEvent},
    Bitrate, Capability, Codec, Frame, FrameHeader, VideoInfo,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
    sync::{watch, Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    config::{BitratePreset, StreamMode},
    state::AppState,
    stream::ClientGuard,
    video::{
        codec::{registry::VideoEncoderType, VideoCodecType},
        codec_constraints::{enforce_constraints_with_stream_manager, StreamCodecConstraints},
    },
    AppError, Result,
};

#[derive(Default)]
pub(super) struct Capabilities {
    sessions: RwLock<HashMap<String, Arc<Session>>>,
    claims: Mutex<HashMap<String, (String, Codec)>>,
    app: RwLock<Weak<AppState>>,
}

pub(super) struct Session {
    pub token: String,
    id: String,
    permissions: Vec<Capability>,
    control_port: watch::Sender<Option<u16>>,
    revoked: CancellationToken,
    hid_used: AtomicBool,
}

impl Session {
    fn require(&self, capability: Capability) -> Result<()> {
        if !self.revoked.is_cancelled() && self.permissions.contains(&capability) {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "Plugin capability was not granted".into(),
            ))
        }
    }

    pub(super) fn port(&self) -> Option<u16> {
        *self.control_port.borrow()
    }

    pub(super) async fn ready(&self) -> u16 {
        let mut rx = self.control_port.subscribe();
        let port = rx
            .wait_for(Option::is_some)
            .await
            .expect("watch sender is retained")
            .unwrap();
        port
    }
}

impl Capabilities {
    pub(super) async fn serve(
        self: &Arc<Self>,
        app: Weak<AppState>,
    ) -> Result<(String, JoinHandle<()>)> {
        *self.app.write().await = app;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let router = Router::new()
            .route("/v1/ready", post(ready))
            .route("/v1/video/info", get(video_info))
            .route("/v1/video/claim", post(claim_video))
            .route("/v1/video/keyframe", post(keyframe))
            .route("/v1/video/bitrate", post(bitrate))
            .route("/v1/video/encoded", get(encoded))
            .route("/v1/video/mjpeg", get(mjpeg))
            .route("/v1/audio/opus", get(opus))
            .route("/v1/hid/keyboard", post(keyboard))
            .route("/v1/hid/mouse", post(mouse))
            .route("/v1/hid/reset", post(reset))
            .route("/v1/events", get(events))
            .route(
                "/v1/images/{name}",
                put(put_image).layer(DefaultBodyLimit::disable()),
            )
            .layer(middleware::from_fn_with_state(self.clone(), authenticate))
            .with_state(self.clone());
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                tracing::error!(%error, "Plugin capability server failed");
            }
        });
        Ok((format!("http://{address}"), task))
    }

    async fn app(&self) -> Result<Arc<AppState>> {
        self.app
            .read()
            .await
            .upgrade()
            .ok_or_else(|| AppError::ServiceUnavailable("One-KVM is shutting down".into()))
    }

    pub(super) async fn register(&self, id: &str, permissions: &[Capability]) -> Arc<Session> {
        let session = Arc::new(Session {
            id: id.into(),
            token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
            permissions: permissions.to_vec(),
            control_port: watch::channel(None).0,
            revoked: CancellationToken::new(),
            hid_used: AtomicBool::new(false),
        });
        self.sessions
            .write()
            .await
            .insert(session.token.clone(), session.clone());
        session
    }

    pub(super) async fn revoke(&self, session: &Session) {
        self.sessions.write().await.remove(&session.token);
        session.revoked.cancel();
        let Ok(app) = self.app().await else { return };
        let _format_guard = app.stream_manager.format_lock.lock().await;
        let mut claims = self.claims.lock().await;
        if claims.remove(&session.token).is_some() {
            app.stream_manager
                .set_runtime_codec_constraints(constraints(&claims))
                .await;
        }
        drop(claims);
        drop(_format_guard);
        if session.hid_used.load(Ordering::Acquire) {
            if let Err(error) = app.hid.reset().await {
                tracing::warn!(plugin = session.id, %error, "Could not release plugin HID state");
            }
        }
    }
}

async fn authenticate(
    State(api): State<Arc<Capabilities>>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));
    let Some(token) = token else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let session = api.sessions.read().await.get(token).cloned();
    let Some(session) = session else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    req.extensions_mut().insert(session);
    next.run(req).await
}

async fn ready(
    Extension(session): Extension<Arc<Session>>,
    Json(port): Json<u16>,
) -> Result<StatusCode> {
    if port == 0 {
        return Err(AppError::BadRequest("Invalid plugin control port".into()));
    }
    session.control_port.send_if_modified(|registered| {
        if registered.is_some() {
            return false;
        }
        *registered = Some(port);
        true
    });
    Ok(StatusCode::NO_CONTENT)
}

async fn video_info(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<Json<VideoInfo>> {
    session.require(Capability::Video)?;
    let app = api.app().await?;
    let stream = &app.stream_manager;
    let stats = stream.stats().await;
    let (_, capture, _, _, _) = stream.streamer().current_capture_config().await;
    let mjpeg = stream.is_mjpeg_enabled().await;
    let resolution = if mjpeg {
        capture
    } else {
        stream
            .get_encoding_config()
            .await
            .map_or(capture, |config| config.resolution)
    };
    Ok(Json(VideoInfo {
        width: resolution.width,
        height: resolution.height,
        mode: if mjpeg {
            Codec::Mjpeg
        } else {
            codec_from_video(stream.current_video_codec().await)
        },
        capture_format: stats.format,
        ready: !stream.is_switching()
            && !stream.is_config_changing()
            && matches!(stats.state.as_str(), "streaming" | "ready"),
    }))
}

async fn claim_video(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
    Json(codec): Json<Codec>,
) -> Result<StatusCode> {
    if codec == Codec::Opus {
        return Err(AppError::BadRequest("Expected a video codec".into()));
    }
    let app = api.app().await?;
    let _format_guard = app.stream_manager.format_lock.lock().await;
    let mut claims = api.claims.lock().await;
    session.require(Capability::VideoControl)?;
    for (token, (id, current)) in claims.iter() {
        if *token != session.token && *current != codec {
            return Err(AppError::BadRequest(format!(
                "Video format {current:?} is in use by {id}"
            )));
        }
    }
    if claims
        .get(&session.token)
        .is_some_and(|(_, current)| *current == codec)
    {
        return Ok(StatusCode::NO_CONTENT);
    }

    await_video_transition(&app.stream_manager).await?;
    let old = claims.insert(session.token.clone(), (session.id.clone(), codec));
    let required = constraints(&claims);
    app.stream_manager
        .set_runtime_codec_constraints(required.clone())
        .await;
    let result = async {
        enforce_constraints_with_stream_manager(&app.stream_manager, &required).await?;
        await_video_transition(&app.stream_manager).await
    }
    .await;
    if let Err(error) = result {
        claims.remove(&session.token);
        if let Some(old) = old {
            claims.insert(session.token.clone(), old);
        }
        app.stream_manager
            .set_runtime_codec_constraints(constraints(&claims))
            .await;
        return Err(error);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn await_video_transition(stream: &crate::video::VideoStreamManager) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        while stream.is_switching() {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
    })
    .await
    .map_err(|_| AppError::ServiceUnavailable("Video mode transition timed out".into()))
}

fn constraints(claims: &HashMap<String, (String, Codec)>) -> StreamCodecConstraints {
    let Some((_, codec)) = claims.values().next() else {
        return StreamCodecConstraints::unrestricted();
    };
    let mut sources: Vec<_> = claims.values().map(|(id, _)| id.clone()).collect();
    sources.sort();
    let mut result = StreamCodecConstraints::unrestricted();
    result.reason = format!("Video format {codec:?} requested by {}", sources.join(", "));
    result.sources = sources;
    result.allow_mjpeg = *codec == Codec::Mjpeg;
    result.locked_codec = video_from_codec(*codec);
    result.allowed_webrtc_codecs = result.locked_codec.into_iter().collect();
    result
}

fn video_from_codec(codec: Codec) -> Option<VideoCodecType> {
    match codec {
        Codec::H264 => Some(VideoCodecType::H264),
        Codec::H265 => Some(VideoCodecType::H265),
        Codec::Vp8 => Some(VideoCodecType::VP8),
        Codec::Vp9 => Some(VideoCodecType::VP9),
        Codec::Mjpeg | Codec::Opus => None,
    }
}

fn codec_from_video(codec: VideoCodecType) -> Codec {
    match codec {
        VideoCodecType::H264 => Codec::H264,
        VideoCodecType::H265 => Codec::H265,
        VideoCodecType::VP8 => Codec::Vp8,
        VideoCodecType::VP9 => Codec::Vp9,
    }
}

async fn keyframe(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<StatusCode> {
    session.require(Capability::Video)?;
    api.app().await?.stream_manager.request_keyframe().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn bitrate(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
    Json(value): Json<Bitrate>,
) -> Result<StatusCode> {
    session.require(Capability::VideoControl)?;
    let preset = match value {
        Bitrate::Speed => BitratePreset::Speed,
        Bitrate::Balanced => BitratePreset::Balanced,
        Bitrate::Quality => BitratePreset::Quality,
    };
    api.app()
        .await?
        .stream_manager
        .set_bitrate_preset(preset)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn encoded(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<Response> {
    session.require(Capability::Video)?;
    let app = api.app().await?;
    let stream = app.stream_manager.clone();
    if stream.current_mode().await != StreamMode::WebRTC {
        return Err(AppError::ServiceUnavailable(
            "Encoded video mode is inactive".into(),
        ));
    }
    let mut receiver = stream
        .subscribe_encoded_frames()
        .await
        .ok_or_else(|| AppError::ServiceUnavailable("Encoded video is unavailable".into()))?;
    let body = async_stream::stream! {
        while let Some(frame) = receiver.recv().await {
            let geometry = stream.get_encoding_config().await.map(|config| config.resolution);
            let codec = match frame.codec {
                VideoEncoderType::H264 => Codec::H264,
                VideoEncoderType::H265 => Codec::H265,
                VideoEncoderType::VP8 => Codec::Vp8,
                VideoEncoderType::VP9 => Codec::Vp9,
            };
            yield Frame {
                header: FrameHeader {
                    codec,
                    width: geometry.map_or(0, |size| size.width),
                    height: geometry.map_or(0, |size| size.height),
                    sequence: frame.sequence,
                    pts_ms: frame.pts_ms.max(0) as u64,
                    duration_us: frame.duration.as_micros() as u64,
                    keyframe: frame.is_keyframe,
                },
                data: frame.data.clone(),
            };
        }
    };
    Ok(frame_response(session, body))
}

async fn mjpeg(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<Response> {
    session.require(Capability::Video)?;
    let app = api.app().await?;
    let stream = app.stream_manager.clone();
    if !stream.is_mjpeg_enabled().await || stream.is_switching() {
        return Err(AppError::ServiceUnavailable(
            "MJPEG mode is inactive".into(),
        ));
    }
    let id = format!("plugin-{}", Uuid::new_v4());
    let handler = stream.mjpeg_handler();
    let guard = ClientGuard::persistent(id.clone(), handler.clone());
    let mut notify = handler.subscribe();
    stream.start().await?;
    let body = async_stream::stream! {
        let _guard = guard;
        let mut last = None;
        loop {
            if !stream.is_mjpeg_enabled().await || stream.is_switching() { break; }
            if let Some(frame) = handler.current_frame().filter(|frame| {
                frame.online && frame.is_valid_jpeg() && last != Some(frame.sequence)
            }) {
                last = Some(frame.sequence);
                handler.record_frame_sent(&id);
                yield Frame {
                    header: FrameHeader {
                        codec: Codec::Mjpeg,
                        width: frame.width(),
                        height: frame.height(),
                        sequence: frame.sequence,
                        pts_ms: 0,
                        duration_us: 0,
                        keyframe: true,
                    },
                    data: frame.data_bytes(),
                };
            }
            // Check for a mode switch even when capture has stopped delivering frames.
            match tokio::time::timeout(std::time::Duration::from_secs(1), notify.recv()).await {
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                _ => {}
            }
        }
    };
    Ok(frame_response(session, body))
}

async fn opus(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<Response> {
    session.require(Capability::Audio)?;
    let app = api.app().await?;
    let mut receiver = app
        .audio
        .subscribe_opus()
        .await
        .ok_or_else(|| AppError::ServiceUnavailable("Audio is unavailable".into()))?;
    let body = async_stream::stream! {
        while let Some(frame) = receiver.recv().await {
            yield Frame {
                header: FrameHeader {
                    codec: Codec::Opus,
                    width: 0,
                    height: 0,
                    sequence: frame.sequence,
                    pts_ms: 0,
                    duration_us: u64::from(frame.duration_ms) * 1000,
                    keyframe: true,
                },
                data: frame.data.clone(),
            };
        }
    };
    Ok(frame_response(session, body))
}

fn frame_response(
    session: Arc<Session>,
    stream: impl Stream<Item = Frame> + Send + 'static,
) -> Response {
    let body = stream
        .take_until(session.revoked.clone().cancelled_owned())
        .filter_map(|frame| futures::future::ready(frame.into_chunks()))
        .flat_map(|chunks| futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>)));
    (
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        Body::from_stream(body),
    )
        .into_response()
}

async fn keyboard(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
    Json(event): Json<KeyboardEvent>,
) -> Result<StatusCode> {
    session.require(Capability::Hid)?;
    session.hid_used.store(true, Ordering::Release);
    api.app().await?.hid.send_keyboard(event).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn mouse(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
    Json(event): Json<MouseEvent>,
) -> Result<StatusCode> {
    session.require(Capability::Hid)?;
    session.hid_used.store(true, Ordering::Release);
    api.app().await?.hid.send_mouse(event).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reset(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<StatusCode> {
    session.require(Capability::Hid)?;
    api.app().await?.hid.reset().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn events(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
) -> Result<Response> {
    session.require(Capability::Events)?;
    let mut events = api.app().await?.events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if let Ok(mut json) = serde_json::to_vec(&event) {
                        json.push(b'\n');
                        yield Ok::<Bytes, std::io::Error>(json.into());
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok(
        Body::from_stream(stream.take_until(session.revoked.clone().cancelled_owned()))
            .into_response(),
    )
}

pub(super) fn valid_filename(name: &str) -> bool {
    use std::path::{Component, Path};
    let mut components = Path::new(name).components();
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let reserved = matches!(
        stem.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) || stem
        .strip_prefix("com")
        .or_else(|| stem.strip_prefix("lpt"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        });
    !reserved
        && !name.ends_with(['.', ' '])
        && name.len() <= 255
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
}

async fn put_image(
    State(api): State<Arc<Capabilities>>,
    Extension(session): Extension<Arc<Session>>,
    Path(name): Path<String>,
    body: Body,
) -> Result<StatusCode> {
    session.require(Capability::Images)?;
    if !valid_filename(&name) {
        return Err(AppError::BadRequest("Invalid image filename".into()));
    }
    let root = api.app().await?.config.get().msd.images_dir();
    tokio::fs::create_dir_all(&root).await?;
    let file = tempfile::NamedTempFile::new_in(&root)?;
    let mut output = tokio::fs::File::from_std(file.reopen()?);
    let mut stream = body.into_data_stream();
    let mut count = 0;
    while let Some(chunk) = tokio::select! {
        _ = session.revoked.cancelled() => return Err(AppError::ServiceUnavailable("Plugin stopped".into())),
        chunk = stream.next() => chunk,
    } {
        let chunk = chunk.map_err(|error| AppError::BadRequest(error.to_string()))?;
        count += chunk.len();
        if count > 192 * 1024 * 1024 {
            return Err(AppError::BadRequest(
                "Image exceeds the plugin import limit".into(),
            ));
        }
        output.write_all(&chunk).await?;
    }
    output.sync_all().await?;
    drop(output);
    session.require(Capability::Images)?;
    file.persist_noclobber(root.join(name))
        .map_err(|error| error.error)?;
    Ok(StatusCode::CREATED)
}
