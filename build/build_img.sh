#!/bin/bash
# ========================================================================== #
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>         #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
# ========================================================================== #

SRCPATH=/mnt/nas/src
BOOTFS=/tmp/bootfs
ROOTFS=/tmp/rootfs
OUTPUTDIR=/mnt/nas/src/output
LOOPDEV=/dev/loop10
DATE=241204
export LC_ALL=C

write_meta() {
    sudo chroot --userspec "root:root" $ROOTFS bash -c "sed -i 's/localhost.localdomain/$1/g' /etc/kvmd/meta.yaml"
}

mount_rootfs() {
    mkdir $ROOTFS
    sudo mount $LOOPDEV  $ROOTFS || exit -1
    sudo mount -t proc proc $ROOTFS/proc || exit -1
    sudo mount -t sysfs sys $ROOTFS/sys || exit -1
    sudo mount -o bind /dev $ROOTFS/dev || exit -1
}

umount_rootfs() {
    sudo umount  $ROOTFS/sys
    sudo umount  $ROOTFS/dev
    sudo umount  $ROOTFS/proc
    sudo umount $ROOTFS
    sudo losetup -d $LOOPDEV  
}

parpare_dns() {
    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
    mkdir -p /run/systemd/resolve/ \
    && touch /run/systemd/resolve/stub-resolv.conf \
    && printf '%s\n' 'nameserver 1.1.1.1' 'nameserver 1.0.0.1' > /etc/resolv.conf \
    && bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) \
        --source mirrors.tuna.tsinghua.edu.cn --updata-software false --web-protocol http "
}

delete_armbain_verify(){
    sudo chroot --userspec "root:root" $ROOTFS bash -c "echo 'deb  http://mirrors.ustc.edu.cn/armbian bullseye main bullseye-utils bullseye-desktop' > /etc/apt/sources.list.d/armbian.list "
}

