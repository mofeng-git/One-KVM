# hwcodec 技术架构报告

## 1. 项目概述

hwcodec 是一个基于 FFmpeg 的硬件视频编解码库，来源于 RustDesk 项目并针对 One-KVM 进行了深度定制优化。该库专注于 IP-KVM 场景，提供 Windows 和 Linux 平台的 GPU 加速视频编码能力。

### 1.1 项目位置

```
libs/hwcodec/
├── src/          # Rust 源代码
└── cpp/          # C++ 源代码
```

### 1.2 核心特性

- **多编解码格式支持**: H.264, H.265 (HEVC), VP8, VP9, MJPEG
- **硬件加速**: NVENC, AMF, Intel QSV (Windows), VAAPI, RKMPP, V4L2 M2M (Linux)
- **跨平台**: Windows, Linux (x86_64, ARM64, ARMv7)
- **低延迟优化**: 专为实时流媒体场景设计
- **Rust/C++ 混合架构**: Rust 提供安全的上层 API，C++ 实现底层编解码逻辑
- **IP-KVM 专用**: 解码仅支持 MJPEG（采集卡输出格式），编码支持多种硬件加速

## 2. 架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     Rust API Layer                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    ffmpeg_ram module                     ││
│  │              (encode.rs + decode.rs)                     ││
│  └──────────────────────────┬──────────────────────────────┘│
├─────────────────────────────┼───────────────────────────────┤
│                             │                               │
│                  FFI Bindings (bindgen)                     │
│                             ▼                               │
├─────────────────────────────────────────────────────────────┤
│                     C++ Core Layer                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │               ffmpeg_ram (encode/decode)                 ││
│  └──────────────────────────┬──────────────────────────────┘│
├─────────────────────────────┼───────────────────────────────┤
│                             │                               │
│                             ▼                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    FFmpeg Libraries                   │   │
│  │  libavcodec │ libavutil │ libavformat │ libswscale   │   │
│  └──────────────────────────────────────────────────────┘   │
│                             │                               │
├─────────────────────────────┼───────────────────────────────┤
│         Hardware Acceleration Backends                      │
│  ┌────────┐ ┌─────┐ ┌─────┐ ┌───────┐ ┌───────┐ ┌───────┐  │
│  │ NVENC  │ │ AMF │ │ QSV │ │ VAAPI │ │ RKMPP │ │V4L2M2M│  │
│  └────────┘ └─────┘ └─────┘ └───────┘ └───────┘ └───────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 模块职责

| 模块 | 职责 | 关键文件 |
|------|------|----------|
| `ffmpeg_ram` | 基于 RAM 的软件/硬件编解码 | `src/ffmpeg_ram/` |
| `common` | 公共定义和 GPU 检测 | `src/common.rs` |
| `ffmpeg` | FFmpeg 日志和初始化 | `src/ffmpeg.rs` |

## 3. 模块详细分析

### 3.1 库入口 (lib.rs)

```rust
// libs/hwcodec/src/lib.rs
pub mod common;
pub mod ffmpeg;
pub mod ffmpeg_ram;
```

**功能**:
- 导出所有子模块
- 提供 C 日志回调函数

### 3.2 公共模块 (common.rs)

**核心类型**:

```rust
pub enum Driver {
    NV,      // NVIDIA
    AMF,     // AMD
    MFX,     // Intel
    FFMPEG,  // 软件编码
}
```

**GPU 检测函数**:

| 平台 | 检测函数 | 检测方式 |
|------|----------|----------|
| Linux | `linux_support_nv()` | 加载 libcuda.so + libnvidia-encode.so |
| Linux | `linux_support_amd()` | 检查 `libamfrt64.so.1` |
| Linux | `linux_support_intel()` | 检查 `libvpl.so`/`libmfx.so` |
| Linux | `linux_support_rkmpp()` | 检查 `/dev/mpp_service` |
| Linux | `linux_support_v4l2m2m()` | 检查 `/dev/video*` 设备 |

### 3.3 FFmpeg RAM 编码模块

#### 3.3.1 Rust 层 (src/ffmpeg_ram/)

**CodecInfo 结构体**:

```rust
pub struct CodecInfo {
    pub name: String,           // 编码器名称如 "h264_nvenc"
    pub mc_name: Option<String>, // MediaCodec 名称 (Android)
    pub format: DataFormat,      // H264/H265/VP8/VP9/MJPEG
    pub priority: i32,           // 优先级 (Best=0, Good=1, Normal=2, Soft=3, Bad=4)
    pub hwdevice: AVHWDeviceType, // 硬件设备类型
}
```

**EncodeContext 结构体**:

```rust
pub struct EncodeContext {
    pub name: String,          // 编码器名称
    pub width: i32,            // 视频宽度
    pub height: i32,           // 视频高度
    pub pixfmt: AVPixelFormat, // 像素格式 (NV12/YUV420P)
    pub align: i32,            // 内存对齐
    pub fps: i32,              // 帧率
    pub gop: i32,              // GOP 大小
    pub rc: RateControl,       // 码率控制模式
    pub quality: Quality,      // 质量级别
    pub kbs: i32,              // 目标码率 (kbps)
    pub q: i32,                // 量化参数
    pub thread_count: i32,     // 线程数
}
```

**Encoder 类**:

```rust
pub struct Encoder {
    codec: *mut c_void,           // C++ 编码器指针
    frames: *mut Vec<EncodeFrame>, // 编码输出帧
    pub ctx: EncodeContext,
    pub linesize: Vec<i32>,       // 行大小
    pub offset: Vec<i32>,         // 平面偏移
    pub length: i32,              // 总数据长度
}
```

**核心方法**:

| 方法 | 功能 |
|------|------|
| `Encoder::new()` | 创建编码器实例 |
| `Encoder::encode()` | 编码一帧 YUV 数据 |
| `Encoder::set_bitrate()` | 动态调整码率 |
| `Encoder::request_keyframe()` | 请求下一帧为关键帧 |
| `Encoder::available_encoders()` | 检测系统可用编码器 |

#### 3.3.2 C++ 层 (cpp/ffmpeg_ram/)

**FFmpegRamEncoder 类** (ffmpeg_ram_encode.cpp):

```cpp
class FFmpegRamEncoder {
    AVCodecContext *c_ = NULL;     // FFmpeg 编码上下文
    AVFrame *frame_ = NULL;        // 输入帧
    AVPacket *pkt_ = NULL;         // 编码输出包
    AVBufferRef *hw_device_ctx_;   // 硬件设备上下文
    AVFrame *hw_frame_ = NULL;     // 硬件帧
    bool force_keyframe_ = false;  // 强制关键帧标志

    // 主要方法
    bool init(int *linesize, int *offset, int *length);
    int encode(const uint8_t *data, int length, const void *obj, uint64_t ms);
    int do_encode(AVFrame *frame, const void *obj, int64_t ms);
    int set_hwframe_ctx();         // 设置硬件帧上下文
};
```

**编码流程**:

```
输入 YUV 数据
      │
      ▼
fill_frame() - 填充 AVFrame 数据指针
      │
      ├──▶ (软件编码) 直接使用 frame_
      │
      └──▶ (硬件编码) av_hwframe_transfer_data() 传输到 GPU
                            │
                            ▼
                    使用 hw_frame_
                            │
                            ▼
              avcodec_send_frame() - 发送帧到编码器
                            │
                            ▼
              avcodec_receive_packet() - 获取编码数据
                            │
                            ▼
                    callback() - 回调输出
```

### 3.4 FFmpeg RAM 解码模块

**IP-KVM 专用设计**: 解码器仅支持 MJPEG 软件解码，因为 IP-KVM 场景中视频采集卡输出的是 MJPEG 格式。

**Decoder 类**:

```rust
pub struct Decoder {
    codec: *mut c_void,
    frames: *mut Vec<DecodeFrame>,
    pub ctx: DecodeContext,
}

pub struct DecodeFrame {
    pub pixfmt: AVPixelFormat,
    pub width: i32,
    pub height: i32,
    pub data: Vec<Vec<u8>>,   // Y, U, V 平面数据
    pub linesize: Vec<i32>,
    pub key: bool,
}
```

**available_decoders()**: 仅返回 MJPEG 软件解码器

