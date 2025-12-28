# ventoy-img CLI 使用说明

## 安装

### 从源码编译

```bash
# 克隆仓库
git clone https://github.com/user/ventoy-img-rs.git
cd ventoy-img-rs

# 编译 release 版本
cargo build --release

# 二进制文件位于 target/release/ventoy-img
```

### 直接使用

```bash
# 复制到 PATH
sudo cp target/release/ventoy-img /usr/local/bin/

# 或添加别名
alias ventoy-img='/path/to/ventoy-img-rs/target/release/ventoy-img'
```

## 命令概览

```
ventoy-img <COMMAND>

Commands:
  create  创建新的 Ventoy IMG 文件
  add     添加文件到镜像（支持子目录和覆盖）
  list    列出镜像中的文件（支持递归列出）
  remove  从镜像删除文件或目录（支持递归删除）
  mkdir   创建目录（支持递归创建父目录）
  info    显示镜像信息
  help    显示帮助信息
```

## 命令详解

### create - 创建镜像

创建新的 Ventoy 可启动 IMG 文件。

```bash
ventoy-img create [OPTIONS]
```

**选项：**

| 选项 | 简写 | 默认值 | 说明 |
|------|------|--------|------|
| `--size` | `-s` | `8G` | 镜像大小 |
| `--output` | `-o` | `ventoy.img` | 输出文件路径 |
| `--label` | `-L` | `Ventoy` | 数据分区卷标 |

**大小格式：**
- `G` 或 `GB`: 千兆字节，如 `8G`, `16G`
- `M` 或 `MB`: 兆字节，如 `512M`, `1024M`
- 纯数字: 字节数，如 `8589934592`

**示例：**

```bash
# 创建 8GB 镜像（默认）
ventoy-img create

# 创建 16GB 镜像，指定输出路径
ventoy-img create -s 16G -o /path/to/my-ventoy.img

# 创建 512MB 小镜像，自定义卷标
ventoy-img create -s 512M -o small.img -L "MyUSB"

# 创建 32GB 镜像
ventoy-img create --size 32G --output ventoy-32g.img --label "Ventoy32"
```

**输出示例：**

```
========================================
  Ventoy IMG Creator (Rust Edition)
========================================

[INFO] Creating 8192MB image: ventoy.img
[INFO] Writing boot code...
[INFO] Writing MBR partition table...
  Data partition: sector 2048 - 16744447 (8160 MB)
  EFI partition:  sector 16744448 - 16809983 (32 MB)
[INFO] Writing Ventoy signature...
[INFO] Writing EFI partition...
[INFO] Formatting data partition as exFAT...
[INFO] Ventoy IMG created successfully!

========================================
Image: ventoy.img
Size:  8G
Label: Ventoy
========================================
```

### add - 添加文件

将 ISO/IMG 文件添加到 Ventoy 镜像的数据分区。

```bash
ventoy-img add [OPTIONS] <IMAGE> <FILE>
```

**参数：**
- `IMAGE`: Ventoy IMG 文件路径
- `FILE`: 要添加的文件路径

**选项：**

| 选项 | 简写 | 说明 |
|------|------|------|
| `--dest` | `-d` | 目标路径（支持子目录，如 `iso/linux/ubuntu.iso`） |
| `--force` | `-f` | 覆盖已存在的文件 |
| `--parents` | `-p` | 自动创建父目录 |

**示例：**

```bash
# 添加单个 ISO 到根目录
ventoy-img add ventoy.img ubuntu-22.04-desktop-amd64.iso

# 添加到子目录（目录必须存在）
ventoy-img add ventoy.img ubuntu.iso -d iso/linux/ubuntu.iso

# 添加到子目录并自动创建父目录
ventoy-img add ventoy.img ubuntu.iso -d iso/linux/ubuntu.iso -p

# 覆盖已存在的文件
ventoy-img add ventoy.img new-ubuntu.iso -d iso/linux/ubuntu.iso -f

# 组合使用：创建目录 + 覆盖
ventoy-img add ventoy.img ubuntu.iso -d iso/linux/ubuntu.iso -p -f
```

**批量添加（使用 shell）：**

```bash
# 添加目录下所有 ISO 到根目录
for iso in /path/to/isos/*.iso; do
    ventoy-img add ventoy.img "$iso"
done

# 添加到子目录并保持目录结构
for iso in /path/to/isos/*.iso; do
    ventoy-img add ventoy.img "$iso" -d "iso/$(basename "$iso")" -p
done
```

### list - 列出文件

列出镜像数据分区中的文件。

```bash
ventoy-img list [OPTIONS] <IMAGE>
```

**选项：**

