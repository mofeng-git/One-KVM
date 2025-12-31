# hwcodec 构建系统与集成指南

## 1. 项目结构

```
libs/hwcodec/
├── Cargo.toml           # 包配置
├── Cargo.lock           # 依赖锁定
├── build.rs             # 构建脚本
└── src/                 # Rust 源码
    ├── lib.rs           # 库入口
    ├── common.rs        # 公共定义
    ├── ffmpeg.rs        # FFmpeg 集成
    └── ffmpeg_ram/      # RAM 编解码
        ├── mod.rs
        ├── encode.rs
        └── decode.rs
└── cpp/                 # C++ 源码
    ├── common/          # 公共代码
    │   ├── log.cpp
    │   ├── log.h
    │   ├── util.cpp
    │   ├── util.h
    │   ├── callback.h
    │   ├── common.h
    │   └── platform/
    │       ├── linux/
    │       │   ├── linux.cpp
    │       │   └── linux.h
    │       └── win/
    │           ├── win.cpp
    │           └── win.h
    ├── ffmpeg_ram/      # FFmpeg RAM 实现
    │   ├── ffmpeg_ram_encode.cpp
    │   ├── ffmpeg_ram_decode.cpp
    │   └── ffmpeg_ram_ffi.h
    └── yuv/             # YUV 处理
        └── yuv.cpp
```

## 2. Cargo 配置

### 2.1 Cargo.toml

```toml
[package]
name = "hwcodec"
version = "0.8.0"
edition = "2021"
description = "Hardware video codec for IP-KVM (Windows/Linux)"

[features]
default = []

[dependencies]
log = "0.4"              # 日志
serde_derive = "1.0"     # 序列化派生宏
serde = "1.0"            # 序列化
serde_json = "1.0"       # JSON 序列化

[build-dependencies]
cc = "1.0"               # C++ 编译
bindgen = "0.59"         # FFI 绑定生成

[dev-dependencies]
env_logger = "0.10"      # 日志输出
```

### 2.2 与原版的区别

| 特性 | 原版 (RustDesk) | 简化版 (One-KVM) |
|------|-----------------|------------------|
| `vram` feature | ✓ | ✗ (已移除) |
| 外部 SDK | 需要 | 不需要 |
| 版本号 | 0.7.1 | 0.8.0 |
| 目标平台 | Windows/Linux/macOS/Android | Windows/Linux |

### 2.3 使用方式

```toml
# 在 One-KVM 项目中使用
[dependencies]
hwcodec = { path = "libs/hwcodec" }
```

## 3. 构建脚本详解 (build.rs)

### 3.1 主入口

```rust
fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut builder = Build::new();

    // 1. 构建公共模块
    build_common(&mut builder);

    // 2. 构建 FFmpeg 相关模块
    ffmpeg::build_ffmpeg(&mut builder);

    // 3. 编译生成静态库
    builder.static_crt(true).compile("hwcodec");
}
```

### 3.2 公共模块构建

```rust
fn build_common(builder: &mut Build) {
    let common_dir = manifest_dir.join("cpp").join("common");

    // 生成 FFI 绑定
    bindgen::builder()
        .header(common_dir.join("common.h"))
        .header(common_dir.join("callback.h"))
        .rustified_enum("*")
        .generate()
        .write_to_file(OUT_DIR.join("common_ffi.rs"));

    // 平台相关代码
    #[cfg(windows)]
    builder.file(common_dir.join("platform/win/win.cpp"));

    #[cfg(target_os = "linux")]
    builder.file(common_dir.join("platform/linux/linux.cpp"));

    // 工具代码
    builder.files([
        common_dir.join("log.cpp"),
        common_dir.join("util.cpp"),
    ]);
}
```

### 3.3 FFmpeg 模块构建

```rust
mod ffmpeg {
    pub fn build_ffmpeg(builder: &mut Build) {
        // 生成 FFmpeg FFI 绑定
        ffmpeg_ffi();

        // 链接 FFmpeg 库
        if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
            link_vcpkg(builder, vcpkg_root.into());
        } else {
            link_system_ffmpeg(builder);  // pkg-config
        }

        // 链接系统库
        link_os();

        // 构建 FFmpeg RAM 模块
        build_ffmpeg_ram(builder);
    }
}
```

