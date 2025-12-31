# hwcodec 硬件加速详解

## 1. 硬件加速架构

### 1.1 整体流程

```
┌─────────────────────────────────────────────────────────────┐
│                     应用层 (Rust)                           │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ Encoder::available_encoders() → 自动检测可用硬件编码器   ││
│  └─────────────────────────────────────────────────────────┘│
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                   硬件检测层 (C++)                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐│
│  │linux_    │ │linux_    │ │linux_    │ │linux_support_    ││
│  │support_nv│ │support_  │ │support_  │ │rkmpp/v4l2m2m     ││
│  └────┬─────┘ │amd       │ │intel     │ └─────────┬────────┘│
│       │       └────┬─────┘ └────┬─────┘           │         │
└───────┼────────────┼────────────┼─────────────────┼─────────┘
        │            │            │                 │
        ▼            ▼            ▼                 ▼
┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────────────┐
│ CUDA/     │ │ AMF       │ │ VPL/MFX   │ │ 设备节点检测       │
│ NVENC     │ │ Runtime   │ │ Library   │ │ /dev/mpp_service  │
│ 动态库    │ │ 动态库    │ │ 动态库    │ │ /dev/video*       │
└───────────┘ └───────────┘ └───────────┘ └───────────────────┘
```

### 1.2 编码器测试验证

每个检测到的硬件编码器都会进行实际编码测试：

```rust
// libs/hwcodec/src/ffmpeg_ram/encode.rs:358-450

// 生成测试用 YUV 数据
let yuv = Encoder::dummy_yuv(ctx.clone())?;

// 尝试创建编码器并编码测试帧
match Encoder::new(c) {
    Ok(mut encoder) => {
        let start = std::time::Instant::now();
        match encoder.encode(&yuv, 0) {
            Ok(frames) => {
                let elapsed = start.elapsed().as_millis();
                // 验证: 必须产生 1 帧且为关键帧，且在 1 秒内完成
                if frames.len() == 1 && frames[0].key == 1
                   && elapsed < TEST_TIMEOUT_MS {
                    res.push(codec);
                }
            }
            Err(_) => { /* 编码失败，跳过 */ }
        }
    }
    Err(_) => { /* 创建失败，跳过 */ }
}
```

## 2. NVIDIA NVENC/NVDEC

### 2.1 检测机制 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp:57-73

int linux_support_nv() {
    CudaFunctions *cuda_dl = NULL;
    NvencFunctions *nvenc_dl = NULL;
    CuvidFunctions *cvdl = NULL;

    // 加载 CUDA 动态库
    if (cuda_load_functions(&cuda_dl, NULL) < 0)
        throw "cuda_load_functions failed";

    // 加载 NVENC 动态库
    if (nvenc_load_functions(&nvenc_dl, NULL) < 0)
        throw "nvenc_load_functions failed";

    // 加载 CUVID (解码) 动态库
    if (cuvid_load_functions(&cvdl, NULL) < 0)
        throw "cuvid_load_functions failed";

    // 全部成功则支持 NVIDIA 硬件加速
    return 0;
}
```

### 2.2 编码配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

// NVENC 低延迟配置
if (name.find("nvenc") != std::string::npos) {
    // 禁用编码延迟
    av_opt_set(priv_data, "delay", "0", 0);
}

// GPU 选择
if (name.find("nvenc") != std::string::npos) {
    av_opt_set_int(priv_data, "gpu", gpu_index, 0);
}

// 质量预设
switch (quality) {
    case Quality_Medium:
        av_opt_set(priv_data, "preset", "p4", 0);
        break;
    case Quality_Low:
        av_opt_set(priv_data, "preset", "p1", 0);
        break;
}

// 码率控制
av_opt_set(priv_data, "rc", "cbr", 0);  // 或 "vbr"
```

### 2.3 环境变量

| 变量 | 说明 |
|------|------|
| `RUSTDESK_HWCODEC_NVENC_GPU` | 指定使用的 GPU 索引 (-1 = 自动) |

### 2.4 依赖库

- `libcuda.so` - CUDA 运行时
- `libnvidia-encode.so` - NVENC 编码器
- `libnvcuvid.so` - NVDEC 解码器

## 3. AMD AMF

### 3.1 检测机制 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp:75-91

