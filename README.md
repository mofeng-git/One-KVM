<h3 align=center><img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="logo" width="300"><br></h3>
<h3 align=center><a href="https://github.com/mofeng-git/One-KVM/blob/master/README.md">简体中文</a> </h3>
<p align=right>&nbsp;</p>

### 介绍

One-KVM 是基于廉价计算机硬件和 [PiKVM]((https://github.com/pikvm/pikvm)) 软件二次开发的 BIOS 级远程控制项目。可以实现远程管理服务器或工作站，无需在被控机安装软件调整设置，实现无侵入式控制，适用范围广泛。

使用文档：[https://one-kvm.mofeng.run](https://one-kvm.mofeng.run)

演示网站：[https://kvmd-demo.mofeng.run](https://kvmd-demo.mofeng.run)

![image-20240926220156381](https://github.com/user-attachments/assets/a7848bca-e43c-434e-b812-27a45fad7910)

### 软件功能

表格仅为 One-KVM 与其他基于 PiKVM 的项目的功能对比，无不良导向，如有错漏请联系更正。

|         功能          |     One-KVM     |           PiKVM           |   ArmKVM    |   BLIKVM    |
| :-------------------: | :-------------: | :-----------------------: | :---------: | :---------: |
|       系统开源        |        √        |             √             |      √      |      √      |
|    简体中文 WebUI     |        √        |             x             |      √      |      √      |
|      远程视频流       |   MJPEG/H.264   |        MJPEG/H.264        | MJPEG/H.264 | MJPEG/H.264 |
|    H.264 视频编码     |       CPU       |            GPU            |    未知     |     GPU     |
|      远程音频流       |        √        |             √             |      √      |      √      |
|   远程鼠键控制        |   OTG/CH9329    | OTG/CH9329/Pico/Bluetooth |     OTG     |     OTG     |
|       VNC 控制        |        √        |             √             |      √      |      √      |
|     ATX 电源控制      | GPIO/USB 继电器 |           GPIO            |    GPIO     |    GPIO     |
| 虚拟存储驱动器挂载     |        √        |             √             |      √      |      √      |
| 2.2G 以上 CD-ROM 挂载 |        x        |             x             |      √      |      √      |
|     WOL 远程唤醒      |        √        |             √             |      √      |      √      |
|      网页剪切板       |        √        |             √             |      √      |      √      |
|     OCR 文字识别      |        √        |             √             |      √      |      √      |
|       网页终端        |        √        |             √             |      √      |      √      |
|     网络串口终端      |        x        |             x             |      √      |      √      |
|    HDMI 切换器支持    |        √        |             √             |      √      |      √      |
|       视频录制        |        √        |             x             |      x      |      x      |
|      Docker 部署      |        √        |             x             |      x      |      x      |
|    官方商业化成品     |        x        |             √             |      √      |      √      |
|       技术支持        |        √        |             √             |      √      |      √      |

### 快速开始

更多详细内容可以查阅 [One-KVM文档](https://one-kvm.mofeng.run/)。

**方式一：Docker 镜像部署（推荐）**

Docker 版本可以使用 OTG 或 CH9329 作为虚拟 HID ，支持 amd64、arm64、armv7 架构的 Linux 系统安装。


如果使用 OTG 作为虚拟 HID，可以使用如下部署命令：
```bash
sudo docker run --name kvmd -itd --privileged=true \
    -v /lib/modules:/lib/modules:ro -v /dev:/dev \
    -v /sys/kernel/config:/sys/kernel/config -e OTG=1 \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    silentwind0/kvmd
```

如果使用 CH9329 作为虚拟 HID，可以使用如下部署命令：
```bash
sudo docker run --name kvmd -itd \
    --device /dev/video0:/dev/video0 \
    --device /dev/ttyUSB0:/dev/ttyUSB0 \
    --device /dev/snd:/dev/snd \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    silentwind0/kvmd
```

**方式二：直刷 One-KVM 整合包**

对于部分平台硬件，本项目制作了深度适配的 One-KVM 打包镜像，开箱即用，刷好后启动设备就可以开始使用 One-KVM。免费 One-KVM 整合包也可以在本项目 Releases 页可以找到。

| 整合包适配概况 | | | |
| :-------------: | :-------------: | :-------------: | :-------------: |
| **固件型号** | **固件代号** | **硬件情况** | **最新版本** |
| 玩客云 | Onecloud | USB 采集卡、OTG | 241018 |
| 私家云二代 | Cumebox2 | USB 采集卡、OTG | 241004 |
| Vmare | Vmare-uefi | USB 采集卡、CH9329 | 241004 |
| Virtualbox | Virtualbox-uefi | USB 采集卡、CH9329 | 241004 |
| s905l3a  通用包 | E900v22c | USB 采集卡、OTG | 241004 |
| 我家云 | Chainedbox | USB 采集卡、OTG | 241004 |
| 龙芯久久派 | 2k0300 | USB 采集卡、CH9329 | 241025 |

### 赞助方式

这个项目基于众多开源项目二次开发，作者为此花费了大量的时间和精力进行测试和维护。若此项目对您有用，您可以考虑通过 **[为爱发电](https://afdian.com/a/silentwind)** 赞助一笔小钱支持作者。作者将能有更多的金钱来测试和维护 One-KVM 的各种配置，并在项目上投入更多的时间和精力。

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

[远方](https://runyf.cn/)（闲鱼用户名：小远技术店铺）

爱发电用户_399fc

[斐斐の](https://www.mmuaa.com/)

爱发电用户_09451

超高校级的錆鱼

爱发电用户_08cff

guoke

mgt

......
</details>

本项目使用了下列开源项目：
1. [pikvm/pikvm: Open and inexpensive DIY IP-KVM based on Raspberry Pi (github.com)](https://github.com/pikvm/pikvm)

### 项目状态

[![Star History Chart](https://api.star-history.com/svg?repos=mofeng-git/One-KVM&type=Date)](https://star-history.com/#mofeng-git/One-KVM&Date)

![Github](https://repobeats.axiom.co/api/embed/7cfaab47e31073107771a7179078aa2a6c3f1108.svg "Repobeats analytics image")


