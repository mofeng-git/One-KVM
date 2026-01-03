# WebRTC 模块文档

## 1. 模块概述

WebRTC 模块提供低延迟的实时音视频流传输，支持多种视频编码格式和 DataChannel HID 控制。

### 1.1 主要功能

- WebRTC 会话管理
- 多编码器支持 (H264/H265/VP8/VP9)
- 音频轨道 (Opus)
- DataChannel HID
- ICE/STUN/TURN 支持

### 1.2 文件结构

```
src/webrtc/
├── mod.rs                  # 模块导出
├── webrtc_streamer.rs      # 统一管理器 (35KB)
├── universal_session.rs    # 会话管理 (32KB)
├── unified_video_track.rs  # 统一视频轨道 (15KB)
├── video_track.rs          # 视频轨道 (19KB)
├── rtp.rs                  # RTP 打包 (24KB)
├── h265_payloader.rs       # H265 RTP (15KB)
├── peer.rs                 # PeerConnection (17KB)
├── config.rs               # 配置 (3KB)
├── signaling.rs            # 信令 (5KB)
├── session.rs              # 会话基类 (8KB)
└── track.rs                # 轨道基类 (11KB)
```

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         WebRTC Architecture                                  │
└─────────────────────────────────────────────────────────────────────────────┘

                Browser
                   │
                   │ HTTP Signaling
                   ▼
         ┌─────────────────┐
         │ WebRtcStreamer  │
         │(webrtc_streamer)│
         └────────┬────────┘
                  │
    ┌─────────────┼─────────────┐
    │             │             │
    ▼             ▼             ▼
┌────────┐  ┌────────┐    ┌────────┐
│Session │  │Session │    │Session │
│   1    │  │   2    │    │   N    │
└───┬────┘  └───┬────┘    └───┬────┘
    │           │             │
    ├───────────┼─────────────┤
    │           │             │
    ▼           ▼             ▼
┌─────────────────────────────────────┐
│       SharedVideoPipeline           │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐  │
│  │H264 │ │H265 │ │VP8  │ │VP9  │  │
│  └─────┘ └─────┘ └─────┘ └─────┘  │
└─────────────────────────────────────┘
                  │
                  ▼
         ┌────────────────┐
         │ VideoCapturer  │
         └────────────────┘
```

### 2.2 会话生命周期

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Session Lifecycle                                       │
└─────────────────────────────────────────────────────────────────────────────┘

1. 创建会话
   POST /webrtc/session
        │
        ▼
   ┌─────────────────┐
   │ Create Session  │
   │ Generate ID     │
   └────────┬────────┘
            │
            ▼
   { session_id: "..." }

2. 发送 Offer
   POST /webrtc/offer
   { session_id, codec, offer_sdp }
        │
        ▼
   ┌─────────────────┐
   │ Process Offer   │
   │ Create Answer   │
   │ Setup Tracks    │
   └────────┬────────┘
            │
            ▼
   { answer_sdp, ice_candidates }

3. ICE 候选
   POST /webrtc/ice
   { session_id, candidate }
        │
        ▼
   ┌─────────────────┐
   │ Add ICE         │
   │ Candidate       │
   └─────────────────┘

4. 连接建立
   ┌─────────────────┐
   │ DTLS Handshake  │
   │ SRTP Setup      │
   │ DataChannel     │
   └────────┬────────┘
            │
            ▼
   开始传输视频/音频

5. 关闭会话
   POST /webrtc/close
   { session_id }
        │
        ▼
   ┌─────────────────┐
   │ Cleanup         │
   │ Release         │
   └─────────────────┘
```

---

## 3. 核心组件

### 3.1 WebRtcStreamer (webrtc_streamer.rs)

WebRTC 服务主类。

