#!/bin/bash

#File List
#src
#└── image
#    ├── cumebox2
#    │   └── Armbian_24.8.1_Khadas-vim1_bookworm_current_6.6.47_minimal.img
#    └── onecloud
#        ├── AmlImg_v0.3.1_linux_amd64
#        ├── Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal.burn.img
#        └── rc.local

#预处理镜像文件
SRCPATH=../src
ROOTFS=/tmp/rootfs
$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 unpack $SRCPATH/image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal.burn.img $SRCPATH/tmp
simg2img $SRCPATH/tmp/7.rootfs.PARTITION.sparse $SRCPATH/tmp/rootfs.img
dd if=/dev/zero of=/tmp/add.img bs=1M count=1024 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
e2fsck -f $SRCPATH/tmp/rootfs.img && resize2fs $SRCPATH/tmp/rootfs.img

#挂载镜像文件
mkdir $ROOTFS
sudo mount $SRCPATH/tmp/rootfs.img $ROOTFS || exit -1
sudo mount -t proc proc $ROOTFS/proc || exit -1
sudo mount -t sysfs sys $ROOTFS/sys || exit -1
sudo mount -o bind /dev $ROOTFS/dev || exit -1

#准备文件
sudo mkdir -p $ROOTFS/etc/kvmd/override.d $ROOTFS/etc/kvmd/vnc $ROOTFS/var/lib/kvmd/msd $ROOTFS/opt/vc/bin $ROOTFS/usr/share/kvmd \
    $ROOTFS/usr/share/janus/javascript $ROOTFS/usr/lib/ustreamer/janus $ROOTFS/run/kvmd
