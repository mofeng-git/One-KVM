<div align="center">
  <img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="One-KVM Logo" width="300">
  <h1>One-KVM</h1>
  <p><strong>Rust 编写的开放轻量 IP-KVM 解决方案，实现 BIOS 级远程管理</strong></p>

  <p><a href="README.md">简体中文</a></p>

  [![GitHub stars](https://img.shields.io/github/stars/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/stargazers)
  [![GitHub forks](https://img.shields.io/github/forks/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/network/members)
  [![GitHub issues](https://img.shields.io/github/issues/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/issues)

  <p>
    <a href="docs/README.md">📖 技术文档</a> •
    <a href="#快速开始">⚡ 快速开始</a> •
    <a href="#功能介绍">📊 功能介绍</a> •
    <a href="#迁移说明">🔁 迁移说明</a>
  </p>
</div>

---

## 📋 目录

- [项目概述](#项目概述)
- [迁移说明](#迁移说明)
- [功能介绍](#功能介绍)
- [快速开始](#快速开始)
- [贡献与反馈](#贡献与反馈)
- [致谢](#致谢)
- [许可证](#许可证)

## 📖 项目概述

**One-KVM Rust** 是一个用 Rust 编写的轻量级 IP-KVM 解决方案，可通过网络远程管理服务器和工作站，实现 BIOS 级远程控制。

项目目标：

- **开放**：不绑定特定硬件配置，尽量适配常见 Linux 设备
- **轻量**：单二进制分发，部署过程更简单
- **易用**：网页界面完成设备与参数配置，尽量减少手动改配置文件

> **注意：** One-KVM Rust 目前仍处于开发早期阶段，功能与细节会快速迭代，欢迎体验与反馈。

## 🔁 迁移说明

开发重心正在从 **One-KVM Python** 逐步转向 **One-KVM Rust**。

- 如果你在使用 **One-KVM Python（基于 PiKVM）**，请查看 [One-KVM Python 文档](https://docs.one-kvm.cn/python/)
- One-KVM Rust 相较于 One-KVM Python：**尚未适配 CSI HDMI 采集卡**、**不支持 VNC 访问**，仍处于开发早期阶段

## 📊 功能介绍

### 核心功能

| 功能 | 说明 |
|------|------|
| 视频采集 | HDMI USB 采集卡支持，提供 MJPEG / WebRTC（H.264/H.265/VP8/VP9） |
| 键鼠控制 | USB OTG HID 或 CH340 + CH9329 HID，支持绝对/相对鼠标模式 |
| 虚拟媒体 | USB Mass Storage，支持 ISO/IMG 镜像挂载和 Ventoy 虚拟U盘模式 |
| ATX 电源控制 | GPIO 控制电源/重启按钮 |
| 音频传输 | ALSA 采集 + Opus 编码（HTTP/WebRTC） |

### 硬件编码

支持自动检测和选择硬件加速：

- **VAAPI**：Intel/AMD GPU
- **RKMPP**：Rockchip SoC
- **V4L2 M2M**：RaspberryPi
- **软件编码**：CPU 编码

### 扩展能力

- Web UI 配置，多语言支持（中文/英文）
- 内置 Web 终端（ttyd）内网穿透支持（gostc）、P2P 组网支持（EasyTier）、RustDesk 协议集成（用于跨平台远程访问能力扩展）

## ⚡ 快速开始

安装方式：Docker / DEB 软件包 / 飞牛 NAS（FPK）。

### 方式一：Docker 安装（推荐）

前提条件：

- Linux 主机已安装 Docker
- 插好 USB HDMI 采集卡
- 启用 USB OTG 或插好 CH340+CH9329 HID 线（用于 HID 模拟）

启动容器：

```bash
docker run --name one-kvm -itd --privileged=true \
  -v /dev:/dev  -v /sys/:/sys \
  --net=host \
  silentwind0/one-kvm
```

访问 Web 界面：`http://<设备IP>:8080`（首次访问会引导创建管理员账户）。默认端口：HTTP `8080`；启用 HTTPS 后为 `8443`。

#### 常用环境变量（Docker）

| 变量名 | 默认值 | 说明 |
|------|------|------|
| `ENABLE_HTTPS` | `false` | 是否启用 HTTPS（`true/false`） |
| `HTTP_PORT` | `8080` | HTTP 端口（`ENABLE_HTTPS=false` 时生效） |
| `HTTPS_PORT` | `8443` | HTTPS 端口（`ENABLE_HTTPS=true` 时生效） |
| `BIND_ADDRESS` | - | 监听地址（如 `0.0.0.0`） |
| `VERBOSE` | `0` | 日志详细程度：`1`（-v）、`2`（-vv）、`3`（-vvv） |
| `DATA_DIR` | `/etc/one-kvm` | 数据目录（等价于 `one-kvm -d <DIR>`，优先级高于 `ONE_KVM_DATA_DIR`） |

> 说明：`--privileged=true` 和挂载 `/dev`、`/sys` 是硬件访问所需配置，当前版本不可省略。
>
> 兼容性：同时支持旧变量名 `ONE_KVM_DATA_DIR`。
>
> HTTPS：未提供证书时会自动生成默认自签名证书。
>
> Ventoy：若修改 `DATA_DIR`，请确保 Ventoy 资源文件位于 `${DATA_DIR}/ventoy`（`boot.img`、`core.img`、`ventoy.disk.img`）。

### 方式二：DEB 软件包安装

前提条件：

- Debian 11+ / Ubuntu 22+
- 插好 USB HDMI 采集卡、HID 线（OTG 或 CH340+CH9329）

安装步骤：

1. 从 GitHub Releases 下载适合架构的 `one-kvm_*.deb`：[Releases](https://github.com/mofeng-git/One-KVM/releases)
2. 安装：

```bash
sudo apt update
sudo apt install ./one-kvm_*_*.deb
```

访问 Web 界面：`http://<设备IP>:8080`。

### 方式三：飞牛 NAS（FPK）安装

前提条件：

- 飞牛 NAS 系统（目前仅支持 x86_64 架构）
- 插好 USB HDMI 采集卡、CH340+CH9329 HID 线

安装步骤：

1. 从 GitHub Releases 下载 `*.fpk` 软件包：[Releases](https://github.com/mofeng-git/One-KVM/releases)
2. 在飞牛应用商店选择“手动安装”，导入 `*.fpk`

访问 Web 界面：`http://<设备IP>:8420`。

## 报告问题

如果您发现了问题，请：
1. 使用 [GitHub Issues](https://github.com/mofeng-git/One-KVM/issues) 报告
2. 提供详细的错误信息和复现步骤
3. 包含您的硬件配置和系统信息

## 赞助支持

本项目基于多个优秀开源项目进行二次开发，作者投入了大量时间进行测试和维护。如果您觉得这个项目有价值，欢迎通过 **[为爱发电](https://afdian.com/a/silentwind)** 支持项目发展。

### 感谢名单

<details>
<summary><strong>点击查看感谢名单</strong></summary>

- 浩龙的电子嵌入式之路

- Tsuki

- H_xiaoming

- 0蓝蓝0

- fairybl

- Will

- 浩龙的电子嵌入式之路

- 自.知

- 观棋不语٩ ི۶

- 爱发电用户_a57a4

- 爱发电用户_2c769

- 霜序

- 远方（闲鱼用户名：小远技术店铺）

- 爱发电用户_399fc

- 斐斐の

- 爱发电用户_09451

- 超高校级的錆鱼

- 爱发电用户_08cff

- guoke

- mgt

- 姜沢掵

- ui_beam

- 爱发电用户_c0dd7

- 爱发电用户_dnjK

- 忍者胖猪

- 永遠の願い

- 爱发电用户_GBrF

- 爱发电用户_fd65c

- 爱发电用户_vhNa

- 爱发电用户_Xu6S

- moss

- woshididi

- 爱发电用户_a0fd1

- 爱发电用户_f6bH

- 码农

- 爱发电用户_6639f

- jeron

- 爱发电用户_CN7y

- 爱发电用户_Up6w

- 爱发电用户_e3202

- 一语念白

- 云边

- 爱发电用户_5a711

- 爱发电用户_9a706

- T0m9ir1SUKI

- 爱发电用户_56d52

- 爱发电用户_3N6F

- DUSK

- 飘零

- .

- 饭太稀

- 葱

- ......

</details>

### 赞助商

本项目得到以下赞助商的支持：

**CDN 加速及安全防护：**
- **[Tencent EdgeOne](https://edgeone.ai/zh?from=github)** - 提供 CDN 加速及安全防护服务

![Tencent EdgeOne](https://edgeone.ai/media/34fe3a45-492d-4ea4-ae5d-ea1087ca7b4b.png)

**文件存储服务：**
- **[Huang1111公益计划](https://pan.huang1111.cn/s/mxkx3T1)** - 提供免登录下载服务

**云服务商**

- **[林枫云](https://www.dkdun.cn)** - 赞助了本项目宁波大带宽服务器

![林枫云](https://docs.one-kvm.cn/img/36076FEFF0898A80EBD5756D28F4076C.png)

林枫云主营国内外地域的精品线路业务服务器、高主频游戏服务器和大带宽服务器。
