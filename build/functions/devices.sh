#!/bin/bash

# --- 设备特定的 Rootfs 准备函数 ---

onecloud_rootfs() {
    local unpacker="$SRCPATH/image/onecloud/AmlImg_v0.3.1_linux_amd64"
    local source_image="$SRCPATH/image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal_support-dvd-emulation.burn.img"
    local bootfs_img="$TMPDIR/bootfs.img"
    local rootfs_img="$TMPDIR/rootfs.img"
    local bootfs_sparse="$TMPDIR/6.boot.PARTITION.sparse"
    local rootfs_sparse="$TMPDIR/7.rootfs.PARTITION.sparse"
    local bootfs_loopdev="" # 存储 bootfs 使用的 loop 设备
    local add_size_mb=600

    echo "信息：准备 Onecloud Rootfs..."
    ensure_dir "$TMPDIR"
    ensure_dir "$BOOTFS"

    # 自动下载 AmlImg 工具（如果不存在）
    download_file_if_missing "$unpacker" || { echo "错误：下载 AmlImg 工具失败" >&2; exit 1; }
    sudo chmod +x "$unpacker" || { echo "错误：设置 AmlImg 工具执行权限失败" >&2; exit 1; }

    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Onecloud 原始镜像失败" >&2; exit 1; }

    echo "信息：解包 Onecloud burn 镜像..."
    sudo "$unpacker" unpack "$source_image" "$TMPDIR" || { echo "错误：解包失败" >&2; exit 1; }

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
    
    # 自动下载 DTB 文件（如果不存在）
    local dtb_file="$SRCPATH/image/onecloud/meson8b-onecloud-fix.dtb"
    download_file_if_missing "$dtb_file" || { echo "错误：下载 Onecloud DTB 文件失败" >&2; exit 1; }
    
    sudo cp "$dtb_file" "$BOOTFS/dtb/meson8b-onecloud.dtb" || { echo "错误：复制修复后的 DTB 文件失败" >&2; exit 1; }
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
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Cumebox2 原始镜像失败" >&2; exit 1; }
    
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
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Chainedbox 原始镜像失败" >&2; exit 1; }
    
    cp "$source_image" "$target_image" || { echo "错误：复制 Chainedbox 原始镜像失败" >&2; exit 1; }

    echo "信息：挂载 boot 分区并修复 DTB..."
    find_loop_device # 找 loop 给 boot
    bootfs_loopdev="$LOOPDEV"
    echo "信息：将 $target_image (偏移 $boot_offset) 关联到 $bootfs_loopdev..."
    sudo losetup --offset "$boot_offset" "$bootfs_loopdev" "$target_image" || { echo "错误：设置 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; exit 1; }
    sudo mount "$bootfs_loopdev" "$BOOTFS" || { echo "错误：挂载 boot 分区 ($bootfs_loopdev) 失败" >&2; exit 1; }
    BOOTFS_MOUNTED=1
    
    # 自动下载 DTB 文件（如果不存在）
    local dtb_file="$SRCPATH/image/chainedbox/rk3328-l1pro-1296mhz-fix.dtb"
    download_file_if_missing "$dtb_file" || { echo "错误：下载 Chainedbox DTB 文件失败" >&2; exit 1; }
    
    sudo cp "$dtb_file" "$BOOTFS/dtb/rockchip/rk3328-l1pro-1296mhz.dtb" || { echo "错误：复制修复后的 DTB 文件失败" >&2; exit 1; }
    sudo umount "$BOOTFS" || { echo "警告：卸载 boot 分区 ($BOOTFS) 失败" >&2; BOOTFS_MOUNTED=0; }
    BOOTFS_MOUNTED=0
    echo "信息：分离 boot loop 设备 $bootfs_loopdev..."
    sudo losetup -d "$bootfs_loopdev" || { echo "警告：分离 boot 分区 loop 设备 $bootfs_loopdev 失败" >&2; }

    echo "信息：设置 rootfs 分区的 loop 设备..."
    find_loop_device # 找 loop 给 rootfs
    echo "信息：将 $target_image (偏移 $rootfs_offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$rootfs_offset" "$LOOPDEV" "$target_image" || { echo "错误：设置 rootfs 分区 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：Chainedbox Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

vm_rootfs() {
    local source_image="$SRCPATH/image/vm/Armbian_25.2.1_Uefi-x86_bookworm_current_6.12.13_minimal.img"
    local target_image="$TMPDIR/rootfs.img"
    local offset=$((540672 * 512))

    echo "信息：准备 Vm Rootfs..."
    ensure_dir "$TMPDIR"
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Vm 原始镜像失败" >&2; exit 1; }
    
    cp "$source_image" "$target_image" || { echo "错误：复制 Vm 原始镜像失败" >&2; exit 1; }

    echo "信息：设置带偏移量的 loop 设备..."
    find_loop_device # 查找设备名
    echo "信息：将 $target_image (偏移 $offset) 关联到 $LOOPDEV..."
    sudo losetup --offset "$offset" "$LOOPDEV" "$target_image" || { echo "错误：设置带偏移量的 loop 设备 $LOOPDEV 失败" >&2; exit 1; }

    echo "信息：Vm Rootfs 准备完成，loop 设备 $LOOPDEV 已就绪。"
}

e900v22c_rootfs() {
    local source_image="$SRCPATH/image/e900v22c/Armbian_23.08.0_amlogic_s905l3a_bookworm_5.15.123_server_2023.08.01.img"
    local target_image="$TMPDIR/rootfs.img"
    local offset=$((532480 * 512))
    local add_size_mb=600

    echo "信息：准备 E900V22C Rootfs..."
    ensure_dir "$TMPDIR"
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 E900V22C 原始镜像失败" >&2; exit 1; }
    
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
    local add_size_mb=600
    local bootfs_loopdev=""

    echo "信息：准备 Octopus-Planet Rootfs..."
    ensure_dir "$TMPDIR"; ensure_dir "$BOOTFS"
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Octopus-Planet 原始镜像失败" >&2; exit 1; }
    
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

onecloud_pro_rootfs() {
    local source_image="$SRCPATH/image/onecloud-pro/Armbian-by-SilentWind_24.5.0_amlogic_Onecloud-Pro_jammy_6.6.28_server.img"
    local target_image="$TMPDIR/rootfs.img"
    local boot_offset=$((8192 * 512))
    local rootfs_offset=$((1056768 * 512))
    local add_size_mb=600
    local bootfs_loopdev=""

    echo "信息：准备 Octopus-Planet Rootfs..."
    ensure_dir "$TMPDIR"; ensure_dir "$BOOTFS"
    
    # 自动下载源镜像文件（如果不存在）
    download_file_if_missing "$source_image" || { echo "错误：下载 Octopus-Planet 原始镜像失败" >&2; exit 1; }
    
    cp "$source_image" "$target_image" || { echo "错误：复制 Octopus-Planet 原始镜像失败" >&2; exit 1; }

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
    
    # 自动下载 Cumebox2 相关文件（如果不存在）
    local dtb_file="$SRCPATH/image/cumebox2/v-fix.dtb"
    local ssd_file="$SRCPATH/image/cumebox2/ssd"
    local config_file="$SRCPATH/image/cumebox2/config.json"
    
    download_file_if_missing "$dtb_file" || echo "警告：下载 Cumebox2 DTB 失败"
    download_file_if_missing "$ssd_file" || echo "警告：下载 Cumebox2 ssd 脚本失败"
    download_file_if_missing "$config_file" || echo "警告：下载 Cumebox2 配置文件失败"
    
    # 注意 DTB 路径可能需要根据实际 Armbian 版本调整
    sudo cp "$dtb_file" "$ROOTFS/boot/dtb/amlogic/meson-gxl-s905x-khadas-vim.dtb" || echo "警告：复制 Cumebox2 DTB 失败"
    sudo cp "$ssd_file" "$ROOTFS/usr/bin/" || echo "警告：复制 Cumebox2 ssd 脚本失败"
	sudo chmod +x "$ROOTFS/usr/bin/ssd" || echo "警告：设置 ssd 脚本执行权限失败"
    sudo cp "$config_file" "$ROOTFS/etc/oled/config.json" || echo "警告：复制 OLED 配置文件失败"
}

config_octopus_flanet_files() {
    echo "信息：为 Octopus-Planet 配置特定文件 (model_database.conf)..."
    
    # 自动下载 Octopus-Planet 相关文件（如果不存在）
    local config_file="$SRCPATH/image/octopus-flanet/model_database.conf"
    
    download_file_if_missing "$config_file" || echo "警告：下载 Octopus-Planet 配置文件失败"
    
    sudo cp "$config_file" "$ROOTFS/etc/model_database.conf" || echo "警告：复制 model_database.conf 失败"
} 