# KVMD-ARMBIAN
This project supports non-Raspberry Pi device to run pikvm on armbian or Raspberry Pi boards on Raspbian Bookworm
- It also supports x86 pikvm with USB uart + ch9329 serial HID if you use the install-x86.sh script and follow the How to Install PiKVM x86.pdf
- As of 04/21/2024, I have left the PiKVM Discord so if you need assistance, please email me:  srepac@kvmnerds.com
- You can also try and see if I'm on my own Discord at https://discord.gg/YaJ87sVznc

# Install
KVMD Install for armbian/raspbian

It supports Allwinner, Amlogic and Rockchip based tv box, tested on phicomm n1, mxq pro 4k, tqc a01. 
Chipset needs to support USB OTG feature, lots of old amglogic chipset does not support otg feature, such as s805 and s905.
You should install armbian with debian jammy as jammy has python 3.10 or python 3.11 (starting with kvmd 3.217) which the newer kvmd code is based on.
Then run this script (2x) to install pikvm.

This also works on rpi boards running Raspbian Bookworm.

Install script is a fork from @srepac raspbian pikvm install script.

**NOTE:  all commands need to be run as root user.**

Original Script [http://148.135.104.55/RPiKVM/install-pikvm-raspbian.sh]

# Hardware for kvmd-armbian project
* A tv box/arm board that supports otg feature:
    - Tests on phicomm n1(Amlogic s905d), mxq pro 4k (rk322x), tqc a01(Allwinner H6). 
    - If you are using arm board you can remove gpio patch to enable gpio feature.
* Video capture device:
    - HDMI to USB dongle (30 RMB On taobao, 10$ on aliexpress.) or USB HDMI loop capture dongle
    - cheap hdmi to usb dongle all use physics USB2.0 port, but fake USB3.0(USB 5GBPS, USB3.2GEN1) version supports 720P 60FPS,
      usb 2.0 version only supports 720P 30FPS.
* USB-A to USB-A cable or micro-USB to USB-A cable:
    - Recommended to cut off usb cable's power line, otherwise it might cause otg disconnect.

## Step 1
- Flash armbian debian [Recommended jammy] for your tv box (If kernel not support otg you should build a kernel enable otg features)
- Flash armbian debian jammy for all others

## Step 2 (skip this step if running on Orange Pi zero and one boards)
- Modify your dtb file to enable otg feature. Change dr_mode from host to peripheral for otg usb port.
- If you use rk322x (rk3228A rk3228B rk3229) series chipset, you can use dtb/4.4/rk332x-box.dtb 
- Add the following to your /boot/armbianEnv.txt file
```
overlays=usbhost0 usbhost1 usbhost2 usbhost3
```

**NOTE:  Skip steps 1 and 2 if you are running raspbian bookworm on rpi boards**
## Step 3 - Perform part 1 of install
```
apt update && apt upgrade -y
apt install -y git vim make python3-dev gcc
git clone https://github.com/srepac/kvmd-armbian.git
cd kvmd-armbian
./install.sh
```
This will ask you to press ENTER to reboot after part 1 completes.

## Step 4 - Perform part 2 of install
- run install.sh again after reboot os to perform part 2 of install.  **NOTE:  May require one more reboot if missing /dev/kvmd-hid-[keyboard|mouse]**
```
cd kvmd-armbian
./install.sh
```
- Enjoy


# Updating RPiKVM
Update armbian pikvm anytime in order to take advantage of new features/updates.

- run the update-rpikvm.sh script to perform update
```
wget -O /usr/local/bin/update-rpikvm.sh http://148.135.104.55/RPiKVM/update-rpikvm.sh
update-rpikvm.sh
```


# Tested device
 - Phicomm N1
 - TQC A01 (Ethernet port not working, only support wireless.)
 - RK322x based tvbox (MXQ, V88)
 - S905L2 based tvbox
 - Orange pi zero (tested by @MrSuicideParrot)
 - Orange Pi Zero and One (tested by @srepac)
 - Nano Pi Neo, Rock64, and Orange Pi Zero Plus (tested by @srepac)
 - Libre Computer Le Potato, La Frite 1GB, Renegade ROC-RK3328-CC and ALL-H3-CC H5 2GB (tested by @srepac)
 - Inovato Quadra tv box (tested by @srepac)
 - Big Tree Tech CB1+Rpi4 board (WIP by @srepac)
 - Orange Pi PC+ and orange pi 3 (tested by @ducs4rs)
 - RPi4B + PiKVM V3 HAT on Raspberry Pi OS bookworm (tested by @srepac on 10/19/23)
 - RPi4B + BliKVM V3 HAT on Raspberry Pi OS bookworm (tested by @srepac)
 - RPiCM4 + BliKVM V2 pcie HAT on Raspberry Pi OS bookworm (tested by @srepac)
 - RPiCM4 + Geekworm A8 pcie HAT on Raspberry Pi OS bookworm (tested by @srepac)
 - RPiCM4 + Geekworm X650 pcie HAT on Raspberry Pi OS bookworm (tested by @srepac)
 - RPiCM4 + Geekworm X635 HAT on Raspberry Pi OS bookworm (tested by @srepac)
