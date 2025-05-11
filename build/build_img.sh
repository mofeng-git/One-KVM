#!/bin/bash

# --- 配置 ---
# 允许通过环境变量覆盖默认路径
SRCPATH="${SRCPATH:-/mnt/nfs/lfs/src}"
BOOTFS="${BOOTFS:-/tmp/bootfs}"
ROOTFS="${ROOTFS:-/tmp/rootfs}"
OUTPUTDIR="${OUTPUTDIR:-/mnt/nfs/lfs/src/output}"
TMPDIR="${TMPDIR:-$SRCPATH/tmp}"

get_git_commit_id() {
    if git rev-parse --is-inside-work-tree &>/dev/null; then
        git rev-parse --short HEAD 2>/dev/null || echo ""
    else
        echo ""
    fi
}

GIT_COMMIT_ID=$(get_git_commit_id)
DATE=$(date +%y%m%d)
if [ -n "$GIT_COMMIT_ID" ]; then
    DATE="${DATE}-${GIT_COMMIT_ID}"
fi
export LC_ALL=C

# 全局变量
LOOPDEV=""
ROOTFS_MOUNTED=0
BOOTFS_MOUNTED=0
PROC_MOUNTED=0
SYS_MOUNTED=0
DEV_MOUNTED=0
DOCKER_CONTAINER_NAME="to_build_rootfs_$$"
PREBUILT_DIR="/tmp/prebuilt_binaries"

