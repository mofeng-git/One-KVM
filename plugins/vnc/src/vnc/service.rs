//! Minimal VNC/RFB service for direct JPEG/H264 frame forwarding.

use super::VncServiceStatus;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::config::{VncConfig, VncEncoding};
use one_kvm_plugin_sdk::runtime::ServiceError as AppError;
type Result<T> = std::result::Result<T, AppError>;
use one_kvm_plugin_sdk::net::{bind_socket_addr, bind_tcp_listener};
use one_kvm_plugin_sdk::runtime::Client;
use one_kvm_plugin_sdk::Codec;

use super::rfb::{self, FrameSendOutcome, RfbClient, RfbFrame, RfbInputEvent};

struct ActiveClientGuard(Arc<AtomicUsize>);

impl Drop for ActiveClientGuard {
    fn drop(&mut self) {
        let _ = self
            .0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                Some(count.saturating_sub(1))
            });
    }
}

pub struct VncService {
    config: Arc<RwLock<VncConfig>>,
    status: Arc<RwLock<VncServiceStatus>>,
    video_manager: Arc<Client>,
    shutdown_tx: broadcast::Sender<()>,
    server_handle: Mutex<Option<JoinHandle<()>>>,
    client_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    active_clients: Arc<AtomicUsize>,
}

impl VncService {
    pub fn new(config: VncConfig, video_manager: Arc<Client>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(VncServiceStatus::Stopped)),
            video_manager,
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
        let bind_addr = match bind_socket_addr(&config.bind, config.port) {
            Ok(addr) => addr,
            Err(err) => {
                let error = AppError::BadRequest(format!("Invalid VNC bind address: {}", err));
                *self.status.write().await = VncServiceStatus::Error(error.to_string());
                return Err(error);
            }
        };
        let listener = match bind_tcp_listener(bind_addr) {
            Ok(listener) => listener,
            Err(err) => {
                let error = AppError::Io(std::io::Error::new(
                    err.kind(),
                    format!("VNC bind failed: {}", err),
                ));
                *self.status.write().await = VncServiceStatus::Error(error.to_string());
                return Err(error);
            }
        };
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                let error = AppError::Io(std::io::Error::new(
                    err.kind(),
                    format!("VNC listener setup failed: {}", err),
                ));
                *self.status.write().await = VncServiceStatus::Error(error.to_string());
                return Err(error);
            }
        };

        if let Err(err) = self.prepare_video_pipeline(&config).await {
            *self.status.write().await = VncServiceStatus::Error(err.to_string());
            return Err(err);
        }

        let config_ref = self.config.clone();
        let video_manager = self.video_manager.clone();
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
                                let reserved = if cfg.allow_one_client {
                                    active_clients
                                        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                                        .is_ok()
                                } else {
                                    active_clients.fetch_add(1, Ordering::AcqRel);
                                    true
                                };
                                if !reserved {
                                    warn!("Rejecting VNC client {} because another client is active", peer);
                                    drop(stream);
                                    continue;
                                }
                                let vm = video_manager.clone();
                                let active = active_clients.clone();
                                let handle = tokio::spawn(async move {
                                    let _active_guard = ActiveClientGuard(active);
                                    let result = handle_client(stream, peer, cfg, vm).await;
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
        self.video_manager.claim_video(output_codec(config)).await?;
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
    video_manager: Arc<Client>,
) -> Result<()> {
    let mut client = RfbClient::new(stream, peer, config.clone());
    let (width, height) = initial_frame_size(&video_manager).await;
    client.set_size(width, height);
    client.handshake().await?;
    tracing::debug!("VNC client {} ClientInit shared={}", peer, client.shared());
    let (_, _, mut frame_rx) = subscribe_frames(&config, &video_manager).await?;
    let mut latest_frame = frame_rx.borrow().clone();
    let mut latest_size = latest_frame.as_ref().map(RfbFrame::size);
    let mut shutdown = client.shutdown_receiver();

    loop {
        tokio::select! {
            biased;
            result = client.read_input_event() => {
                match result? {
                    RfbInputEvent::Disconnected => break,
                    RfbInputEvent::Key(key) => {
                        if let Some(event) = client.key_event_to_hid(key) {
                            video_manager.send_keyboard(event).await?;
                        }
                    }
                    RfbInputEvent::Pointer(pointer) => {
                        let (width, height) = client.framebuffer_size();
                        for event in rfb::pointer_event_to_hid(pointer, width, height) {
                            video_manager.send_mouse(event).await?;
                        }
                    }
                    RfbInputEvent::SetEncodings { encoding_enabled, resumed } => {
                        if !encoding_enabled {
                            tracing::debug!("VNC client {} paused the configured encoding", peer);
                        }
                        if resumed && config.encoding == VncEncoding::H264 {
                            request_vnc_keyframe(&video_manager, "encoding resume").await;
                        }
                    }
                    RfbInputEvent::FramebufferUpdateRequest(request) => {
                        if !request.incremental && config.encoding == VncEncoding::H264 {
                            request_vnc_keyframe(&video_manager, "non-incremental refresh").await;
                        }
                    }
                    RfbInputEvent::SetPixelFormat(format) => {
                        tracing::debug!(
                            "VNC client {} selected {} bpp true-colour={}",
                            peer,
                            format.bits_per_pixel,
                            format.true_colour
                        );
                    }
                    RfbInputEvent::UnsupportedClientCutText => {
                        tracing::debug!("Ignoring unsupported VNC ClientCutText from {}", peer);
                    }
                }
            }
            changed = frame_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                latest_frame = frame_rx.borrow_and_update().clone();
                let new_size = latest_frame.as_ref().map(RfbFrame::size);
                if config.encoding == VncEncoding::H264
                    && latest_size.is_some()
                    && new_size != latest_size
                {
                    request_vnc_keyframe(&video_manager, "source resolution change").await;
                }
                latest_size = new_size;
            }
            _ = shutdown.recv() => break,
        }

        if client.has_pending_request()
            && latest_frame.is_some()
            && !client.has_complete_buffered_input()?
            && send_latest_frame(&mut client, latest_frame.as_ref()).await?
                == FrameSendOutcome::DesktopSizeSent
            && config.encoding == VncEncoding::H264
        {
            request_vnc_keyframe(&video_manager, "framebuffer resize").await;
        }
    }
    Ok(())
}

