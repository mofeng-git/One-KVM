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


SRCPATH=../src
ROOTFS=/tmp/rootfs
LOOPDEV=/dev/loop10
DATE=241004
export LC_ALL=C

mount_onecloud_rootfs() {
    $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 unpack $SRCPATH/image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal.burn.img $SRCPATH/tmp
    simg2img $SRCPATH/tmp/7.rootfs.PARTITION.sparse $SRCPATH/tmp/rootfs.img
    dd if=/dev/zero of=/tmp/add.img bs=1M count=1024 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    e2fsck -f $SRCPATH/tmp/rootfs.img && resize2fs $SRCPATH/tmp/rootfs.img

    mkdir $ROOTFS
    sudo mount $SRCPATH/tmp/rootfs.img $ROOTFS || exit -1
    sudo mount -t proc proc $ROOTFS/proc || exit -1
    sudo mount -t sysfs sys $ROOTFS/sys || exit -1
    sudo mount -o bind /dev $ROOTFS/dev || exit -1
}

mount_cumebox2_rootfs() {
    cp $SRCPATH/image/cumebox2/Armbian_24.8.1_Khadas-vim1_bookworm_current_6.6.47_minimal.img $SRCPATH/tmp/rootfs.img
    dd if=/dev/zero of=/tmp/add.img bs=1M count=1280 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    sudo parted -s $SRCPATH/tmp/rootfs.img resizepart 1 100% || exit -1
    sudo losetup --offset 4194304 $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    #sudo mount -o loop,offset=$((8192*512))  $SRCPATH/tmp/rootfs.img $ROOTFS
    sudo e2fsck -f $LOOPDEV && sudo resize2fs $LOOPDEV

    mkdir $ROOTFS
    sudo mount $LOOPDEV  $ROOTFS || exit -1
    sudo mount -t proc proc $ROOTFS/proc || exit -1
    sudo mount -t sysfs sys $ROOTFS/sys || exit -1
    sudo mount -o bind /dev $ROOTFS/dev || exit -1
}

umount_onecloud_rootfs() {
    sudo umount  $ROOTFS/sys
    sudo umount  $ROOTFS/dev
    sudo umount  $ROOTFS/proc
    sudo umount $ROOTFS
}

umount_cumebox2_rootfs() {
    sudo umount  $ROOTFS/sys
    sudo umount  $ROOTFS/dev
    sudo umount  $ROOTFS/proc
    sudo umount $ROOTFS
    sudo losetup -d $LOOPDEV  
}

