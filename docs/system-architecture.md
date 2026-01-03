# One-KVM 系统架构文档

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

---

## 2. 系统架构图

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              One-KVM System                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Web Frontend (Vue3)                           │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Console  │ │ Settings │ │  Login   │ │  Setup   │ │ Virtual  │  │   │
│  │  │   View   │ │   View   │ │   View   │ │   View   │ │ Keyboard │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  │                                │                                     │   │
│  │              ┌─────────────────┴─────────────────┐                  │   │
│  │              │        Pinia State Store          │                  │   │
│  │              └─────────────────┬─────────────────┘                  │   │
│  │                                │                                     │   │
│  │  ┌──────────────────────────────────────────────────────────────┐  │   │
│  │  │                      API Client Layer                         │  │   │
│  │  │  HTTP REST  │  WebSocket  │  WebRTC Signaling  │  MJPEG      │  │   │
│  │  └──────────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    │ HTTP/WS/WebRTC                         │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Axum Web Server (routes.rs)                      │   │
│  │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐           │   │
│  │  │  Public   │ │   User    │ │   Admin   │ │  Static   │           │   │
│  │  │  Routes   │ │  Routes   │ │  Routes   │ │  Files    │           │   │
│  │  └───────────┘ └───────────┘ └───────────┘ └───────────┘           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       AppState (state.rs)                            │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │                    Central State Hub                         │    │   │
│  │  │  ┌────────────┐ ┌────────────┐ ┌────────────┐              │    │   │
│  │  │  │ConfigStore │ │SessionStore│ │ UserStore  │              │    │   │
│  │  │  │  (SQLite)  │ │  (Memory)  │ │  (SQLite)  │              │    │   │
│  │  │  └────────────┘ └────────────┘ └────────────┘              │    │   │
│  │  │                                                              │    │   │
│  │  │  ┌────────────┐ ┌────────────┐ ┌────────────┐              │    │   │
│  │  │  │  EventBus  │ │ OtgService │ │ Extensions │              │    │   │
│  │  │  │ (Broadcast)│ │   (USB)    │ │  Manager   │              │    │   │
│  │  │  └────────────┘ └────────────┘ └────────────┘              │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│      ┌─────────────────────────────┼─────────────────────────────┐         │
│      │                             │                             │         │
│      ▼                             ▼                             ▼         │
│  ┌────────────┐             ┌────────────┐             ┌────────────┐      │
│  │   Video    │             │    HID     │             │   Audio    │      │
│  │  Module    │             │  Module    │             │  Module    │      │
│  ├────────────┤             ├────────────┤             ├────────────┤      │
│  │ Capture    │             │ Controller │             │ Capture    │      │
│  │ Encoder    │             │ OTG Backend│             │ Encoder    │      │
│  │ Streamer   │             │ CH9329     │             │ Pipeline   │      │
│  │ Pipeline   │             │ Monitor    │             │ (Opus)     │      │
│  │ Manager    │             │ DataChan   │             │ Shared     │      │
│  └────────────┘             └────────────┘             └────────────┘      │
│        │                           │                          │            │
│        └───────────────────────────┼──────────────────────────┘            │
│                                    │                                        │
│      ┌─────────────────────────────┼─────────────────────────────┐         │
│      │                             │                             │         │
│      ▼                             ▼                             ▼         │
│  ┌────────────┐             ┌────────────┐             ┌────────────┐      │
│  │    MSD     │             │    ATX     │             │  RustDesk  │      │
│  │  Module    │             │  Module    │             │  Module    │      │
│  ├────────────┤             ├────────────┤             ├────────────┤      │
│  │ Controller │             │ Controller │             │  Service   │      │
│  │ Image Mgr  │             │ Executor   │             │ Rendezvous │      │
│  │ Ventoy     │             │ LED Monitor│             │ Connection │      │
│  │ Drive      │             │ WOL        │             │ Protocol   │      │
│  └────────────┘             └────────────┘             └────────────┘      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Hardware Layer                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐            │
│  │ V4L2 Video │  │  USB OTG   │  │   GPIO     │  │   ALSA     │            │
│  │   Device   │  │  Gadget    │  │  Sysfs     │  │   Audio    │            │
│  │/dev/video* │  │ ConfigFS   │  │            │  │            │            │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘            │
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
│  └─────┬───────┘  └─────┬───────┘  └─────┬───────┘         │
│        │                │                │                  │
│        ▼                ▼                ▼                  │
│  ┌───────────────────────────────────────────────────────┐ │
│  │                   Event Bus                            │ │
│  │              (tokio broadcast channel)                 │ │
│  └───────────────────────────────────────────────────────┘ │
│        │                │                │                  │
│        ▼                ▼                ▼                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                 Web Server (Axum)                    │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐            │   │
│  │  │  MJPEG   │ │  WebRTC  │ │WebSocket │            │   │
│  │  │  Stream  │ │  Stream  │ │  Events  │            │   │
│  │  └──────────┘ └──────────┘ └──────────┘            │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
        │                      │                      │
        ▼                      ▼                      ▼
