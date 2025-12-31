# Audio 模块文档

## 1. 模块概述

Audio 模块负责音频采集和编码，支持 ALSA 采集和 Opus 编码。

### 1.1 主要功能

- ALSA 音频采集
- Opus 编码
- 多质量配置
- WebSocket/WebRTC 传输

### 1.2 文件结构

```
src/audio/
├── mod.rs              # 模块导出
├── controller.rs       # AudioController (15KB)
├── capture.rs          # ALSA 采集 (12KB)
├── encoder.rs          # Opus 编码 (8KB)
├── shared_pipeline.rs  # 共享管道 (15KB)
├── monitor.rs          # 健康监视 (11KB)
└── device.rs           # 设备枚举 (8KB)
```

---

## 2. 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Audio Architecture                                   │
└─────────────────────────────────────────────────────────────────────────────┘

ALSA Device (hw:0,0)
        │
        │ PCM 48kHz/16bit/Stereo
        ▼
┌─────────────────┐
│ AudioCapturer   │
│  (capture.rs)   │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│         SharedAudioPipeline             │
│  ┌─────────────────────────────────┐   │
│  │      Opus Encoder               │   │
│  │   48kHz → 24-96 kbps            │   │
│  └─────────────────────────────────┘   │
└────────────────┬────────────────────────┘
                 │
       ┌─────────┴─────────┐
       │                   │
       ▼                   ▼
┌─────────────┐    ┌─────────────┐
│  WebSocket  │    │   WebRTC    │
│   Stream    │    │ Audio Track │
└─────────────┘    └─────────────┘
```

---

## 3. 核心组件

### 3.1 AudioController (controller.rs)

```rust
pub struct AudioController {
    /// 采集器
    capturer: Arc<RwLock<Option<AudioCapturer>>>,

    /// 共享管道
    pipeline: Arc<SharedAudioPipeline>,

    /// 配置
    config: Arc<RwLock<AudioConfig>>,

    /// 状态
    state: Arc<RwLock<AudioState>>,

    /// 事件总线
    events: Arc<EventBus>,
}

impl AudioController {
    /// 创建控制器
    pub fn new(config: &AudioConfig, events: Arc<EventBus>) -> Result<Self>;

    /// 启动音频
    pub async fn start(&self) -> Result<()>;

    /// 停止音频
    pub async fn stop(&self) -> Result<()>;

    /// 订阅音频帧
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame>;

    /// 获取状态
    pub fn status(&self) -> AudioStatus;

    /// 设置质量
    pub fn set_quality(&self, quality: AudioQuality) -> Result<()>;

    /// 列出设备
    pub fn list_devices(&self) -> Vec<AudioDeviceInfo>;

    /// 重新加载配置
    pub async fn reload(&self, config: &AudioConfig) -> Result<()>;
}

pub struct AudioStatus {
    pub enabled: bool,
    pub streaming: bool,
    pub device: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
    pub error: Option<String>,
}
```

### 3.2 AudioCapturer (capture.rs)

```rust
pub struct AudioCapturer {
    /// PCM 句柄
    pcm: PCM,

    /// 设备名
    device: String,

    /// 采样率
    sample_rate: u32,

    /// 通道数
    channels: u16,

    /// 帧大小
    frame_size: usize,

    /// 运行状态
    running: AtomicBool,
}

impl AudioCapturer {
    /// 打开设备
    pub fn open(device: &str, config: &CaptureConfig) -> Result<Self>;

    /// 读取音频帧
    pub fn read_frame(&self) -> Result<Vec<i16>>;

    /// 启动采集
    pub fn start(&self) -> Result<()>;

    /// 停止采集
    pub fn stop(&self);

    /// 是否运行中
    pub fn is_running(&self) -> bool;
}

pub struct CaptureConfig {
    pub sample_rate: u32,   // 48000
    pub channels: u16,      // 2
    pub frame_size: usize,  // 960 (20ms)
    pub buffer_size: usize, // 4800
}
```

### 3.3 OpusEncoder (encoder.rs)

```rust
pub struct OpusEncoder {
    /// Opus 编码器
    encoder: audiopus::Encoder,

    /// 采样率
    sample_rate: u32,

    /// 通道数
    channels: u16,

    /// 帧大小
    frame_size: usize,

    /// 码率
    bitrate: u32,
}

impl OpusEncoder {
    /// 创建编码器
    pub fn new(quality: AudioQuality) -> Result<Self>;

    /// 编码 PCM 数据
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>>;

    /// 设置码率
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()>;

    /// 获取码率
    pub fn bitrate(&self) -> u32;

