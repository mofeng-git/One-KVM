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
// libs/hwcodec/src/ffmpeg_ram/encode.rs

// 生成测试用 YUV 数据
let yuv = Encoder::dummy_yuv(ctx.clone())?;

// 尝试创建编码器并编码测试帧
match Encoder::new(c) {
    Ok(mut encoder) => {
        let start = std::time::Instant::now();
        match encoder.encode(&yuv, 0) {
            Ok(frames) => {
                let elapsed = start.elapsed().as_millis();
                // 验证: 必须产生 1 帧且为关键帧，且在超时时间内完成
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

使用简化的动态库检测方法，无需 CUDA SDK 依赖：

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

int linux_support_nv() {
    // 检测 CUDA 运行时库
    void *handle = dlopen("libcuda.so.1", RTLD_LAZY);
    if (!handle) {
        handle = dlopen("libcuda.so", RTLD_LAZY);
    }
    if (!handle) {
        LOG_TRACE("NVIDIA: libcuda.so not found");
        return -1;
    }
    dlclose(handle);

    // 检测 NVENC 编码库
    handle = dlopen("libnvidia-encode.so.1", RTLD_LAZY);
    if (!handle) {
        handle = dlopen("libnvidia-encode.so", RTLD_LAZY);
    }
    if (!handle) {
        LOG_TRACE("NVIDIA: libnvidia-encode.so not found");
        return -1;
    }
    dlclose(handle);

    LOG_TRACE("NVIDIA: driver support detected");
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

- `libcuda.so` / `libcuda.so.1` - CUDA 运行时
- `libnvidia-encode.so` / `libnvidia-encode.so.1` - NVENC 编码器

## 3. AMD AMF

### 3.1 检测机制 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

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

## 4. Intel QSV/MFX

### 4.1 检测机制 (Linux)

```cpp
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

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
- 在 One-KVM 简化版中仅 Windows 平台完全支持

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
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

int linux_support_rkmpp() {
    // 检测 MPP 服务设备
    if (access("/dev/mpp_service", F_OK) == 0) {
        LOG_TRACE("RKMPP: Found /dev/mpp_service");
        return 0;  // MPP 可用
    }
    // 备用: 检测 RGA 设备
    if (access("/dev/rga", F_OK) == 0) {
        LOG_TRACE("RKMPP: Found /dev/rga");
        return 0;  // MPP 可能可用
    }
    LOG_TRACE("RKMPP: No Rockchip MPP device found");
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
// libs/hwcodec/cpp/common/platform/linux/linux.cpp

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
                LOG_TRACE("V4L2 M2M: Found device " + m2m_devices[i]);
                return 0;  // V4L2 M2M 可用
            }
        }
    }
    LOG_TRACE("V4L2 M2M: No M2M device found");
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

## 8. 硬件加速优先级

### 8.1 优先级定义

```rust
pub enum Priority {
    Best = 0,    // 专用硬件编码器
    Good = 1,    // 通用硬件加速
    Normal = 2,  // 基本硬件支持
    Soft = 3,    // 软件编码
    Bad = 4,     // 最低优先级
}
```

### 8.2 各编码器优先级

| 优先级 | 编码器 |
|--------|--------|
| Best (0) | nvenc, amf, qsv, rkmpp |
| Good (1) | vaapi, v4l2m2m |
| Soft (3) | x264, x265, libvpx |

### 8.3 选择策略

```rust
// libs/hwcodec/src/ffmpeg_ram/mod.rs

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

## 9. 故障排除

### 9.1 NVIDIA

```bash
# 检查 NVIDIA 驱动
nvidia-smi

# 检查 NVENC 支持
ls /dev/nvidia*

# 检查 CUDA 库
ldconfig -p | grep cuda
ldconfig -p | grep nvidia-encode
```

### 9.2 AMD

```bash
# 检查 AMD 驱动
lspci | grep AMD

# 检查 AMF 库
ldconfig -p | grep amf
```

### 9.3 Intel

```bash
# 检查 Intel 驱动
vainfo

# 检查 MFX 库
ldconfig -p | grep mfx
ldconfig -p | grep vpl
```

### 9.4 VAAPI

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

### 9.5 Rockchip MPP

```bash
# 检查 MPP 设备
ls -la /dev/mpp_service
ls -la /dev/rga

# 检查 MPP 库
ldconfig -p | grep rockchip_mpp
```

### 9.6 V4L2 M2M

```bash
# 列出 V4L2 设备
v4l2-ctl --list-devices

# 检查设备能力
v4l2-ctl -d /dev/video10 --all
```

## 10. 性能优化建议

### 10.1 编码器选择

1. **优先使用硬件编码**: NVENC > AMF > QSV > VAAPI > V4L2 M2M > 软件
2. **ARM 设备**: 优先检测 RKMPP，其次 V4L2 M2M
3. **x86 设备**: 根据 GPU 厂商自动选择

### 10.2 低延迟配置

所有硬件编码器都启用了低延迟优化：

| 编码器 | 配置 |
|--------|------|
| NVENC | `delay=0` |
| AMF | `query_timeout=1000` |
| QSV | `async_depth=1` |
| VAAPI | `async_depth=1` |
| libvpx | `deadline=realtime`, `cpu-used=6` |

### 10.3 码率控制

- **实时流**: 推荐 CBR 模式，保证稳定码率
- **GOP 大小**: 建议 30-60 帧 (1-2秒)，平衡延迟和压缩效率
