# RustDesk 模块文档

## 1. 模块概述

RustDesk 模块实现 RustDesk 协议集成，允许使用标准 RustDesk 客户端访问 One-KVM 设备。

### 1.1 主要功能

- RustDesk 协议实现
- 渲染服务器 (hbbs) 通信
- 中继服务器 (hbbr) 通信
- 视频/音频/HID 转换
- 端到端加密

### 1.2 文件结构

```
src/rustdesk/
├── mod.rs              # RustDeskService (21KB)
├── connection.rs       # 连接管理 (49KB)
├── rendezvous.rs       # 渲染服务器 (32KB)
├── crypto.rs           # NaCl 加密 (16KB)
├── config.rs           # 配置 (7KB)
├── hid_adapter.rs      # HID 适配 (14KB)
├── frame_adapters.rs   # 帧转换 (9KB)
├── protocol.rs         # 协议包装 (6KB)
└── bytes_codec.rs      # 帧编码 (8KB)
```

---

## 2. 架构设计

### 2.1 RustDesk 网络架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      RustDesk Network Architecture                           │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────┐                                      ┌─────────────┐
│  RustDesk   │                                      │  One-KVM    │
│   Client    │                                      │   Device    │
└──────┬──────┘                                      └──────┬──────┘
       │                                                    │
       │ 1. 查询设备地址                                     │
       │─────────────────────►┌─────────────┐◄──────────────│
       │                      │    hbbs     │               │
       │                      │ (Rendezvous)│               │
       │◄─────────────────────└─────────────┘               │
       │ 2. 返回地址                                         │
       │                                                    │
       │ 3a. 直接连接 (如果可达)                              │
       │────────────────────────────────────────────────────│
       │                                                    │
       │ 3b. 中继连接 (如果 NAT)                              │
       │─────────────────────►┌─────────────┐◄──────────────│
       │                      │    hbbr     │               │
       │                      │  (Relay)    │               │
       │◄─────────────────────└─────────────┘───────────────│
       │                                                    │
       │ 4. 建立加密通道                                     │
       │◄───────────────────────────────────────────────────│
       │                                                    │
       │ 5. 传输视频/音频/HID                                │
       │◄───────────────────────────────────────────────────│
```

### 2.2 模块内部架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      RustDesk Module Architecture                            │
└─────────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────┐
                    │ RustDeskService │
                    │    (mod.rs)     │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  Rendezvous     │ │  Connection     │ │   Crypto        │
│  (rendezvous)   │ │  (connection)   │ │   (crypto)      │
└────────┬────────┘ └────────┬────────┘ └─────────────────┘
         │                   │
         │                   │
         ▼                   ▼
┌─────────────────┐ ┌─────────────────────────────────────┐
│  hbbs Server    │ │              Adapters               │
│  Connection     │ │  ┌──────────┐ ┌──────────────────┐ │
└─────────────────┘ │  │ HID      │ │ Frame            │ │
                    │  │ Adapter  │ │ Adapters         │ │
                    │  └──────────┘ └──────────────────┘ │
                    └─────────────────────────────────────┘
                                      │
                    ┌─────────────────┼─────────────────┐
                    │                 │                 │
                    ▼                 ▼                 ▼
             ┌───────────┐    ┌───────────┐    ┌───────────┐
             │    HID    │    │   Video   │    │   Audio   │
             │ Controller│    │  Pipeline │    │ Pipeline  │
             └───────────┘    └───────────┘    └───────────┘
```

---

## 3. 核心组件

### 3.1 RustDeskService (mod.rs)

RustDesk 服务主类。

