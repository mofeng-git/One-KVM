<div align="center">
  <h1>One-KVM</h1>
  <p><strong>An open, lightweight IP-KVM stack in Rust — remote management down to BIOS level</strong></p>

  <p><a href="README.md">简体中文</a> · <a href="README.en.md">English</a></p>

  [![GitHub Release](https://img.shields.io/github/v/release/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/releases)
  [![GitHub stars](https://img.shields.io/github/stars/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/stargazers)
  [![GitHub forks](https://img.shields.io/github/forks/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/network/members)
  [![GitHub issues](https://img.shields.io/github/issues/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/issues)
</div>

---

## Overview

**One-KVM (Rust)** is a lightweight IP-KVM solution written in Rust. It lets you manage servers and workstations over the network, including at BIOS level.

Goals: an open, lightweight, easy-to-use IP-KVM stack.

- **Open**: not tied to one hardware recipe; runs across many setups.
- **Lightweight**: shipped as a binary with minimal moving parts for deployment.
- **Easy to use**: no hand-edited config files required; settings are done in the web UI.

> **One-KVM (Python)** is no longer maintained. If you still need it, see <https://github.com/mofeng-git/One-KVM/tree/python>.

<div align="center">

![One-KVM web console](https://one-kvm.cn/hero-app-effect.png)

</div>

## Features

### Core

| Area | Capabilities |
|------|----------------|
| Video capture | HDMI USB / MIPI CSI / RK3588 HDMI IN; MJPEG and WebRTC (H.264 / H.265 / VP8 / VP9) |
| Video encoding | VAAPI / QSV / RKMPP / V4L2 M2M hardware paths, with software fallback |
| Keyboard & mouse | USB OTG HID or CH340 + CH9329 HID; absolute / relative mouse |
| Virtual media | USB mass storage; ISO/IMG mount and Ventoy-style virtual USB |
| ATX power | GPIO or USB relay; power and reset control |
| Audio | ALSA capture + Opus (HTTP / WebRTC) |

The web UI supports visual configuration and Chinese/English locales. Built-ins include a web terminal (ttyd), intranet tunnel (gostc), P2P (EasyTier), RustDesk protocol (optional cross-platform remote access), and RTSP streaming.

## Installation

Release artifacts are on [GitHub Releases](https://github.com/mofeng-git/One-KVM/releases). Below are short paths for common setups. For **system requirements, hardware, Docker env vars, USB OTG**, and full troubleshooting, see the [One-KVM documentation](https://docs.one-kvm.cn/) (Chinese; use a translator if needed).

### Debian / Ubuntu (.deb)

Download a `one-kvm_*.deb` matching your CPU architecture from Releases, then from the directory containing the package:

```bash
sudo apt update
sudo apt install ./one-kvm_0.x.x_<arch>.deb
```

Replace the version and architecture in the filename with your actual file name.

### Docker

Images:

- **one-kvm** — main app + ttyd  
- **one-kvm-full** — same plus optional extras (e.g. gostc, easytier-core)

Example:

```bash
docker run --name one-kvm -itd \
  --privileged=true --restart unless-stopped \
  -v /dev:/dev -v /sys:/sys \
  --net=host \
  silentwind0/one-kvm-full
```

If pulls are slow, use the Aliyun mirror, e.g. `registry.cn-hangzhou.aliyuncs.com/silentwind/one-kvm-full` (and `registry.cn-hangzhou.aliyuncs.com/silentwind/one-kvm` for the slim image).

### fnOS NAS (Feiniu / 飞牛)

One-KVM is listed in the fnOS **app store**; search and install on your NAS.

### Web UI and first run

Open `http://<device-ip>:8080` in a browser (**8420** after fnOS install). The first visit runs initial setup.

## Reporting issues

If something breaks:

1. Open [GitHub Issues](https://github.com/mofeng-git/One-KVM/issues) or report in the project QQ group.  
2. Include **useful** error messages and steps to reproduce.  
3. Mention software version, hardware, and OS details.

## Sponsorship

One-KVM builds on many great open-source projects; a lot of time goes into testing and maintenance. If you find it useful, you can support development on **[Afdian (为爱发电)](https://afdian.com/a/silentwind)**.

### Thanks

<details>
<summary><strong>Supporter list</strong></summary>

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

- ......

</details>

### Sponsors

**File hosting**

- **[Huang1111 public-interest program](https://pan.huang1111.cn/s/mxkx3T1)** — login-free downloads

**Cloud**

- **[林枫云](https://www.dkdun.cn)** — project server sponsorship

![林枫云](https://docs.one-kvm.cn/img/36076FEFF0898A80EBD5756D28F4076C.png)

林枫云 offers premium network routes, high-frequency game servers, and high-bandwidth servers in China and abroad.
