# One-KVM 核心架构设计文档

## 1. 项目概述

One-KVM 是一个用 Rust 编写的轻量级、开源 IP-KVM 解决方案。它提供 BIOS 级别的远程服务器管理能力，支持视频流、键鼠控制、虚拟存储、电源管理和音频等功能。

### 1.1 核心特性

- **单一二进制部署**：Web UI + 后端一体化，无需额外配置文件
- **双流模式**：支持 WebRTC（H264/H265/VP8/VP9）和 MJPEG 两种流模式
- **USB OTG**：虚拟键鼠、虚拟存储、虚拟网卡
- **ATX 电源控制**：GPIO/USB 继电器
- **RustDesk 协议集成**：支持跨平台访问
- **Vue3 SPA 前端**：支持中文/英文
- **SQLite 配置存储**：无需配置文件

### 1.2 目标平台

| 平台 | 架构 | 用途 |
|------|------|------|
| aarch64-unknown-linux-gnu | ARM64 | 主要目标（Rockchip RK3328 等） |
| armv7-unknown-linux-gnueabihf | ARMv7 | 备选平台 |
| x86_64-unknown-linux-gnu | x86-64 | 开发/测试环境 |

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      One-KVM System Architecture                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │                    Web Frontend (Vue3)                               │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
│                                    │                                       │
│                                    ▼                                       │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │                      Axum Web Server                                 │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
│                                    │                                       │
│                                    ▼                                       │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │                        AppState (Central State)                      │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │ │
│  │  │   Video     │ │    HID      │ │    MSD      │ │    ATX      │   │ │
│  │  │  Pipeline   │ │ Controller  │ │ Controller  │ │ Controller  │   │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘   │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │ │
│  │  │   Audio     │ │   WebRTC    │ │  RustDesk   │ │   Events    │   │ │
│  │  │ Controller  │ │  Streamer   │ │  Service    │ │    Bus      │   │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
│                                    │                                       │
│                                    ▼                                       │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │                   Hardware Interfaces                               │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │ │
│  │  │  V4L2 Cap   │ │  USB OTG    │ │   GPIO      │ │  ALSA/Audio │   │ │
│  │  │  Device     │ │  Gadget     │ │   Control   │ │   Device    │   │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘   │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 数据流架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Data Flow Overview                              │
└─────────────────────────────────────────────────────────────────────────────┘

                        ┌─────────────────┐
                        │   Target PC     │
                        └────────┬────────┘
                                 │
        ┌────────────────────────┼────────────────────────┐
        │                        │                        │
        ▼                        ▼                        ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  HDMI Capture │      │   USB Port    │      │   GPIO/Relay  │
│    Card       │      │  (OTG Mode)   │      │   (ATX)       │
└───────┬───────┘      └───────┬───────┘      └───────┬───────┘
        │                      │                      │
        ▼                      ▼                      ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  /dev/video0  │      │  /dev/hidg*   │      │  /sys/class/  │
│  (V4L2)       │      │  (USB Gadget) │      │  gpio/gpio*   │
└───────┬───────┘      └───────┬───────┘      └───────┬───────┘
        │                      │                      │
        ▼                      ▼                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    One-KVM Application                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Video     │  │    HID      │  │    ATX      │         │
│  │  Pipeline   │  │ Controller  │  │ Controller  │         │
│  │             │  │             │  │             │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│          │                │                    │            │
│          ▼                ▼                    ▼            │
│  ┌─────────────┐  ┌─────────────┐      ┌─────────────┐     │
│  │   Capture   │  │  Backend    │      │   Power     │     │
│  │   Stage     │  │   (OTG/     │      │  Control    │     │
│  │             │  │  CH9329)    │      │             │     │
│  └─────────────┘  └─────────────┘      └─────────────┘     │
│          │                │                    │            │
│          ▼                ▼                    ▼            │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                        Encode Stage                     │ │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐         │ │
│  │  │  H264  │ │  H265  │ │  VP8   │ │  VP9   │         │ │
│  │  │Encoder │ │Encoder │ │Encoder │ │Encoder │         │ │
│  │  └────────┘ └────────┘ └────────┘ └────────┘         │ │
│  │      │ (VAAPI/RKMPP/V4L2 M2M/Software)              │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
          │
          ├──────────────────────────────┬──────────────────────────────┐
          │                              │                              │
          ▼                              ▼                              ▼
