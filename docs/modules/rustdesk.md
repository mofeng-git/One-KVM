# RustDesk 模块文档

## 1. 模块概述

RustDesk 模块实现 RustDesk 协议集成,允许使用标准 RustDesk 客户端访问 One-KVM 设备。

### 1.1 主要功能

- RustDesk 协议实现 (Protobuf + NaCl 加密)
- Rendezvous 服务器 (hbbs) 通信
- 中继服务器 (hbbr) 通信
- P2P 直连与中继回退
- 局域网直连支持
- 视频/音频/HID 转换
- 端到端加密 (Curve25519 + XSalsa20-Poly1305)
- 签名认证 (Ed25519)
- 公共服务器支持 (通过 secrets.toml)
- 动态编码器协商 (H264/H265/VP8/VP9)
- 输入节流 (防止 HID EAGAIN)
- CapsLock 状态同步
- 管道自动重订阅 (支持热更新)

### 1.2 文件结构

```
src/rustdesk/
├── mod.rs              # RustDeskService 主服务类
├── connection.rs       # 客户端连接处理 (Connection, ConnectionManager)
├── rendezvous.rs       # Rendezvous 中介者 (RendezvousMediator)
├── punch.rs            # P2P 直连尝试与中继回退
├── crypto.rs           # NaCl 加密 (Curve25519 + Ed25519)
├── config.rs           # 配置管理 (RustDeskConfig)
├── hid_adapter.rs      # HID 事件转换 (RustDesk → One-KVM)
├── frame_adapters.rs   # 音视频帧转换 (零拷贝优化)
├── protocol.rs         # Protobuf 协议包装
└── bytes_codec.rs      # 变长帧编解码
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
┌──────────────────────────────────────────────────────────────────────┐
│                    RustDesk Module Architecture                       │
└──────────────────────────────────────────────────────────────────────┘

                         ┌─────────────────┐
                         │ RustDeskService │
                         └────────┬────────┘
                                  │
          ┌───────────────────────┼────────────────────┐
          │                       │                    │
          ▼                       ▼                    ▼
┌──────────────────┐   ┌──────────────────┐   ┌───────────────┐
│ Rendezvous       │   │ Connection       │   │ Crypto        │
│ Mediator         │   │ Manager          │   │ (Keys)        │
└────────┬─────────┘   └────────┬─────────┘   └───────────────┘
         │                      │
         │ UDP                  │ TCP (P2P/Relay/Intranet)
         ▼                      ▼
┌──────────────────┐   ┌──────────────────┐
│ hbbs Server      │   │ Connections      │
│ (Registration)   │   │ ┌──────────────┐ │
└──────────────────┘   │ │ Connection 1 │ │
                       │ │ Connection 2 │ │
                       │ └──────────────┘ │
                       └────────┬─────────┘
                                │
         ┌──────────────────────┼──────────────────────┐
         │                      │                      │
         ▼                      ▼                      ▼
┌───────────────┐   ┌──────────────────┐   ┌─────────────────┐
│ HID Adapter   │   │ Frame Adapters   │   │ Input Throttler │
│ (Event Conv)  │   │ (Zero-Copy)      │   │ (Anti-EAGAIN)   │
└───────┬───────┘   └────────┬─────────┘   └─────────────────┘
        │                    │
        ▼                    ▼
┌───────────────┐   ┌─────────────────────────────┐
│ HID           │   │ Video/Audio Manager         │
│ Controller    │   │ (Shared Pipeline)           │
└───────────────┘   └─────────────────────────────┘
```

---

## 3. 核心组件

### 3.1 RustDeskService (mod.rs)

RustDesk 服务主类,管理整个 RustDesk 协议集成。

