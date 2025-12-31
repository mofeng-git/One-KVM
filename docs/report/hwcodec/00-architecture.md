# hwcodec 技术架构报告

## 1. 项目概述

hwcodec 是一个基于 FFmpeg 的硬件视频编解码库，来源于 RustDesk 项目并针对 One-KVM 进行了定制优化。该库提供跨平台的 GPU 加速视频编解码能力，支持多个 GPU 厂商和多种编码标准。

### 1.1 项目位置

```
libs/hwcodec/
├── src/          # Rust 源代码
├── cpp/          # C++ 源代码
├── externals/    # 外部依赖 (SDK)
├── dev/          # 开发工具
└── examples/     # 示例程序
```

### 1.2 核心特性

- **多编解码格式支持**: H.264, H.265 (HEVC), VP8, VP9, AV1, MJPEG
- **硬件加速**: NVENC/NVDEC, AMF, Intel QSV/MFX, VAAPI, RKMPP, V4L2 M2M, VideoToolbox
- **跨平台**: Windows, Linux, macOS, Android, iOS
- **低延迟优化**: 专为实时流媒体场景设计
- **Rust/C++ 混合架构**: Rust 提供安全的上层 API，C++ 实现底层编解码逻辑

## 2. 架构设计

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     Rust API Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ ffmpeg_ram  │  │    vram     │  │        mux          │  │
│  │   module    │  │   module    │  │      module         │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
├─────────┼────────────────┼───────────────────┼──────────────┤
│         │                │                   │              │
│         │         FFI Bindings (bindgen)     │              │
│         ▼                ▼                   ▼              │
├─────────────────────────────────────────────────────────────┤
│                     C++ Core Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ ffmpeg_ram  │  │ ffmpeg_vram │  │    mux.cpp          │  │
│  │  encode/    │  │  encode/    │  │                     │  │
│  │  decode     │  │  decode     │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
├─────────┼────────────────┼───────────────────┼──────────────┤
│         │                │                   │              │
│         └────────────────┴───────────────────┘              │
│                          │                                  │
│                          ▼                                  │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    FFmpeg Libraries                   │   │
│  │  libavcodec │ libavutil │ libavformat │ libswscale   │   │
│  └──────────────────────────────────────────────────────┘   │
│                          │                                  │
├──────────────────────────┼──────────────────────────────────┤
│         Hardware Acceleration Backends                      │
│  ┌────────┐ ┌─────┐ ┌─────┐ ┌───────┐ ┌───────┐ ┌───────┐  │
│  │ NVENC  │ │ AMF │ │ MFX │ │ VAAPI │ │ RKMPP │ │V4L2M2M│  │
│  └────────┘ └─────┘ └─────┘ └───────┘ └───────┘ └───────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 模块职责

| 模块 | 职责 | 关键文件 |
|------|------|----------|
| `ffmpeg_ram` | 基于 RAM 的软件/硬件编解码 | `src/ffmpeg_ram/` |
| `vram` | GPU 显存直接编解码 (Windows) | `src/vram/` |
| `mux` | 视频混流 (MP4/MKV) | `src/mux.rs` |
| `common` | 公共定义和 GPU 检测 | `src/common.rs` |
| `ffmpeg` | FFmpeg 日志和初始化 | `src/ffmpeg.rs` |

## 3. 模块详细分析

### 3.1 库入口 (lib.rs)

```rust
// libs/hwcodec/src/lib.rs
pub mod common;
pub mod ffmpeg;
pub mod ffmpeg_ram;
pub mod mux;
#[cfg(all(windows, feature = "vram"))]
pub mod vram;
#[cfg(target_os = "android")]
pub mod android;
```