```rust
pub struct WebRtcStreamer {
    /// 会话映射
    sessions: Arc<RwLock<HashMap<String, Arc<UniversalSession>>>>,

    /// 共享视频管道
    video_pipeline: Arc<SharedVideoPipeline>,

    /// 共享音频管道
    audio_pipeline: Arc<SharedAudioPipeline>,

    /// HID 控制器
    hid: Arc<HidController>,

    /// 配置
    config: WebRtcConfig,

    /// 事件总线
    events: Arc<EventBus>,
}

impl WebRtcStreamer {
    /// 创建流服务
    pub async fn new(
        video_pipeline: Arc<SharedVideoPipeline>,
        audio_pipeline: Arc<SharedAudioPipeline>,
        hid: Arc<HidController>,
        config: WebRtcConfig,
        events: Arc<EventBus>,
    ) -> Result<Self>;

    /// 创建会话
    pub async fn create_session(&self) -> Result<String>;

    /// 处理 Offer
    pub async fn process_offer(
        &self,
        session_id: &str,
        offer: &str,
        codec: VideoCodec,
    ) -> Result<OfferResponse>;

    /// 添加 ICE 候选
    pub async fn add_ice_candidate(
        &self,
        session_id: &str,
        candidate: &str,
    ) -> Result<()>;

    /// 关闭会话
    pub async fn close_session(&self, session_id: &str) -> Result<()>;

    /// 获取会话列表
    pub fn list_sessions(&self) -> Vec<SessionInfo>;

    /// 获取统计信息
    pub fn get_stats(&self) -> WebRtcStats;
}

pub struct OfferResponse {
    pub answer_sdp: String,
    pub ice_candidates: Vec<String>,
}

pub struct WebRtcStats {
    pub active_sessions: usize,
    pub total_bytes_sent: u64,
    pub avg_bitrate: u32,
}
```

### 3.2 UniversalSession (universal_session.rs)

单个 WebRTC 会话。

```rust
pub struct UniversalSession {
    /// 会话 ID
    id: String,

    /// PeerConnection
    peer: Arc<RTCPeerConnection>,

    /// 视频轨道
    video_track: Arc<UniversalVideoTrack>,

    /// 音频轨道
    audio_track: Option<Arc<dyn TrackLocal>>,

    /// HID DataChannel
    hid_channel: Arc<RwLock<Option<Arc<RTCDataChannel>>>>,

    /// HID 处理器
    hid_handler: Arc<HidDataChannelHandler>,

    /// 状态
    state: Arc<RwLock<SessionState>>,

    /// 编码器类型
    codec: VideoCodec,
}

impl UniversalSession {
    /// 创建会话
    pub async fn new(
        id: String,
        config: &WebRtcConfig,
        video_pipeline: Arc<SharedVideoPipeline>,
        audio_pipeline: Arc<SharedAudioPipeline>,
        hid_handler: Arc<HidDataChannelHandler>,
        codec: VideoCodec,
    ) -> Result<Self>;

    /// 处理 Offer SDP
    pub async fn handle_offer(&self, offer_sdp: &str) -> Result<String>;

    /// 添加 ICE 候选
    pub async fn add_ice_candidate(&self, candidate: &str) -> Result<()>;

    /// 获取 ICE 候选
    pub fn get_ice_candidates(&self) -> Vec<String>;

    /// 关闭会话
    pub async fn close(&self) -> Result<()>;

    /// 获取状态
    pub fn state(&self) -> SessionState;

    /// 获取统计
    pub fn stats(&self) -> SessionStats;
}

pub enum SessionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

pub struct SessionStats {
    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub bitrate: u32,
    pub frame_rate: f32,
    pub round_trip_time: Duration,
}
```

### 3.3 VideoTrack (video_track.rs)

视频轨道封装。

```rust
pub struct UniversalVideoTrack {
    /// 轨道 ID
    id: String,

    /// 编码类型
    codec: VideoCodec,

    /// RTP 发送器
    rtp_sender: Arc<RtpSender>,

    /// 帧计数
    frame_count: AtomicU64,

    /// 统计
    stats: Arc<RwLock<TrackStats>>,
}

impl UniversalVideoTrack {
    /// 创建轨道
    pub fn new(id: &str, codec: VideoCodec) -> Result<Self>;

    /// 发送编码帧
    pub async fn send_frame(&self, frame: &EncodedFrame) -> Result<()>;

    /// 获取 RTP 参数
    pub fn rtp_params(&self) -> RtpParameters;

    /// 获取统计
    pub fn stats(&self) -> TrackStats;
}

pub struct TrackStats {
    pub frames_sent: u64,
    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub packet_loss: f32,
}
```

### 3.4 RTP 打包 (rtp.rs)

RTP 协议实现。