┌─────────────────────────────────────────────────────────────┐
│                    Client Browser                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Video     │  │    Input    │  │   Control   │         │
│  │  Display    │  │  Events     │  │   Panel     │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 模块依赖关系

### 3.1 模块层次图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Application Layer                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  main.rs ──► state.rs ──► web/routes.rs                                     │
│                 │                                                            │
│     ┌───────────┼───────────┬───────────┬───────────┬───────────┐          │
│     │           │           │           │           │           │          │
│     ▼           ▼           ▼           ▼           ▼           ▼          │
│  ┌──────┐  ┌──────┐   ┌──────┐   ┌──────┐   ┌──────┐   ┌──────┐          │
│  │video/│  │ hid/ │   │ msd/ │   │ atx/ │   │audio/│   │webrtc│          │
│  └──┬───┘  └──┬───┘   └──┬───┘   └──┬───┘   └──┬───┘   └──┬───┘          │
│     │         │          │          │          │          │               │
│     │         └──────────┼──────────┘          │          │               │
│     │                    │                     │          │               │
│     │              ┌─────▼─────┐               │          │               │
│     │              │   otg/    │               │          │               │
│     │              │ (OtgSvc)  │               │          │               │
│     │              └───────────┘               │          │               │
│     │                                          │          │               │
│     └──────────────────────────────────────────┼──────────┘               │
│                                                │                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                           Infrastructure Layer                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐                    │
│  │ config/  │  │  auth/   │  │ events/  │  │extensions│                    │
│  │(ConfigSt)│  │(Session) │  │(EventBus)│  │(ExtMgr)  │                    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘                    │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────┐             │
│  │                    rustdesk/ (RustDeskService)             │             │
│  │  connection.rs │ rendezvous.rs │ crypto.rs │ protocol.rs   │             │
│  └───────────────────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 依赖矩阵

| 模块 | 依赖的模块 |
|------|-----------|
| `main.rs` | state, config, auth, video, hid, msd, atx, audio, webrtc, web, rustdesk, events |
| `state.rs` | config, auth, video, hid, msd, atx, audio, webrtc, rustdesk, events, otg |
| `video/` | events, hwcodec (外部) |
| `hid/` | otg, events |
| `msd/` | otg, events |
| `atx/` | events |
| `audio/` | events |
| `webrtc/` | video, audio, hid, events |
| `web/` | state, auth, config, video, hid, msd, atx, audio, webrtc, events |
| `rustdesk/` | video, audio, hid, events |
| `otg/` | (无内部依赖) |
| `config/` | (无内部依赖) |
| `auth/` | config |
| `events/` | (无内部依赖) |

---

## 4. 核心组件详解

### 4.1 AppState (state.rs)

AppState 是整个应用的状态中枢，通过 `Arc` 包装的方式在所有 handler 之间共享。