**功能**:
- 导出所有子模块
- 提供 C 日志回调函数 `hwcodec_log`
- 条件编译: `vram` 模块仅在 Windows + vram feature 启用时编译

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
| Linux | `linux_support_nv()` | 加载 CUDA/NVENC 动态库 |
| Linux | `linux_support_amd()` | 检查 `libamfrt64.so.1` |
| Linux | `linux_support_intel()` | 检查 `libvpl.so`/`libmfx.so` |
| Linux | `linux_support_rkmpp()` | 检查 `/dev/mpp_service` |
| Linux | `linux_support_v4l2m2m()` | 检查 `/dev/video*` 设备 |
| macOS | `get_video_toolbox_codec_support()` | 调用 VideoToolbox API |
| Windows | 通过 VRAM 模块检测 | 查询 D3D11 设备 |

### 3.3 FFmpeg RAM 编码模块

#### 3.3.1 Rust 层 (src/ffmpeg_ram/)

**CodecInfo 结构体**:

```rust
pub struct CodecInfo {
    pub name: String,           // 编码器名称如 "h264_nvenc"
    pub mc_name: Option<String>, // MediaCodec 名称 (Android)
    pub format: DataFormat,      // H264/H265/VP8/VP9/AV1/MJPEG
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

**FFmpegRamEncoder 类** (ffmpeg_ram_encode.cpp:97-420):

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

**C++ 实现** (ffmpeg_ram_decode.cpp):

```cpp
class FFmpegRamDecoder {
    AVCodecContext *c_ = NULL;
    AVBufferRef *hw_device_ctx_ = NULL;
    AVFrame *sw_frame_ = NULL;   // 软件帧 (用于硬件→软件转换)
    AVFrame *frame_ = NULL;      // 解码输出帧
    AVPacket *pkt_ = NULL;
    bool hwaccel_ = true;

    int do_decode(const void *obj);
};
```

**解码流程**:

```
输入编码数据
      │
      ▼
avcodec_send_packet() - 发送数据到解码器
      │
      ▼
avcodec_receive_frame() - 获取解码帧
      │
      ├──▶ (软件解码) 直接使用 frame_
      │
      └──▶ (硬件解码) av_hwframe_transfer_data()
                            │
                            ▼
                    sw_frame_ (GPU → CPU)
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
| VideoToolbox | Apple | macOS/iOS | hevc_videotoolbox |
| MediaCodec | Google | Android | h264_mediacodec, hevc_mediacodec |

### 4.2 硬件检测逻辑 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

// NVIDIA 检测 - 加载 CUDA 和 NVENC 动态库
int linux_support_nv() {
    CudaFunctions *cuda_dl = NULL;
    NvencFunctions *nvenc_dl = NULL;
    CuvidFunctions *cvdl = NULL;
    load_driver(&cuda_dl, &nvenc_dl, &cvdl);
    // 成功加载则返回 0
}