```rust
pub struct RustDeskService {
    config: Arc<RwLock<RustDeskConfig>>,
    status: Arc<RwLock<ServiceStatus>>,
    rendezvous: Arc<RwLock<Option<Arc<RendezvousMediator>>>>,
    rendezvous_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    tcp_listener_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    listen_port: Arc<RwLock<u16>>,
    connection_manager: Arc<ConnectionManager>,
    video_manager: Arc<VideoStreamManager>,
    hid: Arc<HidController>,
    audio: Arc<AudioController>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RustDeskService {
    /// 创建新服务实例
    pub fn new(
        config: RustDeskConfig,
        video_manager: Arc<VideoStreamManager>,
        hid: Arc<HidController>,
        audio: Arc<AudioController>,
    ) -> Self;

    /// 启动服务 (启动 Rendezvous 注册和 TCP 监听)
    pub async fn start(&self) -> anyhow::Result<()>;

    /// 停止服务
    pub async fn stop(&self) -> anyhow::Result<()>;

    /// 重启服务 (用于配置更新)
    pub async fn restart(&self, config: RustDeskConfig) -> anyhow::Result<()>;

    /// 获取设备 ID
    pub fn device_id(&self) -> String;

    /// 获取服务状态
    pub fn status(&self) -> ServiceStatus;

    /// 获取 Rendezvous 状态
    pub fn rendezvous_status(&self) -> Option<RendezvousStatus>;

    /// 获取连接数量
    pub fn connection_count(&self) -> usize;

    /// 获取 TCP 监听端口
    pub fn listen_port(&self) -> u16;

    /// 保存凭据 (密钥和 UUID) 到配置
    /// 返回更新后的配置 (如果有变更)
    pub fn save_credentials(&self) -> Option<RustDeskConfig>;
}

pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}
```

**主要流程:**
1. **启动流程**: 初始化加密 → 启动 TCP 监听器 → 创建 Rendezvous Mediator → 设置回调 → 开始注册循环
2. **连接处理**: P2P 直连尝试 → 中继回退 → 局域网直连
3. **停止流程**: 发送停止信号 → 关闭所有连接 → 停止 Rendezvous → 等待任务结束

### 3.2 RendezvousMediator (rendezvous.rs)

Rendezvous 服务器通信中介者,处理设备注册、心跳维护和连接请求。

```rust
pub struct RendezvousMediator {
    config: Arc<RwLock<RustDeskConfig>>,
    keypair: Arc<RwLock<Option<KeyPair>>>,             // Curve25519 加密密钥
    signing_keypair: Arc<RwLock<Option<SigningKeyPair>>>, // Ed25519 签名密钥
    status: Arc<RwLock<RendezvousStatus>>,
    uuid: Arc<RwLock<[u8; 16]>>,                       // 设备 UUID (持久化)
    uuid_needs_save: Arc<RwLock<bool>>,
    serial: Arc<RwLock<i32>>,                          // 配置序列号
    key_confirmed: Arc<RwLock<bool>>,                  // 公钥注册确认
    keep_alive_ms: Arc<RwLock<i32>>,
    relay_callback: Arc<RwLock<Option<RelayCallback>>>,
    punch_callback: Arc<RwLock<Option<PunchCallback>>>,
    intranet_callback: Arc<RwLock<Option<IntranetCallback>>>,
    listen_port: Arc<RwLock<u16>>,                     // TCP 监听端口
    shutdown_tx: broadcast::Sender<()>,
}

impl RendezvousMediator {
    /// 创建新的 Rendezvous 中介者
    pub fn new(config: RustDeskConfig) -> Self;

    /// 启动注册循环
    pub async fn start(&self) -> anyhow::Result<()>;

    /// 停止中介者
    pub fn stop(&self);

    /// 获取或生成加密密钥对
    pub fn ensure_keypair(&self) -> KeyPair;

    /// 获取或生成签名密钥对
    pub fn ensure_signing_keypair(&self) -> SigningKeyPair;

    /// 设置 TCP 监听端口
    pub fn set_listen_port(&self, port: u16);

    /// 设置中继请求回调
    pub fn set_relay_callback(&self, callback: RelayCallback);

    /// 设置 P2P 穿孔回调
    pub fn set_punch_callback(&self, callback: PunchCallback);

    /// 设置局域网连接回调
    pub fn set_intranet_callback(&self, callback: IntranetCallback);

    /// 获取当前状态
    pub fn status(&self) -> RendezvousStatus;

    /// 检查 UUID 是否需要保存
    pub fn uuid_needs_save(&self) -> bool;

    /// 标记 UUID 已保存
    pub fn mark_uuid_saved(&self);
}

pub enum RendezvousStatus {
    Disconnected,  // 未连接
    Connecting,    // 正在连接
    Connected,     // 已连接但未注册
    Registered,    // 已注册
    Error(String), // 错误状态
}

/// 中继请求回调
/// 参数: (rendezvous_addr, relay_server, uuid, socket_addr, device_id)
pub type RelayCallback = Arc<dyn Fn(String, String, String, Vec<u8>, String) + Send + Sync>;

/// P2P 穿孔回调
/// 参数: (peer_addr, rendezvous_addr, relay_server, uuid, socket_addr, device_id)
pub type PunchCallback = Arc<dyn Fn(Option<SocketAddr>, String, String, String, Vec<u8>, String) + Send + Sync>;

/// 局域网连接回调
/// 参数: (rendezvous_addr, peer_socket_addr, local_addr, relay_server, device_id)
pub type IntranetCallback = Arc<dyn Fn(String, Vec<u8>, SocketAddr, String, String) + Send + Sync>;
```