```rust
pub struct AppState {
    // 配置和存储
    config: ConfigStore,              // SQLite 配置存储
    sessions: SessionStore,           // 会话存储（内存）
    users: UserStore,                 // SQLite 用户存储

    // 核心服务
    otg_service: Arc<OtgService>,     // USB Gadget 统一管理（HID/MSD 生命周期协调者）
    stream_manager: Arc<VideoStreamManager>,  // 视频流管理器（MJPEG/WebRTC）
    hid: Arc<HidController>,          // HID 控制器（键鼠控制）
    msd: Arc<RwLock<Option<MsdController>>>,  // MSD 控制器（可选，虚拟U盘）
    atx: Arc<RwLock<Option<AtxController>>>,  // ATX 控制器（可选，电源控制）
    audio: Arc<AudioController>,      // 音频控制器（ALSA + Opus）
    rustdesk: Arc<RwLock<Option<Arc<RustDeskService>>>>,  // RustDesk（可选，远程访问）
    extensions: Arc<ExtensionManager>,// 扩展管理器（ttyd, gostc, easytier）

    // 通信和生命周期
    events: Arc<EventBus>,            // 事件总线（tokio broadcast channel）
    shutdown_tx: broadcast::Sender<()>,  // 关闭信号
    data_dir: PathBuf,                // 数据目录
}
```

### 4.2 视频流管道

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Video Pipeline Architecture                          │
└─────────────────────────────────────────────────────────────────────────────┘

┌───────────────────┐
│  V4L2 Device      │
│  /dev/video0      │
└─────────┬─────────┘
          │ Raw MJPEG/YUYV/NV12
          ▼
┌───────────────────┐
│  VideoCapturer    │  ◄─── src/video/capture.rs
│  (capture.rs)     │
└─────────┬─────────┘
          │ VideoFrame
          ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                        SharedVideoPipeline                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                          Decode Stage                                │  │
│  │  ┌─────────────┐                                                    │  │
│  │  │ MJPEG → YUV │  turbojpeg / VAAPI                                 │  │
│  │  └─────────────┘                                                    │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                              │                                             │
│                              ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                         Convert Stage                                │  │
│  │  ┌─────────────┐                                                    │  │
│  │  │YUV → Target │  libyuv (SIMD accelerated)                         │  │
│  │  │   Format    │                                                    │  │
│  │  └─────────────┘                                                    │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                              │                                             │
│                              ▼                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                         Encode Stage                                 │  │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐                       │  │
│  │  │  H264  │ │  H265  │ │  VP8   │ │  VP9   │                       │  │
│  │  │Encoder │ │Encoder │ │Encoder │ │Encoder │                       │  │
│  │  └────────┘ └────────┘ └────────┘ └────────┘                       │  │
│  │      │ (VAAPI/RKMPP/V4L2 M2M/Software)                              │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────┘
          │
          ├──────────────────────────────────────────┐
          │                                          │
          ▼                                          ▼
