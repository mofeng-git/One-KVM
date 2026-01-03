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
├── capture.rs                # V4L2 视频采集
├── device.rs                 # V4L2 设备枚举和能力查询
├── streamer.rs               # 视频流服务 (MJPEG)
├── stream_manager.rs         # 流管理器 (统一管理 MJPEG/WebRTC)
├── video_session.rs          # 视频会话管理 (多编码器会话)
├── shared_video_pipeline.rs  # 共享视频编码管道 (多编解码器)
├── h264_pipeline.rs          # H264 专用编码管道 (WebRTC)
├── format.rs                 # 像素格式定义
├── frame.rs                  # 视频帧结构 (零拷贝)
├── convert.rs                # 格式转换 (libyuv SIMD)
├── decoder/                  # 解码器
│   ├── mod.rs
│   └── mjpeg.rs              # MJPEG 解码 (TurboJPEG/VAAPI)
└── encoder/                  # 编码器
    ├── mod.rs
    ├── traits.rs             # Encoder trait + BitratePreset
    ├── codec.rs              # 编码器类型定义
    ├── h264.rs               # H264 编码
    ├── h265.rs               # H265 编码
    ├── vp8.rs                # VP8 编码
    ├── vp9.rs                # VP9 编码
    ├── jpeg.rs               # JPEG 编码
    └── registry.rs           # 编码器注册表 (硬件探测)
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

VideoStreamManager (单一入口)
    │
    ├── mode: StreamMode (当前激活的模式)
    │
    ├──► MJPEG Mode
    │    └──► Streamer ──► MjpegStreamHandler
    │         └──► VideoCapturer
    │
    └──► WebRTC Mode
         └──► WebRtcStreamer
              ├──► VideoSessionManager (多会话管理)
              │    └──► 多个 VideoSession (每个会话独立的编解码器)
              └──► SharedVideoPipeline (共享编码管道)
                   ├──► VideoCapturer
                   ├──► MjpegDecoder (MJPEG → YUV420P/NV12)
                   │    ├── MjpegTurboDecoder (软件)
                   │    └── MjpegVaapiDecoder (硬件)
                   ├──► PixelConverter (格式转换)
                   │    ├── Nv12Converter (YUYV/RGB → NV12)
                   │    └── Yuv420pConverter
                   └──► Encoders[] (通过 EncoderRegistry 选择)
                        ├── H264Encoder (VAAPI/RKMPP/V4L2/x264)
                        ├── H265Encoder (VAAPI/RKMPP)
                        ├── VP8Encoder (VAAPI)
                        └── VP9Encoder (VAAPI)
```

---

## 3. 核心组件

### 3.1 VideoCapturer (capture.rs)

异步 V4L2 视频采集器，使用 memory-mapped 缓冲区进行高性能视频采集。

#### 主要接口

```rust
pub struct VideoCapturer {
    /// V4L2 设备句柄
    device: Arc<Mutex<Device>>,
    /// 采集任务句柄
    capture_task: Option<JoinHandle<()>>,
    /// 帧广播通道
    frame_tx: broadcast::Sender<VideoFrame>,
    /// 采集状态
    state: Arc<RwLock<CaptureState>>,
    /// 统计信息
    stats: Arc<RwLock<CaptureStats>>,
}

impl VideoCapturer {
    /// 创建采集器 (不立即打开设备)
    pub fn new() -> Arc<Self>;

    /// 启动采集
    pub async fn start(&self, config: CaptureConfig) -> Result<()>;

    /// 停止采集
    pub async fn stop(&self) -> Result<()>;

    /// 订阅帧流 (广播模式)
    pub fn subscribe(&self) -> broadcast::Receiver<VideoFrame>;

    /// 获取当前状态
    pub fn state(&self) -> CaptureState;

