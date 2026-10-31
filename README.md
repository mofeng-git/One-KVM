<div align="center">
  <h1>One-KVM</h1>
  <p><strong>Rust 编写的开放轻量 IP-KVM 解决方案，实现 BIOS 级远程管理</strong></p>

  <p><a href="README.md">简体中文</a> · <a href="README.en.md">English</a></p>

  [![GitHub Release](https://img.shields.io/github/v/release/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/releases)
  [![GitHub stars](https://img.shields.io/github/stars/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/stargazers)
  [![GitHub forks](https://img.shields.io/github/forks/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/network/members)
  [![GitHub issues](https://img.shields.io/github/issues/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/issues)
</div>

---

## 项目概述

**One-KVM Rust** 是一个用 Rust 编写的轻量级 IP-KVM 解决方案，可通过网络远程管理服务器和工作站，实现 BIOS 级远程控制。

项目目标：提供一个开放、轻量、易用的 IPKVM 解决方案。

- **开放**：不绑定特定硬件配置，可在各类硬件环境中稳定运行。
- **轻量**：以二进制文件形式分发，无繁杂的依赖项，部署过程简单。
- **易用**：无需手动编辑配置文件，参数设置均可通过网页界面完成。

更多内容可访问 [One-KVM Rust 文档站点](https://docs.one-kvm.cn/)。

> **One-KVM Python** 已停止开发，如有需要可访问 <https://github.com/mofeng-git/One-KVM/tree/python>。

<div align="center">

![One-KVM Web 控制台界面](https://one-kvm.cn/hero-app-effect.png)

</div>

## 功能介绍

**核心功能**

| 功能 | 能力说明 |
|------|----------|
| 视频采集与传输 | 支持 USB HDMI 采集卡、MIPI CSI 和 RK3588 HDMI IN，提供 MJPEG / WebRTC（H.264 / H.265 / VP8 / VP9）视频流 |
| 视频编码 | 支持 VAAPI / QSV / NVENC / AMF / RKMPP / V4L2 M2M 硬件编码，以及软件编码兜底 |
| 键鼠控制 | 支持 USB OTG HID、CH340 + CH9329 串口 HID 和蓝牙 HID；USB / 串口支持绝对与相对鼠标模式，蓝牙支持相对鼠标模式 |
| 虚拟媒体 | 通过 USB Mass Storage 挂载 ISO / IMG 镜像，支持虚拟光驱、虚拟 U 盘和 Ventoy 多镜像启动 |
| ATX 电源控制 | 支持 GPIO、USB HID / 串口继电器，实现开关机、强制关机和重启；GPIO 可检测电源与硬盘活动指示灯 |
| 网络唤醒 | 支持 Wake-on-LAN（WOL），可选择发送网卡并保存唤醒历史 |
| 音频传输 | 支持 Linux ALSA / Windows WASAPI 采集与 Opus 编码，通过 WebSocket / WebRTC 传输被控机音频 |
| USB 麦克风 | 通过 USB OTG UAC 将浏览器麦克风音频传入被控机 |
| USB 网络共享 | 支持 NCM / ECM / RNDIS 虚拟网卡，将被控机通过 USB 桥接到有线网络 |
| 远程客户端接入 | 内置 RustDesk 与 VNC 服务；RustDesk 支持 ID 服务与 IP 直连，VNC 支持 Tight JPEG / H.264 |
| RTSP 视频流 | 提供 H.264 / H.265 RTSP 视频流，支持用户名与密码认证 |

具体功能可用性取决于运行平台、硬件与驱动。

## 安装使用

构建产物见 [GitHub Releases](https://github.com/mofeng-git/One-KVM/releases)。以下为常见安装方式的简要步骤；**系统要求、硬件准备、Docker 环境变量与 USB OTG 等完整说明**请查阅 [One-KVM Rust 文档站点](https://docs.one-kvm.cn/)。

### 使用 deb 安装（Debian / Ubuntu）

从 Releases 下载与本机架构匹配的 `one-kvm_*.deb`，在包所在目录执行：

```bash
sudo apt update
sudo apt install ./one-kvm_0.x.x_<arch>.deb
```

将文件名中的版本号与架构替换为实际下载的包名。

### 使用 Docker

镜像分为 **one-kvm**（One-KVM 主程序 + ttyd）与 **one-kvm-full**（另含 gostc、easyfrpc、easytier-core、easytier-cli、frpc 等可选扩展），按需选用。

```bash
docker run --name one-kvm -itd \
  --privileged=true --restart unless-stopped \
  -v /dev:/dev -v /sys:/sys \
  --net=host \
  silentwind0/one-kvm-full
```

拉取较慢时，可将镜像名替换为阿里云加速，例如 `registry.cn-hangzhou.aliyuncs.com/silentwind/one-kvm-full`（`one-kvm` 镜像同理，将 `silentwind0/one-kvm` 换为 `registry.cn-hangzhou.aliyuncs.com/silentwind/one-kvm`）。

### 飞牛 NAS

One-KVM 已上架飞牛 **应用市场**，在 NAS 上直接搜索安装即可。

### 访问 Web 与首次配置

浏览器访问 `http://<设备 IP>:8080`（飞牛 NAS 安装后为 8420 端口）。首次访问将引导完成初始配置。

## 报告问题

如果您发现了问题，请：
1. 使用 [GitHub Issues](https://github.com/mofeng-git/One-KVM/issues) 报告，或加入 QQ 群聊反馈。
2. 提供有帮助的错误信息和复现步骤
3. 包含您使用的软件版本、硬件配置和系统信息

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

- 爱发电用户_d3d9c

- 爱发电用户_97b41

- 偶然

- 爱发电用户_dba45

- 爱发电用户_d4f8b

- 故人。

- WindowsHao

- 爱发电用户_d4f8b

- 爱发电用户_59f5a

- 爱发电用户_96f28

- 孤星赏月

- t316161070

- 爱发电用户_f5gW

- 爱发电用户_5fb5c

- 爱发电用户_86d54

- 爱发电用户_maRx

- nb

- 爱发电用户_0aa4a

- 爱发电用户_f75a7

- 爱发电用户_b4646

- n1k0

- Yeskf

- 魔力回旋彪

- 泡面

- 中看台的杰森

- LWF

- 爱发电用户_b38fb

- 丨拒绝啦丶

- 爱发电用户_rGYn

- 爱发电用户_XU9y

- 爱发电用户_ec19b

- 爱发电用户_A4dE

- 爱发电用户_eYRC

- Aleo

- 淼

- 爱发电用户_5d06d

- Ghost_chu

- 爱发电用户_1fa21

- 爱发电用户_a7072

- 爱发电用户_9HrC

- 飒歼外寒

- Resucdueff

- Gentisan

- 爱发电用户_MGAx

- 爱发电用户_bce93

- 爱发电用户_60f87

- Bottling3748

- 爱发电用户_7cbeb

- 爱发电用户_6c721

- 哦哦哦哦

- 盒子先生

- 似水流年

- chnxan

- ......

</details>

### 赞助商

本项目得到以下赞助商的支持：

**文件存储服务：**
- **[Huang1111公益计划](https://pan.huang1111.cn/s/mxkx3T1)** - 提供免登录下载服务

**云服务商**

- **[林枫云](https://www.dkdun.cn)** - 赞助了本项目服务器

  <img  height="128" alt="林枫云" src="https://docs.one-kvm.cn/img/36076FEFF0898A80EBD5756D28F4076C.png" />

  林枫云主营国内外地域的精品线路业务服务器、高主频游戏服务器和大带宽服务器。

- **[贝塔网络](https://my.beita.cc/?ref=github_onekvm)** - 赞助了本项目服务器

  <img height="128" alt="BTBT" src="https://github.com/user-attachments/assets/c442d5f5-d72f-4a07-b9f4-400a6a0c3f1e" />

  远程电脑、消费级GPU服务器、独服物理机，全自动在线交付。