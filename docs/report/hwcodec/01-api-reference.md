# hwcodec 编解码器 API 详解

## 1. 编码器 API

### 1.1 编码器初始化

#### EncodeContext 参数

```rust
pub struct EncodeContext {
    pub name: String,          // 编码器名称
    pub mc_name: Option<String>, // MediaCodec 名称 (保留字段)
    pub width: i32,            // 视频宽度 (必须为偶数)
    pub height: i32,           // 视频高度 (必须为偶数)
    pub pixfmt: AVPixelFormat, // 像素格式
    pub align: i32,            // 内存对齐 (通常为 0 或 32)
    pub fps: i32,              // 帧率
    pub gop: i32,              // GOP 大小 (关键帧间隔)
    pub rc: RateControl,       // 码率控制模式
    pub quality: Quality,      // 编码质量
    pub kbs: i32,              // 目标码率 (kbps)
    pub q: i32,                // 量化参数 (CQ 模式)
    pub thread_count: i32,     // 编码线程数
}
```

#### 参数说明

| 参数 | 类型 | 说明 | 推荐值 |
|------|------|------|--------|
| `name` | String | FFmpeg 编码器名称 | 见下表 |
| `width` | i32 | 视频宽度 | 1920 |
| `height` | i32 | 视频高度 | 1080 |
| `pixfmt` | AVPixelFormat | 像素格式 | NV12 / YUV420P |
| `align` | i32 | 内存对齐 | 0 (自动) |
| `fps` | i32 | 帧率 | 30 |
| `gop` | i32 | GOP 大小 | 30 (1秒) |
| `rc` | RateControl | 码率控制 | CBR / VBR |
| `quality` | Quality | 质量级别 | Medium |
| `kbs` | i32 | 码率 (kbps) | 2000-8000 |
| `thread_count` | i32 | 线程数 | 4 |

#### 编码器名称对照表

| 名称 | 格式 | 加速 | 平台 |
|------|------|------|------|
| `h264_nvenc` | H.264 | NVIDIA GPU | Windows/Linux |
| `hevc_nvenc` | H.265 | NVIDIA GPU | Windows/Linux |
| `h264_amf` | H.264 | AMD GPU | Windows/Linux |
| `hevc_amf` | H.265 | AMD GPU | Windows/Linux |
| `h264_qsv` | H.264 | Intel QSV | Windows |
| `hevc_qsv` | H.265 | Intel QSV | Windows |
| `h264_vaapi` | H.264 | VAAPI | Linux |
| `hevc_vaapi` | H.265 | VAAPI | Linux |
| `vp8_vaapi` | VP8 | VAAPI | Linux |
| `vp9_vaapi` | VP9 | VAAPI | Linux |
| `h264_rkmpp` | H.264 | Rockchip MPP | Linux |
| `hevc_rkmpp` | H.265 | Rockchip MPP | Linux |
| `h264_v4l2m2m` | H.264 | V4L2 M2M | Linux |
| `hevc_v4l2m2m` | H.265 | V4L2 M2M | Linux |
| `h264` | H.264 | 软件 (x264) | 全平台 |
| `hevc` | H.265 | 软件 (x265) | 全平台 |
| `libvpx` | VP8 | 软件 | 全平台 |
| `libvpx-vp9` | VP9 | 软件 | 全平台 |
| `mjpeg` | MJPEG | 软件 | 全平台 |

### 1.2 创建编码器

```rust
use hwcodec::ffmpeg_ram::encode::{Encoder, EncodeContext};
use hwcodec::ffmpeg::{AVPixelFormat};
use hwcodec::common::{RateControl, Quality};

let ctx = EncodeContext {
    name: "h264_vaapi".to_string(),
    mc_name: None,
    width: 1920,
    height: 1080,
    pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
    align: 0,
    fps: 30,
    gop: 30,
    rc: RateControl::RC_CBR,
    quality: Quality::Quality_Medium,
    kbs: 4000,
    q: 0,
    thread_count: 4,
};

let encoder = Encoder::new(ctx)?;
println!("Linesize: {:?}", encoder.linesize);
println!("Offset: {:?}", encoder.offset);
println!("Buffer length: {}", encoder.length);
```

### 1.3 编码帧

```rust
// 准备 YUV 数据
let yuv_data: Vec<u8> = prepare_yuv_frame();

// 编码
let pts_ms: i64 = 0; // 时间戳 (毫秒)
match encoder.encode(&yuv_data, pts_ms) {
    Ok(frames) => {
        for frame in frames.iter() {
            println!("Encoded: {} bytes, pts={}, key={}",
                     frame.data.len(), frame.pts, frame.key);
            // 发送 frame.data
        }
    }
    Err(code) => {
        eprintln!("Encode error: {}", code);
    }
}
```

### 1.4 动态调整码率

```rust
// 动态调整到 6000 kbps
encoder.set_bitrate(6000)?;
```

### 1.5 请求关键帧