**关键机制:**
- **UDP 通信**: 使用 UDP 与 hbbs 服务器通信
- **双密钥系统**: Curve25519 用于加密,Ed25519 用于签名
- **UUID 持久化**: 避免重新注册时的 UUID_MISMATCH 错误
- **地址混淆**: 使用 `AddrMangle` 编码地址避免防火墙篡改
- **三种连接模式**: P2P 直连、中继连接、局域网直连

### 3.3 Connection & ConnectionManager (connection.rs)

客户端连接处理,包含连接生命周期管理和数据传输。

```rust
/// 单个客户端连接
pub struct Connection {
    id: u32,
    device_id: String,
    peer_id: String,
    state: Arc<RwLock<ConnectionState>>,
    signing_keypair: SigningKeyPair,
    temp_keypair: (box_::PublicKey, box_::SecretKey), // 每连接临时密钥
    password: String,
    hid: Option<Arc<HidController>>,
    audio: Option<Arc<AudioController>>,
    video_manager: Option<Arc<VideoStreamManager>>,
    session_key: Option<secretbox::Key>,
    encryption_enabled: bool,
    negotiated_codec: Option<VideoEncoderType>,
    input_throttler: InputThrottler,  // 输入节流防止 EAGAIN
    last_caps_lock: bool,             // CapsLock 状态跟踪
    // ... 更多字段
}

impl Connection {
    /// 创建新连接
    pub fn new(
        id: u32,
        config: &RustDeskConfig,
        signing_keypair: SigningKeyPair,
        hid: Option<Arc<HidController>>,
        audio: Option<Arc<AudioController>>,
        video_manager: Option<Arc<VideoStreamManager>>,
    ) -> (Self, mpsc::UnboundedReceiver<ConnectionMessage>);

    /// 处理 TCP 连接
    pub async fn handle_tcp(&mut self, stream: TcpStream, peer_addr: SocketAddr) -> anyhow::Result<()>;

    /// 关闭连接
    pub fn close(&self);
}

/// 连接管理器
pub struct ConnectionManager {
    connections: Arc<RwLock<Vec<Arc<RwLock<ConnectionInfo>>>>>,
    next_id: Arc<RwLock<u32>>,
    config: Arc<RwLock<RustDeskConfig>>,
    keypair: Arc<RwLock<Option<KeyPair>>>,
    signing_keypair: Arc<RwLock<Option<SigningKeyPair>>>,
    hid: Arc<RwLock<Option<Arc<HidController>>>>,
    audio: Arc<RwLock<Option<Arc<AudioController>>>>,
    video_manager: Arc<RwLock<Option<Arc<VideoStreamManager>>>>,
}

impl ConnectionManager {
    pub fn new(config: RustDeskConfig) -> Self;
    pub async fn accept_connection(&self, stream: TcpStream, peer_addr: SocketAddr) -> anyhow::Result<u32>;
    pub fn connection_count(&self) -> usize;
    pub fn close_all(&self);
}

pub enum ConnectionState {
    Pending,       // 等待连接
    Handshaking,   // 握手中
    Active,        // 活跃
    Closed,        // 已关闭
    Error(String), // 错误
}

/// 输入节流器 (防止 HID EAGAIN 错误)
struct InputThrottler {
    last_mouse_time: Instant,
    mouse_interval: Duration,  // 默认 16ms (≈60Hz)
}
```

**连接流程:**
1. **握手**: 发送 SignedId (含临时公钥) → 接收 PublicKey (含对称密钥) → 解密对称密钥
2. **认证**: 发送 Hash (密码盐) → 接收 LoginRequest (密码哈希) → 验证密码
3. **编码协商**: 根据可用编码器选择最优编解码器(H264 > H265 > VP8 > VP9)
4. **流传输**: 订阅共享视频/音频管道 → 转换为 RustDesk 格式 → 加密发送
5. **输入处理**: 接收 KeyEvent/MouseEvent → 节流 → 转换为 USB HID → 发送到 HID 控制器

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

