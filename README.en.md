<div align="center">
  <img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="One-KVM Logo" width="300">
  <h1>One-KVM</h1>
  <p><strong>DIY IP-KVM solution based on PiKVM</strong></p>

  [![GitHub stars](https://img.shields.io/github/stars/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/stargazers)
  [![GitHub forks](https://img.shields.io/github/forks/mofeng-git/One-KVM?style=social)](https://github.com/mofeng-git/One-KVM/network/members)
  [![GitHub issues](https://img.shields.io/github/issues/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/issues)
  [![GitHub license](https://img.shields.io/github/license/mofeng-git/One-KVM)](https://github.com/mofeng-git/One-KVM/blob/master/LICENSE)

  <p>
    <a href="https://one-kvm.mofeng.run">📖 Documentation</a> •
    <a href="https://kvmd-demo.mofeng.run">🚀 Live Demo</a> •
    <a href="#quick-start">⚡ Quick Start</a> •
    <a href="#features">📊 Features</a>
  </p>
</div>

[简体中文](README.md) | English

---

## 📋 Table of Contents

- [Overview](#project-overview)
- [Features](#features)
- [Quick Start](#quick-start)
- [Contributing](#contributing)
- [Others](#others)

## 📖 Project Overview

**One-KVM** is a DIY IP-KVM solution built upon the open-source [PiKVM](https://github.com/pikvm/pikvm) project. It uses cost-effective hardware to provide BIOS-level remote management for servers and workstations.

### Use Cases

- **Home lab management** – Remotely manage servers and development devices
- **Server maintenance** – Perform system maintenance without physical access
- **System recovery** – Troubleshoot boot and BIOS/UEFI issues remotely

![One-KVM UI Screenshot](https://github.com/user-attachments/assets/a7848bca-e43c-434e-b812-27a45fad7910)

## 📊 Features

### Core Capabilities

| Feature | Description | Benefit |
|------|------|------|
| **Non-intrusive** | No software/driver required on the target machine | OS-agnostic; access BIOS/UEFI |
| **Cost-effective** | Leverages affordable hardware (TV boxes, dev boards) | Lower cost for KVM-over-IP |
| **Extendable** | Added utilities on top of PiKVM | Docker, recording, Chinese UI |
| **Deployment** | Supports Docker and prebuilt images | Preconfigured images for specific devices |

### Limitations

This project is maintained by an individual with limited resources and no commercial plan.

- No built-in free NAT punching/tunneling service
- No 24×7 technical support
- No guarantee on stability/compliance; use at your own risk
- User experience is optimized, but basic technical skills are still required

### Feature Comparison

> 💡 **Note:** The table below compares One-KVM with other PiKVM-based projects for reference only. If there are omissions or inaccuracies, please open an issue to help improve it.

| Feature | One-KVM | PiKVM | ArmKVM | BLIKVM |
|:--------:|:-------:|:-----:|:------:|:------:|
| Simplified Chinese WebUI | ✅ | ❌ | ✅ | ✅ |
| Remote video stream | MJPEG/H.264 | MJPEG/H.264 | MJPEG/H.264 | MJPEG/H.264 |
| H.264 encoding | CPU | GPU | Unknown | GPU |
| Remote audio | ✅ | ✅ | ✅ | ✅ |
| Remote mouse/keyboard | OTG/CH9329 | OTG/CH9329/Pico/Bluetooth | OTG | OTG |
| VNC control | ✅ | ✅ | ✅ | ✅ |
| ATX power control | GPIO/USB relay | GPIO | GPIO | GPIO |
| Virtual drive mounting | ✅ | ✅ | ✅ | ✅ |
| Web terminal | ✅ | ✅ | ✅ | ✅ |
| Docker deployment | ✅ | ❌ | ❌ | ❌ |
| Commercial offering | ❌ | ✅ | ✅ | ✅ |

## ⚡ Quick Start

### Method 1: Docker (Recommended)

The Docker variant supports OTG or CH9329 as virtual HID and runs on Linux for amd64/arm64/armv7.

#### One-liner Script

```bash
curl -sSL https://one-kvm.mofeng.run/quick_start.sh -o quick_start.sh && bash quick_start.sh
```

#### Manual Deployment

It is recommended to use the `--net=host` network mode for better WOL functionality and WebRTC communication support.

Docker host network mode:

    Port 8080: HTTP Web service
    Port 4430: HTTPS Web service
    Port 5900: VNC service
    Port 623: IPMI service
    Ports 20000-40000: WebRTC communication port range for low-latency video
    Port 9 (UDP): Wake-on-LAN (WOL)

Docker host mode:

**Using OTG as virtual HID:**

```bash
sudo docker run --name kvmd -itd --privileged=true \
    -v /lib/modules:/lib/modules:ro -v /dev:/dev \
    -v /sys/kernel/config:/sys/kernel/config -e OTG=1 \
    --net=host \
    silentwind0/kvmd
```

**Using CH9329 as virtual HID:**

```bash
sudo docker run --name kvmd -itd \
    --device /dev/video0:/dev/video0 \
    --device /dev/ttyUSB0:/dev/ttyUSB0 \
    --net=host \
    silentwind0/kvmd
```

Docker bridge mode:

**Using OTG as virtual HID:**

```bash
sudo docker run --name kvmd -itd --privileged=true \
    -v /lib/modules:/lib/modules:ro -v /dev:/dev \
    -v /sys/kernel/config:/sys/kernel/config -e OTG=1 \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    silentwind0/kvmd
```

**Using CH9329 as virtual HID:**

```bash
sudo docker run --name kvmd -itd \
    --device /dev/video0:/dev/video0 \
    --device /dev/ttyUSB0:/dev/ttyUSB0 \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    silentwind0/kvmd
```

### Method 2: Flash Prebuilt One-KVM Images

Preconfigured images are provided for specific hardware platforms to simplify deployment and enable out-of-the-box experience.

#### Download

**GitHub:**
- **GitHub Releases:** [https://github.com/mofeng-git/One-KVM/releases](https://github.com/mofeng-git/One-KVM/releases)

**Other mirrors:**
- **No-login mirror:** [https://pan.huang1111.cn/s/mxkx3T1](https://pan.huang1111.cn/s/mxkx3T1)
- **Baidu Netdisk:** [https://pan.baidu.com/s/166-2Y8PBF4SbHXFkGmFJYg?pwd=o9aj](https://pan.baidu.com/s/166-2Y8PBF4SbHXFkGmFJYg?pwd=o9aj) (code: o9aj)

#### Supported Hardware Platforms

| Firmware | Codename | Hardware | Latest | Status |
|:--------:|:--------:|:--------:|:------:|:----:|
| OneCloud | Onecloud | USB capture card, OTG | 241018 | ✅ |
| CumeBox 2 | Cumebox2 | USB capture card, OTG | 241004 | ✅ |
| Vmare | Vmare-uefi | USB capture card, CH9329 | 241004 | ✅ |
| VirtualBox | Virtualbox-uefi | USB capture card, CH9329 | 241004 | ✅ |
| s905l3a Generic | E900v22c | USB capture card, OTG | 241004 | ✅ |
| Chainedbox | Chainedbox | USB capture card, OTG | 241004 | ✅ |
| Loongson 2K0300 | 2k0300 | USB capture card, CH9329 | 241025 | ✅ |

## 🤝 Contributing

Contributions of all kinds are welcome!

### How to Contribute

1. **Fork this repo**
2. **Create a feature branch:** `git checkout -b feature/AmazingFeature`
3. **Commit your changes:** `git commit -m 'Add some AmazingFeature'`
4. **Push to the branch:** `git push origin feature/AmazingFeature`
5. **Open a Pull Request**

### Report Issues

If you find bugs or have suggestions:
1. Open an issue via [GitHub Issues](https://github.com/mofeng-git/One-KVM/issues)
2. Provide detailed error logs and reproduction steps
3. Include your hardware and system information

### Sponsorship

This project builds upon many great open-source projects and requires considerable time for testing and maintenance. If you find it helpful, consider supporting via **[Afdian](https://afdian.com/a/silentwind)**.

#### Thanks

<details>
<summary><strong>Click to view the thank-you list</strong></summary>

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

- ......

</details>

#### Sponsors

This project is supported by the following sponsors:

**CDN & Security:**
- **[Tencent EdgeOne](https://edgeone.ai/zh?from=github)** – CDN acceleration and security protection

![Tencent EdgeOne](https://edgeone.ai/media/34fe3a45-492d-4ea4-ae5d-ea1087ca7b4b.png)

**File Storage:**
- **[Huang1111公益计划](https://pan.huang1111.cn/s/mxkx3T1)** – No-login download service

## 📚 Others

### Open-source Projects Used

This project is built upon the following excellent open-source projects:

- [PiKVM](https://github.com/pikvm/pikvm) – Open-source DIY IP-KVM solution


