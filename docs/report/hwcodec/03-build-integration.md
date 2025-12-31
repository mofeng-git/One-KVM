# hwcodec 构建系统与集成指南

## 1. 项目结构

```
libs/hwcodec/
├── Cargo.toml           # 包配置
├── Cargo.lock           # 依赖锁定
├── build.rs             # 构建脚本
├── src/                 # Rust 源码
│   ├── lib.rs           # 库入口
│   ├── common.rs        # 公共定义
│   ├── ffmpeg.rs        # FFmpeg 集成
│   ├── mux.rs           # 混流器
│   ├── android.rs       # Android 支持
│   ├── ffmpeg_ram/      # RAM 编解码
│   │   ├── mod.rs
│   │   ├── encode.rs
│   │   └── decode.rs
│   ├── vram/            # GPU 编解码 (Windows)
│   │   ├── mod.rs
│   │   ├── encode.rs
│   │   ├── decode.rs
│   │   └── ...
│   └── res/             # 测试资源
│       ├── 720p.h264
│       └── 720p.h265
├── cpp/                 # C++ 源码
│   ├── common/          # 公共代码
│   ├── ffmpeg_ram/      # FFmpeg RAM 实现
│   ├── ffmpeg_vram/     # FFmpeg VRAM 实现
│   ├── nv/              # NVIDIA 实现
│   ├── amf/             # AMD 实现
│   ├── mfx/             # Intel 实现
│   ├── mux/             # 混流实现
│   └── yuv/             # YUV 处理
├── externals/           # 外部 SDK (Git 子模块)
│   ├── nv-codec-headers_n12.1.14.0/
│   ├── Video_Codec_SDK_12.1.14/
│   ├── AMF_v1.4.35/
│   └── MediaSDK_22.5.4/
├── dev/                 # 开发工具
│   ├── capture/         # 捕获工具
│   ├── render/          # 渲染工具
│   └── tool/            # 通用工具
└── examples/            # 示例程序
```

## 2. Cargo 配置

### 2.1 Cargo.toml

```toml
[package]
name = "hwcodec"
version = "0.7.1"
edition = "2021"

[features]
default = []
vram = []  # GPU VRAM 直接编解码 (仅 Windows)

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
rand = "0.8"             # 随机数
```

### 2.2 Feature 说明

| Feature | 说明 | 平台 |
|---------|------|------|
| `default` | 基础功能 | 全平台 |
| `vram` | GPU VRAM 直接编解码 | 仅 Windows |

### 2.3 使用方式

```toml
# 基础使用
[dependencies]
hwcodec = { path = "libs/hwcodec" }

# 启用 VRAM 功能 (Windows)
[dependencies]
hwcodec = { path = "libs/hwcodec", features = ["vram"] }
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

    // 3. 构建 SDK 模块 (Windows + vram feature)
    #[cfg(all(windows, feature = "vram"))]
    sdk::build_sdk(&mut builder);

    // 4. 编译生成静态库
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

    #[cfg(target_os = "macos")]
    builder.file(common_dir.join("platform/mac/mac.mm"));

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

        // 构建子模块
        build_ffmpeg_ram(builder);
        #[cfg(feature = "vram")]
        build_ffmpeg_vram(builder);
        build_mux(builder);
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
        ("macos", "x86_64") => "x64-osx",
        ("macos", "aarch64") => "arm64-osx",
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
        "macos" | "ios" => vec!["c++", "m"],
        "android" => vec!["z", "m", "android", "atomic", "mediandk"],
        _ => panic!("unsupported os"),
    };

    for lib in libs {
        println!("cargo:rustc-link-lib={}", lib);
    }

    // macOS 框架
    if target_os == "macos" || target_os == "ios" {
        for framework in ["CoreFoundation", "CoreVideo", "CoreMedia",
                         "VideoToolbox", "AVFoundation"] {
            println!("cargo:rustc-link-lib=framework={}", framework);
        }
    }
}
```

### 3.6 SDK 模块构建 (Windows)

