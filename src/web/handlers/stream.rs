use super::*;

use crate::video::streamer::StreamerStats;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

fn stream_mode_label(mode: StreamMode, codec: crate::video::codec::VideoCodecType) -> &'static str {
    match mode {
        StreamMode::Mjpeg => "mjpeg",
        StreamMode::WebRTC => codec_to_id(codec),
    }
}

/// Get stream state
pub async fn stream_state(State(state): State<Arc<AppState>>) -> Json<StreamerStats> {
    Json(state.stream_manager.stats().await)
}

/// Start streaming
pub async fn stream_start(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.stream_manager.start().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Streaming started".to_string()),
    }))
}

/// Stop streaming
pub async fn stream_stop(State(state): State<Arc<AppState>>) -> Result<Json<LoginResponse>> {
    state.stream_manager.stop().await?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Streaming stopped".to_string()),
    }))
}

/// Stream mode request
#[derive(Deserialize)]
pub struct SetStreamModeRequest {
    /// Target mode: "mjpeg" or "webrtc"
    pub mode: String,
}

/// Stream mode response
#[derive(Serialize)]
pub struct StreamModeResponse {
    pub success: bool,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_id: Option<String>,
    pub switching: bool,
    pub message: Option<String>,
}

/// Get current stream mode
pub async fn stream_mode_get(State(state): State<Arc<AppState>>) -> Json<StreamModeResponse> {
    let mode = state.stream_manager.current_mode().await;
    let codec = state.stream_manager.current_video_codec().await;
    let mode_str = stream_mode_label(mode, codec).to_string();

    Json(StreamModeResponse {
        success: true,
        mode: mode_str,
        transition_id: state.stream_manager.current_transition_id().await,
        switching: state.stream_manager.is_switching(),
        message: None,
    })
}

/// Set stream mode (switch between MJPEG and WebRTC)
pub async fn stream_mode_set(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetStreamModeRequest>,
) -> Result<Json<StreamModeResponse>> {
    use crate::video::codec::VideoCodecType;

    let constraints = state.stream_manager.codec_constraints().await;

    let mode_lower = req.mode.to_lowercase();
    let (new_mode, video_codec) = match mode_lower.as_str() {
        "mjpeg" => (StreamMode::Mjpeg, None),
        "webrtc" | "h264" => (StreamMode::WebRTC, Some(VideoCodecType::H264)),
        "h265" => (StreamMode::WebRTC, Some(VideoCodecType::H265)),
        "vp8" => (StreamMode::WebRTC, Some(VideoCodecType::VP8)),
        "vp9" => (StreamMode::WebRTC, Some(VideoCodecType::VP9)),
        _ => {
            return Err(AppError::BadRequest(format!(
                "Invalid mode '{}'. Valid modes: mjpeg, h264, h265, vp8, vp9",
                req.mode
            )));
        }
    };

    if new_mode == StreamMode::Mjpeg && !constraints.is_mjpeg_allowed() {
        return Err(AppError::BadRequest(format!(
            "Codec 'mjpeg' is not allowed: {}",
            constraints.reason
        )));
    }

    if let Some(codec) = video_codec {
        if !constraints.is_webrtc_codec_allowed(codec) {
            return Err(AppError::BadRequest(format!(
                "Codec '{}' is not allowed: {}",
                codec_to_id(codec),
                constraints.reason
            )));
        }
    }

    let requested_mode_str = match (&new_mode, &video_codec) {
        (StreamMode::Mjpeg, _) => "mjpeg",
        (StreamMode::WebRTC, Some(VideoCodecType::H264)) => "h264",
        (StreamMode::WebRTC, Some(VideoCodecType::H265)) => "h265",
        (StreamMode::WebRTC, Some(VideoCodecType::VP8)) => "vp8",
        (StreamMode::WebRTC, Some(VideoCodecType::VP9)) => "vp9",
        (StreamMode::WebRTC, None) => "webrtc",
    };

    // Detect codec-only switch: already in WebRTC mode, just changing codec.
    // switch_mode_transaction treats this as "no switch needed" since StreamMode
    // is still WebRTC, so we handle codec change + event emission here.
    let current_mode = state.stream_manager.current_mode().await;
    let prev_codec = state.stream_manager.current_video_codec().await;

    let codec_changed = video_codec.is_some_and(|c| c != prev_codec);
    let is_codec_only_switch =
        current_mode == StreamMode::WebRTC && new_mode == StreamMode::WebRTC && codec_changed;

    if let Some(codec) = video_codec {
        info!("Setting WebRTC video codec to {:?}", codec);
        if let Err(e) = state.stream_manager.set_video_codec(codec).await {
            warn!("Failed to set video codec: {}", e);
        }
    }

    // For codec-only switch, emit events directly instead of going through
    // switch_mode_transaction (which short-circuits when mode is unchanged).
    if is_codec_only_switch {
        let transition_id = uuid::Uuid::new_v4().to_string();

        state
            .stream_manager
            .notify_codec_switch(&transition_id, requested_mode_str, &codec_to_id(prev_codec))
            .await;

        return Ok(Json(StreamModeResponse {
            success: true,
            mode: requested_mode_str.to_string(),
            transition_id: Some(transition_id),
            switching: false,
            message: Some(format!("Codec switched to {}", requested_mode_str)),
        }));
    }

    let tx = state
        .stream_manager
        .switch_mode_transaction(new_mode.clone())
        .await?;

    let active_mode = state.stream_manager.current_mode().await;
    let active_codec = state.stream_manager.current_video_codec().await;
    let active_mode_str = stream_mode_label(active_mode, active_codec).to_string();

    let no_switch_needed = !tx.accepted && !tx.switching && tx.transition_id.is_none();
    Ok(Json(StreamModeResponse {
        success: tx.accepted || no_switch_needed,
        mode: if tx.accepted {
            requested_mode_str.to_string()
        } else {
            active_mode_str
        },
        transition_id: tx.transition_id,
        switching: tx.switching,
        message: Some(if tx.accepted {
            format!("Switching to {} mode", requested_mode_str)
        } else if tx.switching {
            "Mode switch already in progress".to_string()
        } else {
            "No switch needed".to_string()
        }),
    }))
}