int linux_support_amd() {
#if defined(__x86_64__) || defined(__aarch64__)
    #define AMF_DLL_NAMEA "libamfrt64.so.1"
#else
    #define AMF_DLL_NAMEA "libamfrt32.so.1"
#endif

    void *handle = dlopen(AMF_DLL_NAMEA, RTLD_LAZY);
    if (!handle) {
        return -1;  // AMF 不可用
    }
    dlclose(handle);
    return 0;  // AMF 可用
}
```

### 3.2 编码配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

// AMF 低延迟配置
if (name.find("amf") != std::string::npos) {
    av_opt_set(priv_data, "query_timeout", "1000", 0);
}

// 质量预设
switch (quality) {
    case Quality_High:
        av_opt_set(priv_data, "quality", "quality", 0);
        break;
    case Quality_Medium:
        av_opt_set(priv_data, "quality", "balanced", 0);
        break;
    case Quality_Low:
        av_opt_set(priv_data, "quality", "speed", 0);
        break;
}

// 码率控制
av_opt_set(priv_data, "rc", "cbr", 0);      // 恒定码率
av_opt_set(priv_data, "rc", "vbr_latency", 0);  // 低延迟 VBR
```

### 3.3 依赖库

- `libamfrt64.so.1` (64位) 或 `libamfrt32.so.1` (32位)

### 3.4 外部 SDK

```
externals/AMF_v1.4.35/
├── amf/
│   ├── public/common/    # 公共代码
│   │   ├── AMFFactory.cpp
│   │   ├── Thread.cpp
│   │   └── TraceAdapter.cpp
│   └── public/include/   # 头文件
│       ├── components/   # 组件定义
│       └── core/         # 核心定义
```

## 4. Intel QSV/MFX

### 4.1 检测机制 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp:93-107

int linux_support_intel() {
    const char *libs[] = {
        "libvpl.so",           // oneVPL (新版)
        "libmfx.so",           // Media SDK
        "libmfx-gen.so.1.2",   // 新驱动
        "libmfxhw64.so.1"      // 旧版驱动
    };

    for (size_t i = 0; i < sizeof(libs) / sizeof(libs[0]); i++) {
        void *handle = dlopen(libs[i], RTLD_LAZY);
        if (handle) {
            dlclose(handle);
            return 0;  // 找到可用库
        }
    }
    return -1;  // Intel MFX 不可用
}
```

### 4.2 编码配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

// QSV 低延迟配置
if (name.find("qsv") != std::string::npos) {
    av_opt_set(priv_data, "async_depth", "1", 0);
}

// QSV 特殊码率配置
if (name.find("qsv") != std::string::npos) {
    c->rc_max_rate = c->bit_rate;
    c->bit_rate--;  // 实现 CBR 效果
}

// 质量预设
switch (quality) {
    case Quality_High:
        av_opt_set(priv_data, "preset", "veryslow", 0);
        break;
    case Quality_Medium:
        av_opt_set(priv_data, "preset", "medium", 0);
        break;
    case Quality_Low:
        av_opt_set(priv_data, "preset", "veryfast", 0);
        break;
}

// 严格标准兼容性 (用于某些特殊设置)
c->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
```

### 4.3 限制

- QSV 不支持 `YUV420P` 像素格式，必须使用 `NV12`
- 仅在 Windows 平台完全支持

### 4.4 外部 SDK

```
externals/MediaSDK_22.5.4/
├── api/
│   ├── include/           # MFX 头文件
│   ├── mfx_dispatch/      # MFX 调度器
│   └── mediasdk_structures/ # 数据结构
└── samples/sample_common/ # 示例代码
```

## 5. VAAPI (Linux)

### 5.1 工作原理

VAAPI (Video Acceleration API) 是 Linux 上的通用硬件视频加速接口：

```
┌─────────────────────────────────────────────────────────────┐
│                    Application                               │
├─────────────────────────────────────────────────────────────┤
│                    FFmpeg libavcodec                         │
├─────────────────────────────────────────────────────────────┤
│                    VAAPI (libva)                             │
├──────────────┬──────────────┬──────────────┬────────────────┤
│ Intel i965   │ Intel iHD    │ AMD radeonsi │ NVIDIA VDPAU   │
│ (Gen8-)      │ (Gen9+)      │              │ (via wrapper)  │
├──────────────┴──────────────┴──────────────┴────────────────┤
│                    Kernel DRM Driver                         │
├──────────────┬──────────────┬──────────────┬────────────────┤
│ i915         │ amdgpu       │ nvidia       │ ...            │
└──────────────┴──────────────┴──────────────┴────────────────┘
```