```rust
pub struct RtpPacketizer {
    /// SSRC
    ssrc: u32,

    /// 序列号
    sequence: u16,

    /// 时间戳
    timestamp: u32,

    /// 负载类型
    payload_type: u8,

    /// 时钟频率
    clock_rate: u32,
}

impl RtpPacketizer {
    /// 创建打包器
    pub fn new(codec: VideoCodec) -> Self;

    /// 打包 H264 帧
    pub fn packetize_h264(&mut self, frame: &[u8], keyframe: bool) -> Vec<Vec<u8>>;

    /// 打包 VP8 帧
    pub fn packetize_vp8(&mut self, frame: &[u8], keyframe: bool) -> Vec<Vec<u8>>;

    /// 打包 VP9 帧
    pub fn packetize_vp9(&mut self, frame: &[u8], keyframe: bool) -> Vec<Vec<u8>>;

    /// 打包 Opus 帧
    pub fn packetize_opus(&mut self, frame: &[u8]) -> Vec<u8>;
}

/// H264 NAL 单元分片
pub struct H264Fragmenter;

impl H264Fragmenter {
    /// 分片大于 MTU 的 NAL
    pub fn fragment(nal: &[u8], mtu: usize) -> Vec<Vec<u8>>;

    /// 创建 STAP-A 聚合
    pub fn aggregate(nals: &[&[u8]]) -> Vec<u8>;
}
```

### 3.5 H265 打包器 (h265_payloader.rs)

H265/HEVC RTP 打包。

```rust
pub struct H265Payloader {
    /// MTU 大小
    mtu: usize,
}

impl H265Payloader {
    /// 创建打包器
    pub fn new(mtu: usize) -> Self;

    /// 打包 H265 帧
    pub fn packetize(&self, frame: &[u8]) -> Vec<Vec<u8>>;

    /// 分析 NAL 单元类型
    fn get_nal_type(nal: &[u8]) -> u8;

    /// 是否需要分片
    fn needs_fragmentation(&self, nal: &[u8]) -> bool;
}
```

---

## 4. 信令协议

### 4.1 创建会话

```
POST /api/webrtc/session
Content-Type: application/json

{}

Response:
{
    "session_id": "abc123-def456"
}
```

### 4.2 发送 Offer

```
POST /api/webrtc/offer
Content-Type: application/json

{
    "session_id": "abc123-def456",
    "video_codec": "h264",
    "enable_audio": true,
    "offer_sdp": "v=0\r\no=- ..."
}

Response:
{
    "answer_sdp": "v=0\r\no=- ...",
    "ice_candidates": [
        "candidate:1 1 UDP ...",
        "candidate:2 1 TCP ..."
    ]
}
```

### 4.3 ICE 候选

```
POST /api/webrtc/ice
Content-Type: application/json

{
    "session_id": "abc123-def456",
    "candidate": "candidate:1 1 UDP ..."
}

Response:
{
    "success": true
}
```

### 4.4 关闭会话

```
POST /api/webrtc/close
Content-Type: application/json

{
    "session_id": "abc123-def456"
}

Response:
{
    "success": true
}
```

---

## 5. 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct WebRtcConfig {
    /// STUN 服务器
    pub stun_servers: Vec<String>,

    /// TURN 服务器
    pub turn_servers: Vec<TurnServer>,

    /// 默认编码器
    pub default_codec: VideoCodec,

    /// 码率 (kbps)
    pub bitrate_kbps: u32,

    /// GOP 大小
    pub gop_size: u32,

    /// 启用音频
    pub enable_audio: bool,

    /// 启用 DataChannel HID
    pub enable_datachannel_hid: bool,
}

pub struct TurnServer {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec!["stun:stun.l.google.com:19302".to_string()],
            turn_servers: vec![],
            default_codec: VideoCodec::H264,
            bitrate_kbps: 2000,
            gop_size: 60,
            enable_audio: true,
            enable_datachannel_hid: true,
        }
    }
}
```

---

## 6. DataChannel HID

### 6.1 消息格式

```javascript
// 键盘事件
{
    "type": "keyboard",
    "keys": ["KeyA", "KeyB"],
    "modifiers": {
        "ctrl": false,
        "shift": true,
        "alt": false,
        "meta": false
    }
}

// 鼠标事件
{
    "type": "mouse",
    "x": 16384,
    "y": 16384,
    "button": "left",
    "event": "press"
}

// 鼠标模式
{
    "type": "mouse_mode",
    "mode": "absolute"
}
```

### 6.2 处理流程

```
DataChannel Message
        │
        ▼
