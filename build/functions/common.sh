#!/bin/bash

# --- 辅助函数 ---

# 获取 Git 提交 ID
get_git_commit_id() {
    if git rev-parse --is-inside-work-tree &>/dev/null; then
        git rev-parse --short HEAD 2>/dev/null || echo ""
    else
        echo ""
    fi
}

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

# 挂载根文件系统
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

# 设置元数据
write_meta() {
    local hostname="$1"
    echo "信息：在 chroot 环境中设置主机名/元数据为 $hostname ..."
    run_in_chroot "sed -i 's/localhost.localdomain/$hostname/g' /etc/kvmd/meta.yaml"
}

# 检测是否在 GitHub Actions 环境中
is_github_actions() {
    [[ -n "$GITHUB_ACTIONS" ]]
}

# 记录下载的文件列表（仅在 GitHub Actions 环境中）
DOWNLOADED_FILES_LIST="/tmp/downloaded_files.txt"

# 自动下载文件函数
download_file_if_missing() {
    local file_path="$1"
    local relative_path=""
    
    # 如果文件已存在，直接返回
    if [[ -f "$file_path" ]]; then
        echo "信息：文件已存在: $file_path"
        return 0
    fi
    
    # 计算相对于 SRCPATH 的路径
    if [[ "$file_path" == "$SRCPATH"/* ]]; then
        relative_path="${file_path#$SRCPATH/}"
    else
        echo "错误：文件路径 $file_path 不在 SRCPATH ($SRCPATH) 下" >&2
        return 1
    fi
    
    echo "信息：文件不存在，尝试下载: $file_path"
    echo "信息：相对路径: $relative_path"
    
    # 确保目标目录存在
    local target_dir="$(dirname "$file_path")"
    ensure_dir "$target_dir"
    
    # 首先尝试直接下载
    local remote_url="${REMOTE_PREFIX}/${relative_path}"
    echo "信息：尝试下载: $remote_url"
    
    if curl -f -L -o "$file_path" "$remote_url" 2>/dev/null; then
        echo "信息：下载成功: $file_path"
        # 在 GitHub Actions 环境中记录下载的文件
        if is_github_actions; then
            echo "$file_path" >> "$DOWNLOADED_FILES_LIST"
        fi
        return 0
    fi
    
    # 如果直接下载失败，尝试添加 .xz 后缀
    echo "信息：直接下载失败，尝试 .xz 压缩版本..."
    local xz_url="${remote_url}.xz"
    local xz_file="${file_path}.xz"
    
    if curl -f -L -o "$xz_file" "$xz_url" 2>/dev/null; then
        echo "信息：下载 .xz 文件成功，正在解压..."
        if xz -d "$xz_file"; then
            echo "信息：解压成功: $file_path"
            # 在 GitHub Actions 环境中记录下载的文件
            if is_github_actions; then
                echo "$file_path" >> "$DOWNLOADED_FILES_LIST"
            fi
            return 0
        else
            echo "错误：解压 .xz 文件失败" >&2
            rm -f "$xz_file"
            return 1
        fi
    fi
    
    echo "错误：无法下载文件 $file_path (尝试了原始版本和 .xz 版本)" >&2
    return 1
}

# 下载 rc.local 文件
download_rc_local() {
    local platform_id="$1"
    local rc_local_path="$SRCPATH/image/$platform_id/rc.local"
    local relative_path="image/$platform_id/rc.local"
    local remote_url="$REMOTE_PREFIX/$relative_path"

    echo "信息：检查是否需要下载 rc.local 文件 ($platform_id)..."

    # 如果本地文件不存在，尝试下载
    if [ ! -f "$rc_local_path" ]; then
        echo "信息：本地 rc.local 文件不存在，尝试从远程下载..."
        ensure_dir "$(dirname "$rc_local_path")"

        if curl -sSL --fail "$remote_url" -o "$rc_local_path"; then
            echo "信息：成功下载 rc.local 文件：$remote_url"
            # 在 GitHub Actions 环境中记录下载的文件
            if is_github_actions; then
                echo "$rc_local_path" >> "$DOWNLOADED_FILES_LIST"
            fi
            return 0
        else
            echo "信息：远程 rc.local 文件不存在或下载失败：$remote_url"
            return 1
        fi
    else
        echo "信息：使用本地 rc.local 文件：$rc_local_path"
        return 0
    fi
}

# 清理下载的文件（仅在 GitHub Actions 环境中）
cleanup_downloaded_files() {
    if is_github_actions && [[ -f "$DOWNLOADED_FILES_LIST" ]]; then
        echo "信息：清理 GitHub Actions 环境中下载的文件..."
        while IFS= read -r file_path; do
            if [[ -f "$file_path" ]]; then
                echo "信息：删除下载的文件: $file_path"
                rm -f "$file_path"
            fi
        done < "$DOWNLOADED_FILES_LIST"
        rm -f "$DOWNLOADED_FILES_LIST"
        echo "信息：下载文件清理完成"
    fi
}

# 检查必要的外部工具
check_required_tools() {
    local required_tools="sudo docker losetup mount umount parted e2fsck resize2fs qemu-img curl tar python3 pip3 rsync git simg2img img2simg dd cat rm mkdir mv cp sed chmod chown ln grep printf id xz"
    
    for cmd in $required_tools; do
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
} 