```rust
#[cfg(all(windows, feature = "vram"))]
mod sdk {
    pub fn build_sdk(builder: &mut Build) {
        build_amf(builder);  // AMD AMF
        build_nv(builder);   // NVIDIA
        build_mfx(builder);  // Intel MFX
    }

    fn build_nv(builder: &mut Build) {
        let sdk_path = externals_dir.join("Video_Codec_SDK_12.1.14");

        // 包含 SDK 头文件
        builder.includes([
            sdk_path.join("Interface"),
            sdk_path.join("Samples/Utils"),
            sdk_path.join("Samples/NvCodec"),
        ]);

        // 编译 SDK 源文件
        builder.file(sdk_path.join("Samples/NvCodec/NvEncoder/NvEncoder.cpp"));
        builder.file(sdk_path.join("Samples/NvCodec/NvEncoder/NvEncoderD3D11.cpp"));
        builder.file(sdk_path.join("Samples/NvCodec/NvDecoder/NvDecoder.cpp"));

        // 编译封装代码
        builder.files([
            nv_dir.join("nv_encode.cpp"),
            nv_dir.join("nv_decode.cpp"),
        ]);
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
| `mux_ffi.rs` | `mux_ffi.h` | 混流器函数 |

## 5. 外部依赖管理

### 5.1 Git 子模块

```bash
# 初始化子模块
git submodule update --init --recursive

# 更新子模块
git submodule update --remote externals
```

### 5.2 子模块配置 (.gitmodules)

```
[submodule "externals"]
    path = libs/hwcodec/externals
    url = https://github.com/rustdesk-org/externals.git
```

### 5.3 依赖版本

| 依赖 | 版本 | 用途 |
|------|------|------|
| nv-codec-headers | n12.1.14.0 | NVIDIA FFmpeg 编码头 |
| Video_Codec_SDK | 12.1.14 | NVIDIA 编解码 SDK |
| AMF | v1.4.35 | AMD Advanced Media Framework |
| MediaSDK | 22.5.4 | Intel Media SDK |

## 6. 平台构建指南

### 6.1 Linux 构建

```bash
# 安装 FFmpeg 开发库
sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev

# 安装其他依赖
sudo apt install libdrm-dev libx11-dev pkg-config

# 构建
cargo build --release -p hwcodec
```

### 6.2 Windows 构建 (VCPKG)

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
cargo build --release -p hwcodec --features vram
```

### 6.3 macOS 构建

```bash
# 安装 FFmpeg (Homebrew)
brew install ffmpeg pkg-config

# 或使用 VCPKG
export VCPKG_ROOT=/path/to/vcpkg
vcpkg install ffmpeg:arm64-osx  # Apple Silicon
vcpkg install ffmpeg:x64-osx    # Intel

# 构建
cargo build --release -p hwcodec
```

### 6.4 交叉编译

```bash
# 安装 cross
cargo install cross --git https://github.com/cross-rs/cross

# ARM64 Linux
cross build --release -p hwcodec --target aarch64-unknown-linux-gnu

# ARMv7 Linux
cross build --release -p hwcodec --target armv7-unknown-linux-gnueabihf
```

## 7. 集成到 One-KVM

### 7.1 依赖配置

```toml
# Cargo.toml
[dependencies]
hwcodec = { path = "libs/hwcodec" }
```

### 7.2 使用示例

```rust
use hwcodec::ffmpeg_ram::encode::{Encoder, EncodeContext};
use hwcodec::ffmpeg_ram::decode::{Decoder, DecodeContext};
use hwcodec::ffmpeg::AVPixelFormat;

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
```

### 7.3 日志集成

```rust
// hwcodec 使用 log crate，与 One-KVM 日志系统兼容
use log::{debug, info, warn, error};

// C++ 层日志通过回调传递
#[no_mangle]
pub extern "C" fn hwcodec_log(level: i32, message: *const c_char) {
    match level {
        0 => error!("{}", message),
        1 => warn!("{}", message),
        2 => info!("{}", message),
        3 => debug!("{}", message),
        4 => trace!("{}", message),
        _ => {}
    }
}
```

## 8. 故障排除

### 8.1 编译错误

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

### 8.2 链接错误

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

### 8.3 运行时错误

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
