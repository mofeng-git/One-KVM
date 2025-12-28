# ventoy-img 库使用说明

## 安装

### 作为依赖添加

```toml
[dependencies]
ventoy-img = { path = "path/to/ventoy-img-rs" }
```

或发布到 crates.io 后：

```toml
[dependencies]
ventoy-img = "0.1"
```

## 快速开始

```rust
use ventoy_img::{VentoyImage, Result};
use std::path::Path;

fn main() -> Result<()> {
    // 创建 8GB Ventoy 镜像
    let img = VentoyImage::create(
        Path::new("ventoy.img"),
        "8G",
        "Ventoy"
    )?;

    // 打开已有镜像
    let mut img = VentoyImage::open(Path::new("ventoy.img"))?;

    // 列出文件
    for file in img.list_files()? {
        println!("{}: {} bytes", file.name, file.size);
    }

    Ok(())
}
```

## API 参考

### VentoyImage

主要的镜像操作结构体。

#### 创建镜像

```rust
pub fn create(path: &Path, size_str: &str, label: &str) -> Result<Self>
```

创建新的 Ventoy IMG 文件。

**参数：**
- `path`: 输出文件路径
- `size_str`: 镜像大小，支持格式：`"8G"`, `"16G"`, `"1024M"`, `"1073741824"`
- `label`: 数据分区卷标（最长 11 字符）

**示例：**
```rust
// 创建 8GB 镜像
let img = VentoyImage::create(Path::new("ventoy.img"), "8G", "Ventoy")?;

// 创建 512MB 镜像
let img = VentoyImage::create(Path::new("small.img"), "512M", "MyUSB")?;
```

#### 打开镜像

```rust
pub fn open(path: &Path) -> Result<Self>
```

打开已有的 Ventoy IMG 文件。会验证 Ventoy 签名。

**示例：**
```rust
let img = VentoyImage::open(Path::new("ventoy.img"))?;
```

#### 列出文件

```rust
pub fn list_files(&self) -> Result<Vec<FileInfo>>
pub fn list_files_at(&self, path: &str) -> Result<Vec<FileInfo>>
pub fn list_files_recursive(&self) -> Result<Vec<FileInfo>>
```

列出数据分区中的文件。

**返回：**
```rust
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
    pub path: String,  // 完整路径（用于递归列出）
}
```

**示例：**
```rust
let img = VentoyImage::open(Path::new("ventoy.img"))?;

// 列出根目录
for file in img.list_files()? {
    if file.is_directory {
        println!("[DIR]  {}", file.name);
    } else {
        println!("[FILE] {} ({} bytes)", file.name, file.size);
    }
}

// 列出指定目录
for file in img.list_files_at("iso/linux")? {
    println!("{}", file.name);
}

// 递归列出所有文件
for file in img.list_files_recursive()? {
    println!("{}: {} bytes", file.path, file.size);
}
```

#### 添加文件

```rust
pub fn add_file(&mut self, src_path: &Path) -> Result<()>
pub fn add_file_overwrite(&mut self, src_path: &Path, overwrite: bool) -> Result<()>
pub fn add_file_to_path(&mut self, src_path: &Path, dest_path: &str, create_parents: bool, overwrite: bool) -> Result<()>
```

将文件添加到数据分区。

**示例：**
```rust
let mut img = VentoyImage::open(Path::new("ventoy.img"))?;

// 添加到根目录
img.add_file(Path::new("/path/to/ubuntu.iso"))?;

// 添加并覆盖已存在的文件
img.add_file_overwrite(Path::new("/path/to/new-ubuntu.iso"), true)?;

// 添加到子目录（自动创建父目录）
img.add_file_to_path(
    Path::new("/path/to/ubuntu.iso"),
    "iso/linux/ubuntu.iso",
    true,   // create_parents
    false,  // overwrite
)?;

// 添加并覆盖子目录中的文件
img.add_file_to_path(
    Path::new("/path/to/new-ubuntu.iso"),
    "iso/linux/ubuntu.iso",
    false,  // create_parents (目录已存在)
    true,   // overwrite
)?;
```