┌─────────────────┐
│Parse JSON Event │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│HidDataChannel   │
│   Handler       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ HidController   │
└────────┬────────┘
         │
         ▼
    USB/Serial
```

---

## 7. 支持的编码器

| 编码器 | RTP 负载类型 | 时钟频率 | 硬件加速 |
|--------|-------------|---------|---------|
| H264 | 96 (动态) | 90000 | VAAPI/RKMPP/V4L2 |
| H265 | 97 (动态) | 90000 | VAAPI |
| VP8 | 98 (动态) | 90000 | VAAPI |
| VP9 | 99 (动态) | 90000 | VAAPI |
| Opus | 111 (动态) | 48000 | 无 (软件) |

---

## 8. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum WebRtcError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already exists")]
    SessionExists,

    #[error("Invalid SDP: {0}")]
    InvalidSdp(String),

    #[error("Codec not supported: {0}")]
    CodecNotSupported(String),

    #[error("ICE failed")]
    IceFailed,

    #[error("DTLS failed")]
    DtlsFailed,

    #[error("Track error: {0}")]
    TrackError(String),

    #[error("Connection closed")]
    ConnectionClosed,
}
```

---

## 9. 使用示例

### 9.1 创建会话

```rust
let streamer = WebRtcStreamer::new(
    video_pipeline,
    audio_pipeline,
    hid,
    WebRtcConfig::default(),
    events,
).await?;

// 创建会话
let session_id = streamer.create_session().await?;

// 处理 Offer
let response = streamer.process_offer(
    &session_id,
    &offer_sdp,
    VideoCodec::H264,
).await?;

println!("Answer: {}", response.answer_sdp);
```

### 9.2 前端连接

```javascript
// 创建 PeerConnection
const pc = new RTCPeerConnection({
    iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
});

// 创建 DataChannel
const hidChannel = pc.createDataChannel('hid');

// 创建 Offer
const offer = await pc.createOffer();
await pc.setLocalDescription(offer);

// 发送到服务器
const response = await fetch('/api/webrtc/offer', {
    method: 'POST',
    body: JSON.stringify({
        session_id,
        video_codec: 'h264',
        offer_sdp: offer.sdp
    })
});

const { answer_sdp, ice_candidates } = await response.json();

// 设置 Answer
await pc.setRemoteDescription({ type: 'answer', sdp: answer_sdp });

// 添加 ICE 候选
for (const candidate of ice_candidates) {
    await pc.addIceCandidate({ candidate });
}
```

---

## 10. 管道重启机制

当码率或编码器配置变更时，视频管道需要重启。WebRTC 模块实现了自动重连机制：

### 10.1 重启流程

```
用户修改码率/编码器
        │
        ▼
┌─────────────────────┐
│ set_bitrate_preset  │
│ 1. 保存 frame_tx    │  ← 关键：在停止前保存
│ 2. 停止旧管道       │
│ 3. 等待清理         │
│ 4. 恢复 frame_tx    │
│ 5. 创建新管道       │
│ 6. 重连所有会话     │
└─────────────────────┘
        │
        ▼
所有 WebRTC 会话自动恢复
```

### 10.2 关键代码

```rust
pub async fn set_bitrate_preset(self: &Arc<Self>, preset: BitratePreset) -> Result<()> {
    // 保存 frame_tx (监控任务会在管道停止后清除它)
    let saved_frame_tx = self.video_frame_tx.read().await.clone();

    // 停止管道
    pipeline.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 恢复 frame_tx 并重建管道
    if let Some(tx) = saved_frame_tx {
        *self.video_frame_tx.write().await = Some(tx.clone());
        let pipeline = self.ensure_video_pipeline(tx).await?;

        // 重连所有会话
        for session in sessions {
            session.start_from_video_pipeline(pipeline.subscribe(), ...).await;
        }
    }
}
```

---

## 11. 常见问题

### Q: 连接超时?

1. 检查 STUN/TURN 配置
2. 检查防火墙设置
3. 尝试使用 TURN 中继

### Q: 视频卡顿?

1. 降低分辨率/码率
2. 检查网络带宽
3. 使用硬件编码

### Q: 音频不同步?

1. 检查时间戳同步
2. 调整缓冲区大小
3. 使用 NTP 同步

### Q: 切换码率后视频静止?

1. 检查管道重启逻辑是否正确保存了 `video_frame_tx`
2. 确认会话重连成功
3. 查看日志中是否有 "Reconnecting session" 信息
