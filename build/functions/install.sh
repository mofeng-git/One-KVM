#!/bin/bash

# --- 预准备 ---

prepare_dns_and_mirrors() {
    echo "信息：在 chroot 环境中准备 DNS 和更换软件源..."
    run_in_chroot "
    mkdir -p /run/systemd/resolve/ \\
    && touch /run/systemd/resolve/stub-resolv.conf \\
    && printf '%s\\n' 'nameserver 1.1.1.1' 'nameserver 1.0.0.1' > /etc/resolv.conf \\
    && echo '信息：尝试更换镜像源...' \\
    && bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) \\
        --source mirrors.ustc.edu.cn --upgrade-software false --web-protocol http || echo '警告：更换镜像源脚本执行失败，可能网络不通或脚本已更改'
    "
}

delete_armbian_verify(){
    echo "信息：在 chroot 环境中修改 Armbian 软件源..."
    run_in_chroot "echo 'deb http://mirrors.ustc.edu.cn/armbian bullseye main bullseye-utils bullseye-desktop' > /etc/apt/sources.list.d/armbian.list"
}

prepare_external_binaries() {
    local platform="$1" # linux/armhf or linux/amd64 or linux/aarch64
    # 如果在 GitHub Actions 环境下，使用 silentwind0/kvmd-stage-0，否则用阿里云镜像
    if is_github_actions; then
        local docker_image="silentwind0/kvmd-stage-0"
    else
        local docker_image="registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd-stage-0"
    fi

    echo "信息：准备外部预编译二进制文件 (平台: $platform)..."
    ensure_dir "$PREBUILT_DIR"

    echo "信息：拉取 Docker 镜像 $docker_image (平台: $platform)..."
    sudo docker pull --platform "$platform" "$docker_image" || { echo "错误：拉取 Docker 镜像 $docker_image 失败" >&2; exit 1; }

    echo "信息：创建 Docker 容器 $DOCKER_CONTAINER_NAME ..."
    sudo docker create --name "$DOCKER_CONTAINER_NAME" "$docker_image" || { echo "错误：创建 Docker 容器 $DOCKER_CONTAINER_NAME 失败" >&2; exit 1; }

    echo "信息：从 Docker 容器导出文件到 $PREBUILT_DIR ..."
    sudo docker export "$DOCKER_CONTAINER_NAME" | sudo tar -xf - -C "$PREBUILT_DIR" || { echo "错误：导出并解压 Docker 容器内容失败" >&2; exit 1; }

    echo "信息：预编译二进制文件准备完成，存放于 $PREBUILT_DIR"

    # 删除 Docker 容器
    sudo docker rm -f "$DOCKER_CONTAINER_NAME" || { echo "错误：删除 Docker 容器 $DOCKER_CONTAINER_NAME 失败" >&2; exit 1; }    
}