┌───────────────────┐                      ┌───────────────────┐
│  MJPEG Streamer   │                      │  WebRTC Streamer  │
│  (HTTP Stream)    │                      │  (RTP Packets)    │
└───────────────────┘                      └───────────────────┘
```

### 4.3 OTG 服务架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           OTG Service Architecture                           │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                            OtgService (service.rs)                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Public Interface                                │   │
│  │  enable_hid()  │  disable_hid()  │  enable_msd()  │  disable_msd() │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    OtgGadgetManager (manager.rs)                     │   │
│  │  ┌───────────────────────────────────────────────────────────────┐  │   │
│  │  │                   Gadget Lifecycle                             │  │   │
│  │  │  create_gadget() │ destroy_gadget() │ bind_udc() │ unbind()   │  │   │
│  │  └───────────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│           ┌────────────────────────┼────────────────────────┐              │
│           │                        │                        │              │
│           ▼                        ▼                        ▼              │
│  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐      │
│  │  HID Function   │     │  MSD Function   │     │ Endpoint Alloc  │      │
│  │    (hid.rs)     │     │    (msd.rs)     │     │ (endpoint.rs)   │      │
│  └─────────────────┘     └─────────────────┘     └─────────────────┘      │
│           │                        │                                        │
│           ▼                        ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       ConfigFS Operations                            │   │
│  │  /sys/kernel/config/usb_gadget/one-kvm/                             │   │
│  │  ├── idVendor, idProduct, strings/                                   │   │
│  │  ├── configs/c.1/                                                    │   │
│  │  │   └── functions/ (symlinks)                                       │   │
│  │  └── functions/                                                      │   │
│  │      ├── hid.usb0, hid.usb1, hid.usb2                               │   │
│  │      └── mass_storage.usb0                                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Linux Kernel                                    │
│  ┌─────────────────┐     ┌─────────────────┐                               │
│  │   /dev/hidg*    │     │  Mass Storage   │                               │
│  │  (HID devices)  │     │    Backend      │                               │
│  └─────────────────┘     └─────────────────┘                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 事件系统架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Event System Architecture                          │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                          Event Producers                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐         │
│  │  Video   │ │   HID    │ │   MSD    │ │   ATX    │ │  Audio   │         │
│  │ Module   │ │ Module   │ │ Module   │ │ Module   │ │ Module   │         │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘         │
│       │            │            │            │            │               │
│       └────────────┴────────────┼────────────┴────────────┘               │
│                                 │                                          │
│                                 ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                          EventBus                                    │  │
│  │                   (tokio broadcast channel)                          │  │
│  │  ┌───────────────────────────────────────────────────────────────┐  │  │
│  │  │                    SystemEvent Enum                            │  │  │
│  │  │  StreamStateChanged │ HidStateChanged │ MsdStateChanged       │  │  │
│  │  │  AtxStateChanged │ AudioStateChanged │ DeviceInfo │ Error     │  │  │
│  │  └───────────────────────────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                 │                                          │
│       ┌─────────────────────────┼─────────────────────────┐               │
│       │                         │                         │               │
│       ▼                         ▼                         ▼               │
│  ┌──────────┐            ┌──────────┐            ┌──────────┐            │
│  │WebSocket │            │ DeviceInfo│            │ Internal │            │
│  │ Clients  │            │Broadcaster│            │  Tasks   │            │
│  └──────────┘            └──────────┘            └──────────┘            │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. 初始化流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Application Startup Flow                             │
└─────────────────────────────────────────────────────────────────────────────┘

main()
   │
   ├──► Parse CLI Arguments (clap)
   │      - address, port, data_dir
   │      - enable_https, ssl_cert, ssl_key
   │      - verbosity (-v, -vv, -vvv)
   │
   ├──► Initialize Logging (tracing)
   │
   ├──► Create/Open SQLite Database
   │      └─► ConfigStore::new()
   │      └─► UserStore::new()
   │      └─► SessionStore::new()
   │
   ├──► Initialize Core Services
   │      │
   │      ├──► EventBus::new()
   │      │      └─► Create tokio broadcast channel
   │      │
   │      ├──► OtgService::new()
   │      │      └─► Detect UDC device (/sys/class/udc)
   │      │      └─► Initialize OtgGadgetManager
   │      │
   │      ├──► HidController::new()
   │      │      └─► Detect backend type (OTG/CH9329/None)
   │      │      └─► Create controller with optional OtgService
   │      │
   │      ├──► HidController::init()
   │      │      └─► Request HID function from OtgService
   │      │      └─► Create HID devices (/dev/hidg0-3)
   │      │      └─► Open device files with O_NONBLOCK
   │      │      └─► Initialize HidHealthMonitor
   │      │
   │      ├──► MsdController::init() (if configured)
   │      │      └─► Request MSD function from OtgService
   │      │      └─► Create mass storage device
   │      │      └─► Initialize Ventoy drive (if available)
   │      │
   │      ├──► AtxController::init() (if configured)
   │      │      └─► Setup GPIO pins or USB relay
   │      │
   │      ├──► AudioController::init()
   │      │      └─► Open ALSA device
   │      │      └─► Initialize Opus encoder
   │      │
   │      ├──► VideoStreamManager::new()
   │      │      └─► Initialize SharedVideoPipeline
   │      │      └─► Setup encoder registry (H264/H265/VP8/VP9)
   │      │      └─► Detect hardware acceleration (VAAPI/RKMPP/V4L2 M2M)
   │      │
   │      └──► RustDeskService::new() (if configured)
   │             └─► Load/generate device ID and keys
   │             └─► Connect to rendezvous server
   │
   ├──► Create AppState
   │      └─► Wrap all services in Arc<>
   │
   ├──► Spawn Background Tasks
   │      ├──► spawn_device_info_broadcaster()
   │      ├──► extension_health_check_task()
   │      └──► rustdesk_reconnect_task()
   │
   ├──► Create Axum Router
   │      └─► create_router(app_state)
   │
   └──► Start HTTP/HTTPS Server
          └─► axum::serve() or axum_server with TLS
```

---

## 6. 目录结构