sudo cp -r ../One-KVM $ROOTFS/
sudo cp $SRCPATH/image/onecloud/rc.local $ROOTFS/etc/
sudo cp -r $ROOTFS/One-KVM/configs/kvmd/* $ROOTFS/One-KVM/configs/nginx $ROOTFS/One-KVM/configs/janus \
    $ROOTFS/etc/kvmd
sudo cp -r $ROOTFS/One-KVM/web $ROOTFS/One-KVM/extras $ROOTFS/One-KVM/contrib/keymaps $ROOTFS/usr/share/kvmd
sudo cp $ROOTFS/One-KVM/build/platform/onecloud $ROOTFS/usr/share/kvmd/platform
sudo cp $ROOTFS/One-KVM/testenv/fakes/vcgencmd $ROOTFS/usr/bin/
sudo cp -r $ROOTFS/One-KVM/testenv/js/* $ROOTFS/usr/share/janus/javascript/

#安装依赖
sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    apt update \
    && apt install -y python3-aiofiles python3-aiohttp python3-appdirs python3-asn1crypto python3-async-timeout \
        python3-bottle python3-cffi python3-chardet python3-click python3-colorama python3-cryptography python3-dateutil \
        python3-dbus python3-dev python3-hidapi python3-idna python3-libgpiod python3-mako python3-marshmallow python3-more-itertools \
        python3-multidict python3-netifaces python3-packaging python3-passlib python3-pillow python3-ply python3-psutil \
        python3-pycparser python3-pyelftools python3-pyghmi python3-pygments python3-pyparsing python3-requests \
        python3-semantic-version python3-setproctitle python3-setuptools python3-six python3-spidev python3-systemd \
        python3-tabulate python3-urllib3 python3-wrapt python3-xlib python3-yaml python3-yarl python3-pyotp python3-qrcode \
        python3-serial python3-zstandard python3-dbus-next \
    && apt install -y nginx python3-pip python3-dev python3-build net-tools tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim \
        git gpiod libxkbcommon0 build-essential janus-dev libssl-dev libffi-dev libevent-dev libjpeg-dev libbsd-dev libudev-dev \
        pkg-config libx264-dev libyuv-dev libasound2-dev libsndfile-dev libspeexdsp-dev cpufrequtils iptables\
    && apt clean "

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    pip3 config set global.index-url https://mirrors.tuna.tsinghua.edu.cn/pypi/web/simple \
    && pip3 install --target=/usr/lib/python3/dist-packages --break-system-packages async-lru gpiod \
    && pip3 cache purge "

sudo chroot --userspec "root:root" $ROOTFS sed --in-place --expression 's|^#include "refcount.h"$|#include "../refcount.h"|g' /usr/include/janus/plugins/plugin.h

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    git clone --depth=1 https://github.com/mofeng-git/ustreamer /tmp/ustreamer \
    && make -j WITH_PYTHON=1 WITH_JANUS=1 WITH_LIBX264=1 -C /tmp/ustreamer \
    && mv /tmp/ustreamer/src/ustreamer.bin /usr/bin/ustreamer \
    && mv /tmp/ustreamer/src/ustreamer-dump.bin /usr/bin/ustreamer-dump \
    && chmod +x /usr/bin/ustreamer /usr/bin/ustreamer-dump \
    && mv /tmp/ustreamer/janus/libjanus_ustreamer.so /usr/lib/ustreamer/janus \
    && pip3 install --target=/usr/lib/python3/dist-packages --break-system-packages /tmp/ustreamer/python/dist/*.whl "

#安装 kvmd 主程序
sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    cd /One-KVM \
    && python3 setup.py install \
    && bash scripts/kvmd-gencert --do-the-thing \
    && bash scripts/kvmd-gencert --do-the-thing --vnc \
    && kvmd-nginx-mkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf \
    && kvmd -m "

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.armhf -L -o /usr/bin/ttyd \
    && chmod +x /usr/bin/ttyd \
    && systemd-sysusers /One-KVM/configs/os/kvmd-webterm.conf \
    &&  mkdir -p /home/kvmd-webterm \
    && chown kvmd-webterm /home/kvmd-webterm "


#服务自启
sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    cat /One-KVM/configs/os/sudoers/v2-hdmiusb >> /etc/sudoers \
    && cat /One-KVM/configs/os/udev/v2-hdmiusb-generic.rules > /etc/udev/rules.d/99-kvmd.rules \
    && echo 'libcomposite' >> /etc/modules \
    && mv /usr/local/bin/kvmd* /usr/bin \
    && cp /One-KVM/configs/os/services/* /etc/systemd/system/ \
    && cp /One-KVM/configs/os/tmpfiles.conf /usr/lib/tmpfiles.d/ \
    && chmod +x /etc/update-motd.d/* \
    && echo 'kvmd ALL=\(ALL\) NOPASSWD: /etc/kvmd/custom_atx/gpio.sh' >>  /etc/sudoers \
    && echo 'kvmd ALL=\(ALL\) NOPASSWD: /etc/kvmd/custom_atx/usbrelay_hid.sh' >>  /etc/sudoers \
    && systemd-sysusers /One-KVM/configs/os/sysusers.conf \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
    && sed -i 's/ch9329/otg/g' /etc/kvmd/override.yaml \
    && sed -i 's/device: \/dev\/ttyUSB0//g' /etc/kvmd/override.yaml \
    && sed -i 's/8080/80/g' /etc/kvmd/override.yaml \
    && sed -i 's/4430/443/g' /etc/kvmd/override.yaml \
	&& sed -i 's/localhost.localdomain/onecloud/g' /etc/kvmd/meta.yaml \
    && systemctl enable kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus \
    && systemctl disable nginx janus \
    && rm -r /One-KVM "


sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    sed -i '2c ATX=GPIO' /etc/kvmd/atx.sh \
    && sed -i 's/SHUTDOWNPIN/gpiochip1 7/g' /etc/kvmd/custom_atx/gpio.sh \
    && sed -i 's/REBOOTPIN/gpiochip0 11/g' /etc/kvmd/custom_atx/gpio.sh "

#卸载镜像
sudo umount  $ROOTFS/sys
sudo umount  $ROOTFS/dev
sudo umount  $ROOTFS/proc
sudo umount $ROOTFS

#打包镜像
sudo rm $SRCPATH/tmp/7.rootfs.PARTITION.sparse
sudo img2simg $SRCPATH/tmp/rootfs.img $SRCPATH/tmp/7.rootfs.PARTITION.sparse
sudo $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 pack  $SRCPATH/output/One-KVM_by-SilentWind_Onecloud_241004.burn.img $SRCPATH/tmp/
sudo rm $SRCPATH/tmp/*