### 3.4 FFmpeg 链接方式

#### VCPKG (跨平台静态链接)

```rust
fn link_vcpkg(builder: &mut Build, path: PathBuf) -> PathBuf {
    // 目标平台识别
    let target = match (target_os, target_arch) {
        ("windows", "x86_64") => "x64-windows-static",
        ("linux", arch) => format!("{}-linux", arch),
        _ => panic!("unsupported platform"),
    };

    let lib_path = path.join("installed").join(target).join("lib");

    // 链接 FFmpeg 静态库
    println!("cargo:rustc-link-search=native={}", lib_path);
    ["avcodec", "avutil", "avformat"].iter()
        .for_each(|lib| println!("cargo:rustc-link-lib=static={}", lib));
}
```

#### pkg-config (Linux 动态链接)

```rust
fn link_system_ffmpeg(builder: &mut Build) {
    let libs = ["libavcodec", "libavutil", "libavformat", "libswscale"];

    for lib in &libs {
        // 获取编译标志
        let cflags = Command::new("pkg-config")
            .args(["--cflags", lib])
            .output()?;

        // 获取链接标志
        let libs = Command::new("pkg-config")
            .args(["--libs", lib])
            .output()?;

        // 解析并应用
        for flag in libs.split_whitespace() {
            if flag.starts_with("-L") {
                println!("cargo:rustc-link-search=native={}", &flag[2..]);
            } else if flag.starts_with("-l") {
                println!("cargo:rustc-link-lib={}", &flag[2..]);
            }
        }
    }
}
```

### 3.5 系统库链接

```rust
fn link_os() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    let libs: Vec<&str> = match target_os.as_str() {
        "windows" => vec!["User32", "bcrypt", "ole32", "advapi32"],
        "linux" => vec!["drm", "X11", "stdc++", "z"],
        _ => panic!("unsupported os"),
    };

    for lib in libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
}
```

## 4. FFI 绑定生成

### 4.1 bindgen 配置

```rust
bindgen::builder()
    .header("path/to/header.h")
    .rustified_enum("*")           // 生成 Rust 枚举
    .parse_callbacks(Box::new(Callbacks))  // 自定义回调
    .generate()
    .write_to_file(OUT_DIR.join("ffi.rs"));
```

### 4.2 自定义派生

```rust
#[derive(Debug)]
struct CommonCallbacks;

impl bindgen::callbacks::ParseCallbacks for CommonCallbacks {
    fn add_derives(&self, name: &str) -> Vec<String> {
        // 为特定类型添加序列化支持
        match name {
            "DataFormat" | "SurfaceFormat" | "API" => {
                vec!["Serialize".to_string(), "Deserialize".to_string()]
            }
            _ => vec![],
        }
    }
}
```

### 4.3 生成的文件

| 文件 | 来源 | 内容 |
|------|------|------|
| `common_ffi.rs` | `common.h`, `callback.h` | 枚举、常量、回调类型 |
| `ffmpeg_ffi.rs` | `ffmpeg_ffi.h` | FFmpeg 日志级别、函数 |
| `ffmpeg_ram_ffi.rs` | `ffmpeg_ram_ffi.h` | 编解码器函数 |

## 5. 平台构建指南

### 5.1 Linux 构建

```bash
# 安装 FFmpeg 开发库
sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev

# 安装其他依赖
sudo apt install libdrm-dev libx11-dev pkg-config

# 安装 clang (bindgen 需要)
sudo apt install clang libclang-dev

# 构建
cargo build --release -p hwcodec
```

### 5.2 Windows 构建 (VCPKG)

```powershell
# 安装 VCPKG
git clone https://github.com/microsoft/vcpkg
cd vcpkg
./bootstrap-vcpkg.bat

# 安装 FFmpeg
./vcpkg install ffmpeg:x64-windows-static

# 设置环境变量
$env:VCPKG_ROOT = "C:\path\to\vcpkg"

# 构建
cargo build --release -p hwcodec
```

### 5.3 交叉编译

```bash
# 安装 cross
cargo install cross --git https://github.com/cross-rs/cross

# ARM64 Linux
cross build --release -p hwcodec --target aarch64-unknown-linux-gnu

# ARMv7 Linux
cross build --release -p hwcodec --target armv7-unknown-linux-gnueabihf
```

