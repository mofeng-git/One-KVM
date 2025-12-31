# Video 模块文档

## 1. 模块概述

Video 模块负责视频采集、编码和流传输，是 One-KVM 的核心功能模块。

### 1.1 主要功能

- V4L2 视频设备采集
- 多格式像素转换
- 硬件/软件视频编码
- MJPEG 和 WebRTC 流传输
- 帧去重和质量控制

### 1.2 文件结构

```
src/video/
├── mod.rs                    # 模块导出
├── capture.rs                # V4L2 视频采集 (22KB)
├── streamer.rs               # 视频流服务 (34KB)
├── stream_manager.rs         # 流管理器 (24KB)
├── shared_video_pipeline.rs  # 共享视频管道 (35KB)
├── h264_pipeline.rs          # H264 编码管道 (22KB)
├── format.rs                 # 像素格式定义 (9KB)
├── frame.rs                  # 视频帧结构 (6KB)
├── convert.rs                # 格式转换 (21KB)
└── encoder/                  # 编码器
    ├── mod.rs
    ├── traits.rs             # Encoder trait
    ├── h264.rs               # H264 编码
    ├── h265.rs               # H265 编码
    ├── vp8.rs                # VP8 编码
    ├── vp9.rs                # VP9 编码
    ├── jpeg.rs               # JPEG 编码
    └── registry.rs           # 编码器注册表
```

---

## 2. 架构设计

### 2.1 数据流

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Video Data Flow                                     │
└─────────────────────────────────────────────────────────────────────────────┘

V4L2 Device (/dev/video0)
    │
    │ Raw frames (MJPEG/YUYV/NV12)
    ▼
┌───────────────────┐
│  VideoCapturer    │ ◄─── capture.rs
│  - open_device()  │
│  - read_frame()   │
│  - set_format()   │
└─────────┬─────────┘
          │ VideoFrame
          ▼
┌───────────────────┐
│    Streamer       │ ◄─── streamer.rs
│  - start()        │
│  - stop()         │
│  - get_info()     │
└─────────┬─────────┘
          │
    ┌─────┴─────┐
    │           │
    ▼           ▼
┌────────┐  ┌────────────────────────────┐
│ MJPEG  │  │  SharedVideoPipeline       │
│ Mode   │  │  - Decode (MJPEG→YUV)      │
│        │  │  - Convert (YUV→target)    │
│        │  │  - Encode (H264/H265/VP8)  │
└────────┘  └─────────────┬──────────────┘
    │                     │
    ▼                     ▼
┌────────┐          ┌────────┐
│ HTTP   │          │ WebRTC │
│ Stream │          │ RTP    │
└────────┘          └────────┘
```

### 2.2 组件关系

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Component Relationships                                │
└─────────────────────────────────────────────────────────────────────────────┘

VideoStreamManager (stream_manager.rs)
    │
    ├──► Streamer (MJPEG mode)
    │    └──► VideoCapturer
    │
    └──► WebRtcStreamer (WebRTC mode)
         └──► SharedVideoPipeline
              ├──► VideoCapturer
              ├──► MjpegDecoder
              ├──► YuvConverter
              └──► Encoders[]
                   ├── H264Encoder
                   ├── H265Encoder
                   ├── VP8Encoder
                   └── VP9Encoder
```

---

## 3. 核心组件

### 3.1 VideoCapturer (capture.rs)

V4L2 视频采集器，负责从摄像头/采集卡读取视频帧。

#### 主要接口

```rust
pub struct VideoCapturer {
    device: Device,
    stream: Option<MmapStream<'static>>,
    config: CaptureConfig,
    format: PixelFormat,
    resolution: Resolution,
}

impl VideoCapturer {
    /// 打开视频设备
    pub fn open(device_path: &str) -> Result<Self>;

    /// 设置视频格式
    pub fn set_format(&mut self, config: &CaptureConfig) -> Result<()>;

    /// 开始采集
    pub fn start(&mut self) -> Result<()>;

    /// 停止采集
    pub fn stop(&mut self) -> Result<()>;

    /// 读取一帧
    pub fn read_frame(&mut self) -> Result<VideoFrame>;

    /// 列出设备支持的格式
    pub fn list_formats(&self) -> Vec<FormatInfo>;

    /// 列出支持的分辨率
    pub fn list_resolutions(&self, format: PixelFormat) -> Vec<Resolution>;
}
```

#### 采集配置

