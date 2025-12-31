# One-KVM 技术文档

本目录包含 One-KVM 项目的完整技术文档。

## 文档结构

```
docs/
├── README.md                 # 本文件 - 文档索引
├── system-architecture.md    # 系统架构文档
├── tech-stack.md            # 技术栈文档
└── modules/                 # 模块文档
    ├── video.md            # 视频模块
    ├── hid.md              # HID 模块
    ├── otg.md              # OTG 模块
    ├── msd.md              # MSD 模块
    ├── atx.md              # ATX 模块
    ├── audio.md            # 音频模块
    ├── webrtc.md           # WebRTC 模块
    ├── rustdesk.md         # RustDesk 模块
    ├── auth.md             # 认证模块
    ├── config.md           # 配置模块
    ├── events.md           # 事件模块
    └── web.md              # Web 模块
```

## 快速导航

### 核心文档

| 文档 | 描述 |
|------|------|
| [系统架构](./system-architecture.md) | 整体架构设计、数据流、模块依赖 |
| [技术栈](./tech-stack.md) | 使用的技术、库和开发规范 |

### 功能模块

| 模块 | 描述 | 关键文件 |
|------|------|---------|
| [Video](./modules/video.md) | 视频采集和编码 | `src/video/` |
| [HID](./modules/hid.md) | 键盘鼠标控制 | `src/hid/` |
| [OTG](./modules/otg.md) | USB Gadget 管理 | `src/otg/` |
| [MSD](./modules/msd.md) | 虚拟存储设备 | `src/msd/` |
| [ATX](./modules/atx.md) | 电源控制 | `src/atx/` |
| [Audio](./modules/audio.md) | 音频采集编码 | `src/audio/` |
| [WebRTC](./modules/webrtc.md) | WebRTC 流媒体 | `src/webrtc/` |
| [RustDesk](./modules/rustdesk.md) | RustDesk 协议集成 | `src/rustdesk/` |

### 基础设施

| 模块 | 描述 | 关键文件 |
|------|------|---------|
| [Auth](./modules/auth.md) | 认证和会话 | `src/auth/` |
| [Config](./modules/config.md) | 配置管理 | `src/config/` |
| [Events](./modules/events.md) | 事件系统 | `src/events/` |
| [Web](./modules/web.md) | HTTP API | `src/web/` |

## 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                      One-KVM System                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                 Web Frontend (Vue3)                       │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   Axum Web Server                         │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     AppState                              │   │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐            │   │
│  │  │ Video  │ │  HID   │ │  MSD   │ │  ATX   │            │   │
│  │  │ Module │ │ Module │ │ Module │ │ Module │            │   │
│  │  └────────┘ └────────┘ └────────┘ └────────┘            │   │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐            │   │
│  │  │ Audio  │ │ WebRTC │ │RustDesk│ │ Events │            │   │
│  │  │ Module │ │ Module │ │ Module │ │  Bus   │            │   │
│  │  └────────┘ └────────┘ └────────┘ └────────┘            │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Hardware Layer                           │   │
│  │  V4L2 │ USB OTG │ GPIO │ ALSA │ Network                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## 关键特性

- **单一二进制**: Web UI + 后端一体化部署
- **双流模式**: WebRTC (H264/H265/VP8/VP9) + MJPEG
- **USB OTG**: 虚拟键鼠、虚拟存储
- **硬件加速**: VAAPI/RKMPP/V4L2 M2M
- **RustDesk**: 跨平台远程访问
- **无配置文件**: SQLite 配置存储

## 目标平台

| 平台 | 架构 | 用途 |
|------|------|------|
| aarch64-unknown-linux-gnu | ARM64 | 主要目标 |
| armv7-unknown-linux-gnueabihf | ARMv7 | 备选 |
| x86_64-unknown-linux-gnu | x86-64 | 开发/测试 |

## 快速开始

```bash
# 构建前端
cd web && npm install && npm run build && cd ..

# 构建后端
cargo build --release

# 运行
./target/release/one-kvm --enable-https
```

## 相关链接

- [项目仓库](https://github.com/mofeng-git/One-KVM)
- [开发计划](./DEVELOPMENT_PLAN.md)
- [项目目标](./PROJECT_GOALS.md)