    /// 重置编码器
    pub fn reset(&mut self) -> Result<()>;
}
```

### 3.4 SharedAudioPipeline (shared_pipeline.rs)

```rust
pub struct SharedAudioPipeline {
    /// 采集器
    capturer: Arc<RwLock<Option<AudioCapturer>>>,

    /// 编码器
    encoder: Arc<Mutex<OpusEncoder>>,

    /// 广播通道
    tx: broadcast::Sender<AudioFrame>,

    /// 采集任务
    capture_task: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// 配置
    config: Arc<RwLock<AudioConfig>>,
}

impl SharedAudioPipeline {
    /// 创建管道
    pub fn new(config: &AudioConfig) -> Result<Self>;

    /// 启动管道
    pub async fn start(&self) -> Result<()>;

    /// 停止管道
    pub async fn stop(&self) -> Result<()>;

    /// 订阅音频帧
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame>;

    /// 获取统计
    pub fn stats(&self) -> PipelineStats;
}

pub struct AudioFrame {
    /// Opus 数据
    pub data: Bytes,

    /// 时间戳
    pub timestamp: u64,

    /// 帧序号
    pub sequence: u64,
}
```

---

## 4. 音频质量

```rust
pub enum AudioQuality {
    /// 24 kbps - 最低带宽
    VeryLow,

    /// 48 kbps - 低带宽
    Low,

    /// 64 kbps - 中等
    Medium,

    /// 96 kbps - 高质量
    High,
}

impl AudioQuality {
    pub fn bitrate(&self) -> u32 {
        match self {
            Self::VeryLow => 24000,
            Self::Low => 48000,
            Self::Medium => 64000,
            Self::High => 96000,
        }
    }
}
```

---

## 5. 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct AudioConfig {
    /// 是否启用
    pub enabled: bool,

    /// 设备名
    pub device: Option<String>,

    /// 音频质量
    pub quality: AudioQuality,

    /// 自动启动
    pub auto_start: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            device: None,  // 使用默认设备
            quality: AudioQuality::Medium,
            auto_start: false,
        }
    }
}
```

---

## 6. API 端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/audio/status` | GET | 获取音频状态 |
| `/api/audio/start` | POST | 启动音频 |
| `/api/audio/stop` | POST | 停止音频 |
| `/api/audio/devices` | GET | 列出设备 |
| `/api/audio/quality` | GET | 获取质量 |
| `/api/audio/quality` | POST | 设置质量 |
| `/api/ws/audio` | WS | 音频流 |

### 响应格式

```json
// GET /api/audio/status
{
    "enabled": true,
    "streaming": true,
    "device": "hw:0,0",
    "sample_rate": 48000,
    "channels": 2,
    "bitrate": 64000,
    "error": null
}

// GET /api/audio/devices
{
    "devices": [
        {
            "name": "hw:0,0",
            "description": "USB Audio Device",
            "is_default": true
        }
    ]
}
```

---

## 7. WebSocket 音频流

```javascript
// 连接 WebSocket
const ws = new WebSocket('/api/ws/audio');
ws.binaryType = 'arraybuffer';

// 初始化 Opus 解码器
const decoder = new OpusDecoder();

// 接收音频帧
ws.onmessage = (event) => {
    const frame = new Uint8Array(event.data);
    const pcm = decoder.decode(frame);
    audioContext.play(pcm);
};
```

---

## 8. 事件

```rust
pub enum SystemEvent {
    AudioStateChanged {
        enabled: bool,
        streaming: bool,
        device: Option<String>,
        error: Option<String>,
    },
}
```

---

## 9. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device busy: {0}")]
    DeviceBusy(String),

    #[error("ALSA error: {0}")]
    AlsaError(String),

    #[error("Encoder error: {0}")]
    EncoderError(String),

    #[error("Not streaming")]
    NotStreaming,
}
```

---

## 10. 使用示例

```rust
let controller = AudioController::new(&config, events)?;

// 启动音频
controller.start().await?;

// 订阅音频帧
let mut rx = controller.subscribe();
while let Ok(frame) = rx.recv().await {
    // 处理 Opus 数据
    send_to_client(frame.data);
}

// 停止
controller.stop().await?;
```

---

## 11. 常见问题

### Q: 找不到音频设备?

1. 检查 ALSA 配置
2. 运行 `arecord -l`
3. 检查权限

### Q: 音频延迟高?

1. 减小帧大小
2. 降低质量
3. 检查网络

### Q: 音频断断续续?

1. 增大缓冲区
2. 检查 CPU 负载
3. 使用更低质量
