use rtsp_types as rtsp;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::config::RtspConfig;
use crate::error::{AppError, Result};
use crate::video::VideoStreamManager;

use super::auth::{extract_basic_auth, rtsp_auth_credentials};
use super::codec::rtsp_codec_to_video;
use super::protocol::{
    is_tcp_transport_request, is_valid_rtsp_path, parse_interleaved_channel, parse_rtsp_request,
    take_rtsp_request_from_buffer, OPTIONS_PUBLIC_CAPABILITIES,
};
use super::response::{send_response, send_simple_response};
use super::sdp::build_sdp;
use super::state::SharedRtspState;
use super::streaming::{stream_video_interleaved, RTSP_BUF_SIZE};
use super::types::RtspConnectionState;

pub use super::types::RtspServiceStatus;

pub struct RtspService {
    config: Arc<RwLock<RtspConfig>>,
    status: Arc<RwLock<RtspServiceStatus>>,
    video_manager: Arc<VideoStreamManager>,
    shutdown_tx: broadcast::Sender<()>,
    server_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    client_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    shared_state: SharedRtspState,
}

impl RtspService {
    pub fn new(config: RtspConfig, video_manager: Arc<VideoStreamManager>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(RtspServiceStatus::Stopped)),
            video_manager,
            shutdown_tx,
            server_handle: Arc::new(Mutex::new(None)),
            client_handles: Arc::new(Mutex::new(Vec::new())),
            shared_state: SharedRtspState::new(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        if !config.enabled {
            *self.status.write().await = RtspServiceStatus::Stopped;
            return Ok(());
        }

        if matches!(*self.status.read().await, RtspServiceStatus::Running) {
            return Ok(());
        }

        *self.status.write().await = RtspServiceStatus::Starting;

        let codec = rtsp_codec_to_video(config.codec);

        if let Err(err) = self.video_manager.set_video_codec(codec).await {
            let message = format!("failed to set codec before RTSP start: {}", err);
            *self.status.write().await = RtspServiceStatus::Error(message.clone());
            return Err(AppError::VideoError(message));
        }

        if let Err(err) = self.video_manager.request_keyframe().await {
            tracing::debug!("Failed to request keyframe on RTSP start: {}", err);
        }

        let bind_addr: SocketAddr = format!("{}:{}", config.bind, config.port)
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid RTSP bind address: {}", e)))?;

        let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
            AppError::Io(io::Error::new(e.kind(), format!("RTSP bind failed: {}", e)))
        })?;

        let service_config = self.config.clone();
        let video_manager = self.video_manager.clone();
        let shared_state = self.shared_state.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let status = self.status.clone();
        let client_handles = self.client_handles.clone();

        let handle = tokio::spawn(async move {
            tracing::info!("RTSP service listening on {}", bind_addr);
            *status.write().await = RtspServiceStatus::Running;

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::info!("RTSP service shutdown signal received");
                        break;
                    }
                    result = listener.accept() => {
                        match result {
                            Ok((stream, addr)) => {
                                let cfg = service_config.clone();
                                let vm = video_manager.clone();
                                let shared = shared_state.clone();
                                let handle = tokio::spawn(async move {
                                    if let Err(e) = handle_client(stream, addr, cfg, vm, shared).await {
                                        tracing::warn!("RTSP client {} ended with error: {}", addr, e);
                                    }
                                });
                                let mut handles = client_handles.lock().await;
                                handles.retain(|task| !task.is_finished());
                                handles.push(handle);
                            }
                            Err(e) => {
                                tracing::warn!("RTSP accept failed: {}", e);
                            }
                        }
                    }
                }
            }

            *status.write().await = RtspServiceStatus::Stopped;
        });

        *self.server_handle.lock().await = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.server_handle.lock().await.take() {
            handle.abort();
        }

        let mut client_handles = self.client_handles.lock().await;
        for handle in client_handles.drain(..) {
            handle.abort();
        }

        *self.shared_state.active_client.lock().await = None;
        *self.status.write().await = RtspServiceStatus::Stopped;
        Ok(())
    }

    pub async fn restart(&self, config: RtspConfig) -> Result<()> {
        self.update_config(config).await;
        self.stop().await?;
        self.start().await
    }

    pub async fn update_config(&self, config: RtspConfig) {
        *self.config.write().await = config;
    }

    pub async fn config(&self) -> RtspConfig {
        self.config.read().await.clone()
    }

    pub async fn status(&self) -> RtspServiceStatus {
        self.status.read().await.clone()
    }
}