┌───────────────────┐          ┌───────────────────┐          ┌───────────────────┐
│  MJPEG Streamer   │          │  WebRTC Streamer  │          │ RustDesk Service  │
│  (HTTP Stream)    │          │  (RTP Packets)    │          │  (P2P Stream)     │
│                   │          │                   │          │                   │
│  - HTTP/1.1       │          │  - DataChannel    │          │  - TCP/UDP Relay  │
│  - WebSocket      │          │  - Video Track    │          │  - UDP Hole Punch │
│  - Frame Buffer   │          │  - Audio Track    │          │  - Encryption     │
└───────────────────┘          └───────────────────┘          └───────────────────┘
          │                              │                              │
          ▼                              ▼                              ▼
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│   Web Client (Browser)  │  │   Web Client (Browser)  │  │  RustDesk Client App   │
│                         │  │                         │  │                         │
│  - Vue3 SPA Interface   │  │  - WebRTC Peer         │  │  - Cross-platform      │
│  - MJPEG Stream Display │  │  - Real-time Video/    │  │  - Direct Connection   │
│  - HID Event Send/Recv  │  │    Audio              │  │  - Same Protocol       │
│  - Stream Control       │  │  - HID via DC         │  │  - Hardware Access     │
└─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘
```

## 3. 模块架构详解

### 3.1 Video 模块

Video 模块负责视频采集、编码和流传输，是 One-KVM 的核心功能模块。

#### 3.1.1 文件结构
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

#### 3.1.2 视频处理流水线
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Video Processing Pipeline                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │   V4L2      │  │   Pixel     │  │   Format    │  │   Zero-Copy     │   │
│  │   Capture   │  │  Conversion │  │   Convert   │  │   Frame Buf   │   │
│  │             │  │             │  │             │  │                 │   │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────┘   │
│         │                │                │                  │              │
│         ▼                ▼                ▼                  ▼              │
│  ┌─────────────────────────────────────────────────────────────────────────┤
│  │                        Shared Video Pipeline                            │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │ │
│  │  │    H264     │  │    H265     │  │     VP8     │  │     VP9     │  │ │
│  │  │  Encoder    │  │  Encoder    │  │  Encoder    │  │  Encoder    │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  │ │
│  └─────────────────────────────────────────────────────────────────────────┤
│                              │                                             │
│                              ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┤
│  │                    Stream Management Layer                              │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                   │ │
│  │  │   MJPEG     │  │   WebRTC    │  │   Stream    │                   │ │
│  │  │  Streamer   │  │  Streamer   │  │   Manager   │                   │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                   │ │
│  └─────────────────────────────────────────────────────────────────────────┘
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 HID 模块

HID 模块负责键盘和鼠标控制，支持 USB OTG 模式和 CH9329 串口控制器。

#### 3.2.1 架构设计
```
Web Client --> WebSocket/DataChannel --> HID Events --> Backend --> Target PC
                                               |
                                       [OTG | CH9329]