```
One-KVM-RUST/
├── src/                              # Rust 源代码
│   ├── main.rs                      # 应用入口点
│   ├── lib.rs                       # 库导出
│   ├── state.rs                     # AppState 定义
│   ├── error.rs                     # 错误类型定义
│   │
│   ├── video/                       # 视频模块
│   │   ├── mod.rs
│   │   ├── capture.rs              # V4L2 采集
│   │   ├── streamer.rs             # 视频流服务
│   │   ├── stream_manager.rs       # 流管理器
│   │   ├── shared_video_pipeline.rs # 共享视频管道
│   │   ├── format.rs               # 像素格式
│   │   ├── frame.rs                # 视频帧
│   │   ├── convert.rs              # 格式转换
│   │   └── encoder/                # 编码器
│   │       ├── mod.rs
│   │       ├── traits.rs
│   │       ├── h264.rs
│   │       ├── h265.rs
│   │       ├── vp8.rs
│   │       ├── vp9.rs
│   │       └── jpeg.rs
│   │
│   ├── hid/                         # HID 模块
│   │   ├── mod.rs                  # HidController（主控制器）
│   │   ├── backend.rs              # HidBackend trait 和 HidBackendType
│   │   ├── otg.rs                  # OTG 后端（USB Gadget HID）
│   │   ├── ch9329.rs               # CH9329 串口后端
│   │   ├── consumer.rs             # Consumer Control usage codes
│   │   ├── keymap.rs               # JS keyCode → USB HID 转换表
│   │   ├── types.rs                # 事件类型定义
│   │   ├── monitor.rs              # HidHealthMonitor（错误跟踪与恢复）
│   │   ├── datachannel.rs          # DataChannel 二进制协议解析
│   │   └── websocket.rs            # WebSocket 二进制协议适配
│   │
│   ├── otg/                         # USB OTG 模块
│   │   ├── mod.rs
│   │   ├── service.rs              # OtgService
│   │   ├── manager.rs              # GadgetManager
│   │   ├── hid.rs                  # HID Function
│   │   ├── msd.rs                  # MSD Function
│   │   ├── configfs.rs             # ConfigFS 操作
│   │   ├── endpoint.rs             # 端点分配
│   │   └── report_desc.rs          # HID 报告描述符
│   │
│   ├── msd/                         # MSD 模块
│   │   ├── mod.rs
│   │   ├── controller.rs           # MsdController
│   │   ├── image.rs                # 镜像管理
│   │   ├── ventoy_drive.rs         # Ventoy 驱动
│   │   ├── monitor.rs              # 健康监视
│   │   └── types.rs                # 类型定义
│   │
│   ├── atx/                         # ATX 模块
│   │   ├── mod.rs
│   │   ├── controller.rs           # AtxController
│   │   ├── executor.rs             # 动作执行器
│   │   ├── types.rs                # 类型定义
│   │   ├── led.rs                  # LED 监视
│   │   └── wol.rs                  # Wake-on-LAN
│   │
│   ├── audio/                       # 音频模块
│   │   ├── mod.rs
│   │   ├── controller.rs           # AudioController
│   │   ├── capture.rs              # ALSA 采集
│   │   ├── encoder.rs              # Opus 编码
│   │   ├── shared_pipeline.rs      # 共享管道
│   │   ├── monitor.rs              # 健康监视
│   │   └── device.rs               # 设备枚举
│   │
│   ├── webrtc/                      # WebRTC 模块
│   │   ├── mod.rs
│   │   ├── webrtc_streamer.rs      # WebRTC 管理器
│   │   ├── universal_session.rs    # 会话管理
│   │   ├── video_track.rs          # 视频轨道
│   │   ├── rtp.rs                  # RTP 打包
│   │   ├── h265_payloader.rs       # H265 RTP
│   │   ├── peer.rs                 # PeerConnection
│   │   ├── config.rs               # 配置
│   │   ├── signaling.rs            # 信令
│   │   └── track.rs                # 轨道基类
│   │
│   ├── auth/                        # 认证模块
│   │   ├── mod.rs
│   │   ├── user.rs                 # 用户管理
│   │   ├── session.rs              # 会话管理
│   │   ├── password.rs             # 密码哈希
│   │   └── middleware.rs           # Axum 中间件
│   │
│   ├── config/                      # 配置模块
│   │   ├── mod.rs
│   │   ├── schema.rs               # 配置结构定义
│   │   └── store.rs                # SQLite 存储
│   │
│   ├── events/                      # 事件模块
│   │   └── mod.rs                  # EventBus
│   │
│   ├── rustdesk/                    # RustDesk 模块
│   │   ├── mod.rs                  # RustDeskService
│   │   ├── connection.rs           # 连接管理
│   │   ├── rendezvous.rs           # 渲染服务器通信
│   │   ├── crypto.rs               # NaCl 加密
│   │   ├── config.rs               # 配置
│   │   ├── hid_adapter.rs          # HID 适配
│   │   ├── frame_adapters.rs       # 帧格式转换
│   │   ├── protocol.rs             # 协议包装
│   │   └── bytes_codec.rs          # 帧编码
│   │
│   ├── extensions/                  # 扩展模块
│   │   └── mod.rs                  # ExtensionManager
│   │
│   ├── web/                         # Web 模块
│   │   ├── mod.rs
│   │   ├── routes.rs               # 路由定义
│   │   ├── ws.rs                   # WebSocket
│   │   ├── audio_ws.rs             # 音频 WebSocket
│   │   ├── static_files.rs         # 静态文件
│   │   └── handlers/               # API 处理器
│   │       ├── mod.rs
│   │       └── config/
│   │
│   ├── stream/                      # MJPEG 流
│   │   └── mod.rs
│   │
│   └── utils/                       # 工具函数
│       └── mod.rs
│
├── web/                             # Vue3 前端
│   ├── src/
│   │   ├── views/                  # 页面组件
│   │   ├── components/             # UI 组件
│   │   ├── api/                    # API 客户端
│   │   ├── stores/                 # Pinia 状态
│   │   ├── router/                 # 路由配置
│   │   ├── i18n/                   # 国际化
│   │   └── types/                  # TypeScript 类型
│   └── package.json
│
├── libs/                            # 外部库
│   ├── hwcodec/                    # 硬件视频编码
│   └── ventoy-img-rs/              # Ventoy 支持
│
├── protos/                          # Protobuf 定义
│   ├── message.proto               # RustDesk 消息
│   └── rendezvous.proto            # RustDesk 渲染
│
├── docs/                            # 文档
├── scripts/                         # 脚本
├── Cargo.toml                       # Rust 配置
├── build.rs                         # 构建脚本
└── README.md
```