### 5.2 编码配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

// VAAPI 低延迟配置
if (name.find("vaapi") != std::string::npos) {
    av_opt_set(priv_data, "async_depth", "1", 0);
}
```

### 5.3 硬件上下文初始化

```cpp
// libs/hwcodec/cpp/ffmpeg_ram/ffmpeg_ram_encode.cpp

// 检测 VAAPI 编码器
if (name_.find("vaapi") != std::string::npos) {
    hw_device_type_ = AV_HWDEVICE_TYPE_VAAPI;
    hw_pixfmt_ = AV_PIX_FMT_VAAPI;
}

// 创建硬件设备上下文
ret = av_hwdevice_ctx_create(&hw_device_ctx_, hw_device_type_,
                             NULL,  // 使用默认设备
                             NULL, 0);

// 设置硬件帧上下文
set_hwframe_ctx();

// 分配硬件帧
hw_frame_ = av_frame_alloc();
av_hwframe_get_buffer(c_->hw_frames_ctx, hw_frame_, 0);
```

### 5.4 编码流程

```
输入 YUV (CPU 内存)
       │
       ▼
av_hwframe_transfer_data(hw_frame_, frame_, 0)  // CPU → GPU
       │
       ▼
avcodec_send_frame(c_, hw_frame_)  // 发送 GPU 帧
       │
       ▼
avcodec_receive_packet(c_, pkt_)   // 获取编码数据
       │
       ▼
编码数据 (CPU 内存)
```

### 5.5 依赖库

- `libva.so` - VAAPI 核心库
- `libva-drm.so` - DRM 后端
- `libva-x11.so` - X11 后端 (可选)

## 6. Rockchip MPP

### 6.1 检测机制

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp:122-137

int linux_support_rkmpp() {
    // 检测 MPP 服务设备
    if (access("/dev/mpp_service", F_OK) == 0) {
        return 0;  // MPP 可用
    }
    // 备用: 检测 RGA 设备
    if (access("/dev/rga", F_OK) == 0) {
        return 0;  // MPP 可能可用
    }
    return -1;  // MPP 不可用
}
```

### 6.2 支持的编码器

| 编码器 | 优先级 | 说明 |
|--------|--------|------|
| `h264_rkmpp` | Best (0) | H.264 硬件编码 |
| `hevc_rkmpp` | Best (0) | H.265 硬件编码 |

### 6.3 适用设备

- Rockchip RK3328 (Onecloud, Chainedbox)
- Rockchip RK3399/RK3588 系列
- 其他 Rockchip SoC

## 7. V4L2 M2M

### 7.1 检测机制

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp:139-163

int linux_support_v4l2m2m() {
    const char *m2m_devices[] = {
        "/dev/video10",  // 常见 M2M 编码设备
        "/dev/video11",  // 常见 M2M 解码设备
        "/dev/video0",   // 某些 SoC 使用
    };

    for (size_t i = 0; i < sizeof(m2m_devices) / sizeof(m2m_devices[0]); i++) {
        if (access(m2m_devices[i], F_OK) == 0) {
            int fd = open(m2m_devices[i], O_RDWR | O_NONBLOCK);
            if (fd >= 0) {
                close(fd);
                return 0;  // V4L2 M2M 可用
            }
        }
    }
    return -1;
}
```

### 7.2 支持的编码器

| 编码器 | 优先级 | 说明 |
|--------|--------|------|
| `h264_v4l2m2m` | Good (1) | H.264 V4L2 编码 |
| `hevc_v4l2m2m` | Good (1) | H.265 V4L2 编码 |

### 7.3 适用设备

- 通用 ARM SoC (Allwinner, Amlogic 等)
- 支持 V4L2 M2M API 的设备

## 8. Apple VideoToolbox

### 8.1 检测机制 (macOS)

```rust
// libs/hwcodec/src/common.rs:57-87