// AMD 检测 - 检查 AMF 运行时库
int linux_support_amd() {
    void *handle = dlopen("libamfrt64.so.1", RTLD_LAZY);
    // 成功加载则返回 0
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
    // VideoToolbox: 实时模式
    if (name.find("videotoolbox") != std::string::npos) {
        av_opt_set_int(priv_data, "realtime", 1, 0);
        av_opt_set_int(priv_data, "prio_speed", 1, 0);
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

## 5. 混流模块 (Mux)

### 5.1 功能概述

混流模块提供将编码后的视频流写入容器格式 (MP4/MKV) 的功能。

### 5.2 Rust API

```rust
// libs/hwcodec/src/mux.rs

pub struct MuxContext {
    pub filename: String,    // 输出文件名
    pub width: usize,        // 视频宽度
    pub height: usize,       // 视频高度
    pub is265: bool,         // 是否为 H.265
    pub framerate: usize,    // 帧率
}

pub struct Muxer {
    inner: *mut c_void,      // C++ Muxer 指针
    pub ctx: MuxContext,
    start: Instant,          // 开始时间
}

impl Muxer {
    pub fn new(ctx: MuxContext) -> Result<Self, ()>;
    pub fn write_video(&mut self, data: &[u8], key: bool) -> Result<(), i32>;
    pub fn write_tail(&mut self) -> Result<(), i32>;
}
```

### 5.3 C++ 实现

```cpp
// libs/hwcodec/cpp/mux/mux.cpp

class Muxer {
    OutputStream video_st;       // 视频流
    AVFormatContext *oc = NULL;  // 格式上下文
    int framerate;
    int64_t start_ms;           // 起始时间戳
    int64_t last_pts;           // 上一帧 PTS
    int got_first;              // 是否收到第一帧

    bool init(const char *filename, int width, int height,
              int is265, int framerate);
    int write_video_frame(const uint8_t *data, int len,
                          int64_t pts_ms, int key);
};
```

**写入流程**:

```
write_video_frame()
      │
      ├── 检查是否为关键帧 (第一帧必须是关键帧)
      │
      ├── 计算 PTS (相对于 start_ms)
      │
      ├── 填充 AVPacket
      │
      ├── av_packet_rescale_ts() (ms → stream timebase)
      │
      └── av_write_frame() → 写入文件
```

## 6. 构建系统

### 6.1 Cargo.toml 配置

```toml
[package]
name = "hwcodec"
version = "0.7.1"

[features]
default = []
vram = []  # GPU VRAM 直接编解码 (Windows only)

[dependencies]
log = "0.4"
serde_derive = "1.0"
serde = "1.0"
serde_json = "1.0"

[build-dependencies]
cc = "1.0"      # C++ 编译
bindgen = "0.59" # FFI 绑定生成
```

### 6.2 构建流程 (build.rs)

```
build.rs
    │
    ├── build_common()
    │   ├── 生成 common_ffi.rs (bindgen)
    │   ├── 编译平台相关 C++ 代码
    │   └── 链接系统库 (d3d11, dxgi, stdc++)
    │
    ├── ffmpeg::build_ffmpeg()
    │   ├── 生成 ffmpeg_ffi.rs
    │   ├── 链接 FFmpeg 库 (VCPKG 或 pkg-config)
    │   ├── build_ffmpeg_ram()
    │   │   └── 编译 ffmpeg_ram_encode.cpp, ffmpeg_ram_decode.cpp
    │   ├── build_ffmpeg_vram() [vram feature]
    │   │   └── 编译 ffmpeg_vram_encode.cpp, ffmpeg_vram_decode.cpp
    │   └── build_mux()
    │       └── 编译 mux.cpp
    │
    └── sdk::build_sdk() [Windows + vram feature]
        ├── build_nv() - NVIDIA SDK
        ├── build_amf() - AMD AMF
        └── build_mfx() - Intel MFX
```

### 6.3 FFmpeg 链接方式

| 方式 | 平台 | 条件 |
|------|------|------|
| VCPKG 静态链接 | 跨平台 | 设置 `VCPKG_ROOT` 环境变量 |
| pkg-config 动态链接 | Linux | 默认方式 |

## 7. 外部依赖

### 7.1 SDK 版本

| SDK | 版本 | 用途 |
|-----|------|------|
| nv-codec-headers | n12.1.14.0 | NVIDIA 编码头文件 |
| Video_Codec_SDK | 12.1.14 | NVIDIA 编解码 SDK |
| AMF | v1.4.35 | AMD Advanced Media Framework |
| MediaSDK | 22.5.4 | Intel Media SDK |

### 7.2 FFmpeg 依赖库

```
libavcodec   - 编解码核心
libavutil    - 工具函数
libavformat  - 容器格式
libswscale   - 图像缩放转换
```

## 8. 总结

hwcodec 库通过 Rust/C++ 混合架构，在保证内存安全的同时实现了高性能的视频编解码。其核心设计特点包括:

1. **统一的编解码器 API**: 无论使用硬件还是软件编解码，上层 API 保持一致
2. **自动硬件检测**: 运行时自动检测并选择最优的硬件加速后端
3. **优先级系统**: 基于质量和性能为不同编码器分配优先级
4. **低延迟优化**: 针对实时流媒体场景进行了专门优化
5. **跨平台支持**: 覆盖主流操作系统和 GPU 厂商
