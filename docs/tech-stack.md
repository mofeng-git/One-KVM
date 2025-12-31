# One-KVM 技术栈文档

## 1. 概述

One-KVM 采用现代化的 Rust + Vue3 技术栈，追求高性能、低资源占用和类型安全。本文档详细介绍项目使用的技术、库和开发规范。

---

## 2. 后端技术栈

### 2.1 核心语言和运行时

| 技术 | 版本 | 用途 |
|------|------|------|
| **Rust** | Edition 2021 | 主要开发语言 |
| **Tokio** | 1.x | 异步运行时 |

#### Rust 特性使用

```rust
// Edition 2021 特性
- async/await 异步编程
- 模式匹配 (match, if let)
- 错误处理 (Result, ?)
- 智能指针 (Arc, Mutex, RwLock)
- trait 系统
- 生命周期
- 泛型
```

### 2.2 Web 框架

| 库 | 版本 | 用途 |
|----|------|------|
| **axum** | 0.7 | Web 框架 |
| **axum-extra** | 0.9 | Cookie、TypedHeader 支持 |
| **tower-http** | 0.5 | CORS、压缩、追踪中间件 |
| **axum-server** | 0.7 | TLS/HTTPS 服务器 |

#### Axum 使用模式

```rust
// 路由定义
Router::new()
    .route("/api/stream/start", post(handlers::stream_start))
    .route("/api/stream/stop", post(handlers::stream_stop))
    .with_state(app_state)

// 处理器函数
async fn stream_start(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StreamStartRequest>,
) -> Result<Json<StreamResponse>, AppError>

// 中间件
.layer(CorsLayer::permissive())
.layer(CompressionLayer::new())
.layer(TraceLayer::new_for_http())
```

### 2.3 数据库

| 库 | 版本 | 用途 |
|----|------|------|
| **SQLx** | 0.8 | 异步 SQL 工具包 |
| **SQLite** | (bundled) | 嵌入式数据库 |

#### 数据库设计

```sql
-- 配置表 (JSON blob 存储)
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- 用户表
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

### 2.4 序列化

| 库 | 版本 | 用途 |
|----|------|------|
| **serde** | 1.x | 序列化框架 |
| **serde_json** | 1.x | JSON 序列化 |
| **prost** | 0.13 | Protobuf 序列化 (RustDesk) |

### 2.5 日志和追踪

| 库 | 版本 | 用途 |
|----|------|------|
| **tracing** | 0.1 | 结构化日志 |
| **tracing-subscriber** | 0.3 | 日志订阅器 |

#### 日志级别

```bash
-v      # WARN + INFO
-vv     # + DEBUG
-vvv    # + TRACE
```

### 2.6 错误处理

| 库 | 版本 | 用途 |
|----|------|------|
| **thiserror** | 1.x | 错误类型派生 |
| **anyhow** | 1.x | 通用错误处理 |

#### 错误处理模式

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Authentication failed")]
    AuthError,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::AuthError => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

### 2.7 认证和安全

| 库 | 版本 | 用途 |
|----|------|------|
| **argon2** | 0.5 | 密码哈希 |
| **rand** | 0.8 | 随机数生成 |
| **rustls** | 0.23 | TLS 实现 |
| **rcgen** | 0.13 | 证书生成 |
| **sodiumoxide** | 0.2 | NaCl 加密 (RustDesk) |
| **sha2** | 0.10 | SHA-256 哈希 |

#### 密码哈希

```rust
use argon2::{Argon2, PasswordHasher, PasswordVerifier};

// 哈希密码
let salt = SaltString::generate(&mut OsRng);
let hash = Argon2::default()
    .hash_password(password.as_bytes(), &salt)?
    .to_string();

// 验证密码
Argon2::default()
    .verify_password(password.as_bytes(), &parsed_hash)?;
```

### 2.8 视频处理

| 库 | 版本 | 用途 |
|----|------|------|
| **v4l** | 0.14 | V4L2 视频采集 |
| **turbojpeg** | 1.1 | JPEG 编码 (SIMD) |
| **hwcodec** | (vendored) | 硬件视频编码 |
| **libyuv** | (vendored) | YUV 格式转换 |

#### 视频编码优先级

```
1. VAAPI (Intel/AMD GPU)
2. RKMPP (Rockchip)
3. V4L2 M2M (通用硬件)
4. Software (libx264/libvpx)
```

#### 支持的像素格式

```rust
pub enum PixelFormat {
    // 压缩格式 (优先)
    Mjpeg,      // Motion JPEG
    Jpeg,       // Static JPEG