```rust
// 下一帧强制编码为 IDR 帧
encoder.request_keyframe();
```

### 1.6 检测可用编码器

```rust
use hwcodec::ffmpeg_ram::encode::{Encoder, EncodeContext};
use hwcodec::ffmpeg_ram::CodecInfo;

let ctx = EncodeContext {
    name: String::new(),
    mc_name: None,
    width: 1920,
    height: 1080,
    pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
    align: 0,
    fps: 30,
    gop: 30,
    rc: RateControl::RC_DEFAULT,
    quality: Quality::Quality_Default,
    kbs: 4000,
    q: 0,
    thread_count: 4,
};

let available_encoders = Encoder::available_encoders(ctx, None);
for encoder in available_encoders {
    println!("Available: {} (format: {:?}, priority: {})",
             encoder.name, encoder.format, encoder.priority);
}
```

## 2. 解码器 API

### 2.1 IP-KVM 专用设计

在 One-KVM IP-KVM 场景中，解码器仅支持 MJPEG 软件解码。这是因为视频采集卡输出的格式是 MJPEG，不需要其他格式的硬件解码支持。

### 2.2 解码器初始化

#### DecodeContext 参数

```rust
pub struct DecodeContext {
    pub name: String,           // 解码器名称 ("mjpeg")
    pub device_type: AVHWDeviceType, // 硬件设备类型 (NONE)
    pub thread_count: i32,      // 解码线程数
}
```

### 2.3 创建解码器

```rust
use hwcodec::ffmpeg_ram::decode::{Decoder, DecodeContext};
use hwcodec::ffmpeg::AVHWDeviceType;

let ctx = DecodeContext {
    name: "mjpeg".to_string(),
    device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
    thread_count: 4,
};

let decoder = Decoder::new(ctx)?;
```

### 2.4 解码帧

```rust
// 输入 MJPEG 编码数据
let mjpeg_data: Vec<u8> = receive_mjpeg_frame();

match decoder.decode(&mjpeg_data) {
    Ok(frames) => {
        for frame in frames.iter() {
            println!("Decoded: {}x{}, format={:?}, key={}",
                     frame.width, frame.height, frame.pixfmt, frame.key);

            // 访问 YUV 数据
            let y_plane = &frame.data[0];
            let u_plane = &frame.data[1];
            let v_plane = &frame.data[2];
        }
    }
    Err(code) => {
        eprintln!("Decode error: {}", code);
    }
}
```

### 2.5 DecodeFrame 结构体

```rust
pub struct DecodeFrame {
    pub pixfmt: AVPixelFormat,  // 输出像素格式
    pub width: i32,             // 帧宽度
    pub height: i32,            // 帧高度
    pub data: Vec<Vec<u8>>,     // 平面数据 [Y, U, V] 或 [Y, UV]
    pub linesize: Vec<i32>,     // 每个平面的行字节数
    pub key: bool,              // 是否为关键帧
}
```

#### 像素格式与平面布局

| 像素格式 | 平面数 | data[0] | data[1] | data[2] |
|----------|--------|---------|---------|---------|
| `YUV420P` | 3 | Y | U | V |
| `YUVJ420P` | 3 | Y | U | V |
| `YUV422P` | 3 | Y | U | V |
| `NV12` | 2 | Y | UV (交错) | - |
| `NV21` | 2 | Y | VU (交错) | - |

### 2.6 获取可用解码器

```rust
use hwcodec::ffmpeg_ram::decode::Decoder;

let available_decoders = Decoder::available_decoders();
for decoder in available_decoders {
    println!("Available: {} (format: {:?}, hwdevice: {:?})",
             decoder.name, decoder.format, decoder.hwdevice);
}

// 输出:
// Available: mjpeg (format: MJPEG, hwdevice: AV_HWDEVICE_TYPE_NONE)
```

## 3. 码率控制模式

### 3.1 RateControl 枚举

```rust
pub enum RateControl {
    RC_DEFAULT,  // 使用编码器默认
    RC_CBR,      // 恒定码率
    RC_VBR,      // 可变码率
    RC_CQ,       // 恒定质量 (需设置 q 参数)
}
```

### 3.2 模式说明

| 模式 | 说明 | 适用场景 |
|------|------|----------|
| `RC_CBR` | 码率恒定，质量随场景变化 | 网络带宽受限 |
| `RC_VBR` | 质量优先，码率波动 | 本地存储 |
| `RC_CQ` | 恒定质量，码率波动大 | 质量敏感场景 |

### 3.3 各编码器支持情况

| 编码器 | CBR | VBR | CQ |
|--------|-----|-----|-----|
| nvenc | ✓ | ✓ | ✓ |
| amf | ✓ | ✓ (低延迟) | ✗ |
| qsv | ✓ | ✓ | ✗ |
| vaapi | ✓ | ✓ | ✗ |

## 4. 质量等级

### 4.1 Quality 枚举

