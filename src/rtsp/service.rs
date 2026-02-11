use bytes::Bytes;
use base64::Engine;
use rand::Rng;
use rtp::packet::Packet;
use rtp::packetizer::Payloader;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use webrtc::util::Marshal;
use rtsp_types as rtsp;
use sdp_types as sdp;

use crate::config::{RtspCodec, RtspConfig};
use crate::error::{AppError, Result};
use crate::video::encoder::registry::VideoEncoderType;
use crate::video::encoder::VideoCodecType;
use crate::video::shared_video_pipeline::EncodedVideoFrame;
use crate::video::VideoStreamManager;
use crate::webrtc::h265_payloader::H265Payloader;
use crate::webrtc::rtp::parse_profile_level_id_from_sps;

const RTP_CLOCK_RATE: u32 = 90_000;
const RTP_MTU: usize = 1200;
const RTSP_BUF_SIZE: usize = 8192;

#[derive(Debug, Clone, PartialEq)]
pub enum RtspServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

impl std::fmt::Display for RtspServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error(err) => write!(f, "error: {}", err),
        }
    }
}

#[derive(Debug, Clone)]
struct RtspRequest {
    method: rtsp::Method,
    uri: String,
    version: rtsp::Version,
    headers: HashMap<String, String>,
}

struct RtspConnectionState {
    session_id: String,
    setup_done: bool,
    interleaved_channel: u8,
}

impl RtspConnectionState {
    fn new() -> Self {
        Self {
            session_id: generate_session_id(),
            setup_done: false,
            interleaved_channel: 0,
        }
    }
}

#[derive(Default, Clone)]
struct ParameterSets {
    h264_sps: Option<Bytes>,
    h264_pps: Option<Bytes>,
    h265_vps: Option<Bytes>,
    h265_sps: Option<Bytes>,
    h265_pps: Option<Bytes>,
}

#[derive(Clone)]
struct SharedRtspState {
    active_client: Arc<Mutex<Option<SocketAddr>>>,
    parameter_sets: Arc<RwLock<ParameterSets>>,
}