    // YUV 4:2:2 打包格式
    Yuyv,       // YUYV (最常见)
    Yvyu,       // YVYU
    Uyvy,       // UYVY

    // YUV 半平面格式
    Nv12,       // NV12 (常见)
    Nv16,       // NV16
    Nv24,       // NV24

    // YUV 平面格式
    Yuv420,     // I420/YU12
    Yvu420,     // YV12

    // RGB 格式
    Rgb565,     // RGB565
    Rgb24,      // RGB24
    Bgr24,      // BGR24

    // 灰度
    Grey,       // 8-bit grayscale
}
```

### 2.9 音频处理

| 库 | 版本 | 用途 |
|----|------|------|
| **alsa** | 0.9 | ALSA 音频采集 |
| **audiopus** | 0.2 | Opus 编码 |

#### 音频配置

```rust
// 采样参数
const SAMPLE_RATE: u32 = 48000;
const CHANNELS: u16 = 2;
const FRAME_SIZE: usize = 960;  // 20ms at 48kHz

// 质量配置
pub enum AudioQuality {
    VeryLow,   // 24 kbps
    Low,       // 48 kbps
    Medium,    // 64 kbps
    High,      // 96 kbps
}
```

### 2.10 WebRTC

| 库 | 版本 | 用途 |
|----|------|------|
| **webrtc** | 0.14 | WebRTC 实现 |
| **rtp** | 0.14 | RTP 协议 |

#### WebRTC 配置

```rust
// ICE 服务器配置
pub struct IceServerConfig {
    pub stun_servers: Vec<String>,   // STUN 服务器
    pub turn_servers: Vec<TurnServer>, // TURN 服务器
}

// 默认 STUN
"stun:stun.l.google.com:19302"
```

### 2.11 硬件交互

| 库 | 版本 | 用途 |
|----|------|------|
| **nix** | 0.29 | Unix 系统调用 |
| **gpio-cdev** | 0.6 | GPIO 控制 |
| **serialport** | 4.x | 串口通信 |
| **libc** | 0.2 | C 库绑定 |

#### GPIO 操作

```rust
// 使用 gpio-cdev
let chip = Chip::new("/dev/gpiochip0")?;
let line = chip.get_line(pin)?;
let handle = line.request(LineRequestFlags::OUTPUT, 0, "one-kvm")?;
handle.set_value(1)?;  // 设置高电平
```

#### 串口通信 (CH9329)

```rust
// 打开串口
let port = serialport::new(device, baud_rate)
    .timeout(Duration::from_millis(100))
    .open()?;

// 发送 HID 报告
port.write(&hid_report)?;
```

### 2.12 并发同步

| 库 | 版本 | 用途 |
|----|------|------|
| **parking_lot** | 0.12 | 高性能锁 |
| **arc-swap** | 1.7 | 原子引用交换 |
| **tokio** | 1.x | 异步通道 |

#### 同步模式

```rust
// 共享状态
Arc<RwLock<T>>           // 读多写少
Arc<Mutex<T>>            // 互斥访问
Arc<AtomicU64>           // 原子操作

// 通道
tokio::sync::broadcast   // 多生产者多消费者
tokio::sync::mpsc        // 多生产者单消费者
tokio::sync::oneshot     // 一次性通知
```

### 2.13 网络和 HTTP

| 库 | 版本 | 用途 |
|----|------|------|
| **reqwest** | 0.12 | HTTP 客户端 |
| **tokio-tungstenite** | 0.24 | WebSocket 客户端 |
| **urlencoding** | 2.x | URL 编码 |

### 2.14 工具库

| 库 | 版本 | 用途 |
|----|------|------|
| **uuid** | 1.x | UUID 生成 |
| **chrono** | 0.4 | 时间处理 |
| **base64** | 0.22 | Base64 编码 |
| **bytes** | 1.x | 字节缓冲区 |
| **bytemuck** | 1.14 | 零拷贝类型转换 |
| **xxhash-rust** | 0.8 | 快速哈希 |
| **futures** | 0.3 | Future 工具 |
| **async-trait** | 0.1 | 异步 trait |

### 2.15 静态资源嵌入

| 库 | 版本 | 用途 |
|----|------|------|
| **rust-embed** | 8.x | 资源嵌入 |
| **mime_guess** | 2.x | MIME 类型推断 |

#### 资源嵌入模式

```rust
#[derive(RustEmbed)]
#[folder = "web/dist"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "assets/*"]
struct Assets;