#### 创建目录

```rust
pub fn create_directory(&mut self, path: &str, create_parents: bool) -> Result<()>
```

在数据分区中创建目录。

**示例：**
```rust
let mut img = VentoyImage::open(Path::new("ventoy.img"))?;

// 创建单级目录
img.create_directory("iso", false)?;

// 递归创建多级目录（类似 mkdir -p）
img.create_directory("iso/linux/ubuntu", true)?;
```

#### 删除文件

```rust
pub fn remove_file(&mut self, name: &str) -> Result<()>
pub fn remove_path(&mut self, path: &str) -> Result<()>
pub fn remove_recursive(&mut self, path: &str) -> Result<()>
```

从数据分区删除文件或目录。

**示例：**
```rust
let mut img = VentoyImage::open(Path::new("ventoy.img"))?;

// 删除根目录的文件
img.remove_file("ubuntu.iso")?;

// 删除子目录中的文件或空目录
img.remove_path("iso/linux/ubuntu.iso")?;
img.remove_path("iso/empty-dir")?;

// 递归删除目录及其所有内容
img.remove_recursive("iso")?;
```

#### 获取分区布局

```rust
pub fn layout(&self) -> &PartitionLayout
```

获取分区布局信息。

**示例：**
```rust
let img = VentoyImage::open(Path::new("ventoy.img"))?;
let layout = img.layout();
println!("Data partition: {} MB", layout.data_size() / 1024 / 1024);
println!("EFI partition: {} MB", layout.efi_size_sectors * 512 / 1024 / 1024);
```

### ExfatFs

底层 exFAT 文件系统操作。

#### 打开文件系统

```rust
pub fn open(path: &Path, layout: &PartitionLayout) -> Result<Self>
```

**示例：**
```rust
use ventoy_img::exfat::ExfatFs;

let layout = PartitionLayout::calculate(8 * 1024 * 1024 * 1024)?;
let mut fs = ExfatFs::open(Path::new("ventoy.img"), &layout)?;
```

#### 写入文件

```rust
// 基本写入（根目录）
pub fn write_file(&mut self, name: &str, data: &[u8]) -> Result<()>

// 带覆盖选项
pub fn write_file_overwrite(&mut self, name: &str, data: &[u8], overwrite: bool) -> Result<()>

// 写入到指定路径
pub fn write_file_path(&mut self, path: &str, data: &[u8], create_parents: bool, overwrite: bool) -> Result<()>
```

**示例：**
```rust
// 写入到根目录
fs.write_file("config.txt", b"content")?;

// 覆盖已存在的文件
fs.write_file_overwrite("config.txt", b"new content", true)?;

// 写入到子目录（自动创建父目录）
fs.write_file_path("iso/linux/config.txt", b"content", true, false)?;
```

#### 读取文件

```rust
pub fn read_file(&mut self, name: &str) -> Result<Vec<u8>>
pub fn read_file_path(&mut self, path: &str) -> Result<Vec<u8>>
```

读取文件内容到内存。

**示例：**
```rust
// 从根目录读取
let data = fs.read_file("config.txt")?;

// 从子目录读取
let data = fs.read_file_path("iso/linux/config.txt")?;
println!("{}", String::from_utf8_lossy(&data));
```

#### 流式读取（大文件）

```rust
pub fn read_file_to_writer<W: Write>(&mut self, name: &str, writer: &mut W) -> Result<u64>
pub fn read_file_path_to_writer<W: Write>(&mut self, path: &str, writer: &mut W) -> Result<u64>
```

流式读取文件到 Writer，适合大文件。返回读取的字节数。

**示例：**
```rust
use std::fs::File;
use std::io::BufWriter;

// 从镜像中提取文件
let mut output = BufWriter::new(File::create("extracted.iso")?);
let bytes = fs.read_file_to_writer("ubuntu.iso", &mut output)?;
println!("Extracted {} bytes", bytes);

// 从子目录流式读取
let mut output = Vec::new();
fs.read_file_path_to_writer("iso/linux/ubuntu.iso", &mut output)?;
```