/// Available video codec info
#[derive(Serialize)]
pub struct VideoCodecInfo {
    /// Codec identifier (mjpeg, h264, h265, vp8, vp9)
    pub id: String,
    /// Display name
    pub name: String,
    /// Protocol (http or webrtc)
    pub protocol: String,
    /// Whether hardware accelerated
    pub hardware: bool,
    /// Encoder backend name (e.g., "vaapi", "nvenc", "software")
    pub backend: Option<String>,
    /// Whether this codec is available
    pub available: bool,
}

/// Encoder backend info
#[derive(Serialize)]
pub struct EncoderBackendInfo {
    /// Backend identifier (vaapi, nvenc, qsv, amf, rkmpp, v4l2m2m, software)
    pub id: String,
    /// Display name
    pub name: String,
    /// Whether this is a hardware backend
    pub is_hardware: bool,
    /// Supported video formats (h264, h265, vp8, vp9)
    pub supported_formats: Vec<String>,
}

/// Available codecs response
#[derive(Serialize)]
pub struct AvailableCodecsResponse {
    pub success: bool,
    /// Available encoder backends
    pub backends: Vec<EncoderBackendInfo>,
    /// Available codecs (for backward compatibility)
    pub codecs: Vec<VideoCodecInfo>,
}

/// Stream constraints response
#[derive(Serialize)]
pub struct StreamConstraintsResponse {
    pub success: bool,
    pub allowed_codecs: Vec<String>,
    pub locked_codec: Option<String>,
    pub disallow_mjpeg: bool,
    pub sources: ConstraintSources,
    pub reason: String,
    pub current_mode: String,
}

#[derive(Serialize)]
pub struct ConstraintSources {
    pub rustdesk: bool,
    pub rtsp: bool,
    pub vnc: bool,
}

/// Get stream codec constraints derived from enabled services.
pub async fn stream_constraints_get(
    State(state): State<Arc<AppState>>,
) -> Json<StreamConstraintsResponse> {
    let constraints = state.stream_manager.codec_constraints().await;
    let current_mode = state.stream_manager.current_mode().await;
    let current_codec = state.stream_manager.current_video_codec().await;
    let current_mode = stream_mode_label(current_mode, current_codec).to_string();

    Json(StreamConstraintsResponse {
        success: true,
        allowed_codecs: constraints
            .allowed_codecs_for_api()
            .into_iter()
            .map(str::to_string)
            .collect(),
        locked_codec: constraints
            .locked_codec
            .map(codec_to_id)
            .map(str::to_string),
        disallow_mjpeg: !constraints.allow_mjpeg,
        sources: ConstraintSources {
            rustdesk: constraints.rustdesk_enabled,
            rtsp: constraints.rtsp_enabled,
            vnc: constraints.vnc_enabled,
        },
        reason: constraints.reason,
        current_mode,
    })
}

/// Set bitrate request
#[derive(Deserialize)]
pub struct SetBitrateRequest {
    pub bitrate_preset: BitratePreset,
}