| 选项 | 简写 | 说明 |
|------|------|------|
| `--path` | | 指定要列出的目录路径 |
| `--recursive` | `-r` | 递归列出所有文件和目录 |

**示例：**

```bash
# 列出根目录
ventoy-img list ventoy.img

# 列出指定目录
ventoy-img list ventoy.img --path iso/linux

# 递归列出所有文件
ventoy-img list ventoy.img -r
```

**输出示例（根目录）：**

```
NAME                                                SIZE TYPE
------------------------------------------------------------
ubuntu-22.04-desktop-amd64.iso                    3.6 GB FILE
iso                                                  0 B DIR
```

**输出示例（递归）：**

```
PATH                                                          SIZE TYPE
----------------------------------------------------------------------
iso                                                            0 B DIR
iso/linux                                                      0 B DIR
iso/linux/ubuntu.iso                                        3.6 GB FILE
iso/windows                                                    0 B DIR
iso/windows/win11.iso                                       5.2 GB FILE
```

**空镜像输出：**

```
No files in image
```

### remove - 删除文件或目录

从镜像中删除指定文件或目录。

```bash
ventoy-img remove [OPTIONS] <IMAGE> <PATH>
```

**参数：**
- `IMAGE`: Ventoy IMG 文件路径
- `PATH`: 要删除的文件或目录路径

**选项：**

| 选项 | 简写 | 说明 |
|------|------|------|
| `--recursive` | `-r` | 递归删除目录及其内容 |

**示例：**

```bash
# 删除根目录的文件
ventoy-img remove ventoy.img ubuntu.iso

# 删除子目录中的文件
ventoy-img remove ventoy.img iso/linux/ubuntu.iso

# 删除空目录
ventoy-img remove ventoy.img iso/empty-dir

# 递归删除目录及其所有内容
ventoy-img remove ventoy.img iso -r

# 文件名大小写不敏感
ventoy-img remove ventoy.img ISO/LINUX/UBUNTU.ISO
```

**注意：**
- 删除非空目录时必须使用 `-r` 选项
- 递归删除会删除目录下的所有文件和子目录

### mkdir - 创建目录

在镜像中创建目录。

```bash
ventoy-img mkdir [OPTIONS] <IMAGE> <PATH>
```

**参数：**
- `IMAGE`: Ventoy IMG 文件路径
- `PATH`: 要创建的目录路径

**选项：**

| 选项 | 简写 | 说明 |
|------|------|------|
| `--parents` | `-p` | 递归创建父目录（类似 `mkdir -p`） |

**示例：**

```bash
# 创建单级目录
ventoy-img mkdir ventoy.img iso

# 递归创建多级目录
ventoy-img mkdir ventoy.img iso/linux/ubuntu -p

# 创建多个目录
ventoy-img mkdir ventoy.img iso -p
ventoy-img mkdir ventoy.img iso/linux -p
ventoy-img mkdir ventoy.img iso/windows -p
```

### info - 显示信息

显示镜像的详细信息。

```bash
ventoy-img info <IMAGE>
```

**示例：**

```bash
ventoy-img info ventoy.img
```

**输出示例：**

```
Image: ventoy.img

Partition Layout:
  Data partition:
    Start:  sector 2048 (offset 1.0 MB)
    Size:   16742400 sectors (8.0 GB)
  EFI partition:
    Start:  sector 16744448 (offset 8.0 GB)
    Size:   65536 sectors (32 MB)
```

## 使用场景

### 场景 1: 创建多系统启动盘

```bash
# 1. 创建 32GB 镜像
ventoy-img create -s 32G -o multiboot.img

# 2. 添加各种系统 ISO
ventoy-img add multiboot.img ubuntu-22.04.iso
ventoy-img add multiboot.img windows11.iso
ventoy-img add multiboot.img fedora-39.iso
ventoy-img add multiboot.img archlinux.iso

# 3. 查看文件列表
ventoy-img list multiboot.img

# 4. 写入 U 盘
sudo dd if=multiboot.img of=/dev/sdX bs=4M status=progress
```

### 场景 2: 维护现有镜像

```bash
# 查看当前文件（递归）
ventoy-img list ventoy.img -r

# 删除旧版本
ventoy-img remove ventoy.img iso/linux/ubuntu-20.04.iso

# 添加新版本（覆盖）
ventoy-img add ventoy.img ubuntu-24.04.iso -d iso/linux/ubuntu-24.04.iso -f

# 确认更改
ventoy-img list ventoy.img -r
```

### 场景 2.5: 组织文件到子目录