#[cfg(target_os = "macos")]
pub(crate) fn get_video_toolbox_codec_support() -> (bool, bool, bool, bool) {
    extern "C" {
        fn checkVideoToolboxSupport(
            h264_encode: *mut i32,
            h265_encode: *mut i32,
            h264_decode: *mut i32,
            h265_decode: *mut i32,
        ) -> c_void;
    }

    let mut h264_encode = 0;
    let mut h265_encode = 0;
    let mut h264_decode = 0;
    let mut h265_decode = 0;

    unsafe {
        checkVideoToolboxSupport(&mut h264_encode, &mut h265_encode,
                                 &mut h264_decode, &mut h265_decode);
    }

    (h264_encode == 1, h265_encode == 1,
     h264_decode == 1, h265_decode == 1)
}
```

### 8.2 编码配置

```cpp
// libs/hwcodec/cpp/common/util.cpp

// VideoToolbox 低延迟配置
if (name.find("videotoolbox") != std::string::npos) {
    av_opt_set_int(priv_data, "realtime", 1, 0);
    av_opt_set_int(priv_data, "prio_speed", 1, 0);
}

// 强制硬件编码
if (name.find("videotoolbox") != std::string::npos) {
    av_opt_set_int(priv_data, "allow_sw", 0, 0);
}
```

### 8.3 限制

- H.264 编码不稳定，已禁用
- 仅支持 H.265 编码
- 完全支持 H.264/H.265 解码

### 8.4 依赖框架

```
CoreFoundation
CoreVideo
CoreMedia
VideoToolbox
AVFoundation
```

## 9. 硬件加速优先级

### 9.1 优先级定义

```rust
pub enum Priority {
    Best = 0,    // 专用硬件编码器
    Good = 1,    // 通用硬件加速
    Normal = 2,  // 基本硬件支持
    Soft = 3,    // 软件编码
    Bad = 4,     // 最低优先级
}
```

### 9.2 各编码器优先级

| 优先级 | 编码器 |
|--------|--------|
| Best (0) | nvenc, amf, qsv, rkmpp |
| Good (1) | vaapi, v4l2m2m |
| Soft (3) | x264, x265, libvpx |

### 9.3 选择策略

```rust
// libs/hwcodec/src/ffmpeg_ram/mod.rs:49-117

pub fn prioritized(coders: Vec<CodecInfo>) -> CodecInfos {
    // 对于每种格式，选择优先级最高的编码器
    for coder in coders {
        match coder.format {
            DataFormat::H264 => {
                if h264.is_none() || h264.priority > coder.priority {
                    h264 = Some(coder);
                }
            }
            // ... 其他格式类似
        }
    }
}
```

## 10. 故障排除

### 10.1 NVIDIA

```bash
# 检查 NVIDIA 驱动
nvidia-smi

# 检查 NVENC 支持
ls /dev/nvidia*

# 检查 CUDA 库
ldconfig -p | grep cuda
ldconfig -p | grep nvidia-encode
```

### 10.2 AMD

```bash
# 检查 AMD 驱动
lspci | grep AMD

# 检查 AMF 库
ldconfig -p | grep amf
```

### 10.3 Intel

```bash
# 检查 Intel 驱动
vainfo

# 检查 MFX 库
ldconfig -p | grep mfx
ldconfig -p | grep vpl
```

### 10.4 VAAPI

```bash
# 安装 vainfo
sudo apt install vainfo

# 检查 VAAPI 支持
vainfo

# 输出示例:
# libva info: VA-API version 1.14.0
# libva info: Trying to open /usr/lib/x86_64-linux-gnu/dri/iHD_drv_video.so
# vainfo: Driver version: Intel iHD driver for Intel(R) Gen Graphics
# vainfo: Supported profile and entrypoints
#       VAProfileH264Main               : VAEntrypointVLD
#       VAProfileH264Main               : VAEntrypointEncSlice
#       ...
```

### 10.5 Rockchip MPP

```bash
# 检查 MPP 设备
ls -la /dev/mpp_service
ls -la /dev/rga

# 检查 MPP 库
ldconfig -p | grep rockchip_mpp
```

### 10.6 V4L2 M2M

```bash
# 列出 V4L2 设备
v4l2-ctl --list-devices

# 检查设备能力
v4l2-ctl -d /dev/video10 --all
```