impl SharedRtspState {
    fn new() -> Self {
        Self {
            active_client: Arc::new(Mutex::new(None)),
            parameter_sets: Arc::new(RwLock::new(ParameterSets::default())),
        }
    }
}

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

        let codec = match config.codec {
            RtspCodec::H264 => VideoCodecType::H264,
            RtspCodec::H265 => VideoCodecType::H265,
        };

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

        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| AppError::Io(io::Error::new(e.kind(), format!("RTSP bind failed: {}", e))))?;

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

    let auth_enabled = cfg_snapshot.username.as_ref().is_some_and(|u| !u.is_empty())
        || cfg_snapshot.password.as_ref().is_some_and(|p| !p.is_empty());

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

            if !is_valid_rtsp_path(&req.uri, &cfg_snapshot.path) {
                send_response(
                    &mut stream,
                    &req,
                    404,
                    "Not Found",
                    vec![],
                    "",
                    "",
                )
                .await?;
                continue;
            }

            if auth_enabled {
                let expected_user = cfg_snapshot.username.clone().unwrap_or_default();
                let expected_pass = cfg_snapshot.password.clone().unwrap_or_default();
                let ok = extract_basic_auth(&req)
                    .map(|(u, p)| u == expected_user && p == expected_pass)
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
                            "OPTIONS, DESCRIBE, SETUP, PLAY, TEARDOWN".to_string(),
                        )],
                        "",
                        "",
                    )
                    .await?;
                }
                rtsp::Method::Describe => {
                    let codec = match cfg_snapshot.codec {
                        RtspCodec::H264 => VideoCodecType::H264,
                        RtspCodec::H265 => VideoCodecType::H265,
                    };
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
                        vec![(
                            "Content-Type".to_string(),
                            "application/sdp".to_string(),
                        )],
                        &sdp,
                        &state.session_id,
                    )
                    .await?;
                }
                rtsp::Method::Setup => {
                    let transport = req
                        .headers
                        .get("transport")
                        .cloned()
                        .unwrap_or_default();

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

                    send_response(
                        &mut stream,
                        &req,
                        200,
                        "OK",
                        vec![],
                        "",
                        &state.session_id,
                    )
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
                    send_response(
                        &mut stream,
                        &req,
                        200,
                        "OK",
                        vec![],
                        "",
                        &state.session_id,
                    )
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

async fn stream_video_interleaved(
    stream: TcpStream,
    video_manager: &Arc<VideoStreamManager>,
    rtsp_codec: RtspCodec,
    channel: u8,
    shared: SharedRtspState,
    session_id: String,
) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    let mut rx = video_manager
        .subscribe_encoded_frames()
        .await
        .ok_or_else(|| AppError::VideoError("RTSP failed to subscribe encoded frames".to_string()))?;

    video_manager.request_keyframe().await.ok();

    let payload_type = match rtsp_codec {
        RtspCodec::H264 => 96,
        RtspCodec::H265 => 99,
    };
    let mut sequence_number: u16 = rand::rng().random();
    let ssrc: u32 = rand::rng().random();

    let mut h264_payloader = rtp::codecs::h264::H264Payloader::default();
    let mut h265_payloader = H265Payloader::new();
    let mut ctrl_read_buf = [0u8; RTSP_BUF_SIZE];
    let mut ctrl_buffer = Vec::with_capacity(RTSP_BUF_SIZE);

    loop {
        tokio::select! {
            maybe_frame = rx.recv() => {
                let Some(frame) = maybe_frame else {
                    break;
                };

                if !is_frame_codec_match(&frame, &rtsp_codec) {
                    continue;
                }

                {
                    let mut params = shared.parameter_sets.write().await;
                    update_parameter_sets(&mut params, &frame);
                }

                let rtp_timestamp = pts_to_rtp_timestamp(frame.pts_ms);

                let payloads: Vec<Bytes> = match rtsp_codec {
                    RtspCodec::H264 => h264_payloader
                        .payload(RTP_MTU, &frame.data)
                        .map_err(|e| AppError::VideoError(format!("H264 payload failed: {}", e)))?,
                    RtspCodec::H265 => h265_payloader.payload(RTP_MTU, &frame.data),
                };

                if payloads.is_empty() {
                    continue;
                }

                let total_payloads = payloads.len();
                for (idx, payload) in payloads.into_iter().enumerate() {
                    let marker = idx == total_payloads.saturating_sub(1);
                    let packet = Packet {
                        header: rtp::header::Header {
                            version: 2,
                            padding: false,
                            extension: false,
                            marker,
                            payload_type,
                            sequence_number,
                            timestamp: rtp_timestamp,
                            ssrc,
                            ..Default::default()
                        },
                        payload,
                    };

                    sequence_number = sequence_number.wrapping_add(1);
                    send_interleaved_rtp(&mut writer, channel, &packet).await?;
                }

                if frame.is_keyframe {
                    tracing::debug!("RTSP keyframe sent");
                }
            }
            read_res = reader.read(&mut ctrl_read_buf) => {
                let n = read_res?;
                if n == 0 {
                    break;
                }

                ctrl_buffer.extend_from_slice(&ctrl_read_buf[..n]);

                while strip_interleaved_frames_prefix(&mut ctrl_buffer) {}

                while let Some(raw_req) = take_rtsp_request_from_buffer(&mut ctrl_buffer) {
                    let Some(req) = parse_rtsp_request(&raw_req) else {
                        continue;
                    };

                    if handle_play_control_request(&mut writer, &req, &session_id).await? {
                        return Ok(());
                    }

                    while strip_interleaved_frames_prefix(&mut ctrl_buffer) {}
                }
            }
        }
    }

    Ok(())
}