```bash
# 创建目录结构
ventoy-img mkdir ventoy.img iso/linux -p
ventoy-img mkdir ventoy.img iso/windows -p
ventoy-img mkdir ventoy.img iso/tools -p

# 添加文件到对应目录
ventoy-img add ventoy.img ubuntu.iso -d iso/linux/ubuntu.iso
ventoy-img add ventoy.img fedora.iso -d iso/linux/fedora.iso
ventoy-img add ventoy.img win11.iso -d iso/windows/win11.iso
ventoy-img add ventoy.img hiren.iso -d iso/tools/hiren.iso

# 查看目录结构
ventoy-img list ventoy.img -r
```

### 场景 3: 自动化脚本

```bash
#!/bin/bash
# create-ventoy.sh - 自动创建 Ventoy 镜像

ISO_DIR="/path/to/isos"
OUTPUT="ventoy-$(date +%Y%m%d).img"
SIZE="64G"

# 创建镜像
ventoy-img create -s "$SIZE" -o "$OUTPUT" || exit 1

# 添加所有 ISO
for iso in "$ISO_DIR"/*.iso; do
    if [ -f "$iso" ]; then
        echo "Adding: $(basename "$iso")"
        ventoy-img add "$OUTPUT" "$iso" || echo "Failed: $iso"
    fi
done

# 显示结果
echo ""
echo "=== Created: $OUTPUT ==="
ventoy-img list "$OUTPUT"
```

### 场景 4: 在没有 root 权限的环境中使用

```bash
# 在用户目录创建镜像
ventoy-img create -s 8G -o ~/ventoy.img

# 添加文件
ventoy-img add ~/ventoy.img ~/Downloads/linux.iso

# 之后可以用 dd 写入 U 盘（需要 root）
# 或者复制到有权限的机器上写入
```

## 写入 U 盘

创建的 IMG 文件可以直接写入 U 盘：

### Linux

```bash
# 查找 U 盘设备
lsblk

# 写入（替换 sdX 为实际设备）
sudo dd if=ventoy.img of=/dev/sdX bs=4M status=progress conv=fsync

# 或使用 pv 显示进度
pv ventoy.img | sudo dd of=/dev/sdX bs=4M conv=fsync
```

### macOS

```bash
# 查找 U 盘
diskutil list

# 卸载
diskutil unmountDisk /dev/diskN

# 写入
sudo dd if=ventoy.img of=/dev/rdiskN bs=4m

# 弹出
diskutil eject /dev/diskN
```

### Windows

使用 [Rufus](https://rufus.ie/) 或 [balenaEtcher](https://www.balena.io/etcher/)：
1. 选择 ventoy.img 文件
2. 选择目标 U 盘
3. 点击写入

## 常见问题

### Q: 镜像最小可以多大？

A: 最小 64MB（32MB EFI 分区 + 32MB 数据分区）。但实际使用建议至少 512MB。

### Q: 支持多大的 ISO 文件？

A: 理论上支持 exFAT 的最大文件大小（约 16 EB）。实际受限于镜像大小和可用空间。

### Q: 为什么添加文件失败？

可能原因：
1. 镜像空间不足
2. 文件名已存在（使用 `-f` 选项覆盖，或先删除）
3. 目标目录不存在（使用 `-p` 选项自动创建）
4. 文件名包含非法字符
5. 镜像文件损坏

### Q: 如何覆盖已存在的文件？

使用 `-f` 或 `--force` 选项：
```bash
ventoy-img add ventoy.img new-file.iso -d existing-file.iso -f
```

### Q: 如何创建多级目录？

使用 `-p` 或 `--parents` 选项：
```bash
# 创建目录
ventoy-img mkdir ventoy.img path/to/deep/dir -p

# 或在添加文件时自动创建
ventoy-img add ventoy.img file.iso -d path/to/deep/file.iso -p
```

### Q: 如何删除整个目录？

使用 `-r` 或 `--recursive` 选项：
```bash
ventoy-img remove ventoy.img directory-name -r
```

### Q: 如何验证镜像是否正确？

```bash
# 检查分区表
fdisk -l ventoy.img

# 检查 Ventoy 签名
xxd -s 0x190 -l 16 ventoy.img
# 应显示: 5654 0047 6500 4844 0052 6400 2045 720d

# 列出文件
ventoy-img list ventoy.img
```

### Q: 可以在 Windows 上使用吗？

A: 可以。编译 Windows 版本：
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

## 退出码

| 码 | 含义 |
|----|------|
| 0 | 成功 |
| 1 | 错误（详见错误信息） |

## 环境变量

目前不使用任何环境变量。

## 另请参阅

- [技术文档](TECHNICAL.md) - 内部实现细节
- [库使用说明](LIBRARY.md) - Rust 库 API
- [Ventoy 官方文档](https://www.ventoy.net/en/doc_start.html)