    /// 获取统计信息
    pub fn stats(&self) -> CaptureStats;
}
```

#### 采集配置

```rust
pub struct CaptureConfig {
    /// 设备路径
    pub device_path: PathBuf,        // /dev/video0
    /// 分辨率
    pub resolution: Resolution,      // 1920x1080
    /// 像素格式
    pub format: PixelFormat,         // MJPEG/YUYV/NV12
    /// 帧率 (0 = 最大)
    pub fps: u32,                    // 30
    /// 缓冲区数量 (默认 2)
    pub buffer_count: u32,
    /// 超时时间
    pub timeout: Duration,
    /// JPEG 质量 (1-100)
    pub jpeg_quality: u8,
}
```

#### 采集状态

```rust
#[derive(Clone, Copy)]
pub enum CaptureState {
    Idle,           // 未初始化
    Starting,       // 正在启动
    Running,        // 正在采集
    Stopping,       // 正在停止
    NoSignal,       // 无信号
    DeviceLost,     // 设备丢失
    Error,          // 错误状态
}
```

#### 使用示例

```rust
// 创建采集器
let capturer = VideoCapturer::new();

// 启动采集
let config = CaptureConfig {
    device_path: PathBuf::from("/dev/video0"),
    resolution: Resolution::HD1080,
    format: PixelFormat::Mjpeg,
    fps: 30,
    buffer_count: 2,
    timeout: Duration::from_secs(2),
    jpeg_quality: 80,
};
capturer.start(config).await?;

// 订阅帧流
let mut frame_rx = capturer.subscribe();
while let Ok(frame) = frame_rx.recv().await {
    // 处理帧
    process_frame(frame).await;
}
```

### 3.2 VideoDevice (device.rs)

V4L2 设备枚举和能力查询工具。

```rust
/// 视频设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    /// 设备路径 (/dev/video0)
    pub path: PathBuf,
    /// 设备名称
    pub name: String,
    /// 驱动名称
    pub driver: String,
    /// 总线信息
    pub bus_info: String,
    /// 卡片名称
    pub card: String,
    /// 支持的像素格式列表
    pub formats: Vec<FormatInfo>,
    /// 设备能力
    pub capabilities: DeviceCapabilities,
    /// 是否为采集卡 (自动识别)
    pub is_capture_card: bool,
    /// 优先级分数 (用于自动选择设备)
    pub priority: u32,
}

/// 枚举所有视频设备
pub fn enumerate_devices() -> Result<Vec<VideoDeviceInfo>>;

/// 自动选择最佳设备 (优先级最高的采集卡)
pub fn find_best_device() -> Result<VideoDeviceInfo>;
```

### 3.3 VideoFrame (frame.rs)

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

### 3.4 PixelFormat (format.rs)

支持的像素格式定义 (与实际代码一致)。

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelFormat {
    // 压缩格式
    Mjpeg,      // Motion JPEG
    Jpeg,       // Static JPEG

    // YUV 4:2:2 打包格式
    Yuyv,       // YUYV/YUY2
    Yvyu,       // YVYU
    Uyvy,       // UYVY

    // YUV 半平面格式
    Nv12,       // NV12 (Y + interleaved UV)
    Nv16,       // NV16
    Nv24,       // NV24

    // YUV 平面格式
    Yuv420,     // I420/YU12
    Yvu420,     // YV12

    // RGB 格式
    Rgb565,     // RGB565
    Rgb24,      // RGB24
    Bgr24,      // BGR24

    // 灰度
    Grey,       // 8-bit grayscale
}

impl PixelFormat {
    /// 转换为 V4L2 FourCC
    pub fn to_fourcc(&self) -> FourCC;

    /// 从 V4L2 FourCC 转换
    pub fn from_fourcc(fourcc: FourCC) -> Option<Self>;

    /// 是否压缩格式
    pub fn is_compressed(&self) -> bool;

    /// 获取每像素字节数 (未压缩格式)
    pub fn bytes_per_pixel(&self) -> Option<usize>;

    /// 计算帧大小
    pub fn frame_size(&self, resolution: Resolution) -> Option<usize>;
}
```

### 3.5 SharedVideoPipeline (shared_video_pipeline.rs)

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

### 3.5 SharedVideoPipeline (shared_video_pipeline.rs)

通用共享视频编码管道，支持 H264/H265/VP8/VP9 多种编码器。