```rust
pub enum Quality {
    Quality_Default, // 使用编码器默认
    Quality_High,    // 高质量 (慢速)
    Quality_Medium,  // 中等质量 (平衡)
    Quality_Low,     // 低质量 (快速)
}
```

### 4.2 编码器预设映射

| 质量 | nvenc | amf | qsv |
|------|-------|-----|-----|
| High | - | quality | veryslow |
| Medium | p4 | balanced | medium |
| Low | p1 | speed | veryfast |

## 5. 错误处理

### 5.1 错误码

| 错误码 | 常量 | 说明 |
|--------|------|------|
| 0 | `HWCODEC_SUCCESS` | 成功 |
| -1 | `HWCODEC_ERR_COMMON` | 通用错误 |
| -2 | `HWCODEC_ERR_HEVC_COULD_NOT_FIND_POC` | HEVC 解码参考帧丢失 |

### 5.2 常见错误处理

```rust
match encoder.encode(&yuv_data, pts) {
    Ok(frames) => {
        // 处理编码帧
    }
    Err(-1) => {
        eprintln!("编码失败，可能是输入数据格式错误");
    }
    Err(code) => {
        eprintln!("未知错误: {}", code);
    }
}
```

## 6. 最佳实践

### 6.1 编码器选择策略

```rust
fn select_best_encoder(
    width: i32,
    height: i32,
    format: DataFormat
) -> Option<String> {
    let ctx = EncodeContext {
        width,
        height,
        pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
        // ... 其他参数
    };

    let encoders = Encoder::available_encoders(ctx, None);

    // 按优先级排序，选择最佳
    encoders.into_iter()
        .filter(|e| e.format == format)
        .min_by_key(|e| e.priority)
        .map(|e| e.name)
}
```

### 6.2 帧内存布局

```rust
// 获取 NV12 帧布局信息
let (linesize, offset, length) = ffmpeg_linesize_offset_length(
    AVPixelFormat::AV_PIX_FMT_NV12,
    1920,
    1080,
    0,  // align
)?;

// 分配缓冲区
let mut buffer = vec![0u8; length as usize];

// 填充 Y 平面: buffer[0..offset[0]]
// 填充 UV 平面: buffer[offset[0]..length]
```

### 6.3 关键帧控制

```rust
let mut frame_count = 0;

loop {
    // 每 30 帧强制一个关键帧
    if frame_count % 30 == 0 {
        encoder.request_keyframe();
    }

    encoder.encode(&yuv_data, pts)?;
    frame_count += 1;
}
```

### 6.4 线程安全

```rust
// Decoder 实现了 Send + Sync
unsafe impl Send for Decoder {}
unsafe impl Sync for Decoder {}

// 可以安全地在多线程间传递
let decoder = Arc::new(Mutex::new(Decoder::new(ctx)?));
```

## 7. IP-KVM 典型使用场景

### 7.1 视频采集和转码流程

```
USB 采集卡 (MJPEG)
       │
       ▼
┌─────────────────┐
│ MJPEG Decoder   │ ◄── Decoder::new("mjpeg")
│ (软件解码)      │
└────────┬────────┘
         │ YUV420P
         ▼
┌─────────────────┐
│ H264 Encoder    │ ◄── Encoder::new("h264_vaapi")
│ (硬件加速)      │
└────────┬────────┘
         │ H264 NAL
         ▼
    WebRTC 传输
```

### 7.2 完整示例

```rust
use hwcodec::ffmpeg_ram::decode::{Decoder, DecodeContext};
use hwcodec::ffmpeg_ram::encode::{Encoder, EncodeContext};
use hwcodec::ffmpeg::AVHWDeviceType;

// 创建 MJPEG 解码器
let decode_ctx = DecodeContext {
    name: "mjpeg".to_string(),
    device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
    thread_count: 4,
};
let mut decoder = Decoder::new(decode_ctx)?;

// 检测并选择最佳编码器
let encode_ctx = EncodeContext {
    name: String::new(),
    width: 1920,
    height: 1080,
    // ...
};
let available = Encoder::available_encoders(encode_ctx.clone(), None);
let best_h264 = available.iter()
    .filter(|e| e.format == DataFormat::H264)
    .min_by_key(|e| e.priority)
    .expect("No H264 encoder available");

// 使用最佳编码器创建实例
let encode_ctx = EncodeContext {
    name: best_h264.name.clone(),
    ..encode_ctx
};
let mut encoder = Encoder::new(encode_ctx)?;

// 处理循环
loop {
    let mjpeg_frame = capture_frame();

    // 解码 MJPEG -> YUV
    let decoded = decoder.decode(&mjpeg_frame)?;

    // 编码 YUV -> H264
    for frame in decoded {
        let yuv_data = frame.data.concat();
        let encoded = encoder.encode(&yuv_data, pts)?;

        // 发送编码数据
        for packet in encoded {
            send_to_webrtc(packet.data);
        }
    }
}
```