async fn send_interleaved_rtp<W: AsyncWrite + Unpin>(
    stream: &mut W,
    channel: u8,
    packet: &Packet,
) -> Result<()> {
    let marshaled = packet
        .marshal()
        .map_err(|e| AppError::VideoError(format!("RTP marshal failed: {}", e)))?;
    let len = marshaled.len() as u16;

    let mut header = [0u8; 4];
    header[0] = b'$';
    header[1] = channel;
    header[2] = (len >> 8) as u8;
    header[3] = (len & 0xff) as u8;

    stream.write_all(&header).await?;
    stream.write_all(&marshaled).await?;
    Ok(())
}

async fn handle_play_control_request<W: AsyncWrite + Unpin>(
    stream: &mut W,
    req: &RtspRequest,
    session_id: &str,
) -> Result<bool> {
    match &req.method {
        rtsp::Method::Teardown => {
            send_response(stream, req, 200, "OK", vec![], "", session_id).await?;
            Ok(true)
        }
        rtsp::Method::Options => {
            send_response(
                stream,
                req,
                200,
                "OK",
                vec![(
                    "Public".to_string(),
                    "OPTIONS, DESCRIBE, SETUP, PLAY, GET_PARAMETER, SET_PARAMETER, TEARDOWN"
                        .to_string(),
                )],
                "",
                session_id,
            )
            .await?;
            Ok(false)
        }
        rtsp::Method::GetParameter | rtsp::Method::SetParameter => {
            send_response(stream, req, 200, "OK", vec![], "", session_id).await?;
            Ok(false)
        }
        _ => {
            send_response(
                stream,
                req,
                405,
                "Method Not Allowed",
                vec![],
                "",
                session_id,
            )
            .await?;
            Ok(false)
        }
    }
}

fn strip_interleaved_frames_prefix(buffer: &mut Vec<u8>) -> bool {
    if buffer.len() < 4 || buffer[0] != b'$' {
        return false;
    }

    let payload_len = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
    let frame_len = 4 + payload_len;
    if buffer.len() < frame_len {
        return false;
    }

    buffer.drain(0..frame_len);
    true
}