// Debug: 从文件系统读取
// Release: 嵌入二进制 (gzip 压缩)
```

### 2.16 CLI

| 库 | 版本 | 用途 |
|----|------|------|
| **clap** | 4.x | 命令行解析 |

#### CLI 参数

```rust
#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "0.0.0.0")]
    address: String,

    #[arg(short, long, default_value = "8080")]
    port: u16,

    #[arg(short, long)]
    data_dir: Option<PathBuf>,

    #[arg(long)]
    enable_https: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}
```

### 2.17 类型生成

| 库 | 版本 | 用途 |
|----|------|------|
| **typeshare** | 1.0 | TypeScript 类型生成 |

#### 类型共享

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct VideoConfig {
    pub device: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

// 生成 TypeScript:
// export interface VideoConfig {
//     device?: string;
//     width: number;
//     height: number;
//     fps: number;
// }
```

---

## 3. 前端技术栈

### 3.1 核心框架

| 技术 | 版本 | 用途 |
|------|------|------|
| **Vue 3** | 3.5.x | UI 框架 |
| **Vue Router** | 4.6.x | 路由 |
| **Pinia** | 3.0.x | 状态管理 |
| **TypeScript** | 5.9.x | 类型系统 |

#### Vue 3 组合式 API

```vue
<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useSystemStore } from '@/stores/system'

const store = useSystemStore()
const streaming = computed(() => store.streamState === 'streaming')

onMounted(async () => {
  await store.fetchDeviceInfo()
})
</script>
```

### 3.2 UI 组件库

| 库 | 版本 | 用途 |
|----|------|------|
| **Radix Vue** | 1.9.x | 无头 UI 组件 |
| **Reka UI** | 2.6.x | UI 组件 |
| **shadcn-vue** | - | 组件样式 |
| **Lucide Vue** | 0.556.x | 图标库 |

### 3.3 样式

| 技术 | 版本 | 用途 |
|------|------|------|
| **Tailwind CSS** | 4.1.x | 原子化 CSS |
| **tailwind-merge** | 3.4.x | 类名合并 |
| **class-variance-authority** | 0.7.x | 变体管理 |
| **tw-animate-css** | 1.4.x | 动画 |

#### Tailwind 配置

```javascript
// tailwind.config.js
export default {
  darkMode: 'class',
  content: ['./src/**/*.{vue,ts}'],
  theme: {
    extend: {
      colors: {
        border: 'hsl(var(--border))',
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
      },
    },
  },
}
```

### 3.4 构建工具

| 工具 | 版本 | 用途 |
|------|------|------|
| **Vite** | 7.2.x | 构建工具 |
| **vue-tsc** | 3.1.x | 类型检查 |
| **PostCSS** | 8.5.x | CSS 处理 |
| **Autoprefixer** | 10.4.x | CSS 前缀 |

#### Vite 配置

```typescript
// vite.config.ts
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
  }
})
```

### 3.5 功能库

| 库 | 版本 | 用途 |
|----|------|------|
| **@vueuse/core** | 14.1.x | Vue 组合式工具 |
| **simple-keyboard** | 3.8.x | 虚拟键盘 |
| **opus-decoder** | 0.7.x | Opus 音频解码 |
| **uplot** | 1.6.x | 实时图表 |
| **vue-sonner** | 2.0.x | Toast 通知 |

### 3.6 国际化

| 库 | 版本 | 用途 |
|----|------|------|
| **vue-i18n** | 9.14.x | 国际化 |

#### 多语言支持

```typescript
// i18n/en-US.ts
export default {
  common: {
    start: 'Start',
    stop: 'Stop',
    settings: 'Settings',
  },
  video: {
    noSignal: 'No Signal',
    streaming: 'Streaming',
  },
}

// i18n/zh-CN.ts
export default {
  common: {
    start: '启动',
    stop: '停止',
    settings: '设置',
  },
  video: {
    noSignal: '无信号',
    streaming: '正在推流',
  },
}
```

---

## 4. 外部依赖库

### 4.1 hwcodec (来自 RustDesk)

硬件视频编码库，支持多种硬件加速：