```rust
pub struct CaptureConfig {
    pub device: String,           // /dev/video0
    pub width: u32,               // 1920
    pub height: u32,              // 1080
    pub fps: u32,                 // 30
    pub format: Option<PixelFormat>, // 优先格式
    pub buffer_count: u32,        // 4
}
```

#### 使用示例

```rust
// 打开设备
let mut capturer = VideoCapturer::open("/dev/video0")?;

// 设置格式
capturer.set_format(&CaptureConfig {
    device: "/dev/video0".to_string(),
    width: 1920,
    height: 1080,
    fps: 30,
    format: Some(PixelFormat::Mjpeg),
    buffer_count: 4,
})?;

// 开始采集
capturer.start()?;

// 读取帧
loop {
    let frame = capturer.read_frame()?;
    process_frame(frame);
}
```

### 3.2 VideoFrame (frame.rs)

视频帧数据结构，支持零拷贝和帧去重。

```rust
pub struct VideoFrame {
    /// 帧数据 (引用计数)
    data: Arc<Bytes>,

    /// xxHash64 缓存 (用于去重)
    hash: Arc<OnceLock<u64>>,

    /// 分辨率
    resolution: Resolution,

    /// 像素格式
    format: PixelFormat,

    /// 行步长
    stride: u32,

    /// 是否关键帧
    key_frame: bool,

    /// 帧序号
    sequence: u64,

    /// 采集时间戳
    capture_ts: Instant,

    /// 是否有信号
    online: bool,
}

impl VideoFrame {
    /// 创建新帧
    pub fn new(data: Bytes, resolution: Resolution, format: PixelFormat) -> Self;

    /// 获取帧数据
    pub fn data(&self) -> &[u8];

    /// 计算帧哈希 (懒加载)
    pub fn hash(&self) -> u64;

    /// 检查帧是否相同 (用于去重)
    pub fn is_same_as(&self, other: &Self) -> bool;

    /// 克隆帧 (零拷贝)
    pub fn clone_ref(&self) -> Self;
}
```

### 3.3 PixelFormat (format.rs)

支持的像素格式定义。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    // 压缩格式
    Mjpeg,      // Motion JPEG (优先级: 100)
    Jpeg,       // Static JPEG (优先级: 99)

    // YUV 4:2:2 打包格式
    Yuyv,       // YUYV/YUY2 (优先级: 80)
    Yvyu,       // YVYU (优先级: 64)
    Uyvy,       // UYVY (优先级: 65)

    // YUV 半平面格式
    Nv12,       // NV12 (优先级: 75)
    Nv16,       // NV16 (优先级: 60)
    Nv24,       // NV24 (优先级: 55)

    // YUV 平面格式
    Yuv420,     // I420/YU12 (优先级: 70)
    Yvu420,     // YV12 (优先级: 63)

    // RGB 格式
    Rgb565,     // RGB565 (优先级: 40)
    Rgb24,      // RGB24 (优先级: 50)
    Bgr24,      // BGR24 (优先级: 49)

    // 灰度
    Grey,       // 8-bit grayscale (优先级: 10)
}

impl PixelFormat {
    /// 获取格式优先级 (越高越好)
    pub fn priority(&self) -> u32;

    /// 计算帧大小
    pub fn frame_size(&self, width: u32, height: u32) -> usize;

    /// 转换为 V4L2 FourCC
    pub fn to_fourcc(&self) -> u32;

    /// 从 V4L2 FourCC 转换
    pub fn from_fourcc(fourcc: u32) -> Option<Self>;

    /// 是否压缩格式
    pub fn is_compressed(&self) -> bool;
}
```

### 3.4 SharedVideoPipeline (shared_video_pipeline.rs)

多会话共享的视频编码管道。

```rust
pub struct SharedVideoPipeline {
    /// 视频采集器
    capturer: Arc<Mutex<VideoCapturer>>,

    /// MJPEG 解码器
    decoder: MjpegDecoder,

    /// YUV 转换器
    converter: YuvConverter,

    /// 编码器实例
    encoders: HashMap<VideoCodec, Box<dyn Encoder>>,

    /// 活跃会话
    sessions: Arc<RwLock<Vec<SessionSender>>>,

    /// 配置
    config: PipelineConfig,
}

impl SharedVideoPipeline {
    /// 创建管道
    pub async fn new(config: PipelineConfig) -> Result<Self>;

    /// 启动管道
    pub async fn start(&self) -> Result<()>;