```rust
pub struct SharedVideoPipeline {
    /// 配置
    config: SharedVideoPipelineConfig,
    /// 编码器实例
    encoder: Arc<Mutex<Box<dyn VideoEncoder>>>,
    /// 像素转换器
    converter: Arc<Mutex<Option<Nv12Converter>>>,
    /// MJPEG 解码器
    mjpeg_decoder: Arc<Mutex<Option<Box<dyn MjpegDecoder>>>>,
    /// 编码帧广播通道
    encoded_tx: broadcast::Sender<EncodedVideoFrame>,
    /// 统计信息
    stats: Arc<RwLock<SharedVideoPipelineStats>>,
    /// 运行状态
    running: AtomicBool,
}

impl SharedVideoPipeline {
    /// 创建管道
    pub async fn new(config: SharedVideoPipelineConfig) -> Result<Arc<Self>>;

    /// 启动管道
    pub async fn start(&self, frame_rx: broadcast::Receiver<VideoFrame>) -> Result<()>;

    /// 停止管道
    pub async fn stop(&self) -> Result<()>;

    /// 订阅编码帧
    pub fn subscribe(&self) -> broadcast::Receiver<EncodedVideoFrame>;

    /// 获取统计信息
    pub fn stats(&self) -> SharedVideoPipelineStats;

    /// 编码单帧 (内部方法)
    async fn encode_frame(&self, frame: VideoFrame) -> Result<EncodedVideoFrame>;
}
```

#### 管道配置

```rust
#[derive(Debug, Clone)]
pub struct SharedVideoPipelineConfig {
    /// 输入分辨率
    pub resolution: Resolution,
    /// 输入像素格式
    pub input_format: PixelFormat,
    /// 输出编码器类型
    pub output_codec: VideoEncoderType,
    /// 码率预设 (替代原始 bitrate_kbps)
    pub bitrate_preset: BitratePreset,
    /// 目标帧率
    pub fps: u32,
    /// 编码器后端 (None = 自动选择)
    pub encoder_backend: Option<EncoderBackend>,
}

impl SharedVideoPipelineConfig {
    /// 创建 H264 配置
    pub fn h264(resolution: Resolution, preset: BitratePreset) -> Self;

    /// 创建 H265 配置
    pub fn h265(resolution: Resolution, preset: BitratePreset) -> Self;

    /// 创建 VP8 配置
    pub fn vp8(resolution: Resolution, preset: BitratePreset) -> Self;

    /// 创建 VP9 配置
    pub fn vp9(resolution: Resolution, preset: BitratePreset) -> Self;
}
```

### 3.6 VideoSessionManager (video_session.rs)

管理多个 WebRTC 视频会话，每个会话可使用不同的编解码器。

```rust
pub struct VideoSessionManager {
    /// 会话映射 (session_id -> VideoSession)
    sessions: Arc<RwLock<HashMap<String, VideoSession>>>,
    /// 管道映射 (codec -> SharedVideoPipeline)
    pipelines: Arc<RwLock<HashMap<VideoEncoderType, Arc<SharedVideoPipeline>>>>,
    /// 配置
    config: VideoSessionManagerConfig,
}

impl VideoSessionManager {
    /// 创建会话管理器
    pub fn new(config: VideoSessionManagerConfig) -> Arc<Self>;

    /// 创建新会话
    pub async fn create_session(
        &self,
        session_id: String,
        codec: VideoEncoderType,
    ) -> Result<broadcast::Receiver<EncodedVideoFrame>>;

    /// 关闭会话
    pub async fn close_session(&self, session_id: &str) -> Result<()>;

    /// 获取会话信息
    pub fn get_session_info(&self, session_id: &str) -> Option<VideoSessionInfo>;

    /// 列出所有会话
    pub fn list_sessions(&self) -> Vec<VideoSessionInfo>;

    /// 获取或创建编码管道
    async fn get_or_create_pipeline(
        &self,
        codec: VideoEncoderType,
    ) -> Result<Arc<SharedVideoPipeline>>;
}
```

### 3.7 Streamer (streamer.rs)

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

### 3.7 Streamer (streamer.rs)

高层 MJPEG 视频流服务，集成采集、设备管理和状态监控。

