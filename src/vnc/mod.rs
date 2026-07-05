//! Minimal VNC/RFB service for direct JPEG/H264 frame forwarding.

pub mod rfb;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::config::{VncConfig, VncEncoding};
use crate::error::{AppError, Result};
use crate::hid::HidController;
use crate::stream::mjpeg::ClientGuard;
use crate::utils::{bind_socket_addr, bind_tcp_listener};
use crate::video::codec::{BitratePreset, VideoCodecType};
use crate::video::stream_manager::VideoStreamManager;

use self::rfb::{RfbClient, RfbFrame, RfbInputEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VncServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

impl std::fmt::Display for VncServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error(err) => write!(f, "error: {}", err),
        }
    }
}

pub struct VncService {
    config: Arc<RwLock<VncConfig>>,
    status: Arc<RwLock<VncServiceStatus>>,
    video_manager: Arc<VideoStreamManager>,
    hid: Arc<HidController>,
    shutdown_tx: broadcast::Sender<()>,
    server_handle: Mutex<Option<JoinHandle<()>>>,
    client_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    active_clients: Arc<AtomicUsize>,
}

impl VncService {
    pub fn new(
        config: VncConfig,
        video_manager: Arc<VideoStreamManager>,
        hid: Arc<HidController>,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(VncServiceStatus::Stopped)),
            video_manager,
            hid,
            shutdown_tx,
            server_handle: Mutex::new(None),
            client_handles: Arc::new(Mutex::new(Vec::new())),
            active_clients: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn config(&self) -> VncConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, config: VncConfig) {
        *self.config.write().await = config;
    }

    pub async fn status(&self) -> VncServiceStatus {
        self.status.read().await.clone()
    }

    pub fn connection_count(&self) -> usize {
        self.active_clients.load(Ordering::Relaxed)
    }

    pub async fn start(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        if !config.enabled {
            *self.status.write().await = VncServiceStatus::Stopped;
            return Ok(());
        }
        if matches!(*self.status.read().await, VncServiceStatus::Running) {
            return Ok(());
        }
        if config.password.as_deref().unwrap_or("").is_empty() {
            let msg = "VNC password is required".to_string();
            *self.status.write().await = VncServiceStatus::Error(msg.clone());
            return Err(AppError::BadRequest(msg));
        }

        *self.status.write().await = VncServiceStatus::Starting;
        if let Err(err) = self.prepare_video_pipeline(&config).await {
            *self.status.write().await = VncServiceStatus::Error(err.to_string());
            return Err(err);
        }

        let bind_addr = bind_socket_addr(&config.bind, config.port)
            .map_err(|e| AppError::BadRequest(format!("Invalid VNC bind address: {}", e)))?;
        let listener = bind_tcp_listener(bind_addr).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("VNC bind failed: {}", e),
            ))
        })?;
        let listener = TcpListener::from_std(listener).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("VNC listener setup failed: {}", e),
            ))
        })?;

        let config_ref = self.config.clone();
        let video_manager = self.video_manager.clone();
        let hid = self.hid.clone();
        let status = self.status.clone();
        let client_handles = self.client_handles.clone();
        let active_clients = self.active_clients.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        *self.status.write().await = VncServiceStatus::Running;
        let handle = tokio::spawn(async move {
            info!("VNC service listening on {}", bind_addr);
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("VNC service shutdown signal received");
                        break;
                    }
                    result = listener.accept() => {
                        match result {
                            Ok((stream, peer)) => {
                                let cfg = config_ref.read().await.clone();
                                if cfg.allow_one_client && active_clients.load(Ordering::Relaxed) > 0 {
                                    warn!("Rejecting VNC client {} because another client is active", peer);
                                    drop(stream);
                                    continue;
                                }
                                let vm = video_manager.clone();
                                let hid = hid.clone();
                                let active = active_clients.clone();
                                let handle = tokio::spawn(async move {
                                    active.fetch_add(1, Ordering::Relaxed);
                                    let result = handle_client(stream, peer, cfg, vm, hid).await;
                                    active.fetch_sub(1, Ordering::Relaxed);
                                    if let Err(err) = result {
                                        warn!("VNC client {} ended: {}", peer, err);
                                    }
                                });
                                let mut handles = client_handles.lock().await;
                                handles.retain(|task| !task.is_finished());
                                handles.push(handle);
                            }
                            Err(err) => warn!("VNC accept failed: {}", err),
                        }
                    }
                }
            }
            *status.write().await = VncServiceStatus::Stopped;
        });

        *self.server_handle.lock().await = Some(handle);
        Ok(())
    }

    async fn prepare_video_pipeline(&self, config: &VncConfig) -> Result<()> {
        match config.encoding {
            VncEncoding::TightJpeg => {
                self.video_manager
                    .set_bitrate_preset(BitratePreset::Balanced)
                    .await?;
            }
            VncEncoding::H264 => {
                self.video_manager
                    .set_video_codec(VideoCodecType::H264)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        if let Some(mut handle) = self.server_handle.lock().await.take() {
            match tokio::time::timeout(Duration::from_secs(2), &mut handle).await {
                Ok(Ok(())) => {}
                Ok(Err(err)) if err.is_cancelled() => {}
                Ok(Err(err)) => warn!("VNC server task ended with error: {}", err),
                Err(_) => {
                    warn!("Timed out waiting for VNC server task to stop");
                    handle.abort();
                    let _ = handle.await;
                }
            }
        }
        let mut client_handles = self.client_handles.lock().await;
        for handle in client_handles.drain(..) {
            handle.abort();
        }
        self.active_clients.store(0, Ordering::Relaxed);
        *self.status.write().await = VncServiceStatus::Stopped;
        Ok(())
    }

    pub async fn restart(&self, config: VncConfig) -> Result<()> {
        self.update_config(config).await;
        self.stop().await?;
        self.start().await
    }
}

async fn handle_client(
    stream: TcpStream,
    peer: SocketAddr,
    config: VncConfig,
    video_manager: Arc<VideoStreamManager>,
    hid: Arc<HidController>,
) -> Result<()> {
    let mut client = RfbClient::new(stream, peer, config.clone());
    let (width, height) = initial_frame_size(&config, &video_manager).await;
    client.set_size(width, height);
    client.handshake().await?;
    let (_, _, mut frame_rx) = subscribe_frames(&config, &video_manager).await?;
    let mut shutdown = client.shutdown_receiver();

    loop {
        tokio::select! {
            result = client.read_input_event() => {
                match result? {
                    RfbInputEvent::Ignored => {}
                    RfbInputEvent::Disconnected => break,
                    event => handle_input_event(event, &hid, width, height).await?,
                }
            }
            maybe_frame = frame_rx.recv() => {
                let Some(frame) = maybe_frame else { break };
                client.send_frame(frame).await?;
            }
            _ = shutdown.recv() => break,
        }
    }
    Ok(())
}

async fn initial_frame_size(
    config: &VncConfig,
    video_manager: &Arc<VideoStreamManager>,
) -> (u16, u16) {
    match config.encoding {
        VncEncoding::TightJpeg => {
            let (_, resolution, _, _, _) = video_manager.streamer().current_capture_config().await;
            (resolution.width as u16, resolution.height as u16)
        }
        VncEncoding::H264 => video_manager
            .get_encoding_config()
            .await
            .map(|cfg| (cfg.resolution.width as u16, cfg.resolution.height as u16))
            .unwrap_or((1280, 720)),
    }
}

async fn subscribe_frames(
    config: &VncConfig,
    video_manager: &Arc<VideoStreamManager>,
) -> Result<(u16, u16, tokio::sync::mpsc::Receiver<RfbFrame>)> {
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    match config.encoding {
        VncEncoding::TightJpeg => {
            let handler = video_manager.mjpeg_handler();
            let client_id = format!("vnc-{}", uuid::Uuid::new_v4());
            let guard = ClientGuard::new(client_id.clone(), handler.clone());
            video_manager.streamer().start().await?;
            let current = handler.current_frame();
            let (width, height) = current
                .as_ref()
                .map(|f| (f.width() as u16, f.height() as u16))
                .unwrap_or((800, 600));
            let mut notify = handler.subscribe();
            tokio::spawn(async move {
                let _guard = guard;
                loop {
                    if notify.recv().await.is_err() {
                        break;
                    }
                    let Some(frame) = handler.current_frame() else {
                        continue;
                    };
                    if !frame.online || !frame.is_valid_jpeg() {
                        continue;
                    }
                    let _ = tx
                        .send(RfbFrame::Jpeg {
                            data: frame.data_bytes(),
                            width: frame.width() as u16,
                            height: frame.height() as u16,
                        })
                        .await;
                    handler.record_frame_sent(&client_id);
                }
            });
            Ok((width, height, rx))
        }
        VncEncoding::H264 => {
            video_manager.set_video_codec(VideoCodecType::H264).await?;
            let mut frames = video_manager
                .subscribe_encoded_frames()
                .await
                .ok_or_else(|| {
                    AppError::VideoError("Failed to subscribe to encoded frames".to_string())
                })?;
            let geometry = video_manager
                .get_encoding_config()
                .await
                .map(|cfg| cfg.resolution)
                .unwrap_or(crate::video::format::Resolution::HD720);
            let width = geometry.width as u16;
            let height = geometry.height as u16;
            if let Err(err) = video_manager.request_keyframe().await {
                warn!("Failed to request VNC H264 keyframe: {}", err);
            }
            tokio::spawn(async move {
                while let Some(frame) = frames.recv().await {
                    if frame.codec != crate::video::codec::registry::VideoEncoderType::H264 {
                        continue;
                    }
                    let _ = tx
                        .send(RfbFrame::H264 {
                            data: Bytes::copy_from_slice(&frame.data),
                            width,
                            height,
                            key: frame.is_keyframe,
                        })
                        .await;
                }
            });
            Ok((width, height, rx))
        }
    }
}

async fn handle_input_event(
    event: RfbInputEvent,
    hid: &Arc<HidController>,
    width: u16,
    height: u16,
) -> Result<()> {
    match event {
        RfbInputEvent::Key(key) => {
            if let Some(event) = rfb::key_event_to_hid(key) {
                hid.send_keyboard(event).await?;
            }
        }
        RfbInputEvent::Pointer(pointer) => {
            for event in rfb::pointer_event_to_hid(pointer, width, height) {
                hid.send_mouse(event).await?;
            }
        }
        RfbInputEvent::Clipboard(_) => {}
        RfbInputEvent::Ignored | RfbInputEvent::Disconnected => {}
    }
    Ok(())
}