    /// 停止管道
    pub async fn stop(&self) -> Result<()>;

    /// 添加会话订阅
    pub fn subscribe(&self, codec: VideoCodec) -> Receiver<EncodedFrame>;

    /// 移除会话订阅
    pub fn unsubscribe(&self, session_id: &str);

    /// 编码单帧 (多编码器)
    async fn encode_frame(&self, frame: VideoFrame) -> Result<()>;
}
```

#### 编码流程

```
Input: VideoFrame (MJPEG)
    │
    ▼
┌───────────────────┐
│   MJPEG Decode    │  turbojpeg / VAAPI
│   MJPEG → YUV420  │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│   YUV Convert     │  libyuv (SIMD)
│  YUV420 → target  │
└─────────┬─────────┘
          │
    ┌─────┴─────┬─────────┬─────────┐
    │           │         │         │
    ▼           ▼         ▼         ▼
┌───────┐  ┌───────┐  ┌───────┐  ┌───────┐
│ H264  │  │ H265  │  │  VP8  │  │  VP9  │
│Encoder│  │Encoder│  │Encoder│  │Encoder│
└───┬───┘  └───┬───┘  └───┬───┘  └───┬───┘
    │          │          │          │
    └──────────┴──────────┴──────────┘
                    │
                    ▼
            EncodedFrame[]
            (distribute to sessions)
```

### 3.5 Streamer (streamer.rs)

高层视频流服务，管理采集和分发。

```rust
pub struct Streamer {
    /// 采集器
    capturer: Option<Arc<Mutex<VideoCapturer>>>,

    /// 采集任务句柄
    capture_task: Option<JoinHandle<()>>,

    /// 帧广播通道
    frame_tx: broadcast::Sender<VideoFrame>,

    /// 状态
    state: Arc<RwLock<StreamerState>>,

    /// 配置
    config: StreamerConfig,

    /// 事件总线
    events: Arc<EventBus>,
}

impl Streamer {
    /// 创建流服务
    pub fn new(events: Arc<EventBus>) -> Self;

    /// 启动流
    pub async fn start(&self, config: StreamerConfig) -> Result<()>;

    /// 停止流
    pub async fn stop(&self) -> Result<()>;

    /// 订阅帧
    pub fn subscribe(&self) -> broadcast::Receiver<VideoFrame>;

    /// 获取状态
    pub fn state(&self) -> StreamerState;

    /// 获取信息
    pub fn get_info(&self) -> StreamerInfo;

    /// 应用配置
    pub async fn apply_config(&self, config: StreamerConfig) -> Result<()>;
}

pub struct StreamerState {
    pub status: StreamStatus,
    pub device: Option<String>,
    pub resolution: Option<Resolution>,
    pub format: Option<PixelFormat>,
    pub fps: f32,
    pub frame_count: u64,
    pub error: Option<String>,
}

pub enum StreamStatus {
    Idle,
    Starting,
    Streaming,
    Stopping,
    Error,
}
```

### 3.6 VideoStreamManager (stream_manager.rs)

统一管理 MJPEG 和 WebRTC 流模式。

```rust
pub struct VideoStreamManager {
    /// MJPEG 流服务
    mjpeg_streamer: Arc<Streamer>,

    /// WebRTC 流服务
    webrtc_streamer: Arc<RwLock<Option<WebRtcStreamer>>>,

    /// 当前模式
    mode: Arc<RwLock<StreamMode>>,

    /// 配置存储
    config_store: ConfigStore,

    /// 事件总线
    events: Arc<EventBus>,
}

impl VideoStreamManager {
    /// 创建管理器
    pub fn new(config_store: ConfigStore, events: Arc<EventBus>) -> Self;

    /// 启动流
    pub async fn start(&self) -> Result<()>;

    /// 停止流
    pub async fn stop(&self) -> Result<()>;

    /// 切换模式
    pub async fn set_mode(&self, mode: StreamMode) -> Result<()>;

    /// 获取当前模式
    pub fn get_mode(&self) -> StreamMode;

    /// 获取设备列表
    pub fn list_devices(&self) -> Vec<DeviceInfo>;

    /// 获取统计信息
    pub fn get_stats(&self) -> StreamStats;

    /// 获取 MJPEG 订阅
    pub fn subscribe_mjpeg(&self) -> broadcast::Receiver<VideoFrame>;

    /// 创建 WebRTC 会话
    pub async fn create_webrtc_session(&self, params: SessionParams) -> Result<Session>;
}