```rust
pub struct Streamer {
    /// 配置
    config: RwLock<StreamerConfig>,
    /// 视频采集器
    capturer: RwLock<Option<Arc<VideoCapturer>>>,
    /// MJPEG 流处理器
    mjpeg_handler: Arc<MjpegStreamHandler>,
    /// 当前设备信息
    current_device: RwLock<Option<VideoDeviceInfo>>,
    /// 流状态
    state: RwLock<StreamerState>,
    /// 事件总线 (可选)
    events: RwLock<Option<Arc<EventBus>>>,
}

impl Streamer {
    /// 创建流服务
    pub fn new() -> Arc<Self>;

    /// 启动流
    pub async fn start(&self, config: StreamerConfig) -> Result<()>;

    /// 停止流
    pub async fn stop(&self) -> Result<()>;

    /// 设置事件总线
    pub async fn set_event_bus(&self, events: Arc<EventBus>);

    /// 获取状态
    pub fn state(&self) -> StreamerState;

    /// 获取 MJPEG 处理器
    pub fn mjpeg_handler(&self) -> Arc<MjpegStreamHandler>;

    /// 应用配置 (热更新)
    pub async fn apply_config(&self, config: StreamerConfig) -> Result<()>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StreamerState {
    Uninitialized,  // 未初始化
    Ready,          // 就绪但未流式传输
    Streaming,      // 正在流式传输
    NoSignal,       // 无视频信号
    Error,          // 错误
    DeviceLost,     // 设备丢失
    Recovering,     // 设备恢复中
}
```

### 3.8 VideoStreamManager (stream_manager.rs)

统一视频流管理器，作为唯一入口协调 MJPEG 和 WebRTC 两种流模式。

```rust
pub struct VideoStreamManager {
    /// 当前流模式
    mode: RwLock<StreamMode>,
    /// MJPEG 流服务
    streamer: Arc<Streamer>,
    /// WebRTC 流服务
    webrtc_streamer: Arc<WebRtcStreamer>,
    /// 事件总线
    events: RwLock<Option<Arc<EventBus>>>,
    /// 配置存储
    config_store: RwLock<Option<ConfigStore>>,
    /// 模式切换锁
    switching: AtomicBool,
}

impl VideoStreamManager {
    /// 创建管理器 (指定 WebRtcStreamer)
    pub fn with_webrtc_streamer(
        streamer: Arc<Streamer>,
        webrtc_streamer: Arc<WebRtcStreamer>,
    ) -> Arc<Self>;

    /// 启动流 (启动当前模式)
    pub async fn start(&self) -> Result<()>;

    /// 停止流
    pub async fn stop(&self) -> Result<()>;

    /// 切换流模式 (MJPEG ↔ WebRTC)
    pub async fn set_mode(&self, mode: StreamMode) -> Result<()>;

    /// 获取当前模式
    pub fn mode(&self) -> StreamMode;

    /// 获取 Streamer (MJPEG)
    pub fn streamer(&self) -> Arc<Streamer>;

    /// 获取 WebRtcStreamer
    pub fn webrtc_streamer(&self) -> Arc<WebRtcStreamer>;

    /// 设置事件总线
    pub async fn set_event_bus(&self, events: Arc<EventBus>);

    /// 设置配置存储
    pub async fn set_config_store(&self, config_store: ConfigStore);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    Mjpeg,   // MJPEG over HTTP
    Webrtc,  // H264/H265/VP8/VP9 over WebRTC
}
```

---

## 4. 编码器系统

### 4.1 BitratePreset (encoder/traits.rs)

码率预设简化配置，提供三个常用档位和自定义选项。

```rust
#[typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitratePreset {
    /// 速度优先: 1 Mbps, 最低延迟, 更小的 GOP
    /// 适用于: 慢速网络, 远程管理, 低带宽场景
    Speed,

    /// 平衡: 4 Mbps, 质量/延迟均衡 (推荐默认)
    /// 适用于: 常规使用
    Balanced,

    /// 质量优先: 8 Mbps, 最佳视觉质量
    /// 适用于: 本地网络, 高带宽场景, 详细工作
    Quality,

    /// 自定义码率 (kbps, 高级用户)
    Custom(u32),
}

impl BitratePreset {
    /// 获取码率值 (kbps)
    pub fn bitrate_kbps(&self) -> u32;

    /// 获取推荐 GOP 大小 (基于帧率)
    pub fn gop_size(&self, fps: u32) -> u32;

    /// 获取质量级别 ("low" | "medium" | "high")
    pub fn quality_level(&self) -> &'static str;

    /// 从 kbps 值创建 (自动映射到最近预设或 Custom)
    pub fn from_kbps(kbps: u32) -> Self;
}
```