/// Set stream bitrate (real-time adjustment)
pub async fn stream_set_bitrate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetBitrateRequest>,
) -> Result<Json<LoginResponse>> {
    // Update config
    state
        .config
        .update(|config| {
            config.stream.bitrate_preset = req.bitrate_preset;
        })
        .await?;

    // Apply to WebRTC streamer (real-time adjustment)
    if let Err(e) = state
        .stream_manager
        .set_bitrate_preset(req.bitrate_preset)
        .await
    {
        warn!("Failed to set bitrate dynamically: {}", e);
        // Don't fail the request - config is saved, will apply on next connection
    } else {
        info!("Bitrate updated to {}", req.bitrate_preset);
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Bitrate set to {}", req.bitrate_preset)),
    }))
}

/// Get available video codecs
pub async fn stream_codecs_list() -> Json<AvailableCodecsResponse> {
    use crate::video::codec::registry::{EncoderRegistry, VideoEncoderType};

    let registry = EncoderRegistry::global();

    // Build backends list
    let mut backends = Vec::new();
    for backend in registry.available_backends() {
        let formats = registry.formats_for_backend(backend);
        let format_ids: Vec<String> = formats
            .iter()
            .copied()
            .map(crate::video::codec_constraints::encoder_codec_to_id)
            .map(String::from)
            .collect();

        backends.push(EncoderBackendInfo {
            id: format!("{:?}", backend).to_lowercase(),
            name: backend.display_name().to_string(),
            is_hardware: backend.is_hardware(),
            supported_formats: format_ids,
        });
    }

    // Build codecs list (for backward compatibility)
    let mut codecs = Vec::new();

    // MJPEG is always available (HTTP streaming)
    codecs.push(VideoCodecInfo {
        id: "mjpeg".to_string(),
        name: "MJPEG / HTTP".to_string(),
        protocol: "http".to_string(),
        hardware: false,
        backend: Some("software".to_string()),
        available: true,
    });

    // Check H264 availability (supports software fallback)
    let h264_encoder = registry.best_available_encoder(VideoEncoderType::H264);
    codecs.push(VideoCodecInfo {
        id: "h264".to_string(),
        name: "H.264 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: h264_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: h264_encoder.map(|e| e.backend.to_string()),
        available: h264_encoder.is_some(),
    });

    // Check H265 availability (now supports software too)
    let h265_encoder = registry.best_available_encoder(VideoEncoderType::H265);
    codecs.push(VideoCodecInfo {
        id: "h265".to_string(),
        name: "H.265 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: h265_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: h265_encoder.map(|e| e.backend.to_string()),
        available: h265_encoder.is_some(),
    });

    // Check VP8 availability (now supports software too)
    let vp8_encoder = registry.best_available_encoder(VideoEncoderType::VP8);
    codecs.push(VideoCodecInfo {
        id: "vp8".to_string(),
        name: "VP8 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: vp8_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: vp8_encoder.map(|e| e.backend.to_string()),
        available: vp8_encoder.is_some(),
    });

    // Check VP9 availability (now supports software too)
    let vp9_encoder = registry.best_available_encoder(VideoEncoderType::VP9);
    codecs.push(VideoCodecInfo {
        id: "vp9".to_string(),
        name: "VP9 / WebRTC".to_string(),
        protocol: "webrtc".to_string(),
        hardware: vp9_encoder.map(|e| e.is_hardware).unwrap_or(false),
        backend: vp9_encoder.map(|e| e.backend.to_string()),
        available: vp9_encoder.is_some(),
    });

    Json(AvailableCodecsResponse {
        success: true,
        backends,
        codecs,
    })
}

/// Run hardware encoder smoke tests across common resolutions/codecs.
pub async fn video_encoder_self_check() -> Json<VideoEncoderSelfCheckResponse> {
    let response = tokio::task::spawn_blocking(run_hardware_self_check)
        .await
        .unwrap_or_else(|_| build_hardware_self_check_runtime_error());

    Json(response)
}

/// Query parameters for MJPEG stream
#[derive(Deserialize, Default)]
pub struct MjpegStreamQuery {
    /// Optional client ID (if not provided, a random UUID will be generated)
    pub client_id: Option<String>,
}