帧格式转换，支持零拷贝。

```rust
pub struct VideoFrameAdapter {
    codec: VideoCodec,
    seq: u32,
    timestamp_base: u64,
}

impl VideoFrameAdapter {
    /// 创建适配器
    pub fn new(codec: VideoCodec) -> Self;

    /// 零拷贝转换 (推荐)
    /// Bytes 是引用计数类型，clone 只增加引用计数
    pub fn encode_frame_bytes_zero_copy(
        &mut self,
        data: Bytes,
        is_keyframe: bool,
        timestamp_ms: u64,
    ) -> Bytes;

    /// 转换视频帧到 RustDesk 格式 (会拷贝数据)
    pub fn encode_frame_bytes(
        &mut self,
        data: &[u8],
        is_keyframe: bool,
        timestamp_ms: u64,
    ) -> Bytes;
}

/// RustDesk 视频帧 (protobuf 生成)
/// 注意: data 字段使用 bytes::Bytes 类型以支持零拷贝
pub struct EncodedVideoFrame {
    pub data: Bytes,  // 零拷贝: 引用计数共享
    pub key: bool,
    pub pts: i64,
}

pub enum VideoCodec {
    H264,
    H265,
    VP8,
    VP9,
    AV1,
}
```

### 3.7 协议消息 (protocol.rs)

Protobuf 消息包装，使用 protobuf-rust（与 RustDesk 服务器一致）。

```rust
/// 使用 protobuf-codegen 生成的 protobuf 消息
pub mod hbb {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

// Re-export commonly used types
pub use hbb::rendezvous::{...};
pub use hbb::message::{...};

/// 解码 RendezvousMessage
pub fn decode_rendezvous_message(buf: &[u8]) -> Result<RendezvousMessage, protobuf::Error> {
    RendezvousMessage::parse_from_bytes(buf)
}

/// 解码 Message (session message)
pub fn decode_message(buf: &[u8]) -> Result<Message, protobuf::Error> {
    Message::parse_from_bytes(buf)
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

    /// Rendezvous 服务器地址 (hbbs)
    /// 格式: "rs.example.com" 或 "192.168.1.100:21116"
    /// 如果为空,使用 secrets.toml 中配置的公共服务器
    pub rendezvous_server: String,

    /// 中继服务器地址 (hbbr),默认与 rendezvous 同主机
    /// 格式: "relay.example.com" 或 "192.168.1.100:21117"
    pub relay_server: Option<String>,

    /// 中继服务器认证密钥 (如果中继服务器使用 -k 选项)
    #[typeshare(skip)]
    pub relay_key: Option<String>,

    /// 设备 ID (9 位数字),自动生成
    pub device_id: String,

    /// 设备密码 (客户端连接认证)
    #[typeshare(skip)]
    pub device_password: String,

    /// Curve25519 公钥 (Base64 编码),用于加密
    #[typeshare(skip)]
    pub public_key: Option<String>,

    /// Curve25519 私钥 (Base64 编码),用于加密
    #[typeshare(skip)]
    pub private_key: Option<String>,

    /// Ed25519 签名公钥 (Base64 编码),用于 SignedId 验证
    #[typeshare(skip)]
    pub signing_public_key: Option<String>,

    /// Ed25519 签名私钥 (Base64 编码),用于签名 SignedId
    #[typeshare(skip)]
    pub signing_private_key: Option<String>,

    /// UUID (持久化,避免 UUID_MISMATCH 错误)
    #[typeshare(skip)]
    pub uuid: Option<String>,
}

impl RustDeskConfig {
    /// 检查配置是否有效
    pub fn is_valid(&self) -> bool;

    /// 检查是否使用公共服务器
    pub fn is_using_public_server(&self) -> bool;

    /// 获取有效的 Rendezvous 服务器地址
    pub fn effective_rendezvous_server(&self) -> &str;

    /// 获取公共服务器信息 (如果配置了)
    pub fn public_server_info() -> Option<PublicServerInfo>;

    /// 获取带默认端口的 Rendezvous 地址
    pub fn rendezvous_addr(&self) -> String;

    /// 获取带默认端口的中继服务器地址
    pub fn relay_addr(&self) -> Option<String>;

    /// 确保 UUID 存在 (自动生成并标记需要保存)
    pub fn ensure_uuid(&mut self) -> ([u8; 16], bool);
}

/// 公共服务器信息
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct PublicServerInfo {
    pub server: String,      // 服务器地址
    pub public_key: String,  // 公钥 (Base64)
}
```