### 4.2 VideoEncoder Trait (encoder/traits.rs)

所有编码器的通用接口 (hwcodec 编码器的封装)。

```rust
pub trait VideoEncoder: Send + Sync {
    /// 编码一帧 (输入 NV12, 输出压缩数据)
    fn encode(&mut self, yuv: &[u8], ms: i64) -> Result<EncodedVideoFrame>;

    /// 获取编码器类型
    fn encoder_type(&self) -> VideoEncoderType;

    /// 设置码率 (kbps)
    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()>;

    /// 请求关键帧
    fn request_keyframe(&mut self);

    /// 获取编码器信息
    fn info(&self) -> EncoderInfo;
}

/// 编码后的视频帧
#[derive(Debug, Clone)]
pub struct EncodedVideoFrame {
    /// 编码数据 (Bytes 引用计数，零拷贝)
    pub data: Bytes,
    /// 呈现时间戳 (毫秒)
    pub pts_ms: i64,
    /// 是否关键帧
    pub is_keyframe: bool,
    /// 帧序号
    pub sequence: u64,
    /// 帧时长
    pub duration: Duration,
    /// 编码类型
    pub codec: VideoEncoderType,
}
```

### 4.3 VideoEncoderType & EncoderBackend (encoder/registry.rs)

编码器类型和硬件后端定义。

```rust
/// 视频编码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoEncoderType {
    H264,   // H.264/AVC
    H265,   // H.265/HEVC
    VP8,    // VP8
    VP9,    // VP9
}

impl VideoEncoderType {
    /// 是否仅支持硬件编码 (无软件回退)
    pub fn hardware_only(&self) -> bool {
        match self {
            VideoEncoderType::H264 => false,  // x264 软件回退
            VideoEncoderType::H265 => true,   // 仅硬件
            VideoEncoderType::VP8 => true,    // 仅硬件
            VideoEncoderType::VP9 => true,    // 仅硬件
        }
    }
}

/// 编码器硬件后端
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncoderBackend {
    Vaapi,      // Intel/AMD VAAPI (Linux)
    Nvenc,      // NVIDIA NVENC
    Qsv,        // Intel Quick Sync
    Amf,        // AMD AMF
    Rkmpp,      // Rockchip MPP
    V4l2M2m,    // V4L2 Memory-to-Memory
    Software,   // x264/x265/libvpx (软件)
}
```

### 4.4 EncoderRegistry (encoder/registry.rs)

全局编码器注册表，自动检测硬件并选择最佳编码器。

```rust
/// 编码器注册表 (全局单例)
pub struct EncoderRegistry {
    /// 可用编码器映射
    available_encoders: HashMap<VideoEncoderType, Vec<EncoderBackend>>,
}

impl EncoderRegistry {
    /// 获取全局实例
    pub fn global() -> &'static EncoderRegistry;

    /// 列出可用编码器
    pub fn list_available(&self, codec: VideoEncoderType) -> &[EncoderBackend];

    /// 检查编码器是否可用
    pub fn is_available(&self, codec: VideoEncoderType, backend: EncoderBackend) -> bool;

    /// 获取最佳编码器后端 (自动选择)
    pub fn get_best_backend(&self, codec: VideoEncoderType) -> Option<EncoderBackend>;

    /// 创建编码器实例
    pub fn create_encoder(
        &self,
        codec: VideoEncoderType,
        config: EncoderConfig,
        backend: Option<EncoderBackend>,
    ) -> Result<Box<dyn VideoEncoder>>;
}
```

### 4.5 编码器优先级

实际的硬件检测顺序 (基于 hwcodec 库)：

```
H264 编码器选择顺序:
1. VAAPI (Intel/AMD GPU - 优先)
2. Rkmpp (Rockchip 平台)
3. V4L2 M2M (通用 Linux)
4. x264 (软件回退)

H265 编码器选择顺序:
1. VAAPI
2. Rkmpp
(无软件回退)

VP8/VP9 编码器:
1. VAAPI only
(无软件回退)
```

