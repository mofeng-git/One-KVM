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
DATE=240315
export LC_ALL=C

write_meta() {
    sudo chroot --userspec "root:root" $ROOTFS bash -c "sed -i 's/localhost.localdomain/$1/g' /etc/kvmd/meta.yaml"
}

mount_rootfs() {
    mkdir $ROOTFS $SRCPATH/tmp/rootfs
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
	sudo zerofree $LOOPDEV
    sudo losetup -d $LOOPDEV
	sudo docker rm to_build_rootfs
	sudo rm -rf $SRCPATH/tmp/rootfs/*
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
        $ROOTFS/usr/share/janus/javascript $ROOTFS/usr/lib/ustreamer/janus $ROOTFS/run/kvmd $ROOTFS/var/lib/kvmd/msd/images $ROOTFS/var/lib/kvmd/msd/meta \
		$ROOTFS/tmp/wheel/ $ROOTFS/usr/lib/janus/transports/  $ROOTFS/usr/lib/janus/loggers
    sudo rsync -a  --exclude={src,.github} . $ROOTFS/One-KVM
    sudo cp -r configs/kvmd/* configs/nginx configs/janus $ROOTFS/etc/kvmd
    sudo cp -r web extras contrib/keymaps $ROOTFS/usr/share/kvmd
    sudo cp testenv/fakes/vcgencmd $ROOTFS/usr/bin/
    sudo cp -r testenv/js/* $ROOTFS/usr/share/janus/javascript/
    sudo cp build/platform/$1 $ROOTFS/usr/share/kvmd/platform
    if [ -f "$SRCPATH/image/$1/rc.local" ]; then
        sudo cp $SRCPATH/image/$1/rc.local $ROOTFS/etc/
    fi

	sudo docker pull --platform linux/$2 registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd-stage-0
	sudo docker create --name to_build_rootfs registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd-stage-0
	sudo docker export to_build_rootfs  | sudo tar -xvf - -C $SRCPATH/tmp/rootfs
	sudo cp $SRCPATH/tmp/rootfs/tmp/lib/*  $ROOTFS/lib/*-linux-*/
	sudo cp $SRCPATH/tmp/rootfs/tmp/ustreamer/ustreamer $SRCPATH/tmp/rootfs/tmp/ustreamer/ustreamer-dump $SRCPATH/tmp/rootfs/usr/bin/janus $ROOTFS/usr/bin/
	sudo cp $SRCPATH/tmp/rootfs/tmp/ustreamer/janus/libjanus_ustreamer.so $ROOTFS/usr/lib/ustreamer/janus/
	sudo cp $SRCPATH/tmp/rootfs/tmp/wheel/*.whl $ROOTFS/tmp/wheel/
	sudo cp $SRCPATH/tmp/rootfs/usr/lib/janus/transports/* $ROOTFS/usr/lib/janus/transports/

	sudo mv $ROOTFS/etc/apt/apt.conf.d/50apt-file.conf{,.disabled}
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
    dd if=/dev/zero of=/tmp/add.img bs=1M count=256 && cat /tmp/add.img >> $SRCPATH/tmp/rootfs.img && rm /tmp/add.img
    e2fsck -f $SRCPATH/tmp/rootfs.img && resize2fs $SRCPATH/tmp/rootfs.img
    sudo losetup $LOOPDEV $SRCPATH/tmp/rootfs.img
}

cumebox2_rootfs() {
    cp $SRCPATH/image/cumebox2/Armbian_25.2.2_Khadas-vim1_bookworm_current_6.12.17_minimal.img $SRCPATH/tmp/rootfs.img
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
	sudo chmod +x $ROOTFS/usr/bin/ssd
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
        && apt install -y --no-install-recommends libxkbcommon-x11-0 nginx tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim iptables network-manager \
        	curl kmod libmicrohttpd12 libjansson4 libssl3 libsofia-sip-ua0 libglib2.0-0 libopus0 libogg0 libcurl4 libconfig9 python3-pip net-tools \
        && apt clean \
		&& rm -rf /var/lib/apt/lists/* "

    if [ "$3" = "systemd-networkd" ]; then 
        sudo chroot --userspec "root:root" $ROOTFS bash -c " \
            echo -e '[Match]\nName=eth0\n\n[Network]\nDHCP=yes\n\n[Link]\nMACAddress=B6:AE:B3:21:42:0C' > /etc/systemd/network/99-eth0.network \
            && systemctl mask NetworkManager \
            && systemctl unmask systemd-networkd \
            && systemctl enable systemd-networkd systemd-resolved "        
    fi
    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        pip3 install --no-cache-dir --break-system-packages /tmp/wheel/*.whl \
        && pip3 cache purge \
		&& rm -r /tmp/wheel "

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        cd /One-KVM \
        && python3 setup.py install \
        && bash scripts/kvmd-gencert --do-the-thing \
        && bash scripts/kvmd-gencert --do-the-thing --vnc \
        && kvmd-nginx-mkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf \
        && kvmd -m "

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        cat /One-KVM/configs/os/sudoers/v2-hdmiusb >> /etc/sudoers \
        && cat /One-KVM/configs/os/udev/v2-hdmiusb-rpi4.rules > /etc/udev/rules.d/99-kvmd.rules \
        && echo 'libcomposite' >> /etc/modules \
        && mv /usr/local/bin/kvmd* /usr/bin \
        && cp /One-KVM/configs/os/services/* /etc/systemd/system/ \
        && cp /One-KVM/configs/os/tmpfiles.conf /usr/lib/tmpfiles.d/ \
		&& mv /etc/kvmd/supervisord.conf /etc/supervisord.conf \
        && chmod +x /etc/update-motd.d/* \
        && echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/gpio.sh' >>  /etc/sudoers \
        && echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/usbrelay_hid.sh' >>  /etc/sudoers \
        && systemd-sysusers /One-KVM/configs/os/sysusers.conf \
        && systemd-sysusers /One-KVM/configs/os/kvmd-webterm.conf \
        && ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata \
        && sed -i 's/8080/80/g' /etc/kvmd/override.yaml \
        && sed -i 's/4430/443/g' /etc/kvmd/override.yaml \
        && chown kvmd -R /var/lib/kvmd/msd/ \
        && systemctl enable kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus kvmd-media \
        && systemctl disable nginx \
        && rm -r /One-KVM "

    sudo chroot --userspec "root:root" $ROOTFS bash -c " \
        curl https://gh.llkk.cc/https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.$1 -L -o /usr/bin/ttyd \
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

	sudo chroot --userspec "root:root" $ROOTFS bash -c "df -h"
}

pack_img_onecloud() {
    sudo rm $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo img2simg $SRCPATH/tmp/rootfs.img $SRCPATH/tmp/7.rootfs.PARTITION.sparse
    sudo $SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64 pack $OUTPUTDIR/One-KVM_by-SilentWind_Onecloud_$DATE.burn.img $SRCPATH/tmp/
    sudo rm $SRCPATH/tmp/*
}

#build function

onecloud() {
    onecloud_rootfs
    mount_rootfs
    config_file "onecloud" "arm"
    instal_one-kvm armhf gpio systemd-networkd
    write_meta "onecloud"
    umount_rootfs
    pack_img_onecloud
}

cumebox2() {
    cumebox2_rootfs
    mount_rootfs
    config_file "cumebox2" "aarch64"
    config_cumebox2_file
    parpare_dns
    instal_one-kvm aarch64 video1
    write_meta "cumebox2"
    umount_rootfs
    pack_img "Cumebox2"
}

chainedbox() {
    chainedbox_rootfs_and_fix_dtb
    mount_rootfs
    config_file "chainedbox" "aarch64"
    parpare_dns
    instal_one-kvm aarch64 video1
    write_meta "chainedbox"
    umount_rootfs
    pack_img "Chainedbox"
}

vm() {
    vm_rootfs
    mount_rootfs
    config_file "vm" "amd64"
    parpare_dns
    instal_one-kvm x86_64
    write_meta "vm"
    umount_rootfs
    pack_img "Vm"
}

e900v22c() {
    e900v22c_rootfs
    mount_rootfs
    config_file "e900v22c" "aarch64"
    instal_one-kvm aarch64 video1
    write_meta "e900v22c"
    umount_rootfs
    pack_img "E900v22c"
}

octopus_flanet() {
    octopus-flanet_rootfs
    mount_rootfs
    config_file "octopus-flanet" "aarch64"
    config_octopus-flanet_file
    parpare_dns
    instal_one-kvm aarch64 video1
    write_meta "octopus-flanet"
    umount_rootfs
    pack_img "Octopus-Flanet"
}

if [ "$1" = "all" ]; then
    onecloud
    cumebox2
    chainedbox
    vm
    e900v22c
    octopus_flanet
else
    case $1 in  
        onecloud)  
            onecloud
            ;;
        cumebox2)  
            cumebox2
            ;;
        chainedbox) 
            chainedbox
            ;;
        vm)  
            vm
            ;;
        e900v22c)  
            e900v22c
            ;;
        octopus-flanet)  
            octopus_flanet
            ;;
        *)  
            echo "Do no thing." 
            ;;
    esac
fi