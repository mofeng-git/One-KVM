# One-KVM
### 介绍

One-KVM是基于玩客云硬件和PiKVM软件的远控设备。通过移植PiKVM该软件至玩客云设备上，实现了极高的性价比，不到百元功能即可接近甚至超越部分昂贵的商业设备。

该设备在于帮助用户通过得到控制设备的HDMI 画面和鼠标键盘，去远程管理服务器、工作站或个人PC等。 和基于软件的远程管理方式不同，你无需在被控电脑安装任何软件，做到无侵入式控制。

![image](https://github.com/mofeng-git/One-KVM/assets/62919083/ec7e049f-ca6c-426f-bfa4-314536965db0)

### 功能特性

![image](https://github.com/mofeng-git/One-KVM/assets/62919083/1e9305ee-fd9e-4e4c-ba25-141a924fef29)

### 安装教程
该脚本在玩客云（新旧版，[Armbian 22.11.0-trunk Jammy Linux onecloud 5.10.149-meson]( https://github.com/hzyitc/armbian-onecloud/releases/download/ci-20221026-074131-UTC/Armbian_22.11.0-trunk_Onecloud_jammy_legacy_5.10.149.burn.img.xz)系统）上运行，请确保你的设备已安装好Armbian系统。

#### 快速开始
```bash
git clone https://github.com/mofeng-git/One-KVM.git
cd One-KVM  && ./install.sh
```

对于国内网络环境，可以以下命令
```bash
wget https://ghproxy.net/https://github.com/mofeng-git/One-KVM/archive/refs/heads/main.zip -o One-KVM-main.zip
unzip One-KVM-main.zip
cd One-KVM-main  && ./install.sh
```
详细教程请参照飞书文档：[One-KVM使用手册](https://p1b237lu9xm.feishu.cn/drive/folder/IsOifWmMKlzYpRdWfcocI7jdnQA?from=from_copylink)

### 更新日志
V0.5：通过锁定CPU频率修复ustreamer mjpeg视频流异常的问题；屏蔽主程序找不到温度传感器的报错；优化中文翻译；优化安装流程。
V0.4：利用玩客云自动GPIO实现ATX开关机物理控制功能；初步建立飞书使用文档；制作一键安装脚本，优化安装流程。
V0.3：添加简体中文补丁；实现MSD功能在EMMC和TF卡上的使用；添加WOL和中文OCR功能；优化了安装流程。
V0.2：通过替换系统解决OTG拔插死机问题；初步实现MSD功能；修改启动分区解决开机卡线刷检测；优化安装流程。
V0.1：PiKVM在玩客云上初步运行。
### 感谢
H_xiaoming测试适配OTG正常可用镜像、0蓝蓝0提供开机卡线刷检测解决办法、fairybl关于MSD和线刷检测的其他解决方案、Will的PiKVM测试、浩龙的电子嵌入式之路的充电，各位网友的讨论交流和下列开源项目。
1. [pikvm/pikvm: Open and inexpensive DIY IP-KVM based on Raspberry Pi (github.com)](https://github.com/pikvm/pikvm)
2. [hzyitc/armbian-onecloud: Armbian for onecloud. 玩客云用armbian (github.com)](https://github.com/hzyitc/armbian-onecloud/)
3. [jacobbar/fruity-pikvm: Install Pi-KVM on debian SBCs such as Orange Pi, Banana Pi, Mango Pi, etc (github.com)](https://github.com/jacobbar/fruity-pikvm)

   