config_base_files() {
    local platform_id="$1" # e.g., "onecloud", "cumebox2"
    echo "信息：配置基础文件和目录结构 ($platform_id)..."

    echo "信息：创建 KVMD 相关目录..."
    ensure_dir "$ROOTFS/etc/kvmd/override.d"
    ensure_dir "$ROOTFS/etc/kvmd/vnc"
    ensure_dir "$ROOTFS/var/lib/kvmd/msd/images"
    ensure_dir "$ROOTFS/var/lib/kvmd/msd/meta"
    ensure_dir "$ROOTFS/opt/vc/bin"
    ensure_dir "$ROOTFS/usr/share/kvmd"
    ensure_dir "$ROOTFS/One-KVM"
    ensure_dir "$ROOTFS/usr/share/janus/javascript"
    ensure_dir "$ROOTFS/usr/lib/ustreamer/janus"
    ensure_dir "$ROOTFS/run/kvmd"
    ensure_dir "$ROOTFS/tmp/wheel/"
    ensure_dir "$ROOTFS/usr/lib/janus/transports/"
    ensure_dir "$ROOTFS/usr/lib/janus/loggers"

    echo "信息：复制 One-KVM 源码..."
    sudo rsync -a --exclude={.git,.github,output,tmp} . "$ROOTFS/One-KVM/" || { echo "错误：复制 One-KVM 源码失败" >&2; exit 1; }

    echo "信息：复制配置文件..."
    sudo cp -r configs/kvmd/* configs/nginx configs/janus "$ROOTFS/etc/kvmd/"
    sudo cp -r web extras contrib/keymaps "$ROOTFS/usr/share/kvmd/"
    sudo cp testenv/fakes/vcgencmd "$ROOTFS/usr/bin/"
    sudo cp -r testenv/js/* "$ROOTFS/usr/share/janus/javascript/"
    sudo cp "build/platform/$platform_id" "$ROOTFS/usr/share/kvmd/platform" || { echo "错误：复制平台文件 build/platform/$platform_id 失败" >&2; exit 1; }
    sudo cp scripts/kvmd-gencert scripts/kvmd-bootconfig scripts/kvmd-certbot scripts/kvmd-udev-hdmiusb-check scripts/kvmd-udev-restart-pass build/scripts/kvmd-firstrun.sh "$ROOTFS/usr/bin/"
    sudo chmod +x "$ROOTFS/usr/bin/kvmd-gencert" "$ROOTFS/usr/bin/kvmd-bootconfig" "$ROOTFS/usr/bin/kvmd-certbot" "$ROOTFS/usr/bin/kvmd-udev-hdmiusb-check" "$ROOTFS/usr/bin/kvmd-udev-restart-pass" "$ROOTFS/usr/bin/kvmd-firstrun.sh"
    
    # 尝试下载或使用本地 rc.local 文件
    download_rc_local "$platform_id" || echo "信息：rc.local 文件不存在，跳过"
    if [ -f "$SRCPATH/image/$platform_id/rc.local" ]; then
        echo "信息：复制设备特定的 rc.local 文件..."
        sudo cp "$SRCPATH/image/$platform_id/rc.local" "$ROOTFS/etc/"
    fi

    echo "信息：从预编译目录复制二进制文件和库..."
    sudo cp "$PREBUILT_DIR/tmp/lib/"* "$ROOTFS/lib/"*-linux-*/ || echo "警告：复制 /tmp/lib/* 失败，可能源目录或目标目录不存在或不匹配"
    sudo cp "$PREBUILT_DIR/tmp/ustreamer/ustreamer" "$PREBUILT_DIR/tmp/ustreamer/ustreamer-dump" "$PREBUILT_DIR/usr/bin/janus" "$ROOTFS/usr/bin/" || { echo "错误：复制 ustreamer/janus 二进制文件失败" >&2; exit 1; }
	sudo cp "$PREBUILT_DIR/tmp/ustreamer/janus/libjanus_ustreamer.so" "$ROOTFS/usr/lib/ustreamer/janus/" || { echo "错误：复制 libjanus_ustreamer.so 失败" >&2; exit 1; }
	sudo cp "$PREBUILT_DIR/tmp/wheel/"*.whl "$ROOTFS/tmp/wheel/" || { echo "错误：复制 Python wheel 文件失败" >&2; exit 1; }
	sudo cp "$PREBUILT_DIR/usr/lib/janus/transports/"* "$ROOTFS/usr/lib/janus/transports/" || { echo "错误：复制 Janus transports 失败" >&2; exit 1; }
    
    # 禁用 apt-file
 	if [ -f "$ROOTFS/etc/apt/apt.conf.d/50apt-file.conf" ]; then
        echo "信息：禁用 apt-file 配置..."
        sudo mv "$ROOTFS/etc/apt/apt.conf.d/50apt-file.conf" "$ROOTFS/etc/apt/apt.conf.d/50apt-file.conf.disabled"
    fi
    echo "信息：基础文件配置完成。"
}

# --- KVMD 安装与配置 ---

