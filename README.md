# One-KVM

<p align="center">
  <strong>开放轻量的 IP-KVM 解决方案，实现 BIOS 级远程管理</strong>
</p>

<p align="center">
  <a href="#功能特性">功能特性</a> •
  <a href="#快速开始">快速开始</a> 
</p>

---

## 介绍

One-KVM 是一个用 Rust 编写的开放轻量的 IP-KVM（基于 IP 的键盘、视频、鼠标）解决方案，让你可以通过网络远程控制计算机，包括 BIOS 级别的操作。

**当前软件处于开发早期阶段，各种功能和细节还有待完善，欢迎体验，但请勿应用于生产环境。**

## 功能特性

### 核心功能

| 功能 | 说明 |
|------|------|
| 视频采集 | HDMI USB 采集卡支持，提供 MJPEG/H264/H265/VP8/VP9 视频流 |
| 键鼠控制 | USB OTG HID 或 CH340 + CH39329 HID，支持绝对/相对鼠标模式 |
| 虚拟U盘 | USB Mass Storage，支持 ISO/IMG 镜像挂载和 Ventoy 虚拟U盘模式 |
| ATX 电源控制 | GPIO 控制电源/重启按钮 |
| 音频传输 | ALSA 采集 + Opus 编码（HTTP/WebRTC） |

### 硬件编码

支持自动检测和选择硬件加速：
- **VAAPI** - Intel/AMD GPU
- **RKMPP** - Rockchip SoC (**尚未实现**)
- **V4L2 M2M** - 通用硬件编码器 (**尚未实现**)
- **软件编码** - CPU 编码

### 其他特性

- 单二进制部署，依赖更轻量
- Web UI 配置，无需编辑配置文件，多语言支持 (中文/英文)
- 内置 Web 终端 (ttyd)，内网穿透支持 (gostc)，P2P 组网支持 (EasyTier)

## 快速开始

### Docker 运行

```bash
docker run -d --privileged \
   --name one-kvm \
   -v /dev:/dev \
   -v /sys/kernel/config:/sys/kernel/config \
   --net=host \
   silentwind0/one-kvm
```

访问 http://IP:8080

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `ENABLE_HTTPS` | 启用 HTTPS | `false` |
| `HTTP_PORT` | HTTP 端口 | `8080` |
| `VERBOSE` | 日志级别 (1/2/3) | - |


## 致谢

感谢以下项目：

- [PiKVM](https://github.com/pikvm/pikvm) - 原始 Python 版 IP-KVM
- [RustDesk](https://github.com/rustdesk/rustdesk) - hwcodec 硬件编码库
- [ttyd](https://github.com/tsl0922/ttyd) - Web 终端
- [EasyTier](https://github.com/EasyTier/EasyTier) - P2P 组网

## 许可证

待定