#### 删除文件

```rust
pub fn delete_file(&mut self, name: &str) -> Result<()>
pub fn delete_path(&mut self, path: &str) -> Result<()>
pub fn delete_recursive(&mut self, path: &str) -> Result<()>
```

删除文件或目录并释放空间。

**示例：**
```rust
// 删除根目录的文件
fs.delete_file("config.txt")?;

// 删除子目录中的文件或空目录
fs.delete_path("iso/linux/config.txt")?;

// 递归删除目录
fs.delete_recursive("iso")?;
```

#### 创建目录

```rust
pub fn create_directory(&mut self, path: &str, create_parents: bool) -> Result<()>
```

**示例：**
```rust
// 创建单级目录
fs.create_directory("iso", false)?;

// 递归创建多级目录
fs.create_directory("iso/linux/ubuntu", true)?;
```

#### 列出文件

```rust
pub fn list_files(&mut self) -> Result<Vec<FileInfo>>
pub fn list_files_at(&mut self, path: &str) -> Result<Vec<FileInfo>>
pub fn list_files_recursive(&mut self) -> Result<Vec<FileInfo>>
```

**示例：**
```rust
// 列出根目录
let files = fs.list_files()?;

// 列出指定目录
let files = fs.list_files_at("iso/linux")?;

// 递归列出所有文件
let all_files = fs.list_files_recursive()?;
```

#### 流式写入（大文件）

```rust
pub fn write_file_from_reader<R: Read>(&mut self, name: &str, reader: &mut R, size: u64) -> Result<()>
pub fn write_file_from_reader_overwrite<R: Read>(&mut self, name: &str, reader: &mut R, size: u64, overwrite: bool) -> Result<()>
pub fn write_file_from_reader_path<R: Read>(&mut self, path: &str, reader: &mut R, size: u64, create_parents: bool, overwrite: bool) -> Result<()>
```

从 Reader 流式写入文件，适合大文件。

**示例：**
```rust
use std::fs::File;
use std::io::BufReader;

let file = File::open("large.iso")?;
let size = file.metadata()?.len();
let mut reader = BufReader::new(file);

// 写入到根目录
fs.write_file_from_reader("large.iso", &mut reader, size)?;

// 写入到子目录并覆盖
fs.write_file_from_reader_path(
    "iso/linux/large.iso",
    &mut reader,
    size,
    true,   // create_parents
    true,   // overwrite
)?;
```

### ExfatFileWriter

手动控制的流式写入器。

```rust
use ventoy_img::exfat::ExfatFileWriter;

// 创建写入器（根目录）
let mut writer = ExfatFileWriter::create(&mut fs, "file.iso", total_size)?;

// 创建写入器（带覆盖选项）
let mut writer = ExfatFileWriter::create_overwrite(&mut fs, "file.iso", total_size, true)?;

// 创建写入器（指定路径，支持创建父目录和覆盖）
let mut writer = ExfatFileWriter::create_at_path(
    &mut fs,
    "iso/linux/file.iso",
    total_size,
    true,   // create_parents
    false,  // overwrite
)?;

// 分块写入
loop {
    let n = source.read(&mut buffer)?;
    if n == 0 { break; }
    writer.write(&buffer[..n])?;
}

// 完成写入（创建目录条目）
writer.finish()?;
```

### ExfatFileReader

手动控制的流式读取器，实现 `std::io::Read` 和 `std::io::Seek` 特征。