config_file() {
    sudo mkdir -p $ROOTFS/etc/kvmd/override.d $ROOTFS/etc/kvmd/vnc $ROOTFS/var/lib/kvmd/msd $ROOTFS/opt/vc/bin $ROOTFS/usr/share/kvmd $ROOTFS/One-KVM \
        $ROOTFS/usr/share/janus/javascript $ROOTFS/usr/lib/ustreamer/janus $ROOTFS/run/kvmd $ROOTFS/var/lib/kvmd/msd/images $ROOTFS/var/lib/kvmd/msd/meta
    sudo rsync -a  --exclude={src,.github} . $ROOTFS/One-KVM
    sudo cp -r configs/kvmd/* configs/nginx configs/janus $ROOTFS/etc/kvmd
    sudo cp -r web extras contrib/keymaps $ROOTFS/usr/share/kvmd
    sudo cp testenv/fakes/vcgencmd $ROOTFS/usr/bin/
    sudo cp -r testenv/js/* $ROOTFS/usr/share/janus/javascript/
    sudo cp build/platform/$1 $ROOTFS/usr/share/kvmd/platform
    if [ -f "$SRCPATH/image/$1/rc.local" ]; then
        sudo cp $SRCPATH/image/$1/rc.local $ROOTFS/etc/
    fi
}

pack_img() {
    sudo mv $SRCPATH/tmp/rootfs.img  $OUTPUTDIR/One-KVM_by-SilentWind_$1_$DATE.img
    if [ "$1" = "Vm" ]; then
        sudo qemu-img convert -f raw -O vmdk $OUTPUTDIR/One-KVM_by-SilentWind_Vm_$DATE.img $OUTPUTDIR/One-KVM_by-SilentWind_Vmare-uefi_$DATE.vmdk
        sudo qemu-img convert -f raw -O vdi $OUTPUTDIR/One-KVM_by-SilentWind_Vm_$DATE.img $OUTPUTDIR/One-KVM_by-SilentWind_Virtualbox-uefi_$DATE.vdi
    fi
}

onecloud_rootfs() {
    $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 unpack $SRCPATH/image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal.burn.img $SRCPATH/tmp
    simg2img $SRCPATH/tmp/6.boot.PARTITION.sparse $SRCPATH/tmp/bootfs.img
    simg2img $SRCPATH/tmp/7.rootfs.PARTITION.sparse $SRCPATH/tmp/rootfs.img
    mkdir $BOOTFS
    sudo losetup $LOOPDEV $SRCPATH/tmp/bootfs.img  || exit -1
    sudo mount $LOOPDEV $BOOTFS
    sudo cp $SRCPATH/image/onecloud/meson8b-onecloud-fix.dtb $BOOTFS/dtb/meson8b-onecloud.dtb
    sudo umount $BOOTFS
    sudo losetup -d $LOOPDEV
    dd if=/dev/zero of=/tmp/add.img bs=1M count=1024 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    e2fsck -f $SRCPATH/tmp/rootfs.img && resize2fs $SRCPATH/tmp/rootfs.img
    sudo losetup $LOOPDEV $SRCPATH/tmp/rootfs.img
}

cumebox2_rootfs() {
    cp $SRCPATH/image/cumebox2/Armbian_24.8.1_Khadas-vim1_bookworm_current_6.6.47_minimal.img $SRCPATH/tmp/rootfs.img
    dd if=/dev/zero of=/tmp/add.img bs=1M count=1500 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    sudo parted -s $SRCPATH/tmp/rootfs.img resizepart 1 100% || exit -1
    sudo losetup --offset $((8192*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    sudo e2fsck -f $LOOPDEV && sudo resize2fs $LOOPDEV
}

chainedbox_rootfs_and_fix_dtb() {
    cp $SRCPATH/image/chainedbox/Armbian_24.11.0_rockchip_chainedbox_bookworm_6.1.112_server_2024.10.02_add800m.img $SRCPATH/tmp/rootfs.img
    mkdir $BOOTFS
    sudo losetup --offset $((32768*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    sudo mount $LOOPDEV $BOOTFS
    sudo cp $SRCPATH/image/chainedbox/rk3328-l1pro-1296mhz-fix.dtb $BOOTFS/dtb/rockchip/rk3328-l1pro-1296mhz.dtb
    sudo umount $BOOTFS
    sudo losetup -d $LOOPDEV
    sudo losetup --offset $((1081344*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img
}

vm_rootfs() {
    cp $SRCPATH/image/vm/Armbian_24.8.1_Uefi-x86_bookworm_current_6.6.47_minimal_add1g.img $SRCPATH/tmp/rootfs.img
    sudo losetup --offset $((540672*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
}

e900v22c_rootfs() {
    cp $SRCPATH/image/e900v22c/Armbian_23.08.0_amlogic_s905l3a_bookworm_5.15.123_server_2023.08.01.img $SRCPATH/tmp/rootfs.img
    dd if=/dev/zero of=/tmp/add.img bs=1M count=400 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    sudo parted -s $SRCPATH/tmp/rootfs.img resizepart 2 100% || exit -1
    sudo losetup --offset $((532480*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    sudo e2fsck -f $LOOPDEV && sudo resize2fs $LOOPDEV
}


octopus-flanet_rootfs() {
    cp $SRCPATH/image/octopus-flanet/Armbian_24.11.0_amlogic_s912_bookworm_6.1.114_server_2024.11.01.img $SRCPATH/tmp/rootfs.img
    mkdir $BOOTFS
    sudo losetup --offset $((8192*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    sudo mount $LOOPDEV $BOOTFS
    sudo sed -i "s/meson-gxm-octopus-planet.dtb/meson-gxm-khadas-vim2.dtb/g" $BOOTFS/uEnv.txt
    sudo umount $BOOTFS
    sudo losetup -d $LOOPDEV
    dd if=/dev/zero of=/tmp/add.img bs=1M count=400 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    sudo parted -s $SRCPATH/tmp/rootfs.img resizepart 2 100% || exit -1
    sudo losetup --offset $((1056768*512)) $LOOPDEV $SRCPATH/tmp/rootfs.img  || exit -1
    sudo e2fsck -f $LOOPDEV && sudo resize2fs $LOOPDEV
}


config_cumebox2_file() {
    sudo mkdir $ROOTFS/etc/oled
    sudo cp $SRCPATH/image/cumebox2/v-fix.dtb $ROOTFS/boot/dtb/amlogic/meson-gxl-s905x-khadas-vim.dtb
    sudo cp $SRCPATH/image/cumebox2/ssd $ROOTFS/usr/bin/
    sudo cp $SRCPATH/image/cumebox2/config.json $ROOTFS/etc/oled/config.json
}

config_octopus-flanet_file() {
    sudo cp $SRCPATH/image/octopus-flanet/model_database.conf $ROOTFS/etc/model_database.conf
}

instal_one-kvm() {
    #$1 arch; $2 deivce: "gpio" or "video1"; $3 network: "systemd-networkd",default is network-manager
    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        df -h \
        && apt-get update \
        && apt-get install -y python3-aiofiles python3-aiohttp python3-appdirs python3-asn1crypto python3-async-timeout \
            python3-bottle python3-cffi python3-chardet python3-click python3-colorama python3-cryptography python3-dateutil \
            python3-dbus python3-dev python3-hidapi python3-hid python3-idna python3-libgpiod python3-mako python3-marshmallow python3-more-itertools \
            python3-multidict python3-netifaces python3-packaging python3-passlib python3-pillow python3-ply python3-psutil \
            python3-pycparser python3-pyelftools python3-pyghmi python3-pygments python3-pyparsing python3-requests \
            python3-semantic-version python3-setproctitle python3-setuptools python3-six python3-spidev python3-systemd \
            python3-tabulate python3-urllib3 python3-wrapt python3-xlib python3-yaml python3-yarl python3-pyotp python3-qrcode \
            python3-serial python3-zstandard python3-dbus-next python3-pip python3-dev python3-build python3-wheel \
            nginx net-tools tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim cpufrequtils iptables network-manager \
            git gpiod libxkbcommon0 build-essential janus-dev libssl-dev libffi-dev libevent-dev libjpeg-dev libbsd-dev libudev-dev \
            pkg-config libx264-dev libyuv-dev libasound2-dev libsndfile-dev libspeexdsp-dev \
        && rm -rf /var/lib/apt/lists/* "

    sudo chroot --userspec "root:root" $ROOTFS sed --in-place --expression 's|^#include "refcount.h"$|#include "../refcount.h"|g' /usr/include/janus/plugins/plugin.h

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        git clone --depth=1 https://github.com/mofeng-git/ustreamer /tmp/ustreamer \
        && make -j WITH_PYTHON=1 WITH_JANUS=1 WITH_LIBX264=1 -C /tmp/ustreamer \
        && cp /tmp/ustreamer/src/ustreamer.bin /usr/bin/ustreamer \
        && cp /tmp/ustreamer/src/ustreamer-dump.bin /usr/bin/ustreamer-dump \
        && chmod +x /usr/bin/ustreamer /usr/bin/ustreamer-dump \
        && cp /tmp/ustreamer/janus/libjanus_ustreamer.so /usr/lib/ustreamer/janus \
        && pip3 install --target=/usr/lib/python3/dist-packages --break-system-packages /tmp/ustreamer/python/dist/*.whl "

    if [ "$3" = "systemd-networkd" ]; then 
        sudo chroot --userspec "root:root" $ROOTFS bash -c " \
            echo -e '[Match]\nName=eth0\n\n[Network]\nDHCP=yes\n\n[Link]\nMACAddress=B6:AE:B3:21:42:0C' > /etc/systemd/network/99-eth0.network \
            && systemctl mask NetworkManager \
            && systemctl unmask systemd-networkd \
            && systemctl enable systemd-networkd systemd-resolved "        
    fi
    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        pip3 config set global.index-url https://mirrors.tuna.tsinghua.edu.cn/pypi/web/simple \
        && pip3 install --target=/usr/lib/python3/dist-packages --break-system-packages async-lru gpiod pyfatfs \
        && pip3 cache purge "

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        cd /One-KVM \
        && python3 setup.py install \
        && bash scripts/kvmd-gencert --do-the-thing \
        && bash scripts/kvmd-gencert --do-the-thing --vnc \
        && kvmd-nginx-mkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf \
        && kvmd -m "

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
        && sed -i 's/8080/80/g' /etc/kvmd/override.yaml \
        && sed -i 's/4430/443/g' /etc/kvmd/override.yaml \
        && chown kvmd -R /var/lib/kvmd/msd/ \
        && systemctl enable kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus \
        && systemctl disable nginx janus \
        && rm -r /One-KVM "

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        curl https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.$1 -L -o /usr/bin/ttyd \
        && chmod +x /usr/bin/ttyd \
        && mkdir -p /home/kvmd-webterm \
        && chown kvmd-webterm /home/kvmd-webterm "

    if [ "$1" = "x86_64" ]; then
        sudo chroot --userspec "root:root" $ROOTFS bash -c " \
            systemctl disable kvmd-otg \
            && sed -i '2c ATX=USBRELAY_HID' /etc/kvmd/atx.sh \
            && sed -i 's/device: \/dev\/ttyUSB0/device: \/dev\/kvmd-hid/g' /etc/kvmd/override.yaml "
    else
        if [ "$2" = "gpio" ]; then
            sudo chroot --userspec "root:root" $ROOTFS bash -c " \
                sed -i '2c ATX=GPIO' /etc/kvmd/atx.sh \
                && sed -i 's/SHUTDOWNPIN/gpiochip1 7/g' /etc/kvmd/custom_atx/gpio.sh \
                && sed -i 's/REBOOTPIN/gpiochip0 11/g' /etc/kvmd/custom_atx/gpio.sh "
        else
            sudo chroot --userspec "root:root" $ROOTFS sed -i '2c ATX=USBRELAY_HID' /etc/kvmd/atx.sh
            
        fi
        if [ "$2" = "video1" ]; then
            sudo chroot --userspec "root:root" $ROOTFS sed -i 's/\/dev\/video0/\/dev\/video1/g' /etc/kvmd/override.yaml
        fi
        sudo chroot --userspec "root:root" $ROOTFS bash -c " \
            sed -i 's/ch9329/otg/g' /etc/kvmd/override.yaml \
            && sed -i 's/device: \/dev\/ttyUSB0//g' /etc/kvmd/override.yaml \
            && sed -i 's/#type: otg/type: otg/g' /etc/kvmd/override.yaml "
    fi
}

pack_img_onecloud() {
    sudo rm $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo img2simg $SRCPATH/tmp/rootfs.img $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 pack $OUTPUTDIR/One-KVM_by-SilentWind_Onecloud_$DATE.burn.img $SRCPATH/tmp/
    sudo rm $SRCPATH/tmp/*
}

case $1 in  
    onecloud)  
        onecloud_rootfs
        mount_rootfs
        config_file $1
        instal_one-kvm armhf gpio systemd-networkd
        write_meta $1
        umount_rootfs
        pack_img_onecloud
        ;;
    cumebox2)  
        cumebox2_rootfs
        mount_rootfs
        config_file $1
        config_cumebox2_file
        parpare_dns
        instal_one-kvm aarch64 video1
        write_meta $1
        umount_rootfs
        pack_img Cumebox2
        ;;
    chainedbox) 
        chainedbox_rootfs_and_fix_dtb
        mount_rootfs
        config_file $1
        parpare_dns
        instal_one-kvm aarch64 video1
        write_meta $1
        umount_rootfs
        pack_img Chainedbox
        ;;
    vm)  
        vm_rootfs
        mount_rootfs
        config_file $1
        parpare_dns
        instal_one-kvm x86_64
        write_meta $1
        umount_rootfs
        pack_img Vm
        ;;
    e900v22c)  
        e900v22c_rootfs
        mount_rootfs
        config_file $1
        instal_one-kvm aarch64 video1
        write_meta $1
        umount_rootfs
        pack_img E900v22c
        ;;
    octopus-flanet)  
        octopus-flanet_rootfs
        mount_rootfs
        config_file $1
        config_octopus-flanet_file
        parpare_dns
        instal_one-kvm aarch64 video1
        write_meta $1
        umount_rootfs
        pack_img Octopus-Flanet
        ;;
    *)  
        echo "Do no thing." 
        ;;
esac