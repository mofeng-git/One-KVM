<h3 align=center><img src="https://github.com/mofeng-git/Build-Armbian/assets/62919083/add9743a-0987-4e8a-b2cb-62121f236582" alt="logo" width="300"><br></h3>
<h3 align=center><a href="https://github.com/mofeng-git/One-KVM/blob/master/README.md">简体中文</a> </h3>
<p align=right>&nbsp;</p>

### 介绍

One-KVM 是基于廉价计算机硬件和 [PiKVM]((https://github.com/pikvm/pikvm)) 软件二次开发的 BIOS 级远程控制项目。可以实现远程管理服务器或工作站，无需在被控机安装软件调整设置，实现无侵入式控制，适用范围广泛。

演示网站：[https://kvmd-demo.mofeng.run](https://kvmd-demo.mofeng.run)

![image-20240926220156381](https://github.com/user-attachments/assets/a7848bca-e43c-434e-b812-27a45fad7910)


### 快速开始

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

如果使用 CH9329，可以使用如下部署命令：
```bash
sudo docker run --name kvmd -itd \
    --device /dev/video0:/dev/video0 \
    --device /dev/ttyUSB0:/dev/ttyUSB0 \
    -p 8080:8080 -p 4430:4430 -p 5900:5900 -p 623:623 \
    silentwind0/kvmd
```

部署完成访问 https://IP:4430 ,点击信任自签证书，即可开始使用，默认账号密码：admin/admin。

如无法访问可以使用 `sudo docker logs kvmd` 命令查看日志尝试修复、提交 issue 或在 QQ 群内寻求帮助。

详细内容可以查阅 [One-KVM文档](https://one-kvm.mofeng.run/)。

**方式二：直刷 One-KVM 镜像**

对于玩客云设备，本项目 Releases 页可以找到适配玩客云的 One-KVM 预编译镜像。镜像名称带 One-KVM 前缀、burn 后缀的为线刷镜像，可使用 USB_Burning_Tool 软件线刷至玩客云。预编译线刷镜像为开箱即用，刷好后启动设备就可以开始使用 One-KVM。


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

- 爱发电用户_399fc

- [斐斐の](https://www.mmuaa.com/)

......
</details>

本项目使用了下列开源项目：
1. [pikvm/pikvm: Open and inexpensive DIY IP-KVM based on Raspberry Pi (github.com)](https://github.com/pikvm/pikvm)

**状态**

[![Star History Chart](https://api.star-history.com/svg?repos=mofeng-git/One-KVM&type=Date)](https://star-history.com/#mofeng-git/One-KVM&Date)

![Github](https://repobeats.axiom.co/api/embed/7cfaab47e31073107771a7179078aa2a6c3f1108.svg "Repobeats analytics image")