```rust
pub fn available_decoders() -> Vec<CodecInfo> {
    vec![CodecInfo {
        name: "mjpeg".to_owned(),
        format: MJPEG,
        hwdevice: AV_HWDEVICE_TYPE_NONE,
        priority: Priority::Best as _,
        ..Default::default()
    }]
}
```

**C++ 实现** (ffmpeg_ram_decode.cpp):

```cpp
class FFmpegRamDecoder {
    AVCodecContext *c_ = NULL;
    AVFrame *frame_ = NULL;      // 解码输出帧
    AVPacket *pkt_ = NULL;

    int do_decode(const void *obj);
};
```

**解码流程**:

```
输入 MJPEG 数据
      │
      ▼
avcodec_send_packet() - 发送数据到解码器
      │
      ▼
avcodec_receive_frame() - 获取解码帧 (YUV420P)
      │
      ▼
callback() - 回调输出
```

## 4. 硬件加速支持

### 4.1 支持的硬件加速后端

| 后端 | 厂商 | 平台 | 编码器名称 |
|------|------|------|-----------|
| NVENC | NVIDIA | Windows/Linux | h264_nvenc, hevc_nvenc |
| AMF | AMD | Windows/Linux | h264_amf, hevc_amf |
| QSV | Intel | Windows | h264_qsv, hevc_qsv |
| VAAPI | 通用 | Linux | h264_vaapi, hevc_vaapi, vp8_vaapi, vp9_vaapi |
| RKMPP | Rockchip | Linux | h264_rkmpp, hevc_rkmpp |
| V4L2 M2M | ARM SoC | Linux | h264_v4l2m2m, hevc_v4l2m2m |

### 4.2 硬件检测逻辑 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

// NVIDIA 检测 - 简化的动态库检测
int linux_support_nv() {
    void *handle = dlopen("libcuda.so.1", RTLD_LAZY);
    if (!handle) handle = dlopen("libcuda.so", RTLD_LAZY);
    if (!handle) return -1;
    dlclose(handle);

    handle = dlopen("libnvidia-encode.so.1", RTLD_LAZY);
    if (!handle) handle = dlopen("libnvidia-encode.so", RTLD_LAZY);
    if (!handle) return -1;
    dlclose(handle);
    return 0;
}

// AMD 检测 - 检查 AMF 运行时库
int linux_support_amd() {
    void *handle = dlopen("libamfrt64.so.1", RTLD_LAZY);
    if (!handle) return -1;
    dlclose(handle);
    return 0;
}

// Intel 检测 - 检查 VPL/MFX 库
int linux_support_intel() {
    const char *libs[] = {"libvpl.so", "libmfx.so", ...};
    // 任一成功加载则返回 0
}

// Rockchip MPP 检测 - 检查设备节点
int linux_support_rkmpp() {
    if (access("/dev/mpp_service", F_OK) == 0) return 0;
    if (access("/dev/rga", F_OK) == 0) return 0;
    return -1;
}

// V4L2 M2M 检测 - 检查视频设备
int linux_support_v4l2m2m() {
    const char *devices[] = {"/dev/video10", "/dev/video11", ...};
    // 任一设备可打开则返回 0
}
```

### 4.3 编码器优先级系统

```rust
pub enum Priority {
    Best = 0,    // 最高优先级 (硬件加速)
    Good = 1,    // 良好 (VAAPI, 部分硬件)
    Normal = 2,  // 普通
    Soft = 3,    // 软件编码
    Bad = 4,     // 最低优先级
}
```

**优先级分配**:

| 编码器 | 优先级 |
|--------|--------|
| h264_nvenc, hevc_nvenc | Best (0) |
| h264_amf, hevc_amf | Best (0) |
| h264_qsv, hevc_qsv | Best (0) |
| h264_rkmpp, hevc_rkmpp | Best (0) |
| h264_vaapi, hevc_vaapi | Good (1) |
| h264_v4l2m2m, hevc_v4l2m2m | Good (1) |
| h264 (x264), hevc (x265) | Soft (3) |

### 4.4 低延迟优化配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

bool set_lantency_free(void *priv_data, const std::string &name) {
    // NVENC: 禁用延迟缓冲
    if (name.find("nvenc") != std::string::npos) {
        av_opt_set(priv_data, "delay", "0", 0);
    }
    // AMF: 设置查询超时
    if (name.find("amf") != std::string::npos) {
        av_opt_set(priv_data, "query_timeout", "1000", 0);
    }
    // QSV/VAAPI: 设置异步深度为 1
    if (name.find("qsv") != std::string::npos ||
        name.find("vaapi") != std::string::npos) {
        av_opt_set(priv_data, "async_depth", "1", 0);
    }
    // libvpx: 实时模式
    if (name.find("libvpx") != std::string::npos) {
        av_opt_set(priv_data, "deadline", "realtime", 0);
        av_opt_set_int(priv_data, "cpu-used", 6, 0);
        av_opt_set_int(priv_data, "lag-in-frames", 0, 0);
    }
    return true;
}
```