```

#### 3.2.2 文件结构
```
src/hid/
├── mod.rs                  # HidController（主控制器）
├── backend.rs              # HidBackend trait 和 HidBackendType
├── otg.rs                  # OTG 后端（USB Gadget HID）
├── ch9329.rs               # CH9329 串口后端
├── consumer.rs             # Consumer Control usage codes
├── keymap.rs               # JS keyCode → USB HID 转换表
├── types.rs                # 事件类型定义
├── monitor.rs              # HidHealthMonitor（错误跟踪与恢复）
├── datachannel.rs          # DataChannel 二进制协议解析
└── websocket.rs            # WebSocket 二进制协议适配
```

### 3.3 OTG 模块

USB OTG 模块负责 USB Gadget 管理，实现虚拟设备功能。

#### 3.3.1 文件结构
```
src/otg/
├── mod.rs
├── service.rs              # OtgService
├── manager.rs              # GadgetManager
├── hid.rs                  # HID Function
├── msd.rs                  # MSD Function
├── configfs.rs             # ConfigFS 操作
├── endpoint.rs             # 端点分配
└── report_desc.rs          # HID 报告描述符
```

### 3.4 MSD 模块

MSD 模块负责虚拟存储设备管理，支持 ISO/IMG 镜像挂载和 Ventoy 模式。

#### 3.4.1 文件结构
```
src/msd/
├── mod.rs
├── controller.rs           # MsdController
├── image.rs                # 镜像管理
├── ventoy_drive.rs         # Ventoy 驱动
├── monitor.rs              # 健康监视
└── types.rs                # 类型定义
```

### 3.5 ATX 模块

ATX 模块负责电源控制，支持 GPIO 和 USB 继电器控制。

#### 3.5.1 文件结构
```
src/atx/
├── mod.rs
├── controller.rs
├── executor.rs
├── led.rs
├── types.rs
└── wol.rs
```

### 3.6 Audio 模块

Audio 模块负责音频采集和编码，支持 Opus 编码。

#### 3.6.1 文件结构
```
src/audio/
├── mod.rs
├── capture.rs
├── controller.rs
├── device.rs
├── encoder.rs
├── monitor.rs
├── streamer.rs
└── types.rs
```

### 3.7 WebRTC 模块

WebRTC 模块负责实时音视频流传输。

#### 3.7.1 文件结构
```
src/webrtc/
├── mod.rs
├── config.rs
├── h265_payloader.rs
├── mdns.rs
├── peer.rs
├── rtp.rs
├── session.rs
├── signaling.rs
├── track.rs
├── unified_video_track.rs
├── universal_session.rs
├── video_track.rs
└── webrtc_streamer.rs
```

### 3.8 RustDesk 模块

RustDesk 模块负责 RustDesk 协议集成，实现跨平台远程访问。

#### 3.8.1 文件结构
```
src/rustdesk/
├── mod.rs               # 模块导出
├── config.rs            # 配置类型
├── crypto.rs            # 加密模块
├── bytes_codec.rs       # 帧编码
├── protocol.rs          # 消息辅助
├── rendezvous.rs        # Rendezvous 中介
├── connection.rs        # 连接处理
├── hid_adapter.rs       # HID 转换
└── frame_adapters.rs    # 视频/音频适配
```

## 4. AppState 中央状态管理

AppState 是应用全局状态容器，协调各个模块之间的交互。

```rust
pub struct AppState {
    /// 配置存储
    pub config: ConfigStore,
    /// 会话存储
    pub sessions: SessionStore,
    /// 用户存储
    pub users: UserStore,
    /// OTG 服务 - 集中化的 USB gadget 生命周期管理
    pub otg_service: Arc<OtgService>,
    /// 视频流管理器 (统一 MJPEG/WebRTC 管理)
    pub stream_manager: Arc<VideoStreamManager>,
    /// HID 控制器
    pub hid: Arc<HidController>,
    /// MSD 控制器 (可选，可能未初始化)
    pub msd: Arc<RwLock<Option<MsdController>>>,
    /// ATX 控制器 (可选，可能未初始化)
    pub atx: Arc<RwLock<Option<AtxController>>>,
    /// 音频控制器
    pub audio: Arc<AudioController>,
    /// RustDesk 远程访问服务 (可选)
    pub rustdesk: Arc<RwLock<Option<Arc<RustDeskService>>>>,
    /// 扩展管理器 (ttyd, gostc, easytier)
    pub extensions: Arc<ExtensionManager>,
    /// 事件总线用于实时通知
    pub events: Arc<EventBus>,
    /// 关闭信号发送器
    pub shutdown_tx: broadcast::Sender<()>,
    /// 最近撤销的会话 ID (用于客户端踢出检测)
    pub revoked_sessions: Arc<RwLock<VecDeque<String>>>,
    /// 数据目录路径
    data_dir: std::path::PathBuf,
}
```

## 5. 事件系统

One-KVM 使用事件总线系统实现模块间的松耦合通信。

### 5.1 事件类型
- StreamStateChanged
- StreamConfigApplied  
- StreamModeReady
- HidStateChanged
- MsdStateChanged
- AtxStateChanged
- AudioStateChanged
- DeviceInfo

### 5.2 事件广播机制
- 使用 tokio broadcast channel 实现实时通知
- 支持设备状态变化的自动推送
- 客户端可以实时接收状态更新

## 6. 硬件编码支持

One-KVM 支持多种硬件加速编码：

- **VAAPI**：Intel/AMD GPU
- **RKMPP**：Rockchip SoC
- **V4L2 M2M**：通用硬件编码器
- **软件编码**：CPU 编码

### 6.1 自动硬件检测
- 运行时自动检测可用的硬件加速后端
- 基于质量和性能为不同编码器分配优先级
- 针对实时流媒体场景进行了专门优化

## 7. 网络架构

### 7.1 接入方式
1. **本地访问**: http://one-kvm.local:8080
2. **HTTPS访问**: https://one-kvm.local:8443
3. **RustDesk协议**: 通过 RustDesk 客户端连接

### 7.2 协议支持
- HTTP/1.1 for MJPEG streaming
- WebSocket for real-time HID events
- WebRTC for low-latency streaming
- RustDesk protocol for cross-platform access

## 8. 部署架构

### 8.1 单机部署

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Single Binary Deployment                             │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  ARM64 Device (e.g., Rockchip RK3328)                                       │
│  ┌───────────────────────────────────────────────────────────────────────┐ │
│  │  one-kvm (single binary, ~15MB)                                        │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │ │
│  │  │  Embedded Assets (rust-embed, gzip compressed)                   │  │ │
│  │  │  - index.html, app.js, app.css, assets/*                        │  │ │
│  │  └─────────────────────────────────────────────────────────────────┘  │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │ │
│  │  │  Runtime Data (data_dir)                                         │  │ │
│  │  │  - one-kvm.db (SQLite)                                          │  │ │
│  │  │  - images/ (MSD images)                                          │  │ │
│  │  │  - certs/ (SSL certificates)                                     │  │ │
│  │  └─────────────────────────────────────────────────────────────────┘  │ │
│  └───────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 网络拓扑

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Network Topology                                   │
└─────────────────────────────────────────────────────────────────────────────┘

                    Internet
                        │
                        │
            ┌───────────┴───────────┐
            │                       │
            ▼                       ▼
    ┌───────────────┐       ┌───────────────┐
    │   RustDesk    │       │    Client     │
    │   Server      │       │   Browser     │
    │  (hbbs/hbbr)  │       │               │
    └───────────────┘       └───────────────┘
            │                       │
            │                       │
            └───────────┬───────────┘
                        │
                   ┌────┴────┐
                   │  Router │
                   │   NAT   │
                   └────┬────┘
                        │
              Local Network
                        │
            ┌───────────┴───────────┐
            │                       │
            ▼                       ▼
    ┌───────────────┐       ┌───────────────┐
    │   One-KVM     │───────│  Target PC    │
    │   Device      │  USB  │               │
    │  :8080/:8443  │  HID  │               │
    └───────────────┘       └───────────────┘
```

## 9. 安全特性

### 9.1 认证授权
- 基于会话的用户认证
- HTTPOnly Cookie 存储
- 随机会话 ID
- 会话超时机制
- Argon2id 密码哈希

### 9.2 传输安全
- TLS 1.3 优先
- 自动证书生成
- 证书轮换支持

### 9.3 输入验证
- 类型安全 (serde)
- 路径验证
- SQL 参数化查询

## 10. 扩展能力

One-KVM 提供多种扩展能力：

- Web UI 配置，多语言支持（中文/英文）
- 内置 Web 终端（服务端 PTY，无需安装 ttyd）
- 内网穿透支持（gostc）
- P2P 组网支持（EasyTier）
- RustDesk 协议集成（跨平台远程访问）
- Ventoy 虚拟U盘模式