install_base_packages() {
    echo "信息：在 chroot 环境中更新源并安装基础软件包..."
    run_in_chroot "
    apt-get update && \\
    apt install -y --no-install-recommends \\
        libxkbcommon-x11-0 nginx tesseract-ocr tesseract-ocr-eng tesseract-ocr-chi-sim \\
        iptables network-manager curl kmod libmicrohttpd12 libjansson4 libssl3 \\
        libsofia-sip-ua0 libglib2.0-0 libopus0 libogg0 libcurl4 libconfig9 \\
        python3-pip net-tools libavcodec59 libavformat59 libavutil57 libswscale6 \\
        libavfilter8 libavdevice59 v4l-utils libv4l-0 nano unzip dnsmasq && \\
    apt clean && \\
    rm -rf /var/lib/apt/lists/*
    "
}

configure_network() {
    local network_type="$1" # "systemd-networkd" or others (default network-manager)
    if [ "$network_type" = "systemd-networkd" ]; then
        echo "信息：在 chroot 环境中配置 systemd-networkd..."
        
        # onecloud 与 onecloud-pro 均启用基于 SN 的 MAC 地址生成
        if [ "$TARGET_DEVICE_NAME" = "onecloud" ] || [ "$TARGET_DEVICE_NAME" = "onecloud-pro" ]; then
            echo "信息：为 ${TARGET_DEVICE_NAME} 平台配置基于 SN 的 MAC 地址生成机制..."
            
            # 复制MAC地址生成脚本
            sudo cp "$SCRIPT_DIR/scripts/generate-random-mac.sh" "$ROOTFS/usr/local/bin/"
            sudo chmod +x "$ROOTFS/usr/local/bin/generate-random-mac.sh"
            
            # 复制systemd服务文件
            sudo cp "$SCRIPT_DIR/services/kvmd-generate-mac.service" "$ROOTFS/etc/systemd/system/"
            
            # 创建初始网络配置文件（不包含MAC地址，将由脚本生成）
            run_in_chroot "
            echo -e '[Match]\\nName=eth0\\n\\n[Network]\\nDHCP=yes' > /etc/systemd/network/99-eth0.network && \\
            systemctl mask NetworkManager && \\
            systemctl unmask systemd-networkd && \\
            systemctl enable systemd-networkd systemd-resolved && \\
            systemctl enable kvmd-generate-mac.service
            "
            echo "信息：${TARGET_DEVICE_NAME} 基于 SN 的 MAC 地址生成机制配置完成"
        fi
    else
        echo "信息：使用默认的网络管理器 (NetworkManager)..."
        # 可能需要确保 NetworkManager 是启用的 (通常默认是)
        run_in_chroot "systemctl enable NetworkManager"
    fi
}

install_python_deps() {
    echo "信息：在 chroot 环境中安装 Python 依赖 (wheels)..."
    run_in_chroot "
    pip3 install --no-cache-dir --break-system-packages /tmp/wheel/*.whl && \\
    pip3 cache purge && \\
    rm -rf /tmp/wheel
    "
}

configure_kvmd_core() {
     echo "信息：在 chroot 环境中安装和配置 KVMD 核心..."
     
     # 复制KVMD首次运行脚本和服务
     echo "信息：配置KVMD首次运行初始化服务..."
     sudo cp "build/services/kvmd-firstrun.service" "$ROOTFS/etc/systemd/system/"
     
     # 安装KVMD但不执行需要在首次运行时完成的操作
     run_in_chroot "
     cd /One-KVM && \\
     python3 setup.py install && \\
     systemctl enable kvmd-firstrun.service
     "
     
     echo "信息：KVMD核心安装完成，证书生成等初始化操作将在首次开机时执行"
}

configure_system() {
    echo "信息：在 chroot 环境中配置系统级设置 (sudoers, udev, services)..."
    run_in_chroot "
    cat /One-KVM/configs/os/sudoers/v2-hdmiusb >> /etc/sudoers && \\
    cat /One-KVM/configs/os/udev/v2-hdmiusb-rpi4.rules > /etc/udev/rules.d/99-kvmd.rules && \\
    echo 'libcomposite' >> /etc/modules && \\
    echo 'net.ipv4.ip_forward = 1' > /etc/sysctl.d/99-kvmd-extra.conf && \\
    mv /usr/local/bin/kvmd* /usr/bin/ || echo '信息：/usr/local/bin/kvmd* 未找到或移动失败，可能已在/usr/bin' && \\
    cp -r /One-KVM/configs/os/services/* /etc/systemd/system/ && \\
    cp /One-KVM/configs/os/tmpfiles.conf /usr/lib/tmpfiles.d/ && \\
    chmod +x /etc/update-motd.d/* || echo '警告：chmod /etc/update-motd.d/* 失败' && \\
    echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/gpio.sh' >> /etc/sudoers && \\
    echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/usbrelay_hid.sh' >> /etc/sudoers && \\
    systemd-sysusers /One-KVM/configs/os/sysusers.conf && \\
    systemd-sysusers /One-KVM/configs/os/kvmd-webterm.conf && \\
    ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata || echo '警告：创建 tesseract 链接失败' && \\
    sed -i 's/8080/80/g' /etc/kvmd/override.yaml && \\
    sed -i 's/4430/443/g' /etc/kvmd/override.yaml && \\
    chown kvmd -R /var/lib/kvmd/msd/ && \\
    systemctl enable dnsmasq kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus kvmd-media && \\
    systemctl disable nginx systemd-resolved && \\
    rm -rf /One-KVM
    "
}

install_webterm() {
    local arch="$1" # armhf, aarch64, x86_64
    local ttyd_arch="$arch"

    if [ "$arch" = "armhf" ]; then
        ttyd_arch="armhf"
    elif [ "$arch" = "amd64" ]; then
         ttyd_arch="x86_64"
    elif [ "$arch" = "aarch64" ]; then
         ttyd_arch="aarch64"
    fi

    echo "信息：在 chroot 环境中下载并安装 ttyd ($ttyd_arch)..."
    run_in_chroot "
    curl -L https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.${ttyd_arch} -o /usr/bin/ttyd && \\
    chmod +x /usr/bin/ttyd && \\
    mkdir -p /home/kvmd-webterm && \\
    chown kvmd-webterm /home/kvmd-webterm
    "
}

apply_kvmd_tweaks() {
    local arch="$1" # armhf, aarch64, x86_64
    local device_type="$2" # "gpio" or "video1" or other
    local atx_setting=""
    local hid_setting=""

    echo "信息：根据架构 ($arch) 和设备类型 ($device_type) 调整 KVMD 配置..."

    if [ "$arch" = "x86_64" ] || [ "$arch" = "amd64" ]; then
        echo "信息：目标平台为 x86_64/amd64 架构，禁用 OTG，设置 ATX 为 USBRELAY_HID..."
        run_in_chroot "
        systemctl disable kvmd-otg && \\
        sed -i 's/^ATX=.*/ATX=USBRELAY_HID/' /etc/kvmd/atx.sh && \\
        sed -i 's/device: \/dev\/ttyUSB0/device: \/dev\/kvmd-hid/g' /etc/kvmd/override.yaml
        "
    else
        echo "信息：：目标平台为 ARM 架构 ($arch)..."
        # ARM 架构，配置 HID 为 OTG
        hid_setting="otg"
        run_in_chroot "
        sed -i 's/#type: otg/type: otg/g' /etc/kvmd/override.yaml && \\
        sed -i 's/device: \/dev\/ttyUSB0/#device: \/dev\/ttyUSB0/g' /etc/kvmd/override.yaml # 注释掉 ttyUSB0
        "
        echo "信息：设置 HID 为 $hid_setting"
        run_in_chroot "sed -i 's/type: ch9329/type: $hid_setting/g' /etc/kvmd/override.yaml"


        # 根据 device_type 配置 ATX
        if [[ "$device_type" == *"gpio-onecloud-pro"* ]]; then
            echo "信息：电源控制设备类型为 gpio，设置 ATX 为 GPIO 并配置引脚..."
            atx_setting="GPIO"
             run_in_chroot "
             sed -i 's/^ATX=.*/ATX=GPIO/' /etc/kvmd/atx.sh && \\
             sed -i 's/SHUTDOWNPIN/gpiochip0 7/g' /etc/kvmd/custom_atx/gpio.sh && \\
             sed -i 's/REBOOTPIN/gpiochip0 11/g' /etc/kvmd/custom_atx/gpio.sh
             "
        elif [[ "$device_type" == *"gpio-onecloud"* ]]; then
            echo "信息：电源控制设备类型为 gpio，设置 ATX 为 GPIO 并配置引脚..."
            atx_setting="GPIO"
             run_in_chroot "
             sed -i 's/^ATX=.*/ATX=GPIO/' /etc/kvmd/atx.sh && \\
             sed -i 's/SHUTDOWNPIN/gpiochip1 7/g' /etc/kvmd/custom_atx/gpio.sh && \\
             sed -i 's/REBOOTPIN/gpiochip0 11/g' /etc/kvmd/custom_atx/gpio.sh
             "
        else
            echo "信息：电源控制设备类型不是 gpio ($device_type)，设置 ATX 为 USBRELAY_HID..."
            atx_setting="USBRELAY_HID"
             run_in_chroot "sed -i 's/^ATX=.*/ATX=USBRELAY_HID/' /etc/kvmd/atx.sh"
        fi

        # 配置视频设备
        if [[ "$device_type" == *"video1"* ]]; then
            echo "信息：视频设备类型为 video1，设置视频设备为 /dev/video1..."
            run_in_chroot "sed -i 's|/dev/video0|/dev/video1|g' /etc/kvmd/override.yaml"
        elif [[ "$device_type" == *"video1"* ]]; then
            echo "信息：视频设备类型为 kvmd-video，设置视频设备为 /dev/kvmd-video..."
            run_in_chroot "sed -i 's|/dev/video0|/dev/kvmd-video|g' /etc/kvmd/override.yaml"
        else
             echo "信息：使用默认视频设备 /dev/video0..."
        fi
    fi
    echo "信息：KVMD 配置调整完成。"
}