pub enum StreamMode {
    Mjpeg,
    Webrtc,
}
```

---

## 4. 编码器系统

### 4.1 Encoder Trait (encoder/traits.rs)

```rust
pub trait Encoder: Send + Sync {
    /// 编码一帧
    fn encode(&mut self, frame: &VideoFrame) -> Result<EncodedFrame>;

    /// 获取编码器类型
    fn codec(&self) -> VideoCodec;

    /// 获取当前码率
    fn bitrate(&self) -> u32;

    /// 设置码率
    fn set_bitrate(&mut self, bitrate: u32) -> Result<()>;

    /// 获取 GOP 大小
    fn gop_size(&self) -> u32;

    /// 强制关键帧
    fn force_keyframe(&mut self);

    /// 重置编码器
    fn reset(&mut self) -> Result<()>;

    /// 获取编码器信息
    fn info(&self) -> EncoderInfo;
}

pub struct EncodedFrame {
    pub data: Bytes,
    pub codec: VideoCodec,
    pub key_frame: bool,
    pub pts: u64,
    pub dts: u64,
}

pub enum VideoCodec {
    H264,
    H265,
    VP8,
    VP9,
}
```

### 4.2 编码器优先级

```
H264 编码器选择顺序:
1. VAAPI (Intel/AMD GPU)
2. RKMPP (Rockchip)
3. V4L2 M2M
4. x264 (Software)

H265 编码器选择顺序:
1. VAAPI
2. RKMPP
(无软件后备)

VP8/VP9 编码器:
1. VAAPI only
```

### 4.3 EncoderRegistry (encoder/registry.rs)

```rust
pub struct EncoderRegistry {
    /// 已注册的编码器工厂
    factories: HashMap<VideoCodec, Vec<EncoderFactory>>,
}

impl EncoderRegistry {
    /// 创建注册表
    pub fn new() -> Self;

    /// 注册编码器工厂
    pub fn register(&mut self, codec: VideoCodec, factory: EncoderFactory);

    /// 创建最佳编码器
    pub fn create_encoder(&self, codec: VideoCodec, config: EncoderConfig) -> Result<Box<dyn Encoder>>;

    /// 列出可用编码器
    pub fn list_available(&self, codec: VideoCodec) -> Vec<EncoderInfo>;

    /// 探测硬件能力
    pub fn probe_hardware() -> HardwareCapabilities;
}

pub struct EncoderFactory {
    pub name: String,
    pub priority: u32,
    pub create: Box<dyn Fn(EncoderConfig) -> Result<Box<dyn Encoder>>>,
    pub probe: Box<dyn Fn() -> bool>,
}
```

---

## 5. 格式转换

### 5.1 MjpegDecoder (convert.rs)

```rust
pub struct MjpegDecoder {
    /// turbojpeg 解压缩器
    decompressor: Decompressor,

    /// 输出缓冲区
    output_buffer: Vec<u8>,
}

impl MjpegDecoder {
    /// 创建解码器
    pub fn new() -> Result<Self>;

    /// 解码 MJPEG 到 YUV420
    pub fn decode(&mut self, jpeg_data: &[u8]) -> Result<YuvFrame>;

    /// 获取图像信息
    pub fn get_info(jpeg_data: &[u8]) -> Result<ImageInfo>;
}
```

### 5.2 YuvConverter (convert.rs)

使用 libyuv 进行高性能格式转换。

```rust
pub struct YuvConverter;

impl YuvConverter {
    /// YUYV → YUV420
    pub fn yuyv_to_yuv420(src: &[u8], dst: &mut [u8], width: u32, height: u32);

    /// NV12 → YUV420
    pub fn nv12_to_yuv420(src: &[u8], dst: &mut [u8], width: u32, height: u32);

    /// RGB24 → YUV420
    pub fn rgb24_to_yuv420(src: &[u8], dst: &mut [u8], width: u32, height: u32);

    /// YUV420 → NV12
    pub fn yuv420_to_nv12(src: &[u8], dst: &mut [u8], width: u32, height: u32);

    /// 缩放 YUV420
    pub fn scale_yuv420(
        src: &[u8], src_width: u32, src_height: u32,
        dst: &mut [u8], dst_width: u32, dst_height: u32,
        filter: ScaleFilter,
    );
}