---

## 7. 安全架构

### 7.1 认证流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Authentication Flow                                  │
└─────────────────────────────────────────────────────────────────────────────┘

┌───────────┐        ┌───────────┐        ┌───────────┐        ┌───────────┐
│  Client   │        │  Axum     │        │  Auth     │        │  SQLite   │
│  Browser  │        │  Server   │        │  Module   │        │  Database │
└─────┬─────┘        └─────┬─────┘        └─────┬─────┘        └─────┬─────┘
      │                    │                    │                    │
      │  POST /auth/login  │                    │                    │
      │  {username, pass}  │                    │                    │
      │───────────────────►│                    │                    │
      │                    │  verify_user()     │                    │
      │                    │───────────────────►│                    │
      │                    │                    │  SELECT user       │
      │                    │                    │───────────────────►│
      │                    │                    │◄───────────────────│
      │                    │                    │                    │
      │                    │                    │  Argon2 verify     │
      │                    │                    │  ────────────►     │
      │                    │                    │                    │
      │                    │  session_token     │                    │
      │                    │◄───────────────────│                    │
      │                    │                    │                    │
      │  Set-Cookie:       │                    │                    │
      │  session_id=token  │                    │                    │
      │◄───────────────────│                    │                    │
      │                    │                    │                    │
      │  GET /api/...      │                    │                    │
      │  Cookie: session   │                    │                    │
      │───────────────────►│                    │                    │
      │                    │  validate_session()│                    │
      │                    │───────────────────►│                    │
      │                    │  user_info         │                    │
      │                    │◄───────────────────│                    │
      │                    │                    │                    │
      │  Response          │                    │                    │
      │◄───────────────────│                    │                    │
