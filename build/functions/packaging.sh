#!/bin/bash

# --- 压缩函数 ---

# 压缩镜像文件（仅在 GitHub Actions 环境中）
compress_image_file() {
    local file_path="$1"
    
    if is_github_actions && [[ -f "$file_path" ]]; then
        echo "信息：压缩镜像文件: $file_path"
        if xz -9 -vv "$file_path"; then
            echo "信息：压缩完成: ${file_path}.xz"
        else
            echo "警告：压缩文件 $file_path 失败"
        fi
    fi
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
        
        # 在 GitHub Actions 环境中压缩 VM 镜像文件
        if is_github_actions; then
            echo "信息：在 GitHub Actions 环境中压缩 VM 镜像文件..."
            compress_image_file "$raw_img"
            compress_image_file "$vmdk_img"
            compress_image_file "$vdi_img"
        fi
    else
        # 在 GitHub Actions 环境中压缩镜像文件
        if is_github_actions; then
            echo "信息：在 GitHub Actions 环境中压缩镜像文件..."
            compress_image_file "$OUTPUTDIR/$target_img_name"
        fi
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

    # 自动下载 AmlImg 工具（如果不存在）
    download_file_if_missing "$aml_packer" || { echo "错误：下载 AmlImg 工具失败" >&2; exit 1; }
    sudo chmod +x "$aml_packer" || { echo "错误：设置 AmlImg 工具执行权限失败" >&2; exit 1; }

    echo "信息：将 raw rootfs 转换为 sparse image..."
    # 先删除可能存在的旧 sparse 文件
    sudo rm -f "$rootfs_sparse_img"
    sudo img2simg "$rootfs_raw_img" "$rootfs_sparse_img" || { echo "错误：img2simg 转换失败" >&2; exit 1; }
    sudo rm "$rootfs_raw_img" # 删除 raw 文件，因为它已被转换

    echo "信息：使用 AmlImg 工具打包..."
    sudo "$aml_packer" pack "$OUTPUTDIR/$target_img_name" "$TMPDIR/" || { echo "错误：AmlImg 打包失败" >&2; exit 1; }

    echo "信息：清理 Onecloud 临时文件..."
    sudo rm -f "$TMPDIR/6.boot.PARTITION.sparse" "$TMPDIR/7.rootfs.PARTITION.sparse" "$TMPDIR/dts.img"

    # 在 GitHub Actions 环境中压缩 Onecloud 镜像文件
    if is_github_actions; then
        echo "信息：在 GitHub Actions 环境中压缩 Onecloud 镜像文件..."
        compress_image_file "$OUTPUTDIR/$target_img_name"
    fi

    echo "信息：Onecloud burn 镜像打包完成: $OUTPUTDIR/$target_img_name"
} 