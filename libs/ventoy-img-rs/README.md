# ventoy-img

纯 Rust 实现的 Ventoy 可启动镜像生成工具。无需 root 权限或 loop 设备即可创建完整可用的 Ventoy IMG 文件。

## 特性

- **纯 Rust 实现**: 无外部依赖，单一可执行文件
- **无需 root**: 不需要 loop 设备或管理员权限
- **内嵌资源**: 所有 Ventoy 启动文件内嵌于二进制中
- **完整 exFAT 支持**: 手写 exFAT 实现，支持大于 4GB 的 ISO 文件
- **流式读写**: 支持大文件流式读写，内存占用低
- **Unicode 支持**: 完整的 Unicode 文件名支持（中日韩、西里尔、希腊字母、Emoji 等）
- **动态簇大小**: 根据卷大小自动选择最优簇大小（4KB-128KB）
- **跨平台**: 支持 Linux、macOS、Windows

## 快速开始

### 编译

```bash
cargo build --release
```

### 创建镜像

```bash
# 创建 8GB Ventoy 镜像
./target/release/ventoy-img create -s 8G -o ventoy.img

# 添加 ISO 文件
./target/release/ventoy-img add ventoy.img ubuntu.iso
./target/release/ventoy-img add ventoy.img windows.iso

# 列出文件
./target/release/ventoy-img list ventoy.img

# 写入 U 盘
sudo dd if=ventoy.img of=/dev/sdX bs=4M status=progress
```

## 命令

```
ventoy-img <COMMAND>

Commands:
  create  创建新的 Ventoy IMG 文件
  add     添加文件到镜像
  list    列出镜像中的文件
  remove  从镜像删除文件
  info    显示镜像信息
```

### create

```bash
ventoy-img create [OPTIONS]

Options:
  -s, --size <SIZE>      镜像大小 (如 8G, 16G, 1024M) [默认: 8G]
  -o, --output <OUTPUT>  输出文件路径 [默认: ventoy.img]
  -L, --label <LABEL>    数据分区卷标 [默认: Ventoy]
```

### add

```bash
ventoy-img add <IMAGE> <FILE>
```

### list

```bash
ventoy-img list <IMAGE>
```

### remove

```bash
ventoy-img remove <IMAGE> <NAME>
```

### info

```bash
ventoy-img info <IMAGE>
```

## 作为库使用

```rust
use ventoy_img::{VentoyImage, Result};
use std::path::Path;

fn main() -> Result<()> {
    // 创建镜像
    let mut img = VentoyImage::create(
        Path::new("ventoy.img"),
        "8G",
        "Ventoy"
    )?;

    // 添加文件
    img.add_file(Path::new("ubuntu.iso"))?;

    // 列出文件
    for file in img.list_files()? {
        println!("{}: {} bytes", file.name, file.size);
    }

    Ok(())
}
```

## 文档

- [CLI 使用说明](docs/CLI.md) - 命令行工具详细用法
- [库使用说明](docs/LIBRARY.md) - Rust 库 API 参考
- [技术文档](docs/TECHNICAL.md) - 内部实现细节

## 镜像结构

```
┌────────────────────────────────────────────────────────────┐
│ MBR (512 bytes) - 引导代码 + 分区表                        │
├────────────────────────────────────────────────────────────┤
│ GRUB core.img (Sector 1-2047) - BIOS 引导                  │
├────────────────────────────────────────────────────────────┤
│ 数据分区 (exFAT) - 存放 ISO/IMG 文件                       │
├────────────────────────────────────────────────────────────┤
│ EFI 分区 (FAT16, 32MB) - UEFI 引导                         │
└────────────────────────────────────────────────────────────┘
```

## 依赖

- `clap` - 命令行解析
- `thiserror` - 错误处理
- `lzma-rs` - XZ 解压缩
- `chrono` - 时间处理
- `crc32fast` - CRC32 校验

## 许可证

GPL-3.0

## 致谢

- [Ventoy](https://www.ventoy.net/) - 原始项目
- [GRUB](https://www.gnu.org/software/grub/) - 引导加载器