### 配置文件示例

**使用自建服务器:**
```toml
[rustdesk]
enabled = true
rendezvous_server = "192.168.1.100:21116"
relay_server = "192.168.1.100:21117"
device_id = "123456789"
device_password = "mypassword"
# 密钥和 UUID 由程序自动生成和保存
```

**使用公共服务器:**
```toml
[rustdesk]
enabled = true
rendezvous_server = ""  # 留空使用 secrets.toml 中的公共服务器
device_id = "123456789"
device_password = "mypassword"
```

**secrets.toml 公共服务器配置:**
```toml
[rustdesk]
# 公共服务器配置 (可选)
public_server = "rs-ny.rustdesk.com"
public_key = "xxx...base64...xxx"
relay_key = "xxx...key...xxx"
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
    rendezvous_server: "".to_string(),  // 使用公共服务器
    device_id: "123456789".to_string(),
    device_password: "mypassword".to_string(),
    ..Default::default()
};

let service = RustDeskService::new(
    config,
    video_manager,
    hid,
    audio,
);

service.start().await?;

println!("Device ID: {}", service.device_id());
println!("Listen Port: {}", service.listen_port());
println!("Status: {:?}", service.status());
```

### 9.2 使用自建服务器

```rust
let config = RustDeskConfig {
    enabled: true,
    rendezvous_server: "192.168.1.100:21116".to_string(),
    relay_server: Some("192.168.1.100:21117".to_string()),
    relay_key: Some("your_licence_key".to_string()),  // 如果使用 -k 选项
    device_id: "123456789".to_string(),
    device_password: "mypassword".to_string(),
    ..Default::default()
};

let service = RustDeskService::new(config, video_manager, hid, audio);
service.start().await?;
```

### 9.3 客户端连接

