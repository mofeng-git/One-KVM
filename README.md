<h3 align=center><img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="logo" width="300"><br></h3>
<h3 align=center><a href="https://github.com/mofeng-git/One-KVM/blob/master/README.md">简体中文</a> </h3>
<p align=right>&nbsp;</p>

### 介绍

One-KVM 是基于廉价计算机硬件和PiKVM软件的硬件级远程控制项目。KVM over IP 可以远程管理服务器或工作站，实现无侵入式控制，无论被控机为什么操作系统或是否安装了操作系统，具有更广泛的适用性。此项目基于 [PiKVM](https://github.com/pikvm/pikvm)，和基于远控软件的远程管理方式不同，无需在被控电脑安装任何软件，实现无侵入式控制。

### 快速开始

**方式一：Docker 镜像部署（推荐）**

目前 Docker 版只能使用 CH9329 作为虚拟 HID ，支持 amd64、arm64、armv7 架构的 Linux 系统安装。

当前只有dev分支，尚未发布稳定版本，演示网站（账号密码：admin/admin）：https://kvmd-demo.mofeng.run/

部署命令：
```bash
sudo docker run --name kvmd -itd \
    --device /dev/video0:/dev/kvmd-video \
    --device /dev/ttyUSB0:/dev/kvmd-hid \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd:dev
```

部署完成访问 http://IP:8080 即可开始使用，默认密码：admin/admin。如无法访问可以使用 `sudo docker logs kvmd` 命令查看日志尝试修复、提交 issue 或在 QQ 群内寻求帮助。

如果暂时相关没有 USB 设备或只想要查看新版特性，可以使用以下命令启动一个无 USB 硬件的应用（演示模式）：
```bash
sudo docker run --name kvmd -itd \
    --device /dev/tty10:/dev/kvmd-hid \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd:dev
```

详细内容可以参照 [One-KVM文档](https://one-kvm.mofeng.run/)。

**方式二：直刷 One-KVM 镜像**

对于玩客云设备，本项目 Releases 页可以找到适配玩客云的 One-KVM 预编译镜像。镜像名称带 One-KVM 前缀、burn 后缀的为线刷镜像，可使用 USB_Burning_Tool 软件线刷至玩客云。预编译线刷镜像为开箱即用，刷好后启动设备就可以开始使用 One-KVM。

**方式三：One-KVM 脚本安装**（暂停维护）

**作者目前只是个人业余开发者，能力有限，难以覆盖和测试多种多样硬件设备和系统，故此一键脚本暂时停止维护。**

目前已将开发中心转移至 Docker 平台，推荐使用 Docker 方式部署。若仍有需要可通过  Releases 页找到项目历史存档。

### 功能特性

**Docker 版本中以下特性尚未全部实现，但包含在将来的开发计划中**

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
 - Vmare 虚拟机
 - VPS（仅演示模式）


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


**Star历史**

[![Star 历史](https://api.star-history.com/svg?repos=mofeng-git/One-KVM&type=Date)](https://star-history.com/#mofeng-git/One-KVM&Date)

本项目使用了下列开源项目：
1. [pikvm/pikvm: Open and inexpensive DIY IP-KVM based on Raspberry Pi (github.com)](https://github.com/pikvm/pikvm)