config_file() {
    sudo mkdir -p $ROOTFS/etc/kvmd/override.d $ROOTFS/etc/kvmd/vnc $ROOTFS/var/lib/kvmd/msd $ROOTFS/opt/vc/bin $ROOTFS/usr/share/kvmd \
        $ROOTFS/usr/share/janus/javascript $ROOTFS/usr/lib/ustreamer/janus $ROOTFS/run/kvmd $ROOTFS/var/lib/kvmd/msd/images $ROOTFS/var/lib/kvmd/msd/meta
    sudo cp -r ../One-KVM $ROOTFS/
    sudo cp -r $ROOTFS/One-KVM/configs/kvmd/* $ROOTFS/One-KVM/configs/nginx $ROOTFS/One-KVM/configs/janus \
        $ROOTFS/etc/kvmd
    sudo cp -r $ROOTFS/One-KVM/web $ROOTFS/One-KVM/extras $ROOTFS/One-KVM/contrib/keymaps $ROOTFS/usr/share/kvmd
    sudo cp $ROOTFS/One-KVM/testenv/fakes/vcgencmd $ROOTFS/usr/bin/
    sudo cp -r $ROOTFS/One-KVM/testenv/js/* $ROOTFS/usr/share/janus/javascript/
}

config_onecloud_file() {
    sudo cp $SRCPATH/image/onecloud/rc.local $ROOTFS/etc/
    sudo cp $ROOTFS/One-KVM/build/platform/onecloud $ROOTFS/usr/share/kvmd/platform
}

config_cumebox2_file() {
    sudo cp $ROOTFS/One-KVM/build/platform/cumebox2 $ROOTFS/usr/share/kvmd/platform
}


instal_one-kvm() {
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


#服务自启
sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    cat /One-KVM/configs/os/sudoers/v2-hdmiusb >> /etc/sudoers \
    && cat /One-KVM/configs/os/udev/v2-hdmiusb-generic.rules > /etc/udev/rules.d/99-kvmd.rules \
    && echo 'libcomposite' >> /etc/modules \
    && mv /usr/local/bin/kvmd* /usr/bin \
    && cp /One-KVM/configs/os/services/* /etc/systemd/system/ \
    && cp /One-KVM/configs/os/tmpfiles.conf /usr/lib/tmpfiles.d/ \
    && chmod +x /etc/update-motd.d/* \
    && echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/gpio.sh' >>  /etc/sudoers \
    && echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/usbrelay_hid.sh' >>  /etc/sudoers \
    && systemd-sysusers /One-KVM/configs/os/sysusers.conf \
    && systemd-sysusers /One-KVM/configs/os/kvmd-webterm.conf \
    && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
    && sed -i 's/ch9329/otg/g' /etc/kvmd/override.yaml \
    && sed -i 's/device: \/dev\/ttyUSB0//g' /etc/kvmd/override.yaml \
    && sed -i 's/8080/80/g' /etc/kvmd/override.yaml \
    && sed -i 's/4430/443/g' /etc/kvmd/override.yaml \
    && sed -i 's/#type: otg/type: otg/g' /etc/kvmd/override.yaml \
    && chown kvmd -R /var/lib/kvmd/msd/ \
	&& sed -i 's/localhost.localdomain/onecloud/g' /etc/kvmd/meta.yaml \
    && systemctl enable kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus \
    && systemctl disable nginx janus \
    && rm -r /One-KVM "

}


instal_one-kvm_for_onecloud() {

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.armhf -L -o /usr/bin/ttyd \
    && chmod +x /usr/bin/ttyd \
    && mkdir -p /home/kvmd-webterm \
    && chown kvmd-webterm /home/kvmd-webterm "

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    sed -i '2c ATX=GPIO' /etc/kvmd/atx.sh \
    && sed -i 's/SHUTDOWNPIN/gpiochip1 7/g' /etc/kvmd/custom_atx/gpio.sh \
    && sed -i 's/REBOOTPIN/gpiochip0 11/g' /etc/kvmd/custom_atx/gpio.sh "

}

instal_one-kvm_for_cumebox2() {

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.aarch64 -L -o /usr/bin/ttyd \
    && chmod +x /usr/bin/ttyd \
    && mkdir -p /home/kvmd-webterm \
    && chown kvmd-webterm /home/kvmd-webterm "

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    sed -i '2c ATX=USBRELAY_HID' /etc/kvmd/atx.sh \
    && sed -i 's/\/dev\/video0/\/dev\/video1/g' /etc/kvmd/override.yaml "

}

pack_img_onecloud() {
    sudo rm $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo img2simg $SRCPATH/tmp/rootfs.img $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 pack  $SRCPATH/output/One-KVM_by-SilentWind_Onecloud_$DATE.burn.img $SRCPATH/tmp/
    sudo rm $SRCPATH/tmp/*
}

pack_img_cumebox2() {
    sudo mv $SRCPATH/tmp/rootfs.img   $SRCPATH/output/One-KVM_by-SilentWind_Cumebox2_$DATE.burn.img
}



parpare_install_cumebox2() {

sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    mkdir -p /run/systemd/resolve/ \
    && touch /run/systemd/resolve/stub-resolv.conf \
    && printf '%s\n' 'nameserver 1.1.1.1' 'nameserver 1.0.0.1' > /etc/resolv.conf \
    && bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) \
        --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http "

}



case $1 in  
    onecloud)  
        mount_onecloud_rootfs
        config_file
        config_onecloud_file
        instal_one-kvm
        instal_one-kvm_for_onecloud
        umount_onecloud_rootfs
        pack_img_onecloud
        ;;  
    cumebox2)  
        mount_cumebox2_rootfs
        config_file
        config_cumebox2_file
        parpare_install_cumebox2
        instal_one-kvm
        instal_one-kvm_for_cumebox2
        umount_cumebox2_rootfs
        pack_img_cumebox2
        ;;  
    *)  
        echo "Do no thing." 
        ;;  
esac