```

### 7.2 权限层级

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Permission Levels                                   │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  Public (No Auth)                                                            │
│  ├── GET /health                                                             │
│  ├── POST /auth/login                                                        │
│  ├── GET /setup                                                              │
│  └── POST /setup/init                                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│  User (Authenticated)                                                        │
│  ├── GET /info                     (系统信息)                                │
│  ├── GET /devices                  (设备列表)                                │
│  ├── GET/POST /stream/*            (流控制)                                  │
│  ├── POST /webrtc/*                (WebRTC 信令)                            │
│  ├── POST /hid/*                   (HID 控制)                               │
│  ├── POST /audio/*                 (音频控制)                               │
│  └── WebSocket endpoints           (实时通信)                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  Admin (Admin Role)                                                          │
│  ├── GET/PATCH /config/*           (配置管理)                               │
│  ├── POST /msd/*                   (MSD 操作)                               │
│  ├── POST /atx/*                   (电源控制)                               │
│  ├── POST /extensions/*            (扩展管理)                               │
│  ├── POST /rustdesk/*              (RustDesk 配置)                          │
│  └── POST /users/*                 (用户管理)                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

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
│                                                                             │
│  Hardware Connections:                                                      │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐      │
│  │  HDMI Input       │  │  USB OTG Port     │  │  GPIO Header      │      │
│  │  (/dev/video0)    │  │  (USB Gadget)     │  │  (ATX Control)    │      │
│  └───────────────────┘  └───────────────────┘  └───────────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
                          │
                          │ USB Cable
                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  Target PC                                                                   │
│  - Receives USB HID events (keyboard/mouse)                                 │
│  - Provides HDMI video output                                               │
│  - Can boot from virtual USB drive                                          │
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

Access Methods:
1. Local: http://one-kvm.local:8080
2. HTTPS: https://one-kvm.local:8443
3. RustDesk: Via RustDesk client with device ID
```

---

## 9. 扩展点

### 9.1 添加新编码器

```rust
// 1. 实现 Encoder trait
impl Encoder for MyEncoder {
    fn encode(&mut self, frame: &VideoFrame) -> Result<Vec<u8>>;
    fn codec(&self) -> Codec;
    fn bitrate(&self) -> u32;
    // ...
}

// 2. 在 registry 中注册
encoder_registry.register("my-encoder", || Box::new(MyEncoder::new()));
```

### 9.2 添加新 HID 后端

```rust
// 1. 在 backend.rs 中定义新后端类型
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HidBackendType {
    Otg,
    Ch9329 { port: String, baud_rate: u32 },
    MyBackend { /* 配置参数 */ },  // 新增
    None,
}

// 2. 实现 HidBackend trait
#[async_trait]
impl HidBackend for MyBackend {
    fn name(&self) -> &'static str { "MyBackend" }
    async fn init(&self) -> Result<()> { /* ... */ }
    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()> { /* ... */ }
    async fn send_mouse(&self, event: MouseEvent) -> Result<()> { /* ... */ }
    async fn send_consumer(&self, event: ConsumerEvent) -> Result<()> { /* ... */ }
    async fn reset(&self) -> Result<()> { /* ... */ }
    async fn shutdown(&self) -> Result<()> { /* ... */ }
    fn supports_absolute_mouse(&self) -> bool { true }
    fn screen_resolution(&self) -> Option<(u32, u32)> { None }
    fn set_screen_resolution(&mut self, width: u32, height: u32) { /* ... */ }
}

// 3. 在 HidController::init() 中添加分支
match backend_type {
    HidBackendType::MyBackend { /* params */ } => {
        Box::new(MyBackend::new(/* params */)?)
    }
    // ...
}
```

### 9.3 添加新扩展

```rust
// 通过 ExtensionManager 管理外部进程
extension_manager.register("my-extension", ExtensionConfig {
    command: "my-binary",
    args: vec!["--port", "9000"],
    health_check: HealthCheckConfig::Http { url: "http://localhost:9000/health" },
});
```

---

## 10. 参考资料

- [Axum Web Framework](https://github.com/tokio-rs/axum)
- [webrtc-rs](https://github.com/webrtc-rs/webrtc)
- [V4L2 Documentation](https://www.kernel.org/doc/html/latest/userspace-api/media/v4l/v4l2.html)
- [Linux USB Gadget](https://www.kernel.org/doc/html/latest/usb/gadget_configfs.html)
- [RustDesk Protocol](https://github.com/rustdesk/rustdesk)