## 5. 构建系统

### 5.1 Cargo.toml 配置

```toml
[package]
name = "hwcodec"
version = "0.8.0"
edition = "2021"
description = "Hardware video codec for IP-KVM (Windows/Linux)"

[features]
default = []

[dependencies]
log = "0.4"
serde_derive = "1.0"
serde = "1.0"
serde_json = "1.0"

[build-dependencies]
cc = "1.0"      # C++ 编译
bindgen = "0.59" # FFI 绑定生成
```

### 5.2 构建流程 (build.rs)

```
build.rs
    │
    ├── build_common()
    │   ├── 生成 common_ffi.rs (bindgen)
    │   ├── 编译平台相关 C++ 代码
    │   └── 链接系统库 (stdc++)
    │
    └── ffmpeg::build_ffmpeg()
        ├── 生成 ffmpeg_ffi.rs
        ├── 链接 FFmpeg 库 (VCPKG 或 pkg-config)
        └── build_ffmpeg_ram()
            └── 编译 ffmpeg_ram_encode.cpp, ffmpeg_ram_decode.cpp
```

### 5.3 FFmpeg 链接方式

| 方式 | 平台 | 条件 |
|------|------|------|
| VCPKG 静态链接 | 跨平台 | 设置 `VCPKG_ROOT` 环境变量 |
| pkg-config 动态链接 | Linux | 默认方式 |

## 6. 与原版 hwcodec 的区别

针对 One-KVM IP-KVM 场景，对原版 RustDesk hwcodec 进行了以下简化：

### 6.1 移除的功能

| 移除项 | 原因 |
|--------|------|
| VRAM 模块 | IP-KVM 不需要 GPU 显存直接编解码 |
| Mux 模块 | IP-KVM 不需要录制到文件 |
| macOS 支持 | IP-KVM 目标平台不包含 macOS |
| Android 支持 | IP-KVM 目标平台不包含 Android |
| 外部 SDK | 简化构建，减少依赖 |
| 多格式解码 | IP-KVM 仅需 MJPEG 解码 |

### 6.2 保留的功能

| 保留项 | 用途 |
|--------|------|
| FFmpeg RAM 编码 | WebRTC 视频编码 |
| FFmpeg RAM 解码 | MJPEG 采集卡解码 |
| 硬件加速编码 | 低延迟高效编码 |
| 软件编码后备 | 无硬件加速时的兜底方案 |

### 6.3 代码量对比

| 指标 | 原版 | 简化版 | 减少 |
|------|------|--------|------|
| 外部 SDK | ~9MB | 0 | 100% |
| C++ 文件 | ~30 | ~10 | ~67% |
| Rust 模块 | 6 | 3 | 50% |

## 7. 总结

hwcodec 库通过 Rust/C++ 混合架构，在保证内存安全的同时实现了高性能的视频编解码。针对 One-KVM IP-KVM 场景的优化设计特点包括:

1. **精简的编解码器 API**: 解码仅支持 MJPEG，编码支持多种硬件加速
2. **自动硬件检测**: 运行时自动检测并选择最优的硬件加速后端
3. **优先级系统**: 基于质量和性能为不同编码器分配优先级
4. **低延迟优化**: 针对实时流媒体场景进行了专门优化
5. **简化的构建系统**: 无需外部 SDK，仅依赖系统 FFmpeg
6. **Windows/Linux 跨平台**: 支持 x86_64、ARM64、ARMv7 架构