---

## 5. 格式转换与解码

### 5.1 MJPEG 解码器 (decoder/mjpeg.rs)

支持硬件和软件两种 MJPEG 解码方式。

```rust
/// MJPEG 解码器 trait
pub trait MjpegDecoder: Send + Sync {
    /// 解码 MJPEG 到 YUV420P
    fn decode(&mut self, jpeg_data: &[u8]) -> Result<DecodedYuv420pFrame>;

    /// 获取解码器类型
    fn decoder_type(&self) -> &str;
}

/// MJPEG TurboJPEG 软件解码器
pub struct MjpegTurboDecoder {
    decompressor: Decompressor,
    output_buffer: Vec<u8>,
}

/// MJPEG VAAPI 硬件解码器 (输出 NV12)
pub struct MjpegVaapiDecoder {
    decoder: VaapiDecoder,
    config: MjpegVaapiDecoderConfig,
}

impl MjpegVaapiDecoder {
    /// 创建 VAAPI 解码器
    pub fn new(config: MjpegVaapiDecoderConfig) -> Result<Self>;

    /// 解码 MJPEG 到 NV12 (硬件加速)
    pub fn decode_to_nv12(&mut self, jpeg_data: &[u8]) -> Result<Vec<u8>>;
}
```

### 5.2 像素转换器 (convert.rs)

使用 libyuv SIMD 加速的格式转换。

```rust
/// NV12 转换器 (YUYV/RGB → NV12)
pub struct Nv12Converter {
    input_format: PixelFormat,
    resolution: Resolution,
    nv12_buffer: Nv12Buffer,
}

impl Nv12Converter {
    /// 创建转换器
    pub fn new(input_format: PixelFormat, resolution: Resolution) -> Self;

    /// 转换到 NV12
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8]>;
}

/// YUV420P 缓冲区
pub struct Yuv420pBuffer {
    data: Vec<u8>,
    width: u32,
    height: u32,
    y_offset: usize,
    u_offset: usize,
    v_offset: usize,
}

impl Yuv420pBuffer {
    /// 获取 Y 平面
    pub fn y_plane(&self) -> &[u8];

    /// 获取 U 平面
    pub fn u_plane(&self) -> &[u8];

    /// 获取 V 平面
    pub fn v_plane(&self) -> &[u8];
}

/// 像素转换器 (通用接口)
pub trait PixelConverter: Send + Sync {
    /// YUYV → YUV420P
    fn yuyv_to_yuv420p(src: &[u8], width: u32, height: u32) -> Yuv420pBuffer;

    /// NV12 → YUV420P
    fn nv12_to_yuv420p(src: &[u8], width: u32, height: u32) -> Yuv420pBuffer;

    /// RGB24 → YUV420P
    fn rgb24_to_yuv420p(src: &[u8], width: u32, height: u32) -> Yuv420pBuffer;
}
```

---

## 6. 配置说明

### 6.1 视频配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct VideoConfig {
    /// 设备路径 (None = 自动检测)
    pub device: Option<String>,

    /// 像素格式 (None = 自动选择: MJPEG > YUYV > NV12)
    pub format: Option<String>,

    /// 宽度
    pub width: u32,

    /// 高度
    pub height: u32,

    /// 帧率
    pub fps: u32,

    /// JPEG 质量 (1-100, 仅 MJPEG)
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

### 6.2 WebRTC 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct WebRtcConfig {
    /// 码率预设
    pub bitrate_preset: BitratePreset,

    /// 首选编码器 (H264/H265/VP8/VP9)
    pub preferred_codec: String,

    /// STUN 服务器
    pub stun_server: Option<String>,

    /// TURN 服务器
    pub turn_server: Option<String>,

    /// TURN 用户名
    pub turn_username: Option<String>,

    /// TURN 密码
    pub turn_password: Option<String>,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            bitrate_preset: BitratePreset::Balanced,
            preferred_codec: "H264".to_string(),
            stun_server: Some("stun:stun.l.google.com:19302".to_string()),
            turn_server: None,
            turn_username: None,
            turn_password: None,
        }
    }
}
```

---

## 7. API 端点

### 7.1 视频流控制 (用户权限)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/stream/status` | GET | 获取流状态 |
| `/stream/start` | POST | 启动流 |
| `/stream/stop` | POST | 停止流 |
| `/stream/mode` | GET | 获取流模式 (MJPEG/WebRTC) |
| `/stream/mode` | POST | 设置流模式 |
| `/stream/bitrate` | POST | 设置码率 (WebRTC) |
| `/stream/codecs` | GET | 列出可用编码器 |