/// MJPEG stream endpoint
pub async fn mjpeg_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MjpegStreamQuery>,
) -> impl IntoResponse {
    // Check if MJPEG mode is active
    if !state.stream_manager.is_mjpeg_enabled().await {
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                r#"{"error":"MJPEG mode not active. Current mode is WebRTC."}"#,
            ))
            .unwrap();
    }

    // Check if config is being changed - reject new connections during config change
    if state.stream_manager.is_config_changing() {
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                r#"{"error":"Video configuration is being changed. Please retry shortly."}"#,
            ))
            .unwrap();
    }

    // Ensure stream is started (but not during config change)
    if !state.stream_manager.is_streaming().await && !state.stream_manager.is_config_changing() {
        if let Err(e) = state.stream_manager.start().await {
            tracing::error!("Failed to auto-start stream: {}", e);
        }
    }

    let handler = state.stream_manager.mjpeg_handler();

    // Use provided client ID or generate a new one
    let client_id = query
        .client_id
        .filter(|id| !id.is_empty() && id.len() <= 64) // Validate: non-empty, max 64 chars
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Create RAII guard - this will automatically register and unregister the client
    let guard = Arc::new(crate::stream::mjpeg::ClientGuard::new(
        client_id.clone(),
        handler.clone(),
    ));

    let (tx, mut rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);

    let guard_clone = guard.clone();
    let handler_clone = handler.clone();
    tokio::spawn(async move {
        let _guard = guard_clone; // Keep guard alive
        let mut notify_rx = handler_clone.subscribe();
        let mut last_seq = 0u64;
        let mut timeout_count = 0;

        // Send initial frame if available
        if let Some(frame) = handler_clone.current_frame() {
            if frame.is_valid_jpeg() {
                let data = create_mjpeg_part(frame.data());
                // send() blocks until receiver is ready (backpressure)
                if tx.send(data).await.is_ok() {
                    // FPS recording moved to async_stream after yield
                    last_seq = frame.sequence;
                } else {
                    return; // Receiver closed
                }
            }
        }

        loop {
            // Check if stream went offline (e.g., during config change)
            if !handler_clone.is_online() {
                break;
            }

            // Wait for new frame notification with timeout
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(5), notify_rx.recv()).await;

            match result {
                Ok(Ok(())) => {
                    // Check online status after receiving notification
                    // set_offline() sends a notification, so we need to check here
                    if !handler_clone.is_online() {
                        break;
                    }
                    timeout_count = 0;
                    if let Some(frame) = handler_clone.current_frame() {
                        // Use != instead of > to handle sequence reset when capturer restarts
                        // (e.g., after video config change, new capturer starts from seq=0)
                        if frame.sequence != last_seq && frame.is_valid_jpeg() {
                            let data = create_mjpeg_part(frame.data());
                            if tx.send(data).await.is_ok() {
                                last_seq = frame.sequence;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    break;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    // Receiver was too slow - skip missed frames and jump to latest
                    if !handler_clone.is_online() {
                        break;
                    }
                    timeout_count = 0;

                    if let Some(frame) = handler_clone.current_frame() {
                        if frame.is_valid_jpeg() {
                            // Send current frame immediately and reset sequence tracking
                            if tx.send(create_mjpeg_part(frame.data())).await.is_ok() {
                                last_seq = frame.sequence;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Timeout - check if still online
                    timeout_count += 1;
                    if timeout_count > 6 || !handler_clone.is_online() {
                        break;
                    }
                    // Send last frame again to keep connection alive
                    let Some(frame) = handler_clone.current_frame() else {
                        continue;
                    };

                    if frame.is_valid_jpeg()
                        && tx.send(create_mjpeg_part(frame.data())).await.is_err()
                    {
                        break;
                    }
                }
            }
        }
    });

    // Create stream that receives from channel and forwards to the HTTP
    // body. Record FPS *before* yield so the final frame of a session
    // still gets counted (after-yield code in async_stream! only runs
    // when the consumer polls again, which never happens for the last
    // frame of a closing connection).
    let handler_for_stream = handler.clone();
    let guard_for_stream = guard.clone();
    let body_stream = async_stream::stream! {
        while let Some(data) = rx.recv().await {
            handler_for_stream.record_frame_sent(guard_for_stream.id());
            yield Ok::<bytes::Bytes, std::io::Error>(data);
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::PRAGMA, "no-cache")
        .header(header::EXPIRES, "0")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(body_stream))
        .unwrap()
}

/// Single JPEG snapshot
pub async fn snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let handler = state.stream_manager.mjpeg_handler();

    match handler.current_frame() {
        Some(frame) if frame.is_valid_jpeg() => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(frame.data_bytes()))
            .unwrap(),
        _ => Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from("No frame available"))
            .unwrap(),
    }
}

/// Create MJPEG multipart frame bytes
fn create_mjpeg_part(jpeg_data: &[u8]) -> bytes::Bytes {
    use bytes::{BufMut, BytesMut};

    let mut buf = BytesMut::with_capacity(128 + jpeg_data.len());

    // Write boundary and headers
    buf.put_slice(b"--frame\r\n");
    buf.put_slice(b"Content-Type: image/jpeg\r\n");
    buf.put_slice(format!("Content-Length: {}\r\n", jpeg_data.len()).as_bytes());
    buf.put_slice(b"\r\n");

    // Write JPEG data
    buf.put_slice(jpeg_data);
    buf.put_slice(b"\r\n");

    buf.freeze()
}
