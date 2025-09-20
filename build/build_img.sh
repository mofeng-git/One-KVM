#!/bin/bash

# --- 配置 ---
# 允许通过环境变量覆盖默认路径
SRCPATH="${SRCPATH:-/mnt/src}"
BOOTFS="${BOOTFS:-/tmp/bootfs}"
ROOTFS="${ROOTFS:-/tmp/rootfs}"
OUTPUTDIR="${OUTPUTDIR:-/mnt/output}"
TMPDIR="${TMPDIR:-$SRCPATH/tmp}"

# 远程文件下载配置
REMOTE_PREFIX="${REMOTE_PREFIX:-https://files.mofeng.run/src}"

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

# --- 引入模块化脚本 ---
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
source "$SCRIPT_DIR/functions/common.sh"
source "$SCRIPT_DIR/functions/devices.sh"
source "$SCRIPT_DIR/functions/install.sh"
source "$SCRIPT_DIR/functions/packaging.sh"

# 获取日期与Git版本
GIT_COMMIT_ID=$(get_git_commit_id)
DATE=$(date +%y%m%d)
if [ -n "$GIT_COMMIT_ID" ]; then
    DATE="${DATE}-${GIT_COMMIT_ID}"
fi

# --- 注册清理函数 ---
# 在脚本退出、收到错误信号、中断信号、终止信号时执行 cleanup
trap cleanup EXIT ERR INT TERM

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
            local device_type="gpio-onecloud"
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
        onecloud-pro)
            onecloud_pro_rootfs
            local arch="aarch64"
            local device_type="gpio-onecloud-pro video1"
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        orangepi-zero)
            orangepizero_rootfs
            local arch="armhf"
            local device_type=""
            local network_type=""
            NEED_PREPARE_DNS=true
            ;;
        oec-turbo)
            oec_turbo_rootfs
            local arch="aarch64"
            local device_type="vpu"
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
        onecloud-pro)
            pack_img "Onecloud-Pro"
            ;;
        orangepi-zero)
            pack_img "Orangepi-Zero"
            ;;
        oec-turbo)
            pack_img "OEC-Turbo"
            ;;
        *)
            echo "错误：未知的打包类型 for '$target'" >&2
            ;;
    esac

    # 在 GitHub Actions 环境中清理下载的文件
    cleanup_downloaded_files

    echo "=================================================="
    echo "信息：目标 $target 构建完成！"
    echo "=================================================="
}

# --- 主逻辑 ---

# 检查是否提供了目标参数
if [ -z "$1" ]; then
    echo "用法: $0 <target|all>"
    echo "可用目标: onecloud, cumebox2, chainedbox, vm, e900v22c, octopus-flanet, onecloud-pro, orangepi-zero, oec-turbo"
    exit 1
fi

# 设置脚本立即退出模式
set -eo pipefail

# 检查必要的外部工具
check_required_tools "$1"

# 执行构建
if [ "$1" = "all" ]; then
    echo "信息：开始构建所有目标..."
    build_target "onecloud"
    build_target "cumebox2"
    build_target "chainedbox"
    build_target "vm"
    build_target "e900v22c"
    build_target "octopus-flanet"
    build_target "onecloud-pro"
    build_target "orangepi-zero"
    build_target "oec-turbo"
    echo "信息：所有目标构建完成。"
else
    build_target "$1"
fi

exit 0