### 7.2 WebRTC 端点 (用户权限)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/webrtc/session` | POST | 创建 WebRTC 会话 |
| `/webrtc/offer` | POST | 发送 SDP offer |
| `/webrtc/ice` | POST | 发送 ICE candidate |
| `/webrtc/ice-servers` | GET | 获取 STUN/TURN 配置 |
| `/webrtc/status` | GET | 获取 WebRTC 状态 |
| `/webrtc/close` | POST | 关闭会话 |

### 7.3 设备管理 (用户权限)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/devices` | GET | 列出所有视频设备 |

### 7.4 配置管理 (管理员权限)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/config/video` | GET | 获取视频配置 |
| `/config/video` | PATCH | 更新视频配置 |
| `/config/stream` | GET | 获取流配置 |
| `/config/stream` | PATCH | 更新流配置 |

### 7.5 响应格式

```json
// GET /stream/status
{
    "state": "streaming",
    "device": "/dev/video0",
    "resolution": { "width": 1920, "height": 1080 },
    "format": "MJPEG",
    "fps": 30.0,
    "mode": "mjpeg"
}

// GET /devices
{
    "devices": [
        {
            "path": "/dev/video0",
            "name": "USB Capture HDMI",
            "driver": "uvcvideo",
            "bus_info": "usb-0000:00:14.0-1",
            "formats": ["MJPEG", "YUYV"],
            "is_capture_card": true,
            "priority": 100
        }
    ]
}

// GET /stream/codecs
{
    "codecs": [
        {
            "codec": "H264",
            "backends": ["VAAPI", "x264"]
        },
        {
            "codec": "H265",
            "backends": ["VAAPI"]
        }
    ]
}
```

---

## 8. 事件系统

视频模块通过 EventBus 发布的实时事件 (通过 WebSocket `/ws` 推送到前端)：

```rust
pub enum SystemEvent {
    /// 流状态变化
    StreamStateChanged {
        state: String,      // "uninitialized" | "ready" | "streaming" | "no_signal" | "error" | "device_lost" | "recovering"
        device: Option<String>,
        resolution: Option<Resolution>,
        fps: Option<f32>,
        mode: String,       // "mjpeg" | "webrtc"
    },

    /// 视频设备插拔事件
    VideoDeviceChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },

    /// WebRTC 会话状态变化
    WebRtcSessionChanged {
        session_id: String,
        state: String,      // "created" | "active" | "paused" | "closing" | "closed"
        codec: String,
    },

    /// 编码器变化 (硬件/软件切换)
    EncoderChanged {
        codec: String,
        backend: String,    // "VAAPI" | "RKMPP" | "x264" | ...
        hardware: bool,
    },
}
```

前端订阅示例：