# --- 整体安装流程 ---
install_and_configure_kvmd() {
    local arch="$1"         # 架构: armhf, aarch64, x86_64/amd64
    local device_type="$2"  # 设备特性: "gpio", "video1", "" (空或其他)
    local network_type="$3" # 网络配置: "systemd-networkd", "" (默认 network-manager)
    local host_arch=""      # Docker 平台架构: arm, aarch64, amd64

    # 映射架构名称
    case "$arch" in
        armhf) host_arch="arm" ;;
        aarch64) host_arch="arm64" ;; # docker aarch64 平台名是 arm64
        x86_64|amd64) host_arch="amd64"; arch="x86_64" ;; # 统一内部使用 x86_64
        *) echo "错误：不支持的架构 $arch"; exit 1 ;;
    esac


    prepare_external_binaries "linux/$host_arch"
    config_base_files "$TARGET_DEVICE_NAME" # 使用全局变量传递设备名

    # 特定设备的额外文件配置 (如果存在)
    # 将设备名中的连字符转换为下划线以匹配函数名
    local device_func_name="${TARGET_DEVICE_NAME//-/_}"
    if declare -f "config_${device_func_name}_files" > /dev/null; then
        echo "信息：执行特定设备的文件配置函数 config_${device_func_name}_files ..."
        "config_${device_func_name}_files"
    fi

    # 某些镜像可能需要准备DNS和换源
    if [[ "$NEED_PREPARE_DNS" = true ]]; then
        prepare_dns_and_mirrors
    fi
    # 可选：强制使用特定armbian源
    # delete_armbian_verify

    # 执行安装步骤
    install_base_packages
    configure_network "$network_type"
    install_python_deps
    configure_kvmd_core
    configure_system
    install_webterm "$arch" # 传递原始架构名给ttyd下载
    apply_kvmd_tweaks "$arch" "$device_type"

    run_in_chroot "df -h" # 显示最终磁盘使用情况
    echo "信息：One-KVM 安装和配置完成。"
} 