```rust
use ventoy_img::exfat::ExfatFileReader;
use std::io::{Read, Seek, SeekFrom};

// 打开文件读取器（根目录）
let mut reader = ExfatFileReader::open(&mut fs, "file.iso")?;

// 打开文件读取器（指定路径）
let mut reader = ExfatFileReader::open_path(&mut fs, "iso/linux/file.iso")?;

// 获取文件信息
println!("File size: {} bytes", reader.file_size());
println!("Current position: {}", reader.position());
println!("Remaining: {} bytes", reader.remaining());

// 读取数据
let mut buffer = [0u8; 4096];
let n = reader.read(&mut buffer)?;

// 读取全部内容
let mut data = Vec::new();
reader.read_to_end(&mut data)?;

// Seek 操作
reader.seek(SeekFrom::Start(1000))?;       // 从开头偏移
reader.seek(SeekFrom::Current(100))?;      // 从当前位置偏移
reader.seek(SeekFrom::End(-100))?;         // 从结尾偏移

// 精确读取
let mut exact_buffer = [0u8; 100];
reader.read_exact(&mut exact_buffer)?;
```

**特性：**
- 实现 `std::io::Read` 和 `std::io::Seek`
- 自动 cluster 缓存，减少 I/O 次数
- 支持任意位置的 seek
- 内存占用低，只缓存当前 cluster

## 错误处理

所有操作返回 `Result<T, VentoyError>`：

```rust
use ventoy_img::{Result, VentoyError};

fn example() -> Result<()> {
    let img = VentoyImage::open(Path::new("ventoy.img"))
        .map_err(|e| {
            match &e {
                VentoyError::Io(io_err) => eprintln!("IO error: {}", io_err),
                VentoyError::ImageError(msg) => eprintln!("Image error: {}", msg),
                VentoyError::FilesystemError(msg) => eprintln!("FS error: {}", msg),
                _ => eprintln!("Error: {}", e),
            }
            e
        })?;
    Ok(())
}
```

## 完整示例

### 创建镜像并添加 ISO

```rust
use ventoy_img::{VentoyImage, Result};
use std::path::Path;

fn main() -> Result<()> {
    // 创建 16GB 镜像
    println!("Creating Ventoy image...");
    let mut img = VentoyImage::create(
        Path::new("ventoy.img"),
        "16G",
        "Ventoy"
    )?;

    // 添加 ISO 文件
    println!("Adding ISO files...");
    img.add_file(Path::new("/isos/ubuntu-22.04.iso"))?;
    img.add_file(Path::new("/isos/windows11.iso"))?;

    // 列出文件
    println!("\nFiles in image:");
    for file in img.list_files()? {
        println!("  {} ({:.1} MB)", file.name, file.size as f64 / 1024.0 / 1024.0);
    }

    println!("\nDone! Write ventoy.img to USB drive.");
    Ok(())
}
```

### 批量处理 ISO 文件

```rust
use ventoy_img::{VentoyImage, Result};
use std::path::Path;
use std::fs;

fn main() -> Result<()> {
    let iso_dir = Path::new("/path/to/isos");

    // 计算所需大小
    let total_size: u64 = fs::read_dir(iso_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "iso"))
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();

    // 添加 1GB 余量 + 32MB EFI
    let image_size = total_size + 1024 * 1024 * 1024 + 32 * 1024 * 1024;
    let size_str = format!("{}M", image_size / 1024 / 1024);

    // 创建镜像
    let mut img = VentoyImage::create(Path::new("ventoy.img"), &size_str, "Ventoy")?;

    // 添加所有 ISO
    for entry in fs::read_dir(iso_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "iso") {
            println!("Adding: ", path.display());
            img.add_file(&path)?;
        }
    }

    Ok(())
}
```

## 注意事项

1. **文件大小限制**: 单个文件最大支持 exFAT 限制（约 16 EB）
2. **文件名**: 最长 255 个 UTF-16 字符，大小写不敏感
3. **Unicode 支持**: 完整支持国际字符（中日韩、西里尔、希腊字母、Emoji 等）
4. **并发**: `ExfatFs` 不是线程安全的，需要外部同步
5. **磁盘空间**: 创建镜像时会预分配全部空间（稀疏文件）
6. **最小大小**: 镜像最小 64MB（32MB EFI + 32MB 数据）
7. **动态簇大小**: 根据卷大小自动选择（<256MB: 4KB, 256MB-8GB: 32KB, >8GB: 128KB）
