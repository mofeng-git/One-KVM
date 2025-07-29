# 自动下载功能说明

## 概述

构建脚本现在支持自动下载缺失的文件。当构建过程中发现所需的镜像文件、DTB文件、配置文件或工具不存在时，系统会自动尝试从远程服务器下载。

## 功能特性

### 1. 智能下载策略
- **首先尝试直接下载**：使用原始文件名从远程服务器下载
- **失败后尝试压缩版本**：如果直接下载失败，自动尝试添加 `.xz` 后缀下载压缩版本
- **自动解压**：如果下载的是 `.xz` 压缩文件，会自动解压

### 2. 支持的文件类型
- **镜像文件**：各种设备的 Armbian 镜像
- **DTB 文件**：设备树二进制文件
- **配置文件**：JSON、CONF 等配置文件
- **工具文件**：如 AmlImg 工具

### 3. 错误处理
- 文件已存在时跳过下载
- 下载失败时提供详细的错误信息
- 解压失败时自动清理临时文件

## 配置

### 环境变量
```bash
# 远程文件服务器前缀（默认值）
REMOTE_PREFIX="https://files.mofeng.run"

# 本地源码路径（默认值）
SRCPATH="/mnt/src"
```

### 文件路径映射
本地文件路径：`$SRCPATH/image/device/filename`
远程下载URL：`$REMOTE_PREFIX/image/device/filename`

## 使用示例

### 1. 构建特定设备
```bash
# 构建 Cumebox2（会自动下载所需文件）
./build/build_img.sh cumebox2
```

### 2. 构建所有设备
```bash
# 构建所有设备（会自动下载所有所需文件）
./build/build_img.sh all
```

### 3. 自定义远程服务器
```bash
# 使用自定义的远程服务器
REMOTE_PREFIX="https://your-server.com/files" ./build/build_img.sh cumebox2
```

## 支持的文件列表

### Onecloud 设备
- `image/onecloud/AmlImg_v0.3.1_linux_amd64` - AmlImg 工具
- `image/onecloud/Armbian_by-SilentWind_24.5.0-trunk_Onecloud_bookworm_legacy_5.9.0-rc7_minimal_support-dvd-emulation.burn.img` - 源镜像
- `image/onecloud/meson8b-onecloud-fix.dtb` - DTB 文件

### Cumebox2 设备
- `image/cumebox2/Armbian_25.2.2_Khadas-vim1_bookworm_current_6.12.17_minimal.img` - 源镜像
- `image/cumebox2/v-fix.dtb` - DTB 文件
- `image/cumebox2/ssd` - SSD 脚本
- `image/cumebox2/config.json` - OLED 配置文件

### Chainedbox 设备
- `image/chainedbox/Armbian_24.11.0_rockchip_chainedbox_bookworm_6.1.112_server_2024.10.02_add800m.img` - 源镜像
- `image/chainedbox/rk3328-l1pro-1296mhz-fix.dtb` - DTB 文件

### VM 设备
- `image/vm/Armbian_25.2.1_Uefi-x86_bookworm_current_6.12.13_minimal.img` - 源镜像

### E900V22C 设备
- `image/e900v22c/Armbian_23.08.0_amlogic_s905l3a_bookworm_5.15.123_server_2023.08.01.img` - 源镜像

### Octopus-Planet 设备
- `image/octopus-flanet/Armbian_24.11.0_amlogic_s912_bookworm_6.1.114_server_2024.11.01.img` - 源镜像
- `image/octopus-flanet/model_database.conf` - 配置文件

## 技术实现

### 核心函数
```bash
download_file_if_missing(file_path)
```

### 工作流程
1. 检查文件是否已存在
2. 计算相对于 SRCPATH 的路径
3. 确保目标目录存在
4. 尝试直接下载
5. 如果失败，尝试下载 .xz 版本
6. 如果是 .xz 文件，自动解压
7. 返回成功或失败状态

### 依赖工具
- `curl` - 用于下载文件
- `xz` - 用于解压 .xz 文件
- `mkdir` - 用于创建目录

## 注意事项

1. **网络连接**：确保构建环境能够访问远程服务器
2. **磁盘空间**：下载的文件可能很大，确保有足够的磁盘空间
3. **权限**：确保脚本有权限创建目录和文件
4. **防火墙**：确保防火墙允许 HTTP/HTTPS 连接

## 故障排除

### 常见问题

1. **下载失败**
   - 检查网络连接
   - 验证 REMOTE_PREFIX 是否正确
   - 确认远程文件是否存在

2. **解压失败**
   - 检查 xz 工具是否安装
   - 验证下载的文件是否完整

3. **权限错误**
   - 确保脚本有足够的权限
   - 检查目标目录的权限设置

### 调试模式
可以通过设置环境变量来启用更详细的输出：
```bash
export DEBUG=1
./build/build_img.sh cumebox2
``` 