fn take_rtsp_request_from_buffer(buffer: &mut Vec<u8>) -> Option<String> {
    let delimiter = b"\r\n\r\n";
    let pos = find_bytes(buffer, delimiter)?;
    let req_end = pos + delimiter.len();
    let req_bytes: Vec<u8> = buffer.drain(0..req_end).collect();
    Some(String::from_utf8_lossy(&req_bytes).to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn parse_rtsp_request(raw: &str) -> Option<RtspRequest> {
    let (message, consumed): (rtsp::Message<Vec<u8>>, usize) = rtsp::Message::parse(raw.as_bytes()).ok()?;
    if consumed != raw.len() {
        return None;
    }

    let request = match message {
        rtsp::Message::Request(req) => req,
        _ => return None,
    };

    let uri = request
        .request_uri()
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();

    let mut headers = HashMap::new();
    for (name, value) in request.headers() {
        headers.insert(name.to_string().to_ascii_lowercase(), value.to_string());
    }

    Some(RtspRequest {
        method: request.method().clone(),
        uri,
        version: request.version(),
        headers,
    })
}

fn extract_basic_auth(req: &RtspRequest) -> Option<(String, String)> {
    let value = req.headers.get("authorization")?;
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let b64 = parts.next()?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let raw = String::from_utf8(decoded).ok()?;
    let (user, pass) = raw.split_once(':')?;
    Some((user.to_string(), pass.to_string()))
}

fn parse_interleaved_channel(transport: &str) -> Option<u8> {
    let lower = transport.to_ascii_lowercase();
    if let Some((_, v)) = lower.split_once("interleaved=") {
        let head = v.split(';').next().unwrap_or(v);
        let first = head.split('-').next().unwrap_or(head).trim();
        return first.parse::<u8>().ok();
    }
    None
}

fn update_parameter_sets(params: &mut ParameterSets, frame: &EncodedVideoFrame) {
    let nal_units = split_annexb_nal_units(frame.data.as_ref());

    match frame.codec {
        VideoEncoderType::H264 => {
            for nal in nal_units {
                match h264_nal_type(nal) {
                    Some(7) => params.h264_sps = Some(Bytes::copy_from_slice(nal)),
                    Some(8) => params.h264_pps = Some(Bytes::copy_from_slice(nal)),
                    _ => {}
                }
            }
        }
        VideoEncoderType::H265 => {
            for nal in nal_units {
                match h265_nal_type(nal) {
                    Some(32) => params.h265_vps = Some(Bytes::copy_from_slice(nal)),
                    Some(33) => params.h265_sps = Some(Bytes::copy_from_slice(nal)),
                    Some(34) => params.h265_pps = Some(Bytes::copy_from_slice(nal)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn split_annexb_nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut nal_units = Vec::new();
    let mut cursor = 0usize;

    while let Some((start, start_code_len)) = find_annexb_start_code(data, cursor) {
        let nal_start = start + start_code_len;
        if nal_start >= data.len() {
            break;
        }

        let next_start = find_annexb_start_code(data, nal_start)
            .map(|(idx, _)| idx)
            .unwrap_or(data.len());

        let mut nal_end = next_start;
        while nal_end > nal_start && data[nal_end - 1] == 0 {
            nal_end -= 1;
        }

        if nal_end > nal_start {
            nal_units.push(&data[nal_start..nal_end]);
        }

        cursor = next_start;
    }

    nal_units
}

fn find_annexb_start_code(data: &[u8], from: usize) -> Option<(usize, usize)> {
    if from >= data.len() {
        return None;
    }

    let mut i = from;
    while i + 3 <= data.len() {
        if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            return Some((i, 4));
        }

        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            return Some((i, 3));
        }

        i += 1;
    }

    None
}

fn h264_nal_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|value| value & 0x1f)
}

fn h265_nal_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|value| (value >> 1) & 0x3f)
}

fn build_h264_fmtp(payload_type: u8, params: &ParameterSets) -> String {
    let mut attrs = vec!["packetization-mode=1".to_string()];

    if let Some(sps) = params.h264_sps.as_ref() {
        if let Some(profile_level_id) = parse_profile_level_id_from_sps(sps) {
            attrs.push(format!("profile-level-id={}", profile_level_id));
        }
    } else {
        attrs.push("profile-level-id=42e01f".to_string());
    }

    if let (Some(sps), Some(pps)) = (params.h264_sps.as_ref(), params.h264_pps.as_ref()) {
        let sps_b64 = base64::engine::general_purpose::STANDARD.encode(sps.as_ref());
        let pps_b64 = base64::engine::general_purpose::STANDARD.encode(pps.as_ref());
        attrs.push(format!("sprop-parameter-sets={},{}", sps_b64, pps_b64));
    }

    format!("{} {}", payload_type, attrs.join(";"))
}

fn build_h265_fmtp(payload_type: u8, params: &ParameterSets) -> String {
    let mut attrs = Vec::new();

    if let Some(vps) = params.h265_vps.as_ref() {
        attrs.push(format!(
            "sprop-vps={}",
            base64::engine::general_purpose::STANDARD.encode(vps.as_ref())
        ));
    }

    if let Some(sps) = params.h265_sps.as_ref() {
        attrs.push(format!(
            "sprop-sps={}",
            base64::engine::general_purpose::STANDARD.encode(sps.as_ref())
        ));
    }

    if let Some(pps) = params.h265_pps.as_ref() {
        attrs.push(format!(
            "sprop-pps={}",
            base64::engine::general_purpose::STANDARD.encode(pps.as_ref())
        ));
    }

    if attrs.is_empty() {
        format!("{} profile-id=1", payload_type)
    } else {
        format!("{} {}", payload_type, attrs.join(";"))
    }
}

fn build_sdp(config: &RtspConfig, codec: VideoCodecType, params: &ParameterSets) -> String {
    let (payload_type, codec_name, fmtp_value) = match codec {
        VideoCodecType::H264 => (96u8, "H264", build_h264_fmtp(96, params)),
        VideoCodecType::H265 => (99u8, "H265", build_h265_fmtp(99, params)),
        _ => (96u8, "H264", build_h264_fmtp(96, params)),
    };

    let session = sdp::Session {
        origin: sdp::Origin {
            username: Some("-".to_string()),
            sess_id: "0".to_string(),
            sess_version: 0,
            nettype: "IN".to_string(),
            addrtype: "IP4".to_string(),
            unicast_address: config.bind.clone(),
        },
        session_name: "One-KVM RTSP Stream".to_string(),
        session_description: None,
        uri: None,
        emails: Vec::new(),
        phones: Vec::new(),
        connection: Some(sdp::Connection {
            nettype: "IN".to_string(),
            addrtype: "IP4".to_string(),
            connection_address: "0.0.0.0".to_string(),
        }),
        bandwidths: Vec::new(),
        times: vec![sdp::Time {
            start_time: 0,
            stop_time: 0,
            repeats: Vec::new(),
        }],
        time_zones: Vec::new(),
        key: None,
        attributes: vec![sdp::Attribute {
            attribute: "control".to_string(),
            value: Some("*".to_string()),
        }],
        medias: vec![sdp::Media {
            media: "video".to_string(),
            port: 0,
            num_ports: None,
            proto: "RTP/AVP".to_string(),
            fmt: payload_type.to_string(),
            media_title: None,
            connections: Vec::new(),
            bandwidths: Vec::new(),
            key: None,
            attributes: vec![
                sdp::Attribute {
                    attribute: "rtpmap".to_string(),
                    value: Some(format!("{} {}/90000", payload_type, codec_name)),
                },
                sdp::Attribute {
                    attribute: "fmtp".to_string(),
                    value: Some(fmtp_value),
                },
                sdp::Attribute {
                    attribute: "control".to_string(),
                    value: Some("trackID=0".to_string()),
                },
            ],
        }],
    };

    let mut output = Vec::new();
    if let Err(err) = session.write(&mut output) {
        tracing::warn!("Failed to serialize SDP with sdp-types: {}", err);
        return String::new();
    }

    match String::from_utf8(output) {
        Ok(sdp_text) => sdp_text,
        Err(err) => {
            tracing::warn!("Failed to convert SDP bytes to UTF-8: {}", err);
            String::new()
        }
    }
}

async fn send_simple_response<W: AsyncWrite + Unpin>(
    stream: &mut W,
    code: u16,
    _reason: &str,
    cseq: Option<&str>,
    body: &str,
) -> Result<()> {
    let mut builder = rtsp::Response::builder(rtsp::Version::V1_0, status_code_from_u16(code));
    if let Some(cseq) = cseq {
        builder = builder.header(rtsp::headers::CSEQ, cseq);
    }

    let response = builder.build(body.as_bytes().to_vec());

    let mut data = Vec::new();
    response
        .write(&mut data)
        .map_err(|e| AppError::BadRequest(format!("failed to serialize RTSP response: {}", e)))?;
    stream.write_all(&data).await?;
    Ok(())
}

async fn send_response<W: AsyncWrite + Unpin>(
    stream: &mut W,
    req: &RtspRequest,
    code: u16,
    _reason: &str,
    extra_headers: Vec<(String, String)>,
    body: &str,
    session_id: &str,
) -> Result<()> {
    let cseq = req
        .headers
        .get("cseq")
        .cloned()
        .unwrap_or_else(|| "1".to_string());

    let mut builder = rtsp::Response::builder(req.version, status_code_from_u16(code))
        .header(rtsp::headers::CSEQ, cseq.as_str());

    if !session_id.is_empty() {
        builder = builder.header(rtsp::headers::SESSION, session_id);
    }

    for (name, value) in extra_headers {
        let header_name = rtsp::HeaderName::try_from(name.as_str()).map_err(|e| {
            AppError::BadRequest(format!("invalid RTSP header name {}: {}", name, e))
        })?;
        builder = builder.header(header_name, value);
    }

    let response = builder.build(body.as_bytes().to_vec());

    let mut data = Vec::new();
    response
        .write(&mut data)
        .map_err(|e| AppError::BadRequest(format!("failed to serialize RTSP response: {}", e)))?;
    stream.write_all(&data).await?;
    Ok(())
}

fn status_code_from_u16(code: u16) -> rtsp::StatusCode {
    match code {
        200 => rtsp::StatusCode::Ok,
        400 => rtsp::StatusCode::BadRequest,
        401 => rtsp::StatusCode::Unauthorized,
        404 => rtsp::StatusCode::NotFound,
        405 => rtsp::StatusCode::MethodNotAllowed,
        453 => rtsp::StatusCode::NotEnoughBandwidth,
        455 => rtsp::StatusCode::MethodNotValidInThisState,
        _ => rtsp::StatusCode::InternalServerError,
    }
}

fn is_valid_rtsp_path(uri: &str, configured_path: &str) -> bool {
    let normalized_cfg = configured_path.trim_matches('/');
    if normalized_cfg.is_empty() {
        return false;
    }

    let request_path = extract_rtsp_path(uri);
    request_path == normalized_cfg
}

fn extract_rtsp_path(uri: &str) -> String {
    let raw_path = if let Some((_, remainder)) = uri.split_once("://") {
        match remainder.find('/') {
            Some(idx) => &remainder[idx..],
            None => "/",
        }
    } else {
        uri
    };

    raw_path
        .split('?')
        .next()
        .unwrap_or(raw_path)
        .split('#')
        .next()
        .unwrap_or(raw_path)
        .trim_matches('/')
        .to_string()
}

fn is_frame_codec_match(frame: &EncodedVideoFrame, codec: &RtspCodec) -> bool {
    matches!(
        (frame.codec, codec),
        (crate::video::encoder::registry::VideoEncoderType::H264, RtspCodec::H264)
            | (crate::video::encoder::registry::VideoEncoderType::H265, RtspCodec::H265)
    )
}

fn pts_to_rtp_timestamp(pts_ms: i64) -> u32 {
    if pts_ms <= 0 {
        return 0;
    }
    ((pts_ms as u64 * RTP_CLOCK_RATE as u64) / 1000) as u32
}

fn generate_session_id() -> String {
    let mut rng = rand::rng();
    let value: u64 = rng.random();
    format!("{:016x}", value)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncReadExt};

    fn make_test_request(method: rtsp::Method) -> RtspRequest {
        let mut headers = HashMap::new();
        headers.insert("cseq".to_string(), "7".to_string());
        RtspRequest {
            method,
            uri: "rtsp://127.0.0.1/live".to_string(),
            version: rtsp::Version::V1_0,
            headers,
        }
    }

    async fn read_response_from_duplex(mut client: tokio::io::DuplexStream) -> rtsp::Response<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        let n = client.read(&mut buf).await.expect("failed to read rtsp response");
        assert!(n > 0);
        let (message, consumed): (rtsp::Message<Vec<u8>>, usize) =
            rtsp::Message::parse(&buf[..n]).expect("failed to parse rtsp response");
        assert_eq!(consumed, n);

        match message {
            rtsp::Message::Response(response) => response,
            _ => panic!("expected RTSP response"),
        }
    }

    #[tokio::test]
    async fn play_control_teardown_returns_ok_and_stop() {
        let req = make_test_request(rtsp::Method::Teardown);
        let (client, mut server) = duplex(4096);

        let should_stop = handle_play_control_request(&mut server, &req, "session-1")
            .await
            .expect("control handling failed");
        assert!(should_stop);

        drop(server);
        let response = read_response_from_duplex(client).await;
        assert_eq!(response.status(), rtsp::StatusCode::Ok);
    }

    #[tokio::test]
    async fn play_control_pause_returns_method_not_allowed() {
        let req = make_test_request(rtsp::Method::Pause);
        let (client, mut server) = duplex(4096);

        let should_stop = handle_play_control_request(&mut server, &req, "session-1")
            .await
            .expect("control handling failed");
        assert!(!should_stop);

        drop(server);
        let response = read_response_from_duplex(client).await;
        assert_eq!(response.status(), rtsp::StatusCode::MethodNotAllowed);
    }

    #[test]
    fn build_sdp_h264_is_parseable_with_expected_video_attributes() {
        let config = RtspConfig::default();
        let mut params = ParameterSets::default();
        params.h264_sps = Some(Bytes::from_static(&[0x67, 0x42, 0xe0, 0x1f, 0x96, 0x54]));
        params.h264_pps = Some(Bytes::from_static(&[0x68, 0xce, 0x06, 0xe2]));

        let sdp_text = build_sdp(&config, VideoCodecType::H264, &params);
        assert!(!sdp_text.is_empty());

        let session = sdp::Session::parse(sdp_text.as_bytes()).expect("sdp parse failed");
        assert_eq!(session.session_name, "One-KVM RTSP Stream");
        assert_eq!(session.medias.len(), 1);

        let media = &session.medias[0];
        assert_eq!(media.media, "video");
        assert_eq!(media.proto, "RTP/AVP");
        assert_eq!(media.fmt, "96");

        let has_rtpmap = media.attributes.iter().any(|attr| {
            attr.attribute == "rtpmap" && attr.value.as_deref() == Some("96 H264/90000")
        });
        assert!(has_rtpmap);

        let fmtp_value = media
            .attributes
            .iter()
            .find(|attr| attr.attribute == "fmtp")
            .and_then(|attr| attr.value.as_deref())
            .expect("missing fmtp value");
        assert!(fmtp_value.starts_with("96 "));
        assert!(fmtp_value.contains("packetization-mode=1"));
        assert!(fmtp_value.contains("sprop-parameter-sets="));
    }


    #[test]
    fn rtsp_path_matching_is_exact_after_normalization() {
        assert!(is_valid_rtsp_path("rtsp://127.0.0.1/live", "live"));
        assert!(is_valid_rtsp_path("rtsp://127.0.0.1/live/?token=1", "/live/"));
        assert!(!is_valid_rtsp_path("rtsp://127.0.0.1/live2", "live"));
        assert!(!is_valid_rtsp_path("rtsp://127.0.0.1/", "/"));
    }

    #[test]
    fn build_sdp_h265_is_parseable_with_expected_video_attributes() {
        let config = RtspConfig::default();
        let mut params = ParameterSets::default();
        params.h265_vps = Some(Bytes::from_static(&[0x40, 0x01, 0x0c, 0x01]));
        params.h265_sps = Some(Bytes::from_static(&[0x42, 0x01, 0x01, 0x60]));
        params.h265_pps = Some(Bytes::from_static(&[0x44, 0x01, 0xc0, 0x73]));

        let sdp_text = build_sdp(&config, VideoCodecType::H265, &params);
        assert!(!sdp_text.is_empty());

        let session = sdp::Session::parse(sdp_text.as_bytes()).expect("sdp parse failed");
        assert_eq!(session.medias.len(), 1);

        let media = &session.medias[0];
        assert_eq!(media.media, "video");
        assert_eq!(media.proto, "RTP/AVP");
        assert_eq!(media.fmt, "99");

        let has_rtpmap = media.attributes.iter().any(|attr| {
            attr.attribute == "rtpmap" && attr.value.as_deref() == Some("99 H265/90000")
        });
        assert!(has_rtpmap);

        let fmtp_value = media
            .attributes
            .iter()
            .find(|attr| attr.attribute == "fmtp")
            .and_then(|attr| attr.value.as_deref())
            .expect("missing fmtp value");
        assert!(fmtp_value.starts_with("99 "));
        assert!(fmtp_value.contains("sprop-vps="));
        assert!(fmtp_value.contains("sprop-sps="));
        assert!(fmtp_value.contains("sprop-pps="));
    }
}