| 后端 | 平台 | 编码格式 |
|------|------|---------|
| **VAAPI** | Intel/AMD Linux | H264, H265, VP8, VP9 |
| **RKMPP** | Rockchip | H264, H265 |
| **V4L2 M2M** | 通用 Linux | H264 |
| **Software** | 通用 | H264 (x264) |

### 4.2 libyuv (来自 Google)

高性能 YUV/RGB 转换库：

```rust
// 支持的转换
MJPEG → YUV420
YUYV → YUV420
NV12 → YUV420
RGB24 → YUV420

// SIMD 加速
- SSE2/SSE4.1/AVX2 (x86)
- NEON (ARM)
```

### 4.3 ventoy-img-rs

Ventoy 启动盘支持库：

- 创建可启动 USB 镜像
- 支持多 ISO 文件
- exFAT 文件系统

---

## 5. 协议和规范

### 5.1 RustDesk 协议

使用 Protobuf 定义的消息格式：

```protobuf
// protos/message.proto
message VideoFrame {
    bytes data = 1;
    int32 width = 2;
    int32 height = 3;
    VideoCodec codec = 4;
}

message MouseEvent {
    int32 x = 1;
    int32 y = 2;
    MouseButton button = 3;
}
```

### 5.2 WebRTC 规范

遵循标准 WebRTC 协议：

- **ICE** (RFC 8445) - 连接建立
- **DTLS** (RFC 6347) - 安全传输
- **SRTP** (RFC 3711) - 媒体加密
- **RTP** (RFC 3550) - 媒体传输

### 5.3 HID 报告描述符

USB HID 报告格式：

```rust
// 键盘报告 (8 字节)
struct KeyboardReport {
    modifiers: u8,      // Ctrl, Shift, Alt, GUI
    reserved: u8,
    keys: [u8; 6],      // 最多 6 个按键
}

// 鼠标报告 (相对模式, 4 字节)
struct MouseRelativeReport {
    buttons: u8,
    x: i8,
    y: i8,
    wheel: i8,
}

// 鼠标报告 (绝对模式, 6 字节)
struct MouseAbsoluteReport {
    buttons: u8,
    x: u16,             // 0-32767
    y: u16,             // 0-32767
    wheel: i8,
}
```

### 5.4 V4L2 接口

Video4Linux2 采集接口：

```rust
// 设备能力查询
VIDIOC_QUERYCAP

// 格式设置
VIDIOC_S_FMT
VIDIOC_G_FMT

// 缓冲区管理
VIDIOC_REQBUFS
VIDIOC_QUERYBUF
VIDIOC_QBUF
VIDIOC_DQBUF

// 流控制
VIDIOC_STREAMON
VIDIOC_STREAMOFF
```

---

## 6. 开发规范

### 6.1 Rust 代码规范

#### 命名约定

```rust
// 结构体: PascalCase
pub struct VideoConfig { }

// 函数/方法: snake_case
fn start_streaming() { }

// 常量: SCREAMING_SNAKE_CASE
const MAX_FRAME_SIZE: usize = 1920 * 1080 * 4;

// 模块: snake_case
mod video_capture;

// trait: PascalCase
trait Encoder { }
```

#### 错误处理

```rust
// 使用 Result 返回可能失败的操作
fn open_device() -> Result<Device, DeviceError>;

// 使用 ? 传播错误
let device = open_device()?;

// 自定义错误类型使用 thiserror
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
}
```

#### 异步代码

```rust
// 使用 async/await
async fn fetch_frame(&self) -> Result<VideoFrame> {
    let frame = self.capture.read_frame().await?;
    Ok(frame)
}

// 使用 tokio::spawn 启动后台任务
tokio::spawn(async move {
    loop {
        // 后台工作
    }
});
```

### 6.2 TypeScript 代码规范

#### 类型定义

```typescript
// 使用 interface 定义数据结构
interface VideoConfig {
  device?: string
  width: number
  height: number
  fps: number
}

// 使用 type 定义联合类型
type StreamState = 'idle' | 'starting' | 'streaming' | 'stopping'
```

#### 组合式 API

```typescript
// 使用 <script setup> 语法
<script setup lang="ts">
import { ref, computed, watch } from 'vue'

const count = ref(0)
const doubled = computed(() => count.value * 2)

watch(count, (newVal) => {
  console.log(`Count changed to ${newVal}`)
})
</script>
```

### 6.3 Git 提交规范

```
<type>(<scope>): <subject>

<body>

<footer>
```

#### 类型 (type)

- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式 (不影响功能)
- `refactor`: 重构
- `perf`: 性能优化
- `test`: 测试
- `chore`: 构建/工具

#### 示例

```
feat(video): add H265 hardware encoding support

- Add VAAPI H265 encoder
- Update encoder registry
- Add fallback to H264

Closes #123
```

### 6.4 代码组织

#### 模块结构

```rust
// mod.rs 作为模块入口
pub mod controller;
pub mod types;
mod internal;

// 重导出公共 API
pub use controller::Controller;
pub use types::{Config, State};
```

#### 文件大小建议

- 单个文件 < 1000 行
- 单个函数 < 100 行
- 嵌套深度 < 4 层

---

## 7. 构建和部署

### 7.1 构建配置

#### Release Profile

```toml
[profile.release]
opt-level = 3        # 最高优化
lto = true           # 链接时优化
codegen-units = 1    # 单代码生成单元
strip = true         # 移除符号
panic = "abort"      # panic 时中止
```

#### 静态链接 Profile

```toml
[profile.release-static]
inherits = "release"
opt-level = "z"      # 优化大小
```

### 7.2 目标平台

| 目标 | 用途 | 工具链 |
|------|------|--------|
| `aarch64-unknown-linux-gnu` | ARM64 (主要) | cross |
| `armv7-unknown-linux-gnueabihf` | ARMv7 | cross |
| `x86_64-unknown-linux-gnu` | x86-64 | native |

### 7.3 Docker 构建

```dockerfile
# 多阶段构建
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/one-kvm /usr/local/bin/
CMD ["one-kvm"]
```

---

## 8. 测试规范

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = VideoConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
    }

    #[tokio::test]
    async fn test_async_operation() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### 8.2 集成测试

```rust
// tests/integration_test.rs
use one_kvm::AppState;

#[tokio::test]
async fn test_full_flow() {
    let state = AppState::new_test().await;
    // 测试完整流程
}
```

---

## 9. 性能优化

### 9.1 视频流优化

- **零拷贝帧传递**: 使用 `Arc<Bytes>` 共享帧数据
- **帧去重**: xxHash64 快速比较
- **硬件编码优先**: VAAPI > RKMPP > V4L2 M2M > Software

### 9.2 内存优化

- **静态链接**: 减少运行时依赖
- **嵌入资源压缩**: gzip 压缩静态文件
- **缓冲区复用**: 预分配帧缓冲区

### 9.3 网络优化

- **HTTP/2**: 多路复用
- **gzip 压缩**: 响应压缩
- **WebSocket**: 双向实时通信
- **WebRTC**: P2P 低延迟

---

## 10. 安全规范

### 10.1 密码存储

- 使用 Argon2id 哈希
- 随机盐值
- 不存储明文

### 10.2 会话管理

- HTTPOnly Cookie
- 随机会话 ID
- 会话超时

### 10.3 输入验证

- 类型安全 (serde)
- 路径验证
- SQL 参数化查询

### 10.4 TLS 配置

- TLS 1.3 优先
- 自动证书生成
- 证书轮换支持

---

## 11. 版本兼容性

### 11.1 最低系统要求

| 组件 | 最低版本 |
|------|---------|
| Linux Kernel | 4.19+ |
| glibc | 2.28+ |
| V4L2 | 5.4+ (推荐) |
| USB Gadget | ConfigFS 支持 |

### 11.2 浏览器支持

| 浏览器 | 最低版本 |
|--------|---------|
| Chrome | 90+ |
| Firefox | 88+ |
| Safari | 14+ |
| Edge | 90+ |

---

## 12. 参考资源

### 官方文档

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Tokio 文档](https://tokio.rs/)
- [Axum 文档](https://docs.rs/axum/)
- [Vue 3 文档](https://vuejs.org/)
- [Tailwind CSS 文档](https://tailwindcss.com/)

### 协议规范

- [WebRTC 规范](https://www.w3.org/TR/webrtc/)
- [USB HID 规范](https://www.usb.org/hid)
- [V4L2 API](https://www.kernel.org/doc/html/latest/userspace-api/media/v4l/v4l2.html)
- [Linux USB Gadget](https://www.kernel.org/doc/html/latest/usb/gadget_configfs.html)

### 相关项目

- [RustDesk](https://github.com/rustdesk/rustdesk) - hwcodec 来源
- [PiKVM](https://pikvm.org/) - 参考实现
