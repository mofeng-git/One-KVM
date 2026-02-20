<div align="center">
  <h1>One-KVM</h1>
  <p><strong>Rust 编写的开放轻量 IP-KVM 解决方案，实现 BIOS 级远程管理</strong></p>

  <p><a href="README.md">简体中文</a></p>

  [![GitHub stars](https://img.shields.io/github/stars/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/stargazers)
  [![GitHub forks](https://img.shields.io/github/forks/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/network/members)
  [![GitHub issues](https://img.shields.io/github/issues/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/issues)
</div>

---

## 📖 项目概述

**One-KVM Rust** 是一个用 Rust 编写的轻量级 IP-KVM 解决方案，可通过网络远程管理服务器和工作站，实现 BIOS 级远程控制。

项目目标：

- **开放**：不绑定特定硬件配置，尽量适配常见 Linux 设备
- **轻量**：单二进制分发，部署过程更简单
- **易用**：网页界面完成设备与参数配置，无需手动改配置文件

> **注意：** One-KVM Rust 目前仍处于开发早期阶段，功能与细节会快速迭代，欢迎体验与反馈。

## 🔁 迁移说明

开发重心正在从 **One-KVM Python** 逐步转向 **One-KVM Rust**。

- 如果你在使用 **One-KVM Python（基于 PiKVM）**，请查看 [One-KVM Python 文档](https://docs.one-kvm.cn/python/)
- One-KVM Rust 相较于 One-KVM Python：**尚未完全适配 CSI HDMI 采集卡**、**不支持 VNC 访问**，仍处于开发早期阶段

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
- **V4L2 M2M**：通用硬件编码器
- **软件编码**：CPU 编码

### 扩展能力

- Web UI 配置，多语言支持（中文/英文）
- 内置 Web 终端（ttyd）、内网穿透支持（gostc）、P2P 组网支持（EasyTier）、RustDesk 协议集成（用于跨平台远程访问能力扩展）和 RTSP 视频流（用于视频推流）

## ⚡ 安装使用

可以访问 [One-KVM Rust 文档站点](https://docs.one-kvm.cn/) 获取详细信息。

## 报告问题

如果您发现了问题，请：
1. 使用 [GitHub Issues](https://github.com/mofeng-git/One-KVM/issues) 报告，或加入 QQ 群聊反馈。
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

- MaxZ

- 爱发电用户_c5f33

- 爱发电用户_09386

- 爱发电用户_JT6c

- MaxZ

- 爱发电用户_d3d9c

- ......

</details>

### 赞助商

本项目得到以下赞助商的支持：

**文件存储服务：**
- **[Huang1111公益计划](https://pan.huang1111.cn/s/mxkx3T1)** - 提供免登录下载服务

**云服务商**

- **[林枫云](https://www.dkdun.cn)** - 赞助了本项目宁波大带宽服务器

![林枫云](https://docs.one-kvm.cn/img/36076FEFF0898A80EBD5756D28F4076C.png)

林枫云主营国内外地域的精品线路业务服务器、高主频游戏服务器和大带宽服务器。