async fn initial_frame_size(video_manager: &Arc<Client>) -> (u16, u16) {
    video_manager
        .video_info()
        .await
        .map(|info| (info.width as u16, info.height as u16))
        .unwrap_or((1280, 720))
}

fn output_codec(config: &VncConfig) -> Codec {
    match config.encoding {
        VncEncoding::TightJpeg => Codec::Mjpeg,
        VncEncoding::H264 => Codec::H264,
    }
}

async fn subscribe_frames(
    config: &VncConfig,
    video_manager: &Arc<Client>,
) -> Result<(u16, u16, watch::Receiver<Option<RfbFrame>>)> {
    let codec = output_codec(config);
    let (width, height) = initial_frame_size(video_manager).await;
    let mut frames = video_manager.subscribe_video(codec).await?;
    let (tx, rx) = watch::channel(None);
    if codec == Codec::H264 {
        request_vnc_keyframe(video_manager, "initial frame").await;
    }
    tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                _ = tx.closed() => break,
                frame = frames.recv() => frame,
            };
            let Some(frame) = frame else { break };
            let width = frame.header.width.max(1) as u16;
            let height = frame.header.height.max(1) as u16;
            let sequence = frame.header.sequence;
            let frame = if codec == Codec::Mjpeg {
                RfbFrame::Jpeg {
                    data: frame.data,
                    width,
                    height,
                    sequence,
                }
            } else {
                RfbFrame::H264 {
                    data: frame.data,
                    width,
                    height,
                    key: frame.header.keyframe,
                    sequence,
                }
            };
            tx.send_replace(Some(frame));
        }
    });
    Ok((width, height, rx))
}

async fn send_latest_frame(
    client: &mut RfbClient,
    frame: Option<&RfbFrame>,
) -> Result<FrameSendOutcome> {
    match frame {
        Some(frame) => client.send_frame(frame).await,
        None => Ok(FrameSendOutcome::NotSent),
    }
}

async fn request_vnc_keyframe(video_manager: &Client, reason: &str) {
    if let Err(err) = video_manager.request_keyframe().await {
        warn!(
            "Failed to request VNC H264 keyframe for {}: {}",
            reason, err
        );
    }
}