```rust
pub struct RustDeskService {
    /// 服务配置
    config: Arc<RwLock<RustDeskConfig>>,

    /// 渲染连接
    rendezvous: Arc<RwLock<Option<RendezvousConnection>>>,

    /// 客户端连接
    connections: Arc<RwLock<HashMap<String, Arc<ClientConnection>>>>,

    /// 加密密钥
    keys: Arc<RustDeskKeys>,

    /// 视频管道
    video_pipeline: Arc<SharedVideoPipeline>,

    /// 音频管道
    audio_pipeline: Arc<SharedAudioPipeline>,

    /// HID 控制器
    hid: Arc<HidController>,

    /// 服务状态
    status: Arc<RwLock<ServiceStatus>>,

    /// 事件总线
    events: Arc<EventBus>,
}

impl RustDeskService {
    /// 创建服务
    pub async fn new(
        config: RustDeskConfig,
        video_pipeline: Arc<SharedVideoPipeline>,
        audio_pipeline: Arc<SharedAudioPipeline>,
        hid: Arc<HidController>,
        events: Arc<EventBus>,
    ) -> Result<Arc<Self>>;

    /// 启动服务
    pub async fn start(&self) -> Result<()>;

    /// 停止服务
    pub async fn stop(&self) -> Result<()>;

    /// 获取设备 ID
    pub fn device_id(&self) -> String;

    /// 获取状态
    pub fn status(&self) -> ServiceStatus;

    /// 更新配置
    pub async fn update_config(&self, config: RustDeskConfig) -> Result<()>;

    /// 获取连接列表
    pub fn connections(&self) -> Vec<ConnectionInfo>;

    /// 断开连接
    pub async fn disconnect(&self, connection_id: &str) -> Result<()>;
}

pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

pub struct ConnectionInfo {
    pub id: String,
    pub peer_id: String,
    pub connected_at: DateTime<Utc>,
    pub ip: String,
}
```

### 3.2 RendezvousConnection (rendezvous.rs)

渲染服务器连接管理。

```rust
pub struct RendezvousConnection {
    /// 服务器地址
    server_addr: SocketAddr,

    /// TCP 连接
    stream: TcpStream,

    /// 设备 ID
    device_id: String,

    /// 公钥
    public_key: [u8; 32],

    /// 注册状态
    registered: AtomicBool,

    /// 心跳任务
    heartbeat_task: Option<JoinHandle<()>>,
}

impl RendezvousConnection {
    /// 连接到渲染服务器
    pub async fn connect(
        server: &str,
        device_id: &str,
        keys: &RustDeskKeys,
    ) -> Result<Self>;

    /// 注册设备
    pub async fn register(&self) -> Result<()>;

    /// 发送心跳
    async fn heartbeat(&self) -> Result<()>;

    /// 接收消息
    pub async fn recv_message(&mut self) -> Result<RendezvousMessage>;

    /// 处理穿孔请求
    pub async fn handle_punch_request(&self, peer_id: &str) -> Result<SocketAddr>;
}

pub enum RendezvousMessage {
    RegisterOk,
    PunchRequest { peer_id: String, socket_addr: SocketAddr },
    Heartbeat,
    Error(String),
}
```

### 3.3 ClientConnection (connection.rs)

客户端连接处理。

```rust
pub struct ClientConnection {
    /// 连接 ID
    id: String,

    /// 对端 ID
    peer_id: String,

    /// 加密通道
    channel: EncryptedChannel,

    /// 帧适配器
    frame_adapter: FrameAdapter,

    /// HID 适配器
    hid_adapter: HidAdapter,

    /// 状态
    state: Arc<RwLock<ConnectionState>>,
}

impl ClientConnection {
    /// 创建连接
    pub async fn new(
        stream: TcpStream,
        keys: &RustDeskKeys,
        peer_public_key: &[u8],
    ) -> Result<Self>;

    /// 处理连接
    pub async fn handle(
        &self,
        video_rx: broadcast::Receiver<EncodedFrame>,
        audio_rx: broadcast::Receiver<AudioFrame>,
        hid: Arc<HidController>,
    ) -> Result<()>;

    /// 发送视频帧
    async fn send_video_frame(&self, frame: &EncodedFrame) -> Result<()>;

    /// 发送音频帧
    async fn send_audio_frame(&self, frame: &AudioFrame) -> Result<()>;

    /// 处理输入事件
    async fn handle_input(&self, msg: &InputMessage) -> Result<()>;

    /// 关闭连接
    pub async fn close(&self) -> Result<()>;
}

pub enum ConnectionState {
    Handshaking,
    Authenticating,
    Connected,
    Closing,
    Closed,
}
```

### 3.4 RustDeskKeys (crypto.rs)

加密密钥管理。