**使用 RustDesk 客户端:**
1. 下载并安装 RustDesk 客户端 ([https://rustdesk.com](https://rustdesk.com))
2. 如果使用自建服务器,在设置中配置 ID 服务器地址
3. 输入设备 ID (9 位数字)
4. 输入密码 (如果设置)
5. 点击连接

**连接过程:**
```
客户端                           One-KVM
  │                                 │
  │ 1. 查询设备 (hbbs)               │
  │──────────────►┌─────────┐◄──────│
  │               │  hbbs   │       │
  │◄──────────────└─────────┘       │
  │ 2. 返回地址信息                  │
  │                                 │
  │ 3a. 尝试 P2P 直连 (3s 超时)      │
  │─────────────────────────────────│
  │                                 │
  │ 3b. 失败则通过中继 (hbbr)        │
  │──────────►┌─────────┐◄──────────│
  │           │  hbbr   │           │
  │◄──────────└─────────┘───────────│
  │                                 │
  │ 4. 握手 + 认证                   │
  │◄────────────────────────────────│
  │                                 │
  │ 5. 视频/音频/HID 传输            │
  │◄────────────────────────────────│
```

### 9.4 保存凭据

```rust
// 启动后,凭据会自动生成
// 定期保存凭据到配置文件,避免重启后 UUID 变化
if let Some(updated_config) = service.save_credentials() {
    // 保存到配置存储
    config_manager.save_rustdesk_config(&updated_config).await?;
}
```

---

## 10. 性能优化

### 10.1 零拷贝设计

RustDesk 模块使用 `bytes::Bytes` 类型实现零拷贝:

```rust
// build.rs 配置 protobuf 使用 Bytes
protobuf_codegen::Codegen::new()
    .pure()
    .out_dir(&protos_dir)
    .inputs(["protos/rendezvous.proto", "protos/message.proto"])
    .include("protos")
    .customize(protobuf_codegen::Customize::default().tokio_bytes(true))
    .run()
    .expect("Failed to compile protobuf files");

// 帧转换时直接传递 Bytes,只增加引用计数
let msg_bytes = video_adapter.encode_frame_bytes_zero_copy(
    frame.data.clone(),  // clone 只增加引用计数,不拷贝数据
    frame.is_keyframe,
    frame.pts_ms as u64,
);

// TCP 发送也使用 Bytes,避免拷贝
writer.write_all(&msg_bytes).await?;
```

### 10.2 输入节流 (Input Throttling)

防止 HID 设备 EAGAIN 错误和提高性能:

```rust
pub struct InputThrottler {
    last_mouse_time: Instant,
    mouse_interval: Duration,  // 默认 16ms (≈60Hz)
}

impl InputThrottler {
    /// 检查是否应该发送鼠标事件
    pub fn should_send_mouse(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_mouse_time) >= self.mouse_interval {
            self.last_mouse_time = now;
            true
        } else {
            false  // 跳过此事件,避免 HID 缓冲区溢出
        }
    }
}
```

**效果:**
- 防止 HID write() 返回 EAGAIN (资源暂时不可用)
- 减少 CPU 使用率 (过滤冗余的鼠标移动事件)
- 保持流畅的鼠标体验 (60Hz 已足够)

### 10.3 共享管道架构

视频/音频使用共享管道,多个连接订阅同一数据流:

```rust
// 视频管道 (broadcast channel)
let (tx, _rx) = broadcast::channel(4);  // 容量 4 帧

// 连接 1 订阅
let mut rx1 = tx.subscribe();

// 连接 2 订阅 (共享同一编码数据)
let mut rx2 = tx.subscribe();

// 发送帧时,所有订阅者都会收到 (零拷贝)
tx.send(frame).unwrap();
```

**优点:**
- 单次编码,多次使用 (减少 CPU/GPU 负载)
- 零拷贝共享 (使用 `Bytes` 引用计数)
- 自动背压控制 (慢客户端会 lag,不影响快客户端)

### 10.4 管道重订阅机制

当视频管道重启时 (如切换码率),连接自动重新订阅:

```rust
async fn run_video_streaming(...) {
    // 外层循环: 处理管道重启
    'subscribe_loop: loop {
        // 订阅视频管道
        let mut encoded_frame_rx = video_manager.subscribe_encoded_frames().await;

        // 内层循环: 接收帧
        loop {
            match encoded_frame_rx.recv().await {
                Ok(frame) => { /* 发送帧 */ }
                Err(RecvError::Lagged(n)) => {
                    warn!("Video lagged {} frames", n);
                }
                Err(RecvError::Closed) => {
                    // 管道重启,重新订阅
                    info!("Video pipeline closed, re-subscribing...");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue 'subscribe_loop;
                }
            }
        }
    }
}
```

### 10.5 预分配缓冲区

TCP 发送使用预分配缓冲区减少内存分配:

```rust
// 预分配 128KB 缓冲区 (足够大部分视频帧)
let mut frame_buf = BytesMut::with_capacity(128 * 1024);

// 复用缓冲区发送多个帧
loop {
    frame_buf.clear();
    write_frame_buffered(&mut writer, &data, &mut frame_buf).await?;
}
```

### 10.6 编码器协商

根据硬件能力动态选择最优编码器:

```rust
// 优先级: H264 > H265 > VP8 > VP9
let available_encoders = video_manager.available_encoders();
let preferred_order = [
    VideoEncoderType::H264,
    VideoEncoderType::H265,
    VideoEncoderType::VP8,
    VideoEncoderType::VP9,
];

for codec in preferred_order {
    if available_encoders.contains(&codec) {
        // 使用此编码器
        negotiated_codec = Some(codec);
        break;
    }
}
```

**效果:**
- 优先使用硬件加速 (H264/H265)
- 回退到软件编码 (VP8/VP9) 如果硬件不可用
- 客户端自动适配 (RustDesk 支持所有编码器)

---

## 11. P2P 直连与中继回退

### 11.1 连接策略

当收到 PunchHole 请求时,系统会先尝试 P2P 直连,失败后自动回退到中继:

```
PunchHole 请求
      │
      ▼
┌─────────────────┐
│ 尝试 P2P 直连   │ ◄── 3秒超时
│ (TCP connect)   │
└────────┬────────┘
         │
    ┌────┴────┐
    │ 成功?   │
    └────┬────┘
    Yes  │  No
    ▼    │   ▼
┌───────┐│┌─────────────┐
│ 直连  │││ 中继回退    │
│ 通信  │││ (hbbr)      │
└───────┘│└─────────────┘
```

### 11.2 punch.rs 模块

```rust
/// P2P 直连超时时间
const DIRECT_CONNECT_TIMEOUT_MS: u64 = 3000;

/// 直连结果
pub enum PunchResult {
    DirectConnection(TcpStream),
    NeedRelay,
}

/// 尝试 P2P 直连
pub async fn try_direct_connection(peer_addr: SocketAddr) -> PunchResult;

/// Punch hole 处理器
pub struct PunchHoleHandler {
    connection_manager: Arc<ConnectionManager>,
}
```

### 11.3 中继认证

如果中继服务器配置了 `-k` 选项，需��在配置中设置 `relay_key`：

```rust
// 发送 RequestRelay 时包含 licence_key
let request_relay = make_request_relay(uuid, relay_key, socket_addr);
```

---

## 12. 常见问题

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

### Q: 切换画质后视频静止?

1. 检查日志是否有 "re-subscribing" 信息
2. 确认管道重启后成功重新订阅
3. 检查 broadcast channel 是否正常关闭和重建

### Q: 如何自建服务器?

参考 RustDesk Server 部署文档:
- hbbs: Rendezvous 服务器 (默认端口 21116)
- hbbr: 中继服务器 (默认端口 21117)

**Docker 快速部署:**
```bash
docker run -d --name hbbs \
  -p 21116:21116 \
  -p 21116:21116/udp \
  -p 21118:21118 \
  -v ./hbbs:/root \
  rustdesk/rustdesk-server hbbs

docker run -d --name hbbr \
  -p 21117:21117 \
  -p 21119:21119 \
  -v ./hbbr:/root \
  rustdesk/rustdesk-server hbbr
```

### Q: 输入节流是什么?

输入节流限制鼠标事件发送频率为 60Hz (16ms),防止:
- HID 设备写入错误 (EAGAIN)
- CPU 使用率过高
- 网络带宽浪费

这对用户体验几乎无影响,因为 60Hz 已经足够流畅。

---

## 13. 实现亮点

### 13.1 双密钥系统

- **Curve25519**: 用于 ECDH 密钥交换和加密
- **Ed25519**: 用于 SignedId 签名和验证
- 每个连接使用临时 Curve25519 密钥对,提高安全性

### 13.2 三种连接模式

1. **P2P 直连**: 最快,延迟最低,优先尝试
2. **中继连接**: 通过 hbbr 中继,适用于 NAT 环境
3. **局域网直连**: 同一局域网内的优化路径

### 13.3 零拷贝架构

- 使用 `bytes::Bytes` 引用计数,避免内存拷贝
- 视频/音频数据在管道中共享,单次编码多次使用
- Protobuf 消息直接使用 `Bytes` 字段 (tokio_bytes = true)

### 13.4 容错与恢复

- **管道重订阅**: 视频/音频管道重启时自动重新连接
- **UUID 持久化**: 避免重启后 UUID_MISMATCH 错误
- **连接重试**: Rendezvous 连接失败时自动重试,指数退避

### 13.5 性能优化

- 预分配缓冲区 (128KB)
- 输入节流 (60Hz 鼠标)
- 共享管道 (broadcast channel)
- 编码器协商 (硬件优先)
- 零拷贝传输 (Bytes)

---

## 14. 与原版 RustDesk 的差异

| 特性 | One-KVM RustDesk | 原版 RustDesk Server |
|------|------------------|---------------------|
| 角色 | 被控端 (受控设备) | 服务端 (中继/注册) |
| 视频源 | V4L2 硬件捕获 | 屏幕捕获 (桌面) |
| HID | USB OTG Gadget | 操作系统 API |
| 加密 | NaCl (Curve25519) | 同 |
| 协议 | RustDesk Protocol | 同 |
| P2P | 支持 | 支持 |
| 中继 | 支持 | 提供中继服务 |
| 公共服务器 | 可配置 (secrets.toml) | N/A |
| 多连接 | 支持 | N/A |
| 输入节流 | 60Hz 限流 | 无限制 |

**关键区别**: One-KVM 实现的是 RustDesk **被控端** (类似 RustDesk Desktop 的服务器模式),而不是 RustDesk Server (hbbs/hbbr)。

---

## 15. 参考资料

- [RustDesk 官方网站](https://rustdesk.com)
- [RustDesk GitHub](https://github.com/rustdesk/rustdesk)
- [RustDesk Server GitHub](https://github.com/rustdesk/rustdesk-server)
- [RustDesk 协议文档](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common/protos)
- [NaCl 加密库](https://nacl.cr.yp.to/)
- [Protobuf 文档](https://protobuf.dev/)