## 6. 集成到 One-KVM

### 6.1 依赖配置

```toml
# Cargo.toml
[dependencies]
hwcodec = { path = "libs/hwcodec" }
```

### 6.2 使用示例

```rust
use hwcodec::ffmpeg_ram::encode::{Encoder, EncodeContext};
use hwcodec::ffmpeg_ram::decode::{Decoder, DecodeContext};
use hwcodec::ffmpeg::{AVPixelFormat, AVHWDeviceType};

// 检测可用编码器
let encoders = Encoder::available_encoders(ctx, None);

// 创建编码器
let encoder = Encoder::new(EncodeContext {
    name: "h264_vaapi".to_string(),
    width: 1920,
    height: 1080,
    pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
    fps: 30,
    gop: 30,
    kbs: 4000,
    // ...
})?;

// 编码
let frames = encoder.encode(&yuv_data, pts_ms)?;

// 创建 MJPEG 解码器 (IP-KVM 专用)
let decoder = Decoder::new(DecodeContext {
    name: "mjpeg".to_string(),
    device_type: AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
    thread_count: 4,
})?;

// 解码
let frames = decoder.decode(&mjpeg_data)?;
```

### 6.3 日志集成

```rust
// hwcodec 使用 log crate，与 One-KVM 日志系统兼容
use log::{debug, info, warn, error};

// C++ 层日志通过回调传递到 Rust
#[no_mangle]
pub extern "C" fn hwcodec_av_log_callback(level: i32, message: *const c_char) {
    // 转发到 Rust log 系统
    match level {
        AV_LOG_ERROR => error!("{}", message),
        AV_LOG_WARNING => warn!("{}", message),
        AV_LOG_INFO => info!("{}", message),
        AV_LOG_DEBUG => debug!("{}", message),
        _ => {}
    }
}
```

## 7. 故障排除

### 7.1 编译错误

**FFmpeg 未找到**:
```
error: pkg-config failed for libavcodec
```
解决: 安装 FFmpeg 开发库
```bash
sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev
```

**bindgen 错误**:
```
error: failed to run custom build command for `hwcodec`
```
解决: 安装 clang
```bash
sudo apt install clang libclang-dev
```

### 7.2 链接错误

**符号未定义**:
```
undefined reference to `av_log_set_level'
```
解决: 检查 FFmpeg 库链接顺序，确保 pkg-config 正确配置

**动态库未找到**:
```
error while loading shared libraries: libavcodec.so.59
```
解决:
```bash
sudo ldconfig
# 或设置 LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
```

### 7.3 运行时错误

**硬件编码器不可用**:
```
Encoder h264_vaapi test failed
```
检查:
1. 驱动是否正确安装: `vainfo`
2. 权限是否足够: `ls -la /dev/dri/`
3. 用户是否在 video 组: `groups`

**解码失败**:
```
avcodec_receive_frame failed, ret = -11
```
解决: 这通常表示需要更多输入数据 (EAGAIN)，是正常行为

## 8. 与原版 RustDesk hwcodec 的构建差异

### 8.1 移除的构建步骤

| 步骤 | 原因 |
|------|------|
| `build_mux()` | 移除了 Mux 模块 |
| `build_ffmpeg_vram()` | 移除了 VRAM 模块 |
| `sdk::build_sdk()` | 移除了外部 SDK 依赖 |
| macOS 框架链接 | 移除了 macOS 支持 |
| Android NDK 链接 | 移除了 Android 支持 |

### 8.2 简化的构建流程

```
原版构建流程:
build.rs
├── build_common()
├── ffmpeg::build_ffmpeg()
│   ├── build_ffmpeg_ram()
│   ├── build_ffmpeg_vram()  [已移除]
│   └── build_mux()          [已移除]
└── sdk::build_sdk()         [已移除]

简化版构建流程:
build.rs
├── build_common()
└── ffmpeg::build_ffmpeg()
    └── build_ffmpeg_ram()
```

### 8.3 优势

1. **更快的编译**: 无需编译外部 SDK 代码
2. **更少的依赖**: 无需下载 ~9MB 的外部 SDK
3. **更简单的维护**: 代码量减少约 67%
4. **更小的二进制**: 不包含未使用的功能