```rust
pub struct RustDeskKeys {
    /// 设备 ID
    pub device_id: String,

    /// Curve25519 公钥
    pub public_key: [u8; 32],

    /// Curve25519 私钥
    secret_key: [u8; 32],

    /// Ed25519 签名公钥
    pub sign_public_key: [u8; 32],

    /// Ed25519 签名私钥
    sign_secret_key: [u8; 64],
}

impl RustDeskKeys {
    /// 生成新密钥
    pub fn generate() -> Self;

    /// 从配置加载
    pub fn from_config(config: &KeyConfig) -> Result<Self>;

    /// 保存到配置
    pub fn to_config(&self) -> KeyConfig;

    /// 计算共享密钥
    pub fn shared_secret(&self, peer_public_key: &[u8; 32]) -> [u8; 32];

    /// 签名消息
    pub fn sign(&self, message: &[u8]) -> [u8; 64];

    /// 验证签名
    pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool;
}

pub struct EncryptedChannel {
    /// 发送密钥
    send_key: [u8; 32],

    /// 接收密钥
    recv_key: [u8; 32],

    /// 发送 nonce
    send_nonce: AtomicU64,

    /// 接收 nonce
    recv_nonce: AtomicU64,
}

impl EncryptedChannel {
    /// 加密消息
    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8>;

    /// 解密消息
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}
```

### 3.5 HidAdapter (hid_adapter.rs)

RustDesk HID 事件转换。

```rust
pub struct HidAdapter {
    hid: Arc<HidController>,
}

impl HidAdapter {
    /// 创建适配器
    pub fn new(hid: Arc<HidController>) -> Self;

    /// 处理键盘事件
    pub async fn handle_keyboard(&self, event: &RdKeyboardEvent) -> Result<()>;

    /// 处理鼠标事件
    pub async fn handle_mouse(&self, event: &RdMouseEvent) -> Result<()>;

    /// 转换键码
    fn convert_keycode(rd_key: u32) -> Option<KeyCode>;

    /// 转换鼠标按钮
    fn convert_button(rd_button: u32) -> Option<MouseButton>;
}

/// RustDesk 键盘事件
pub struct RdKeyboardEvent {
    pub keycode: u32,
    pub down: bool,
    pub modifiers: u32,
}

/// RustDesk 鼠标事件
pub struct RdMouseEvent {
    pub x: i32,
    pub y: i32,
    pub mask: u32,
}
```

### 3.6 FrameAdapter (frame_adapters.rs)

帧格式转换。

```rust
pub struct FrameAdapter;

impl FrameAdapter {
    /// 转换视频帧到 RustDesk 格式
    pub fn to_rd_video_frame(frame: &EncodedFrame) -> RdVideoFrame;

    /// 转换音频帧到 RustDesk 格式
    pub fn to_rd_audio_frame(frame: &AudioFrame) -> RdAudioFrame;
}

/// RustDesk 视频帧
pub struct RdVideoFrame {
    pub data: Vec<u8>,
    pub key_frame: bool,
    pub pts: i64,
    pub format: RdVideoFormat,
}

pub enum RdVideoFormat {
    H264,
    H265,
    VP8,
    VP9,
}

/// RustDesk 音频帧
pub struct RdAudioFrame {
    pub data: Vec<u8>,
    pub timestamp: u64,
}
```

### 3.7 协议消息 (protocol.rs)

Protobuf 消息包装。

```rust
/// 使用 prost 生成的 protobuf 消息
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/rendezvous.rs"));
    include!(concat!(env!("OUT_DIR"), "/message.rs"));
}

pub struct MessageCodec;

impl MessageCodec {
    /// 编码消息
    pub fn encode<M: prost::Message>(msg: &M) -> Vec<u8>;

    /// 解码消息
    pub fn decode<M: prost::Message + Default>(data: &[u8]) -> Result<M>;
}
```

### 3.8 帧编码 (bytes_codec.rs)

变长帧协议。

```rust
pub struct BytesCodec {
    state: DecodeState,
    buffer: BytesMut,
}

impl BytesCodec {
    /// 编码帧
    pub fn encode_frame(data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + data.len());
        buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
        buf.extend_from_slice(data);
        buf
    }

    /// 解码帧
    pub fn decode_frame(&mut self, src: &mut BytesMut) -> Result<Option<Bytes>>;
}

enum DecodeState {
    Length,
    Data(usize),
}
```

---

## 4. 协议详解

### 4.1 Protobuf 定义

```protobuf
// protos/rendezvous.proto
message RegisterPeer {
    string id = 1;
    bytes public_key = 2;
}

message RegisterPeerResponse {
    bool ok = 1;
    string error = 2;
}

message PunchHoleRequest {
    string id = 1;
    string nat_type = 2;
}

// protos/message.proto
message VideoFrame {
    bytes data = 1;
    bool key = 2;
    int64 pts = 3;
    VideoCodec codec = 4;
}

message AudioFrame {
    bytes data = 1;
    int64 timestamp = 2;
}

message KeyboardEvent {
    uint32 keycode = 1;
    bool down = 2;
    uint32 modifiers = 3;
}

message MouseEvent {
    int32 x = 1;
    int32 y = 2;
    uint32 mask = 3;
}
```