async fn handle_client(
    mut stream: TcpStream,
    peer: SocketAddr,
    config: Arc<RwLock<RtspConfig>>,
    video_manager: Arc<VideoStreamManager>,
    shared: SharedRtspState,
) -> Result<()> {
    let cfg_snapshot = config.read().await.clone();
    let expected_auth = rtsp_auth_credentials(&cfg_snapshot);

    if cfg_snapshot.allow_one_client {
        let mut active_guard = shared.active_client.lock().await;
        if let Some(active) = *active_guard {
            if active != peer {
                send_simple_response(
                    &mut stream,
                    453,
                    "Not Enough Bandwidth",
                    None,
                    "another client is active",
                )
                .await?;
                return Ok(());
            }
        } else {
            *active_guard = Some(peer);
        }
    }

    let mut state = RtspConnectionState::new();
    let mut read_buf = [0u8; RTSP_BUF_SIZE];
    let mut request_buffer = Vec::with_capacity(RTSP_BUF_SIZE);

    'client_loop: loop {
        let n = stream.read(&mut read_buf).await?;
        if n == 0 {
            break;
        }

        request_buffer.extend_from_slice(&read_buf[..n]);

        while let Some(req_text) = take_rtsp_request_from_buffer(&mut request_buffer) {
            let req = match parse_rtsp_request(&req_text) {
                Some(r) => r,
                None => {
                    send_simple_response(&mut stream, 400, "Bad Request", None, "").await?;
                    continue;
                }
            };

            if !is_valid_rtsp_path(&req.method, &req.uri, &cfg_snapshot.path) {
                send_response(&mut stream, &req, 404, "Not Found", vec![], "", "").await?;
                continue;
            }

            if let Some((expected_user, expected_pass)) = expected_auth.as_ref() {
                let ok = extract_basic_auth(&req)
                    .map(|(u, p)| u == expected_user.as_str() && p == expected_pass.as_str())
                    .unwrap_or(false);
                if !ok {
                    send_response(
                        &mut stream,
                        &req,
                        401,
                        "Unauthorized",
                        vec![(
                            "WWW-Authenticate".to_string(),
                            "Basic realm=\"One-KVM RTSP\"".to_string(),
                        )],
                        "",
                        "",
                    )
                    .await?;
                    continue;
                }
            }

            match &req.method {
                rtsp::Method::Options => {
                    send_response(
                        &mut stream,
                        &req,
                        200,
                        "OK",
                        vec![(
                            "Public".to_string(),
                            OPTIONS_PUBLIC_CAPABILITIES.to_string(),
                        )],
                        "",
                        "",
                    )
                    .await?;
                }
                rtsp::Method::Describe => {
                    let codec = rtsp_codec_to_video(cfg_snapshot.codec.clone());
                    let params = shared.parameter_sets.read().await.clone();
                    let sdp = build_sdp(&cfg_snapshot, codec, &params);
                    if sdp.is_empty() {
                        send_response(
                            &mut stream,
                            &req,
                            500,
                            "Internal Server Error",
                            vec![],
                            "",
                            &state.session_id,
                        )
                        .await?;
                        continue;
                    }

                    send_response(
                        &mut stream,
                        &req,
                        200,
                        "OK",
                        vec![("Content-Type".to_string(), "application/sdp".to_string())],
                        &sdp,
                        &state.session_id,
                    )
                    .await?;
                }
                rtsp::Method::Setup => {
                    let transport = req.headers.get("transport").cloned().unwrap_or_default();

                    if !is_tcp_transport_request(&transport) {
                        send_response(
                            &mut stream,
                            &req,
                            461,
                            "Unsupported Transport",
                            vec![(
                                "Transport".to_string(),
                                "RTP/AVP/TCP;unicast;interleaved=0-1".to_string(),
                            )],
                            "",
                            &state.session_id,
                        )
                        .await?;
                        continue;
                    }

                    let interleaved = parse_interleaved_channel(&transport).unwrap_or(0);
                    state.setup_done = true;
                    state.interleaved_channel = interleaved;

                    let transport_resp = format!(
                        "RTP/AVP/TCP;unicast;interleaved={}-{}",
                        interleaved,
                        interleaved.saturating_add(1)
                    );

                    send_response(
                        &mut stream,
                        &req,
                        200,
                        "OK",
                        vec![("Transport".to_string(), transport_resp)],
                        "",
                        &state.session_id,
                    )
                    .await?;
                }
                rtsp::Method::Play => {
                    if !state.setup_done {
                        send_response(
                            &mut stream,
                            &req,
                            455,
                            "Method Not Valid in This State",
                            vec![],
                            "",
                            &state.session_id,
                        )
                        .await?;
                        continue;
                    }

                    send_response(&mut stream, &req, 200, "OK", vec![], "", &state.session_id)
                        .await?;

                    if let Err(e) = stream_video_interleaved(
                        stream,
                        &video_manager,
                        cfg_snapshot.codec.clone(),
                        state.interleaved_channel,
                        shared.clone(),
                        state.session_id.clone(),
                    )
                    .await
                    {
                        tracing::warn!("RTSP stream loop ended for {}: {}", peer, e);
                    }

                    break 'client_loop;
                }
                rtsp::Method::Teardown => {
                    send_response(&mut stream, &req, 200, "OK", vec![], "", &state.session_id)
                        .await?;
                    break 'client_loop;
                }
                _ => {
                    send_response(
                        &mut stream,
                        &req,
                        405,
                        "Method Not Allowed",
                        vec![],
                        "",
                        &state.session_id,
                    )
                    .await?;
                }
            }
        }
    }

    if cfg_snapshot.allow_one_client {
        let mut active_guard = shared.active_client.lock().await;
        if active_guard.as_ref().copied() == Some(peer) {
            *active_guard = None;
        }
    }

    Ok(())
}