# --- 清理函数 ---
cleanup() {
    echo "信息：执行清理操作..."
    # 尝试卸载 chroot 环境下的挂载点
    if [[ "$DEV_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/dev ..."
        sudo umount "$ROOTFS/dev" || echo "警告：卸载 $ROOTFS/dev 失败，可能已被卸载"
        DEV_MOUNTED=0
    fi
    if [[ "$SYS_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/sys ..."
        sudo umount "$ROOTFS/sys" || echo "警告：卸载 $ROOTFS/sys 失败，可能已被卸载"
        SYS_MOUNTED=0
    fi
    if [[ "$PROC_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/proc ..."
        sudo umount "$ROOTFS/proc" || echo "警告：卸载 $ROOTFS/proc 失败，可能已被卸载"
        PROC_MOUNTED=0
    fi

    # 尝试卸载主根文件系统
    if [[ "$ROOTFS_MOUNTED" -eq 1 && -d "$ROOTFS" ]]; then
        echo "信息：卸载 $ROOTFS ..."
        sudo umount "$ROOTFS" || sudo umount -l "$ROOTFS" || echo "警告：卸载 $ROOTFS 失败"
        ROOTFS_MOUNTED=0
    fi
     # 尝试卸载引导文件系统 (如果使用)
    if [[ "$BOOTFS_MOUNTED" -eq 1 && -d "$BOOTFS" ]]; then
        echo "信息：卸载 $BOOTFS ..."
        sudo umount "$BOOTFS" || sudo umount -l "$BOOTFS" || echo "警告：卸载 $BOOTFS 失败"
        BOOTFS_MOUNTED=0
    fi

    # 尝试分离 loop 设备
    if [[ -n "$LOOPDEV" && -b "$LOOPDEV" ]]; then
        echo "信息：尝试 zerofree $LOOPDEV ..."
        sudo zerofree "$LOOPDEV" || echo "警告：zerofree $LOOPDEV 失败，可能文件系统不支持或未干净卸载"
        echo "信息：分离 loop 设备 $LOOPDEV ..."
        sudo losetup -d "$LOOPDEV" || echo "警告：分离 $LOOPDEV 失败"
        LOOPDEV=""
    fi

    # 尝试删除 Docker 容器
    echo "信息：检查并删除 Docker 容器 $DOCKER_CONTAINER_NAME ..."
    if sudo docker ps -a --format '{{.Names}}' | grep -q "^${DOCKER_CONTAINER_NAME}$"; then
        sudo docker rm -f "$DOCKER_CONTAINER_NAME" || echo "警告：删除 Docker 容器 $DOCKER_CONTAINER_NAME 失败"
    else
        echo "信息：Docker 容器 $DOCKER_CONTAINER_NAME 不存在或已被删除。"
    fi

    # 清理临时目录和挂载点目录
    echo "信息：清理临时文件和目录..."
    sudo rm -rf "$PREBUILT_DIR"
    # 注意：不自动删除 $TMPDIR/rootfs.img 等原始或处理中的镜像文件，由各函数管理
    # 只删除挂载点目录本身
    if [[ -d "$ROOTFS" ]]; then
        sudo rmdir "$ROOTFS" || echo "警告：删除目录 $ROOTFS 失败，可能非空"
    fi
     if [[ -d "$BOOTFS" ]]; then
        sudo rmdir "$BOOTFS" || echo "警告：删除目录 $BOOTFS 失败，可能非空"
    fi

    echo "信息：清理完成。"
}

# 在打包镜像前调用此函数，确保干净卸载所有挂载点和loop设备
unmount_all() {
    echo "信息：执行卸载操作，准备打包..."
    # 卸载 chroot 环境下的挂载点
    if [[ "$DEV_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/dev ..."
        sudo umount "$ROOTFS/dev" || echo "警告：卸载 $ROOTFS/dev 失败，可能已被卸载"
        DEV_MOUNTED=0
    fi
    if [[ "$SYS_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/sys ..."
        sudo umount "$ROOTFS/sys" || echo "警告：卸载 $ROOTFS/sys 失败，可能已被卸载"
        SYS_MOUNTED=0
    fi
    if [[ "$PROC_MOUNTED" -eq 1 ]]; then
        echo "信息：卸载 $ROOTFS/proc ..."
        sudo umount "$ROOTFS/proc" || echo "警告：卸载 $ROOTFS/proc 失败，可能已被卸载"
        PROC_MOUNTED=0
    fi

    # 卸载主根文件系统
    if [[ "$ROOTFS_MOUNTED" -eq 1 && -d "$ROOTFS" ]]; then
        echo "信息：卸载 $ROOTFS ..."
        sudo umount "$ROOTFS" || sudo umount -l "$ROOTFS" || echo "警告：卸载 $ROOTFS 失败"
        ROOTFS_MOUNTED=0
    fi
    
    # 尝试分离 loop 设备前执行 zerofree（如果文件系统支持）
    if [[ -n "$LOOPDEV" && -b "$LOOPDEV" ]]; then
        echo "信息：尝试 zerofree $LOOPDEV ..."
        sudo zerofree "$LOOPDEV" || echo "警告：zerofree $LOOPDEV 失败，可能文件系统不支持或未干净卸载"
        echo "信息：分离 loop 设备 $LOOPDEV ..."
        sudo losetup -d "$LOOPDEV" || echo "警告：分离 $LOOPDEV 失败"
        LOOPDEV=""
    fi

    sudo rm -rf "$PREBUILT_DIR"

    echo "信息：卸载操作完成，可以安全打包镜像。"
}

# --- 注册清理函数 ---
# 在脚本退出、收到错误信号、中断信号、终止信号时执行 cleanup
trap cleanup EXIT ERR INT TERM

# --- 辅助函数 ---

# 查找并设置一个可用的 loop 设备
find_loop_device() {
    echo "信息：查找可用的 loop 设备..."
    # 只使用 --find 来获取设备名
    LOOPDEV=$(sudo losetup --find)
    if [[ -z "$LOOPDEV" || ! -e "$LOOPDEV" ]]; then
        echo "错误：再次尝试后仍无法找到可用的 loop 设备。" >&2
        exit 1
    fi
    echo "信息：找到可用 loop 设备名：$LOOPDEV"
}

# 检查并创建目录
ensure_dir() {
    if [[ ! -d "$1" ]]; then
        echo "信息：创建目录 $1 ..."
        sudo mkdir -p "$1" || { echo "错误：创建目录 $1 失败" >&2; exit 1; }
    fi
}

# 执行 chroot 命令
run_in_chroot() {
    echo "信息：在 chroot 环境 ($ROOTFS) 中执行命令..."
    sudo chroot --userspec "root:root" "$ROOTFS" bash -ec "$1" || { echo "错误：在 chroot 环境中执行命令失败" >&2; exit 1; }
    echo "信息：chroot 命令执行完成。"
}


# --- 核心功能函数 ---

write_meta() {
    local hostname="$1"
    echo "信息：在 chroot 环境中设置主机名/元数据为 $hostname ..."
    run_in_chroot "sed -i 's/localhost.localdomain/$hostname/g' /etc/kvmd/meta.yaml"
}

mount_rootfs() {
    echo "信息：挂载根文件系统到 $ROOTFS ..."
    ensure_dir "$ROOTFS"
    sudo mount "$LOOPDEV" "$ROOTFS" || { echo "错误：挂载 $LOOPDEV 到 $ROOTFS 失败" >&2; exit 1; }
    ROOTFS_MOUNTED=1

    echo "信息：挂载 proc, sys, dev 到 chroot 环境..."
    ensure_dir "$ROOTFS/proc"
    sudo mount -t proc proc "$ROOTFS/proc" || { echo "错误：挂载 proc 到 $ROOTFS/proc 失败" >&2; exit 1; }
    PROC_MOUNTED=1

    ensure_dir "$ROOTFS/sys"
    sudo mount -t sysfs sys "$ROOTFS/sys" || { echo "错误：挂载 sys 到 $ROOTFS/sys 失败" >&2; exit 1; }
    SYS_MOUNTED=1

    ensure_dir "$ROOTFS/dev"
    sudo mount -o bind /dev "$ROOTFS/dev" || { echo "错误：绑定挂载 /dev 到 $ROOTFS/dev 失败" >&2; exit 1; }
    DEV_MOUNTED=1
    echo "信息：根文件系统及虚拟文件系统挂载完成。"
}

prepare_dns_and_mirrors() {
    echo "信息：在 chroot 环境中准备 DNS 和更换软件源..."
    run_in_chroot "
    mkdir -p /run/systemd/resolve/ \\
    && touch /run/systemd/resolve/stub-resolv.conf \\
    && printf '%s\\n' 'nameserver 1.1.1.1' 'nameserver 1.0.0.1' > /etc/resolv.conf \\
    && echo '信息：尝试更换镜像源...' \\
    && bash <(curl -sSL https://gitee.com/SuperManito/LinuxMirrors/raw/main/ChangeMirrors.sh) \\
        --source mirrors.tuna.tsinghua.edu.cn --upgrade-software false --web-protocol http || echo '警告：更换镜像源脚本执行失败，可能网络不通或脚本已更改'
    "
}

delete_armbian_verify(){
    echo "信息：在 chroot 环境中修改 Armbian 软件源..."
    run_in_chroot "echo 'deb http://mirrors.ustc.edu.cn/armbian bullseye main bullseye-utils bullseye-desktop' > /etc/apt/sources.list.d/armbian.list"
}

prepare_external_binaries() {
    local platform="$1" # linux/armhf or linux/amd64 or linux/aarch64
    local docker_image="registry.cn-hangzhou.aliyuncs.com/silentwind/kvmd-stage-0"

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
        python3-pip net-tools && \\
    apt clean && \\
    rm -rf /var/lib/apt/lists/*
    "
}

configure_network() {
    local network_type="$1" # "systemd-networkd" or others (default network-manager)
    if [ "$network_type" = "systemd-networkd" ]; then
        echo "信息：在 chroot 环境中配置 systemd-networkd..."
        run_in_chroot "
        echo -e '[Match]\\nName=eth0\\n\\n[Network]\\nDHCP=yes\\n\\n[Link]\\nMACAddress=B6:AE:B3:21:42:0C' > /etc/systemd/network/99-eth0.network && \\
        systemctl mask NetworkManager && \\
        systemctl unmask systemd-networkd && \\
        systemctl enable systemd-networkd systemd-resolved
        "
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
     run_in_chroot "
     cd /One-KVM && \\
     python3 setup.py install && \\
     bash scripts/kvmd-gencert --do-the-thing && \\
     bash scripts/kvmd-gencert --do-the-thing --vnc && \\
     kvmd-nginx-mkconf /etc/kvmd/nginx/nginx.conf.mako /etc/kvmd/nginx/nginx.conf && \\
     kvmd -m
     "
}

configure_system() {
    echo "信息：在 chroot 环境中配置系统级设置 (sudoers, udev, services)..."
    run_in_chroot "
    cat /One-KVM/configs/os/sudoers/v2-hdmiusb >> /etc/sudoers && \\
    cat /One-KVM/configs/os/udev/v2-hdmiusb-rpi4.rules > /etc/udev/rules.d/99-kvmd.rules && \\
    echo 'libcomposite' >> /etc/modules && \\
    mv /usr/local/bin/kvmd* /usr/bin/ || echo '信息：/usr/local/bin/kvmd* 未找到或移动失败，可能已在/usr/bin' && \\
    cp /One-KVM/configs/os/services/* /etc/systemd/system/ && \\
    cp /One-KVM/configs/os/tmpfiles.conf /usr/lib/tmpfiles.d/ && \\
    mv /etc/kvmd/supervisord.conf /etc/supervisord.conf && \\
    chmod +x /etc/update-motd.d/* || echo '警告：chmod /etc/update-motd.d/* 失败' && \\
    echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/gpio.sh' >> /etc/sudoers && \\
    echo 'kvmd ALL=(ALL) NOPASSWD: /etc/kvmd/custom_atx/usbrelay_hid.sh' >> /etc/sudoers && \\
    systemd-sysusers /One-KVM/configs/os/sysusers.conf && \\
    systemd-sysusers /One-KVM/configs/os/kvmd-webterm.conf && \\
    ln -sf /usr/share/tesseract-ocr/*/tessdata /usr/share/tessdata || echo '警告：创建 tesseract 链接失败' && \\
    sed -i 's/8080/80/g' /etc/kvmd/override.yaml && \\
    sed -i 's/4430/443/g' /etc/kvmd/override.yaml && \\
    chown kvmd -R /var/lib/kvmd/msd/ && \\
    systemctl enable kvmd kvmd-otg kvmd-nginx kvmd-vnc kvmd-ipmi kvmd-webterm kvmd-janus kvmd-media && \\
    systemctl disable nginx && \\
    rm -rf /One-KVM
    "
}

install_webterm() {
    local arch="$1" # armhf, aarch64, x86_64
    local ttyd_arch="$arch"

    if [ "$arch" = "armhf" ]; then
        ttyd_arch="armv7"
    elif [ "$arch" = "amd64" ]; then
         ttyd_arch="x86_64" # ttyd 通常用 x86_64
    fi

    echo "信息：在 chroot 环境中下载并安装 ttyd ($ttyd_arch)..."
    run_in_chroot "
    curl -L https://gh.llkk.cc/https://github.com/tsl0922/ttyd/releases/download/1.7.7/ttyd.${ttyd_arch} -o /usr/bin/ttyd && \\
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
        if [ "$device_type" = "gpio" ]; then
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
        if [ "$device_type" = "video1" ]; then
            echo "信息：视频设备类型为 video1，设置视频设备为 /dev/video1..."
            run_in_chroot "sed -i 's|/dev/video0|/dev/video1|g' /etc/kvmd/override.yaml"
        elif [ "$device_type" = "kvmd-video" ]; then
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
    if declare -f "config_${TARGET_DEVICE_NAME}_files" > /dev/null; then
        echo "信息：执行特定设备的文件配置函数 config_${TARGET_DEVICE_NAME}_files ..."
        "config_${TARGET_DEVICE_NAME}_files"
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

# --- 打包函数 ---

pack_img() {
    local device_name_friendly="$1" # e.g., "Vm", "Cumebox2"
    local target_img_name="One-KVM_by-SilentWind_${device_name_friendly}_${DATE}.img"
    local source_img="$TMPDIR/rootfs.img"

    echo "信息：开始打包镜像 ($device_name_friendly)..."
    ensure_dir "$OUTPUTDIR"

    # 确保在打包前已经正确卸载了所有挂载点和loop设备
    if [[ "$ROOTFS_MOUNTED" -eq 1 || "$DEV_MOUNTED" -eq 1 || "$SYS_MOUNTED" -eq 1 || "$PROC_MOUNTED" -eq 1 || -n "$LOOPDEV" && -b "$LOOPDEV" ]]; then
        echo "警告：发现未卸载的挂载点或loop设备，尝试再次卸载..."
        unmount_all
    fi

    echo "信息：移动镜像文件 $source_img 到 $OUTPUTDIR/$target_img_name ..."
    sudo mv "$source_img" "$OUTPUTDIR/$target_img_name" || { echo "错误：移动镜像文件失败" >&2; exit 1; }

    if [ "$device_name_friendly" = "Vm" ]; then
        echo "信息：为 Vm 目标转换镜像格式 (vmdk, vdi)..."
        local raw_img="$OUTPUTDIR/$target_img_name"
        local vmdk_img="$OUTPUTDIR/One-KVM_by-SilentWind_Vmare-uefi_${DATE}.vmdk"
        local vdi_img="$OUTPUTDIR/One-KVM_by-SilentWind_Virtualbox-uefi_${DATE}.vdi"

        echo "信息：转换为 VMDK..."
        sudo qemu-img convert -f raw -O vmdk "$raw_img" "$vmdk_img" || echo "警告：转换为 VMDK 失败"
        echo "信息：转换为 VDI..."
        sudo qemu-img convert -f raw -O vdi "$raw_img" "$vdi_img" || echo "警告：转换为 VDI 失败"
    fi
    echo "信息：镜像打包完成: $OUTPUTDIR/$target_img_name"
}

pack_img_onecloud() {
    local target_img_name="One-KVM_by-SilentWind_Onecloud_${DATE}.burn.img"
    local rootfs_raw_img="$TMPDIR/rootfs.img"
    local rootfs_sparse_img="$TMPDIR/7.rootfs.PARTITION.sparse"
    local aml_packer="$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64"

    echo "信息：开始为 Onecloud 打包 burn 镜像..."
    ensure_dir "$OUTPUTDIR"

    # 确保在打包前已经正确卸载了所有挂载点和loop设备
    if [[ "$ROOTFS_MOUNTED" -eq 1 || "$DEV_MOUNTED" -eq 1 || "$SYS_MOUNTED" -eq 1 || "$PROC_MOUNTED" -eq 1 || -n "$LOOPDEV" && -b "$LOOPDEV" ]]; then
        echo "警告：发现未卸载的挂载点或loop设备，尝试再次卸载..."
        unmount_all
    fi

    echo "信息：将 raw rootfs 转换为 sparse image..."
    # 先删除可能存在的旧 sparse 文件
    sudo rm -f "$rootfs_sparse_img"
    sudo img2simg "$rootfs_raw_img" "$rootfs_sparse_img" || { echo "错误：img2simg 转换失败" >&2; exit 1; }
    sudo rm "$rootfs_raw_img" # 删除 raw 文件，因为它已被转换

    echo "信息：使用 AmlImg 工具打包..."
    sudo chmod +x "$aml_packer"
    sudo "$aml_packer" pack "$OUTPUTDIR/$target_img_name" "$TMPDIR/" || { echo "错误：AmlImg 打包失败" >&2; exit 1; }

    echo "信息：清理 Onecloud 临时文件..."
    sudo rm -f "$TMPDIR/6.boot.PARTITION.sparse" "$TMPDIR/7.rootfs.PARTITION.sparse" "$TMPDIR/dts.img"

    echo "信息：Onecloud burn 镜像打包完成: $OUTPUTDIR/$target_img_name"
}

# --- 设备特定的 Rootfs 准备函数 ---

onecloud_rootfs() {
    local unpacker="$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64"
    local source_image="$SRCPATH/image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal.burn.img"
    local bootfs_img="$TMPDIR/bootfs.img"
    local rootfs_img="$TMPDIR/rootfs.img"
    local bootfs_sparse="$TMPDIR/6.boot.PARTITION.sparse"
    local rootfs_sparse="$TMPDIR/7.rootfs.PARTITION.sparse"
    local bootfs_loopdev="" # 存储 bootfs 使用的 loop 设备
    local add_size_mb=400

    echo "信息：准备 Onecloud Rootfs..."
    ensure_dir "$TMPDIR"
    ensure_dir "$BOOTFS"

    echo "信息：解包 Onecloud burn 镜像..."
    sudo "$unpacker" unpack "$source_image" "$TMPDIR" || { echo "错误：解包失败" >&2; exit 1; }
    # ... (unpacker logic) ...

    echo "信息：转换 bootfs 和 rootfs sparse 镜像到 raw 格式..."
    sudo simg2img "$bootfs_sparse" "$bootfs_img" || { echo "错误：转换 bootfs sparse 镜像失败" >&2; exit 1; }
    sudo simg2img "$rootfs_sparse" "$rootfs_img" || { echo "错误：转换 rootfs sparse 镜像失败" >&2; exit 1; }

    echo "信息：挂载 bootfs 并修复 DTB..."
    find_loop_device # 查找一个 loop 设备给 bootfs
    bootfs_loopdev="$LOOPDEV" # 保存这个设备名
    echo "信息：将 $bootfs_img 关联到 $bootfs_loopdev..."
    sudo losetup "$bootfs_loopdev" "$bootfs_img" || { echo "错误：关联 bootfs 镜像到 $bootfs_loopdev 失败" >&2; exit 1; }
    sudo mount "$bootfs_loopdev" "$BOOTFS" || { echo "错误：挂载 bootfs ($bootfs_loopdev) 失败" >&2; exit 1; }
    BOOTFS_MOUNTED=1
    sudo cp "$SRCPATH/image/onecloud/meson8b-onecloud-fix.dtb" "$BOOTFS/dtb/meson8b-onecloud.dtb" || { echo "错误：复制修复后的 DTB 文件失败" >&2; exit 1; }
    sudo umount "$BOOTFS" || { echo "警告：卸载 bootfs ($BOOTFS) 失败" >&2; BOOTFS_MOUNTED=0; } # 卸载失败不应中断流程
    BOOTFS_MOUNTED=0
    echo "信息：分离 bootfs loop 设备 $bootfs_loopdev..."
    sudo losetup -d "$bootfs_loopdev" || { echo "警告：分离 bootfs loop 设备 $bootfs_loopdev 失败" >&2; }
    # bootfs_loopdev 对应的设备现在是空闲的

    echo "信息：扩展 rootfs 镜像 (${add_size_mb}MB)..."
    sudo dd if=/dev/zero bs=1M count="$add_size_mb" >> "$rootfs_img" || { echo "错误：扩展 rootfs 镜像失败" >&2; exit 1; }

    echo "信息：检查并调整 rootfs 文件系统大小 (在文件上)..."
    # 注意：e2fsck/resize2fs 现在直接操作镜像文件，而不是 loop 设备
    sudo e2fsck -f -y "$rootfs_img" || { echo "警告：e2fsck 检查 rootfs 镜像文件失败" >&2; exit 1; }
    sudo resize2fs "$rootfs_img" || { echo "错误：resize2fs 调整 rootfs 镜像文件大小失败" >&2; exit 1; }

    echo "信息：设置 rootfs loop 设备..."
    find_loop_device # 重新查找一个可用的 loop 设备 (可能是刚才释放的那个)
    echo "信息：将 $rootfs_img 关联到 $LOOPDEV..."
    sudo losetup "$LOOPDEV" "$rootfs_img" || { echo "错误：关联 rootfs 镜像到 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：Onecloud Rootfs 准备完成。 Loop 设备 $LOOPDEV 已关联 $rootfs_img"
}

cumebox2_rootfs() {
    local source_image="$SRCPATH/image/cumebox2/Armbian_25.2.2_Khadas-vim1_bookworm_current_6.12.17_minimal.img"
    local target_image="$TMPDIR/rootfs.img"
    local offset=$((8192 * 512))

    echo "信息：准备 Cumebox2 Rootfs..."
    ensure_dir "$TMPDIR"
    cp "$source_image" "$target_image" || { echo "错误：复制 Cumebox2 原始镜像失败" >&2; exit 1; }

    echo "信息：调整镜像分区大小..."
    sudo parted -s "$target_image" resizepart 1 100% || { echo "错误：使用 parted 调整分区大小失败" >&2; exit 1; }

    echo "信息：设置带偏移量的 loop 设备..."
    find_loop_device # 查找设备名
    echo "信息：将 $target_image (偏移 $offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$offset" "$LOOPDEV" "$target_image" || { echo "错误：设置带偏移量的 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：检查并调整文件系统大小 (在 loop 设备上)..."
    sudo e2fsck -f -y "$LOOPDEV" || { echo "警告：e2fsck 检查 $LOOPDEV 失败" >&2; exit 1; }
    sudo resize2fs "$LOOPDEV" || { echo "错误：resize2fs 调整 $LOOPDEV 大小失败" >&2; exit 1; }

    echo "信息：Cumebox2 Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

chainedbox_rootfs_and_fix_dtb() {
    local source_image="$SRCPATH/image/chainedbox/Armbian_24.11.0_rockchip_chainedbox_bookworm_6.1.112_server_2024.10.02_add800m.img"
    local target_image="$TMPDIR/rootfs.img"
    local boot_offset=$((32768 * 512))
    local rootfs_offset=$((1081344 * 512))
    local bootfs_loopdev=""

    echo "信息：准备 Chainedbox Rootfs 并修复 DTB..."
    ensure_dir "$TMPDIR"; ensure_dir "$BOOTFS"
    cp "$source_image" "$target_image" || { echo "错误：复制 Chainedbox 原始镜像失败" >&2; exit 1; }

    echo "信息：挂载 boot 分区并修复 DTB..."
    find_loop_device # 找 loop 给 boot
    bootfs_loopdev="$LOOPDEV"
    echo "信息：将 $target_image (偏移 $boot_offset) 关联到 $bootfs_loopdev..."
    sudo losetup --offset "$boot_offset" "$bootfs_loopdev" "$target_image" || { echo "错误：设置 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; exit 1; }
    sudo mount "$bootfs_loopdev" "$BOOTFS" || { echo "错误：挂载 boot 分区 ($bootfs_loopdev) 失败" >&2; exit 1; }
    BOOTFS_MOUNTED=1
    sudo cp "$SRCPATH/image/chainedbox/rk3328-l1pro-1296mhz-fix.dtb" "$BOOTFS/dtb/rockchip/rk3328-l1pro-1296mhz.dtb" || { echo "错误：复制修复后的 DTB 文件失败" >&2; exit 1; }
    sudo umount "$BOOTFS" || { echo "警告：卸载 boot 分区 ($BOOTFS) 失败" >&2; BOOTFS_MOUNTED=0; }
    BOOTFS_MOUNTED=0
    echo "信息：分离 boot loop 设备 $bootfs_loopdev..."
    sudo losetup -d "$bootfs_loopdev" || { echo "警告：分离 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; }

    echo "信息：设置 rootfs 分区的 loop 设备..."
    find_loop_device # 找 loop 给 rootfs
    echo "信息：将 $target_image (偏移 $rootfs_offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$rootfs_offset" "$LOOPDEV" "$target_image" || { echo "错误：设置 rootfs 分区 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    # 如果需要 resize rootfs 分区，可以在这里操作 $LOOPDEV
    # echo "信息：检查并调整文件系统大小 (在 loop 设备上)..."
    # sudo e2fsck -f -y "$LOOPDEV" || { echo "警告：e2fsck 检查 $LOOPDEV 失败" >&2; exit 1; }
    # sudo resize2fs "$LOOPDEV" || { echo "错误：resize2fs 调整 $LOOPDEV 大小失败" >&2; exit 1; }

    echo "信息：Chainedbox Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}


vm_rootfs() {
    local source_image="$SRCPATH/image/vm/Armbian_25.2.1_Uefi-x86_bookworm_current_6.12.13_minimal.img"
    local target_image="$TMPDIR/rootfs.img"
    local offset=$((540672 * 512))

    echo "信息：准备 Vm Rootfs..."
    ensure_dir "$TMPDIR"
    cp "$source_image" "$target_image" || { echo "错误：复制 Vm 原始镜像失败" >&2; exit 1; }

    echo "信息：设置带偏移量的 loop 设备..."
    find_loop_device # 查找设备名
    echo "信息：将 $target_image (偏移 $offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$offset" "$LOOPDEV" "$target_image" || { echo "错误：设置带偏移量的 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    # Optional resize on $LOOPDEV here if needed

    echo "信息：Vm Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

e900v22c_rootfs() {
    local source_image="$SRCPATH/image/e900v22c/Armbian_23.08.0_amlogic_s905l3a_bookworm_5.15.123_server_2023.08.01.img"
    local target_image="$TMPDIR/rootfs.img"
    local offset=$((532480 * 512))
    local add_size_mb=400

    echo "信息：准备 E900V22C Rootfs..."
    ensure_dir "$TMPDIR"
    cp "$source_image" "$target_image" || { echo "错误：复制 E900V22C 原始镜像失败" >&2; exit 1; }

    echo "信息：扩展镜像文件 (${add_size_mb}MB)..."
    sudo dd if=/dev/zero bs=1M count="$add_size_mb" >> "$target_image" || { echo "错误：扩展镜像文件失败" >&2; exit 1; }

    echo "信息：调整镜像分区大小 (分区 2)..."
    sudo parted -s "$target_image" resizepart 2 100% || { echo "错误：使用 parted 调整分区 2 大小失败" >&2; exit 1; }

    echo "信息：设置带偏移量的 loop 设备..."
    find_loop_device # 查找设备名
    echo "信息：将 $target_image (偏移 $offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$offset" "$LOOPDEV" "$target_image" || { echo "错误：设置带偏移量的 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：检查并调整文件系统大小 (在 loop 设备上)..."
    sudo e2fsck -f -y "$LOOPDEV" || { echo "警告：e2fsck 检查 $LOOPDEV 失败" >&2; exit 1; }
    sudo resize2fs "$LOOPDEV" || { echo "错误：resize2fs 调整 $LOOPDEV 大小失败" >&2; exit 1; }

    echo "信息：E900V22C Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

octopus_flanet_rootfs() {
    local source_image="$SRCPATH/image/octopus-flanet/Armbian_24.11.0_amlogic_s912_bookworm_6.1.114_server_2024.11.01.img"
    local target_image="$TMPDIR/rootfs.img"
    local boot_offset=$((8192 * 512))
    local rootfs_offset=$((1056768 * 512))
    local add_size_mb=400
    local bootfs_loopdev=""

    echo "信息：准备 Octopus-Planet Rootfs..."
    ensure_dir "$TMPDIR"; ensure_dir "$BOOTFS"
    cp "$source_image" "$target_image" || { echo "错误：复制 Octopus-Planet 原始镜像失败" >&2; exit 1; }

    echo "信息：挂载 boot 分区并修改 uEnv.txt (使用 VIM2 DTB)..."
    find_loop_device # 找 loop 给 boot
    bootfs_loopdev="$LOOPDEV"
    echo "信息：将 $target_image (偏移 $boot_offset) 关联到 $bootfs_loopdev..."
    sudo losetup --offset "$boot_offset" "$bootfs_loopdev" "$target_image" || { echo "错误：设置 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; exit 1; }
    sudo mount "$bootfs_loopdev" "$BOOTFS" || { echo "错误：挂载 boot 分区 ($bootfs_loopdev) 失败" >&2; exit 1; }
    BOOTFS_MOUNTED=1
    sudo sed -i "s/meson-gxm-octopus-planet.dtb/meson-gxm-khadas-vim2.dtb/g" "$BOOTFS/uEnv.txt" || { echo "错误：修改 uEnv.txt 失败" >&2; exit 1; }
    sudo umount "$BOOTFS" || { echo "警告：卸载 boot 分区 ($BOOTFS) 失败" >&2; BOOTFS_MOUNTED=0; }
    BOOTFS_MOUNTED=0
    echo "信息：分离 boot loop 设备 $bootfs_loopdev..."
    sudo losetup -d "$bootfs_loopdev" || { echo "警告：分离 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; }

    echo "信息：扩展镜像文件 (${add_size_mb}MB)..."
    # ... (dd, cat, rm add.img logic) ...

    echo "信息：调整镜像分区大小 (分区 2)..."
    sudo parted -s "$target_image" resizepart 2 100% || { echo "错误：使用 parted 调整分区 2 大小失败" >&2; exit 1; }

    echo "信息：设置 rootfs 分区的 loop 设备..."
    find_loop_device # 找 loop 给 rootfs
    echo "信息：将 $target_image (偏移 $rootfs_offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$rootfs_offset" "$LOOPDEV" "$target_image" || { echo "错误：设置 rootfs 分区 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：检查并调整文件系统大小 (在 loop 设备上)..."
    sudo e2fsck -f -y "$LOOPDEV" || { echo "警告：e2fsck 检查 $LOOPDEV 失败" >&2; exit 1; }
    sudo resize2fs "$LOOPDEV" || { echo "错误：resize2fs 调整 $LOOPDEV 大小失败" >&2; exit 1; }

    echo "信息：Octopus-Planet Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

# --- 特定设备的文件配置函数 ---

config_cumebox2_files() {
    echo "信息：为 Cumebox2 配置特定文件 (OLED, DTB)..."
    ensure_dir "$ROOTFS/etc/oled"
    # 注意 DTB 路径可能需要根据实际 Armbian 版本调整
    sudo cp "$SRCPATH/image/cumebox2/v-fix.dtb" "$ROOTFS/boot/dtb/amlogic/meson-gxl-s905x-khadas-vim.dtb" || echo "警告：复制 Cumebox2 DTB 失败"
    sudo cp "$SRCPATH/image/cumebox2/ssd" "$ROOTFS/usr/bin/" || echo "警告：复制 Cumebox2 ssd 脚本失败"
	sudo chmod +x "$ROOTFS/usr/bin/ssd" || echo "警告：设置 ssd 脚本执行权限失败"
    sudo cp "$SRCPATH/image/cumebox2/config.json" "$ROOTFS/etc/oled/config.json" || echo "警告：复制 OLED 配置文件失败"
}

config_octopus_flanet_files() {
    echo "信息：为 Octopus-Planet 配置特定文件 (model_database.conf)..."
    sudo cp "$SRCPATH/image/octopus-flanet/model_database.conf" "$ROOTFS/etc/model_database.conf" || echo "警告：复制 model_database.conf 失败"
}


# --- 构建流程函数 ---

build_target() {
    local target="$1"
    local build_time=$(date "+%Y-%m-%d %H:%M:%S")
    echo "=================================================="
    echo "信息：构建目标: $target"
    echo "信息：构建时间: $build_time"
    echo "=================================================="

    # 设置全局变量，供后续函数使用
    TARGET_DEVICE_NAME="$target"
    NEED_PREPARE_DNS=false # 默认不需要准备 DNS

    case "$target" in
        onecloud)
            onecloud_rootfs
            local arch="armhf"
            local device_type="gpio"
            local network_type="systemd-networkd"
            ;;
        cumebox2)
            cumebox2_rootfs
            local arch="aarch64"
            local device_type="video1"
            local network_type="" # 默认 NetworkManager
            NEED_PREPARE_DNS=true
            ;;
        chainedbox)
            chainedbox_rootfs_and_fix_dtb
            local arch="aarch64"
            local device_type="video1"
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        vm)
            vm_rootfs
            local arch="amd64"
            local device_type=""
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        e900v22c)
            e900v22c_rootfs
            local arch="aarch64"
            local device_type="video1"
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        octopus-flanet)
            octopus_flanet_rootfs
            local arch="aarch64"
            local device_type="video1"
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        *)
            echo "错误：未知或不支持的目标 '$target'" >&2
            exit 1
            ;;
    esac

    mount_rootfs

    install_and_configure_kvmd "$arch" "$device_type" "$network_type"

    write_meta "$target"
    
    unmount_all

    case "$target" in
        onecloud)
            pack_img_onecloud
            ;;
        vm)
            pack_img "Vm"
            ;;
        cumebox2)
            pack_img "Cumebox2"
            ;;
        chainedbox)
            pack_img "Chainedbox"
            ;;
         e900v22c)
            pack_img "E900v22c"
            ;;
         octopus-flanet)
            pack_img "Octopus-Flanet"
            ;;
        *)
            echo "错误：未知的打包类型 for '$target'" >&2
            ;;
    esac

    echo "=================================================="
    echo "信息：目标 $target 构建完成！"
    echo "=================================================="
}


# --- 主逻辑 ---

# 检查是否提供了目标参数
if [ -z "$1" ]; then
    echo "用法: $0 <target|all>"
    echo "可用目标: onecloud, cumebox2, chainedbox, vm, e900v22c, octopus-flanet"
    exit 1
fi

# 设置脚本立即退出模式
set -eo pipefail

# 检查必要的外部工具
for cmd in sudo docker losetup mount umount parted e2fsck resize2fs qemu-img curl tar python3 pip3 rsync git simg2img img2simg dd cat rm mkdir mv cp sed chmod chown ln grep printf id; do
    if ! command -v "$cmd" &> /dev/null; then
        echo "错误：必需的命令 '$cmd' 未找到。请安装相应软件包。" >&2
        exit 1
    fi
done
# 检查特定工具 (如果脚本中使用了)
if ! command -v "$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64" &> /dev/null && [[ "$1" == "onecloud" || "$1" == "all" ]]; then
     if [ -f "$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64" ]; then
        echo "信息：找到 AmlImg 工具，尝试设置执行权限..."
        sudo chmod +x "$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64" || echo "警告：设置 AmlImg 执行权限失败"
     else
        echo "错误：构建 onecloud 需要 '$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64'，但未找到。" >&2
     fi
fi

# 执行构建
if [ "$1" = "all" ]; then
    echo "信息：开始构建所有目标..."
    build_target "onecloud"
    build_target "cumebox2"
    build_target "chainedbox"
    build_target "vm"
    build_target "e900v22c"
    build_target "octopus-flanet"
    echo "信息：所有目标构建完成。"
else
    build_target "$1"
fi

exit 0