pub enum ScaleFilter {
    None,       // 最近邻
    Linear,     // 双线性
    Bilinear,   // 双线性 (同 Linear)
    Box,        // 盒式滤波
}
```

---

## 6. 配置说明

### 6.1 视频配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct VideoConfig {
    /// 设备路径 (/dev/video0)
    pub device: Option<String>,

    /// 像素格式 (MJPEG/YUYV/NV12)
    pub format: Option<String>,

    /// 宽度
    pub width: u32,

    /// 高度
    pub height: u32,

    /// 帧率
    pub fps: u32,

    /// JPEG 质量 (1-100)
    pub quality: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            device: None,
            format: None,
            width: 1920,
            height: 1080,
            fps: 30,
            quality: 80,
        }
    }
}
```

### 6.2 流配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct StreamConfig {
    /// 流模式
    pub mode: StreamMode,

    /// 码率 (kbps)
    pub bitrate_kbps: u32,

    /// GOP 大小
    pub gop_size: u32,

    /// 编码器类型
    pub encoder: EncoderType,

    /// STUN 服务器
    pub stun_server: Option<String>,

    /// TURN 服务器
    pub turn_server: Option<String>,

    /// TURN 用户名
    pub turn_username: Option<String>,

    /// TURN 密码
    pub turn_password: Option<String>,
}
```

---

## 7. API 端点

### 7.1 流控制

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/stream/status` | GET | 获取流状态 |
| `/api/stream/start` | POST | 启动流 |
| `/api/stream/stop` | POST | 停止流 |
| `/api/stream/mode` | GET | 获取流模式 |
| `/api/stream/mode` | POST | 设置流模式 |
| `/api/stream/mjpeg` | GET | MJPEG 流 |
| `/api/stream/snapshot` | GET | 获取快照 |

### 7.2 设备管理

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/devices/video` | GET | 列出视频设备 |
| `/api/devices/video/:id/formats` | GET | 列出设备格式 |
| `/api/devices/video/:id/resolutions` | GET | 列出分辨率 |

### 7.3 响应格式

```json
// GET /api/stream/status
{
    "status": "streaming",
    "device": "/dev/video0",
    "resolution": { "width": 1920, "height": 1080 },
    "format": "MJPEG",
    "fps": 30.0,
    "frame_count": 12345,
    "mode": "mjpeg"
}

// GET /api/devices/video
{
    "devices": [
        {
            "path": "/dev/video0",
            "name": "USB Capture",
            "driver": "uvcvideo",
            "bus": "usb-0000:00:14.0-1"
        }
    ]
}
```

---

## 8. 事件

视频模块发布的事件:

```rust
pub enum SystemEvent {
    /// 流状态变化
    StreamStateChanged {
        state: String,      // "idle" | "starting" | "streaming" | "stopping" | "error"
        device: Option<String>,
        resolution: Option<Resolution>,
        fps: Option<f32>,
    },

    /// 设备变化
    VideoDeviceChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },

    /// 编码器变化
    EncoderChanged {
        codec: String,
        hardware: bool,
    },
}
```

---

## 9. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device busy: {0}")]
    DeviceBusy(String),

    #[error("Format not supported: {0:?}")]
    FormatNotSupported(PixelFormat),

    #[error("Resolution not supported: {0}x{1}")]
    ResolutionNotSupported(u32, u32),

    #[error("Capture error: {0}")]
    CaptureError(String),

    #[error("Encoder error: {0}")]
    EncoderError(String),

    #[error("No signal")]
    NoSignal,

    #[error("Device lost")]
    DeviceLost,
}
```

---

## 10. 性能优化

### 10.1 零拷贝

- `Arc<Bytes>` 共享帧数据
- 引用计数避免复制

### 10.2 帧去重

- xxHash64 快速哈希
- 相同帧跳过编码

### 10.3 硬件加速

- VAAPI 优先
- 自动后备软件编码

### 10.4 内存池

- 预分配帧缓冲区
- 复用编码器缓冲区

---

## 11. 常见问题

### Q: 如何添加新的视频格式?

1. 在 `format.rs` 添加枚举值
2. 实现 `to_fourcc()` 和 `from_fourcc()`
3. 在 `convert.rs` 添加转换函数

### Q: 如何添加新的编码器?

1. 实现 `Encoder` trait
2. 创建 `EncoderFactory`
3. 在 `EncoderRegistry` 注册

### Q: 帧率不稳定怎么办?

1. 检查 USB 带宽
2. 降低分辨率
3. 使用 MJPEG 格式
4. 启用硬件编码