### 4.2 连接握手

```
1. TCP 连接
   Client ────► Device

2. 公钥交换
   Client ◄───► Device

3. DH 密钥协商
   shared_secret = X25519(my_private, peer_public)

4. 密钥派生
   send_key = HKDF(shared_secret, "send")
   recv_key = HKDF(shared_secret, "recv")

5. 认证 (可选)
   Client ────► Device: encrypted(password)
   Client ◄──── Device: encrypted(ok/fail)

6. 开始传输
```

---

## 5. 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct RustDeskConfig {
    /// 是否启用
    pub enabled: bool,

    /// 渲染服务器地址
    pub rendezvous_server: String,

    /// 中继服务器地址
    pub relay_server: Option<String>,

    /// 设备 ID (自动生成)
    pub device_id: Option<String>,

    /// 访问密码
    pub password: Option<String>,

    /// 允许的客户端 ID
    pub allowed_clients: Vec<String>,
}

impl Default for RustDeskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rendezvous_server: "rs-ny.rustdesk.com:21116".to_string(),
            relay_server: None,
            device_id: None,
            password: None,
            allowed_clients: vec![],
        }
    }
}
```

---

## 6. API 端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/rustdesk/status` | GET | 获取服务状态 |
| `/api/rustdesk/start` | POST | 启动服务 |
| `/api/rustdesk/stop` | POST | 停止服务 |
| `/api/rustdesk/config` | GET | 获取配置 |
| `/api/rustdesk/config` | PATCH | 更新配置 |
| `/api/rustdesk/device-id` | GET | 获取设备 ID |
| `/api/rustdesk/connections` | GET | 获取连接列表 |
| `/api/rustdesk/connections/:id` | DELETE | 断开连接 |

### 响应格式

```json
// GET /api/rustdesk/status
{
    "status": "running",
    "device_id": "123456789",
    "rendezvous_connected": true,
    "active_connections": 1
}

// GET /api/rustdesk/connections
{
    "connections": [
        {
            "id": "conn-abc",
            "peer_id": "987654321",
            "connected_at": "2024-01-15T10:30:00Z",
            "ip": "192.168.1.100"
        }
    ]
}
```

---

## 7. 事件

```rust
pub enum SystemEvent {
    RustDeskStatusChanged {
        status: String,
        device_id: Option<String>,
        error: Option<String>,
    },

    RustDeskConnectionOpened {
        connection_id: String,
        peer_id: String,
    },

    RustDeskConnectionClosed {
        connection_id: String,
        peer_id: String,
        reason: String,
    },
}
```

---

## 8. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum RustDeskError {
    #[error("Service not running")]
    NotRunning,

    #[error("Already running")]
    AlreadyRunning,

    #[error("Rendezvous connection failed: {0}")]
    RendezvousFailed(String),

    #[error("Authentication failed")]
    AuthFailed,

    #[error("Connection refused")]
    ConnectionRefused,

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Timeout")]
    Timeout,
}
```

---

## 9. 使用示例

### 9.1 启动服务

```rust
let config = RustDeskConfig {
    enabled: true,
    rendezvous_server: "rs-ny.rustdesk.com:21116".to_string(),
    password: Some("mypassword".to_string()),
    ..Default::default()
};

let service = RustDeskService::new(
    config,
    video_pipeline,
    audio_pipeline,
    hid,
    events,
).await?;

service.start().await?;

println!("Device ID: {}", service.device_id());
```

### 9.2 客户端连接

```
1. 打开 RustDesk 客户端
2. 输入设备 ID
3. 输入密码 (如果设置)
4. 连接成功后即可控制
```

---

## 10. 常见问题

### Q: 无法连接到渲染服务器?

1. 检查网络连接
2. 检查服务器地址
3. 检查防火墙

### Q: 客户端连接失败?

1. 检查设备 ID
2. 检查密码
3. 检查 NAT 穿透

### Q: 视频延迟高?

1. 使用更近的中继服务器
2. 检查网络带宽
3. 降低视频质量

### Q: 如何自建服务器?

参考 RustDesk Server 部署文档:
- hbbs: 渲染服务器
- hbbr: 中继服务器