```typescript
const ws = new WebSocket('ws://localhost:8080/ws');
ws.onmessage = (event) => {
    const systemEvent = JSON.parse(event.data);
    if (systemEvent.type === 'StreamStateChanged') {
        console.log('Stream state:', systemEvent.state);
    }
};
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

### 10.1 零拷贝架构

- `Arc<Bytes>` 共享帧数据，避免内存拷贝
- 引用计数多播，单次采集多个消费者
- `broadcast::Sender` 高效分发帧到多个订阅者

### 10.2 帧去重 (Frame Deduplication)

- xxHash64 快速哈希计算 (懒加载)
- 相同帧跳过编码，降低 CPU 使用
- 适用于静态画面场景

### 10.3 硬件加速优先

编码器自动选择优先级：
1. **VAAPI** (Intel/AMD GPU) - 最优先
2. **Rkmpp** (Rockchip 平台)
3. **V4L2 M2M** (通用 Linux)
4. **Software** (x264) - 仅 H264 有软件回退

解码器优先级：
1. **VAAPI** (硬件 MJPEG 解码 → NV12)
2. **TurboJPEG** (软件 MJPEG 解码 → YUV420P)

### 10.4 SIMD 加速

- libyuv 库提供 NEON/SSE 优化的像素转换
- 自动检测 CPU 指令集并使用最快路径
- YUYV → NV12 转换性能提升 3-4 倍

### 10.5 低延迟优化

- 缓冲区数量减少至 2 (降低采集延迟)
- WebRTC 模式下直接 RTP 封装，无额外缓冲
- GOP 大小可调 (Speed 预设: 0.5s, Balanced: 1s, Quality: 2s)

---

## 11. 常见问题

### Q: 如何添加新的视频格式支持?

1. 在 `format.rs` 添加 `PixelFormat` 枚举值
2. 实现 `to_fourcc()` 和 `from_fourcc()` 映射
3. 在 `convert.rs` 添加转换函数 (如果需要转为 NV12/YUV420P)
4. 更新 `Nv12Converter` 或 `PixelConverter`

### Q: 如何添加新的编码器后端?

1. 在 `encoder/registry.rs` 添加 `EncoderBackend` 枚举值
2. 在对应编码器 (如 `h264.rs`) 中实现新后端
3. 更新 `EncoderRegistry::create_encoder()` 的后端选择逻辑
4. 添加硬件探测代码

### Q: 帧率不稳定或丢帧怎么办?

**诊断步骤：**
1. 检查 `/stream/status` API，查看实际 FPS
2. 检查 USB 带宽是否充足 (使用 `lsusb -t`)
3. 检查 CPU 使用率，确认编码器负载

**解决方案：**
- **降低分辨率**: 1080p → 720p
- **使用 MJPEG 格式**: 减少主机侧解码负担
- **启用硬件编码**: 检查 `/stream/codecs` 确认有 VAAPI/Rkmpp
- **降低码率预设**: Quality → Balanced → Speed
- **关闭帧去重**: 如果画面高度动态

### Q: WebRTC 无法连接?

1. 检查 STUN/TURN 服务器配置 (`/webrtc/ice-servers`)
2. 确认防火墙允许 UDP 流量
3. 检查浏览器控制台 ICE 连接状态
4. 尝试使用公共 STUN 服务器: `stun:stun.l.google.com:19302`

### Q: 如何在 MJPEG 和 WebRTC 模式之间切换?

```bash
# 切换到 MJPEG 模式 (高兼容性)
curl -X POST http://localhost:8080/stream/mode \
  -H "Content-Type: application/json" \
  -d '{"mode": "mjpeg"}'

# 切换到 WebRTC 模式 (低延迟)
curl -X POST http://localhost:8080/stream/mode \
  -H "Content-Type: application/json" \
  -d '{"mode": "webrtc"}'
```

### Q: 支持同时多个 WebRTC 连接吗?

是的，`VideoSessionManager` 支持最多 8 个并发 WebRTC 会话。每个会话共享同一个视频采集源，但可以使用不同的编码器 (H264/H265/VP8/VP9)。

### Q: 如何查看当前使用的编码器后端?

监听 `EncoderChanged` 事件 (通过 WebSocket)，或查看日志中的编码器初始化信息。

---

## 12. 架构亮点

### 12.1 单一入口设计

`VideoStreamManager` 是所有视频操作的唯一入口，封装了 MJPEG 和 WebRTC 两种模式的复杂性。

### 12.2 模式隔离

MJPEG 和 WebRTC 模式完全分离，避免相互干扰。切换模式时会完全停止旧模式再启动新模式。

### 12.3 硬件自适应

通过 `EncoderRegistry` 自动检测硬件能力，优先使用硬件加速，无需手动配置。

### 12.4 多编解码器支持

WebRTC 模式支持 H264/H265/VP8/VP9 四种编码器，可根据客户端能力协商最佳编码器。

### 12.5 零配置设备发现

自动扫描 `/dev/video*`，识别 HDMI 采集卡并计算优先级，优先选择最佳设备。
