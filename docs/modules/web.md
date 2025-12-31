# Web 模块文档

## 1. 模块概述

Web 模块提供 HTTP API 和静态文件服务。

### 1.1 主要功能

- REST API
- WebSocket
- 静态文件服务
- 认证中间件
- CORS 支持

### 1.2 文件结构

```
src/web/
├── mod.rs              # 模块导出
├── routes.rs           # 路由定义 (9KB)
├── ws.rs               # WebSocket (8KB)
├── audio_ws.rs         # 音频 WebSocket (8KB)
├── static_files.rs     # 静态文件 (6KB)
└── handlers/           # API 处理器
    ├── mod.rs
    └── config/
        ├── mod.rs
        ├── apply.rs
        ├── types.rs
        └── rustdesk.rs
```

---

## 2. 路由结构

### 2.1 公共路由 (无认证)

| 路由 | 方法 | 描述 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/auth/login` | POST | 登录 |
| `/setup` | GET | 获取设置状态 |
| `/setup/init` | POST | 初始化设置 |

### 2.2 用户路由 (需认证)

| 路由 | 方法 | 描述 |
|------|------|------|
| `/info` | GET | 系统信息 |
| `/devices` | GET | 设备列表 |
| `/stream/*` | * | 流控制 |
| `/webrtc/*` | * | WebRTC 信令 |
| `/hid/*` | * | HID 控制 |
| `/audio/*` | * | 音频控制 |
| `/ws` | WS | 事件 WebSocket |
| `/ws/audio` | WS | 音频 WebSocket |

### 2.3 管理员路由 (需 Admin)

| 路由 | 方法 | 描述 |
|------|------|------|
| `/config/*` | * | 配置管理 |
| `/msd/*` | * | MSD 操作 |
| `/atx/*` | * | ATX 控制 |
| `/extensions/*` | * | 扩展管理 |
| `/rustdesk/*` | * | RustDesk |
| `/users/*` | * | 用户管理 |

---

## 3. 路由定义

```rust
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // 公共路由
        .route("/health", get(handlers::health))
        .route("/auth/login", post(handlers::login))
        .route("/setup", get(handlers::setup_status))
        .route("/setup/init", post(handlers::setup_init))

        // 用户路由
        .nest("/api", user_routes())

        // 管理员路由
        .nest("/api/admin", admin_routes())

        // 静态文件
        .fallback(static_files::serve)

        // 中间件
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())

        // 状态
        .with_state(state)
}

fn user_routes() -> Router {
    Router::new()
        .route("/info", get(handlers::system_info))
        .route("/devices", get(handlers::list_devices))

        // 流控制
        .route("/stream/status", get(handlers::stream_status))
        .route("/stream/start", post(handlers::stream_start))
        .route("/stream/stop", post(handlers::stream_stop))
        .route("/stream/mjpeg", get(handlers::mjpeg_stream))

        // WebRTC
        .route("/webrtc/session", post(handlers::webrtc_create_session))
        .route("/webrtc/offer", post(handlers::webrtc_offer))
        .route("/webrtc/ice", post(handlers::webrtc_ice))
        .route("/webrtc/close", post(handlers::webrtc_close))

        // HID
        .route("/hid/status", get(handlers::hid_status))
        .route("/hid/reset", post(handlers::hid_reset))

        // WebSocket
        .route("/ws", get(handlers::ws_handler))
        .route("/ws/audio", get(handlers::audio_ws_handler))

        // 认证中间件
        .layer(middleware::from_fn(auth_middleware))
}

fn admin_routes() -> Router {
    Router::new()
        // 配置
        .route("/config", get(handlers::config::get_config))
        .route("/config", patch(handlers::config::update_config))

        // MSD
        .route("/msd/status", get(handlers::msd_status))
        .route("/msd/connect", post(handlers::msd_connect))

        // ATX
        .route("/atx/status", get(handlers::atx_status))
        .route("/atx/power/short", post(handlers::atx_power_short))

        // 认证中间件
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn(admin_middleware))
}
```

---

## 4. 静态文件服务

```rust
#[derive(RustEmbed)]
#[folder = "web/dist"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "assets/*"]
struct Assets;

pub async fn serve(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // 尝试获取文件
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream();

        return (
            [(header::CONTENT_TYPE, mime.as_ref())],
            content.data.into_owned(),
        ).into_response();
    }

    // SPA 回退到 index.html
    if let Some(content) = Assets::get("index.html") {
        return (
            [(header::CONTENT_TYPE, "text/html")],
            content.data.into_owned(),
        ).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
```

---

## 5. WebSocket 处理

### 5.1 事件 WebSocket (ws.rs)

```rust
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    // 发送初始设备信息
    let device_info = state.get_device_info().await;
    let json = serde_json::to_string(&device_info).unwrap();
    let _ = socket.send(Message::Text(json)).await;

    // 订阅事件
    let mut rx = state.events.subscribe();

    loop {
        tokio::select! {
            // 发送事件
            result = rx.recv() => {
                if let Ok(event) = result {
                    let json = serde_json::to_string(&event).unwrap();
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }

            // 接收消息 (心跳/关闭)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

### 5.2 音频 WebSocket (audio_ws.rs)

```rust
pub async fn audio_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_audio_ws(socket, state))
}

async fn handle_audio_ws(mut socket: WebSocket, state: Arc<AppState>) {
    // 订阅音频帧
    let mut rx = state.audio.subscribe();

    loop {
        tokio::select! {
            // 发送音频帧
            result = rx.recv() => {
                if let Ok(frame) = result {
                    if socket.send(Message::Binary(frame.data.to_vec())).await.is_err() {
                        break;
                    }
                }
            }

            // 处理关闭
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

---

## 6. MJPEG 流

```rust
pub async fn mjpeg_stream(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let boundary = "frame";

    // 订阅视频帧
    let rx = state.stream_manager.subscribe_mjpeg();

    // 创建流
    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Ok(frame) = rx.recv().await {
            let header = format!(
                "--{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                boundary,
                frame.data.len()
            );
            yield Ok::<_, std::io::Error>(Bytes::from(header));
            yield Ok(frame.data.clone());
            yield Ok(Bytes::from("\r\n"));
        }
    };

    (
        [(
            header::CONTENT_TYPE,
            format!("multipart/x-mixed-replace; boundary={}", boundary),
        )],
        Body::from_stream(stream),
    )
}
```

---

## 7. 错误处理

```rust
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::AuthError => (StatusCode::UNAUTHORIZED, "Authentication failed"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            AppError::Internal(err) => {
                tracing::error!("Internal error: {:?}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            // ...
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

---

## 8. 请求提取器

```rust
// 从 Cookie 获取会话
pub struct AuthUser(pub Session);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, state).await?;
        let token = cookies
            .get("session_id")
            .map(|c| c.value().to_string())
            .ok_or(AppError::Unauthorized)?;

        let state = parts.extensions.get::<Arc<AppState>>().unwrap();
        let session = state.sessions
            .get_session(&token)
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthUser(session))
    }
}
```

---

## 9. 中间件

### 9.1 认证中间件

```rust
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Response {
    let token = cookies
        .get("session_id")
        .map(|c| c.value().to_string());

    if let Some(session) = token.and_then(|t| state.sessions.get_session(&t)) {
        request.extensions_mut().insert(session);
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}
```

### 9.2 Admin 中间件

```rust
pub async fn admin_middleware(
    Extension(session): Extension<Session>,
    request: Request,
    next: Next,
) -> Response {
    if session.role == UserRole::Admin {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}
```

---

## 10. HTTPS 支持

```rust
// 使用 axum-server 提供 TLS
let tls_config = RustlsConfig::from_pem_file(cert_path, key_path).await?;

axum_server::bind_rustls(addr, tls_config)
    .serve(app.into_make_service())
    .await?;

// 或自动生成自签名证书
let (cert, key) = generate_self_signed_cert()?;
let tls_config = RustlsConfig::from_pem(cert, key).await?;
```
