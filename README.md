<h3 align=center><img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="logo" width="300"><br></h3>
<h3 align=center><a href="https://github.com/mofeng-git/One-KVM/blob/master/README.md">简体中文</a> </h3>
<p align=right>&nbsp;</p>

### 介绍

One-KVM 是基于廉价计算机硬件（目前为玩客云和 X64 兼容机）和PiKVM软件的硬件级远程控制项目。KVM over IP 可以远程管理服务器或工作站，实现无侵入式控制，无论被控机为什么操作系统或是否安装了操作系统，具有更广泛的适用性。此项目基于 [PiKVM](https://github.com/pikvm/pikvm)，和基于远控软件的远程管理方式不同，无需在被控电脑安装任何软件，实现无侵入式控制。

### 快速开始

**方式一：直刷 One-KVM 镜像**

对于玩客云设备，本项目 Releases 页可以找到适配玩客云的 One-KVM 预编译镜像。镜像名称带 One-KVM 前缀、burn 后缀的为线刷镜像，可使用 USB_Burning_Tool 软件线刷至玩客云。预编译线刷镜像为开箱即用，刷好后启动设备就可以开始使用 One-KVM。

**方式二：One-KVM 脚本安装**

适用于 arm 设备，在玩客云上经过测试。
```bash
git clone --depth=1 https://github.com/mofeng-git/One-KVM.git
cd One-KVM
sudo bash install.sh
#第一阶段安装完成需要重启，再进行第二阶段安装
sudo bash install.sh

#可选功能：H.264 视频编码
sudo bash kvmd_h264_install.sh
```
适用于 X86 设备，在 X64 主机上经过测试。
```bash
git clone --depth=1 https://github.com/mofeng-git/One-KVM.git
cd One-KVM
sudo bash install-x86.sh
#第一阶段安装完成需要重启，再进行第二阶段安装
sudo bash install-x86.sh

#可选功能：H.264 视频编码
sudo bash kvmd_h264_install.sh
```

**方式三：Docker 镜像部署**

目前仅有 pikvm-ch9329_amd64，后续将支持更多控制方式和处理器架构。
```bash
#使用示例：
docker run -itd -p443:443 -p80:80 --name pikvm-docker --device=/dev/ttyUSB0:/dev/kvmd-hid --device=/dev/video0:/dev/kvmd-video silentwind0/pikvm-ch9329:0.61
```

详细内容可以参照 [One-KVM文档](https://one-kvm.mofeng.run/)。

### 功能特性

主要功能比较，TinyPilot 社区版本、PiKVMv3 版本出现在这里仅做比较目的。
|      功能      |         One-KVM         | TinyPilot 社区版本 | PiKVMv3版本  |
| :------------: | :---------------------: | :----------------: | :----------: |
| HTML5界面语言  |        简体中文         |        英文        |     英文     |
|    BIOS控制    |            √            |         √          |      √       |
|    视频捕捉    |            √            |         √          |      √       |
|    音频捕捉    |            ×            |         √          |      √       |
|  鼠键捕获类型  |       OTG CH9329        |        OTG         |  OTG CH9329  |
|  从剪贴板粘贴  |            √            |         √          |      √       |
|    OCR识别     |            √            |         ×          |      √       |
|    LAN唤醒     |            √            |         ×          |      √       |
|    VNC支持     |            √            |         ×          |      √       |
|    HDMI环出    | √（含HDMI设备初步支持） |         ×          |      ×       |
| 虚拟存储驱动器 |  √（仅含OTG设备支持）   |         ×          |      √       |
|   ATX开关机    |  √（仅含GPIO设备支持）  |         ×          |      √       |
|    板载WiFi    |            ×            |         √          |      √       |
|   视频流格式   | MJPEG  H.264（软编码）  |    MJPEG, H.264    | MJPEG, H.264 |
| 最大视频分辨率 |        1920x1080        |     1920x1080      |  1920x1080   |

### 已测试设备
 - 玩客云
 - X64 主机

 此脚本删除了对上游对树莓派设备的支持，如有需要请访问 [srepac/kvmd-armbian](https://github.com/srepac/kvmd-armbian/blob/master/install.sh)。

### 其他

目前此脚本基于[srepac/kvmd-armbian](https://github.com/srepac/kvmd-armbian/)项目重构了One-KVM安装脚本，做了如下修改：
1. 适配玩客云，添加了初步CHROOT自动化支持
2. 资源本地化，减小网络原因的影响
3. 添加kvmd-ffmpeg和kvmd-display服务安装脚本
4. HTML汉化和一些微调


**赞助**

这个项目基于众多开源项目二次开发，作者为此花费了大量的时间和精力进行测试和维护。若此项目对您有用，您可以考虑通过 [为爱发电](https://afdian.com/a/silentwind) 赞助一笔小钱支持作者。作者将能够购买新的硬件（玩客云和周边设备）来测试和维护 One-KVM 的各种配置，并在项目上投入更多的时间。

**感谢名单**

<details>

浩龙的电子嵌入式之路（赞助）

Tsuki（赞助）

H_xiaoming

0蓝蓝0

fairybl

Will

浩龙的电子嵌入式之路

自.知

观棋不语٩ ི۶

爱发电用户_a57a4

爱发电用户_2c769

霜序

[远方](https://runyf.cn/)

......
</details>

**更新日志**

[One-KVM/ChangeLogs.txt](https://github.com/mofeng-git/One-KVM/blob/main/ChangeLogs.txt)

**Star历史**

[![Star 历史](https://api.star-history.com/svg?repos=mofeng-git/One-KVM&type=Date)](https://star-history.com/#mofeng-git/One-KVM&Date)

本项目间接或直接使用了下下列开源项目：
1. [pikvm/pikvm: Open and inexpensive DIY IP-KVM based on Raspberry Pi (github.com)](https://github.com/pikvm/pikvm)
2. [hzyitc/armbian-onecloud: Armbian for onecloud. 玩客云用armbian (github.com)](https://github.com/hzyitc/armbian-onecloud/)
3. [jacobbar/fruity-pikvm: Install Pi-KVM on debian SBCs such as Orange Pi, Banana Pi, Mango Pi, etc (github.com)](https://github.com/jacobbar/fruity-pikvm)
4. [kvmd-armbian/install.sh at master · srepac/kvmd-armbian (github.com)](https://github.com/srepac/kvmd-armbian/blob/master/install.sh)
