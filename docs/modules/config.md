# Config 模块文档

## 1. 模块概述

Config 模块提供配置管理功能，所有配置存储在 SQLite 数据库中，使用 ArcSwap 实现无锁读取，提供高性能配置访问。

### 1.1 主要功能

- SQLite 配置存储（持久化）
- 无锁配置读取（ArcSwap）
- 类型安全的配置结构
- 配置变更通知（broadcast channel）
- TypeScript 类型生成（typeshare）
- RESTful API（按功能域分离）

### 1.2 文件结构

```
src/config/
├── mod.rs              # 模块导出
├── schema.rs           # 配置结构定义（包含所有子配置）
└── store.rs            # SQLite 存储与无锁缓存
```

---

## 2. 核心组件

### 2.1 ConfigStore (store.rs)

配置存储使用 **ArcSwap** 实现无锁读取，提供接近零成本的配置访问性能：

```rust
pub struct ConfigStore {
    pool: Pool<Sqlite>,
    /// 无锁缓存，使用 ArcSwap 实现零成本读取
    cache: Arc<ArcSwap<AppConfig>>,
    /// 配置变更通知通道
    change_tx: broadcast::Sender<ConfigChange>,
}

impl ConfigStore {
    /// 创建存储
    pub async fn new(db_path: &Path) -> Result<Self>;

    /// 获取当前配置（无锁，零拷贝）
    ///
    /// 返回 Arc<AppConfig>，高效共享无需克隆
    /// 这是一个无锁操作，开销极小
    pub fn get(&self) -> Arc<AppConfig>;

    /// 设置完整配置
    pub async fn set(&self, config: AppConfig) -> Result<()>;

    /// 使用闭包更新配置
    ///
    /// 读-修改-写模式。并发更新时，最后的写入获胜。
    /// 对于不频繁的用户触发配置更改来说是可接受的。
    pub async fn update<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut AppConfig);

    /// 订阅配置变更事件
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChange>;

    /// 检查系统是否已初始化（无锁）
    pub fn is_initialized(&self) -> bool;

    /// 获取数据库连接池（用于会话管理）
    pub fn pool(&self) -> &Pool<Sqlite>;
}
```

**性能特点**：
- `get()` 是无锁读取操作，返回 `Arc<AppConfig>`，无需克隆
- 配置读取频率远高于写入，ArcSwap 优化了读取路径
- 写入操作先持久化到数据库，再原子性更新内存缓存
- 使用 broadcast channel 通知配置变更，支持多订阅者

**数据库连接池配置**：
```rust
SqlitePoolOptions::new()
    .max_connections(2)           // SQLite 单写模式，2 个连接足够
    .acquire_timeout(Duration::from_secs(5))
    .idle_timeout(Duration::from_secs(300))
```

### 2.2 AppConfig (schema.rs)

主应用配置结构，包含所有子系统的配置：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct AppConfig {
    /// 初始设置是否完成
    pub initialized: bool,

    /// 认证配置
    pub auth: AuthConfig,

    /// 视频采集配置
    pub video: VideoConfig,

    /// HID（键盘/鼠标）配置
    pub hid: HidConfig,

    /// MSD（大容量存储）配置
    pub msd: MsdConfig,

    /// ATX 电源控制配置
    pub atx: AtxConfig,

    /// 音频配置
    pub audio: AudioConfig,

    /// 流媒体配置
    pub stream: StreamConfig,

    /// Web 服务器配置
    pub web: WebConfig,

    /// 扩展配置（ttyd, gostc, easytier）
    pub extensions: ExtensionsConfig,

    /// RustDesk 远程访问配置
    pub rustdesk: RustDeskConfig,
}
```

### 2.3 主要子配置结构

#### AuthConfig - 认证配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct AuthConfig {
    /// 会话超时时间（秒）
    pub session_timeout_secs: u32,  // 默认 86400（24小时）
    /// 启用双因素认证
    pub totp_enabled: bool,
    /// TOTP 密钥（加密存储）
    pub totp_secret: Option<String>,
}
```

#### VideoConfig - 视频采集配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[typeshare]
pub struct VideoConfig {
    /// 视频设备路径（如 /dev/video0）
    pub device: Option<String>,
    /// 像素格式（如 "MJPEG", "YUYV", "NV12"）
    pub format: Option<String>,
    /// 分辨率宽度
    pub width: u32,      // 默认 1920
    /// 分辨率高度
    pub height: u32,     // 默认 1080
    /// 帧率
    pub fps: u32,        // 默认 30
    /// JPEG 质量（1-100）
    pub quality: u32,    // 默认 80
}
```

#### HidConfig - HID 配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[typeshare]
pub struct HidConfig {
    /// HID 后端类型
    pub backend: HidBackend,  // Otg | Ch9329 | None
    /// OTG 键盘设备路径
    pub otg_keyboard: String,  // 默认 "/dev/hidg0"
    /// OTG 鼠标设备路径
    pub otg_mouse: String,     // 默认 "/dev/hidg1"
    /// OTG UDC（USB 设备控制器）名称
    pub otg_udc: Option<String>,
    /// OTG USB 设备描述符配置
    pub otg_descriptor: OtgDescriptorConfig,
    /// CH9329 串口路径
    pub ch9329_port: String,   // 默认 "/dev/ttyUSB0"
    /// CH9329 波特率
    pub ch9329_baudrate: u32,  // 默认 9600
    /// 鼠标模式：绝对定位或相对定位
    pub mouse_absolute: bool,  // 默认 true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[typeshare]
pub struct OtgDescriptorConfig {
    pub vendor_id: u16,        // 默认 0x1d6b（Linux Foundation）
    pub product_id: u16,       // 默认 0x0104
    pub manufacturer: String,  // 默认 "One-KVM"
    pub product: String,       // 默认 "One-KVM USB Device"
    pub serial_number: Option<String>,
}
```

#### StreamConfig - 流媒体配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct StreamConfig {
    /// 流模式（WebRTC | Mjpeg）
    pub mode: StreamMode,
    /// 编码器类型
    pub encoder: EncoderType,  // Auto | Software | Vaapi | Nvenc | Qsv | Amf | Rkmpp | V4l2m2m
    /// 码率预设（Speed | Balanced | Quality）
    pub bitrate_preset: BitratePreset,
    /// 自定义 STUN 服务器
    pub stun_server: Option<String>,  // 默认 "stun:stun.l.google.com:19302"
    /// 自定义 TURN 服务器
    pub turn_server: Option<String>,
    /// TURN 用户名
    pub turn_username: Option<String>,
    /// TURN 密码（加密存储，不通过 API 暴露）
    pub turn_password: Option<String>,
    /// 无客户端时自动暂停
    #[typeshare(skip)]
    pub auto_pause_enabled: bool,
    /// 自动暂停延迟（秒）
    #[typeshare(skip)]
    pub auto_pause_delay_secs: u64,
    /// 客户端超时清理（秒）
    #[typeshare(skip)]
    pub client_timeout_secs: u64,
}
```

#### MsdConfig - 大容量存储配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct MsdConfig {
    /// 启用 MSD 功能
    pub enabled: bool,          // 默认 true
    /// ISO/IMG 镜像存储路径
    pub images_path: String,    // 默认 "./data/msd/images"
    /// Ventoy 启动驱动器文件路径
    pub drive_path: String,     // 默认 "./data/msd/ventoy.img"
    /// 虚拟驱动器大小（MB，最小 1024）
    pub virtual_drive_size_mb: u32,  // 默认 16384（16GB）
}
```

#### AtxConfig - ATX 电源控制配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct AtxConfig {
    /// 启用 ATX 功能
    pub enabled: bool,
    /// 电源按钮配置（短按和长按共用）
    pub power: AtxKeyConfig,
    /// 重置按钮配置
    pub reset: AtxKeyConfig,
    /// LED 检测配置（可选）
    pub led: AtxLedConfig,
    /// WOL 数据包使用的网络接口（空字符串 = 自动）
    pub wol_interface: String,
}
```

#### AudioConfig - 音频配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct AudioConfig {
    /// 启用音频采集
    pub enabled: bool,          // 默认 false
    /// ALSA 设备名称
    pub device: String,         // 默认 "default"
    /// 音频质量预设："voice" | "balanced" | "high"
    pub quality: String,        // 默认 "balanced"
}
```

**注意**：采样率固定为 48000Hz，声道固定为 2（立体声），这是 Opus 编码和 WebRTC 的最佳配置。

#### WebConfig - Web 服务器配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[typeshare]
pub struct WebConfig {
    /// HTTP 端口
    pub http_port: u16,         // 默认 8080
    /// HTTPS 端口
    pub https_port: u16,        // 默认 8443
    /// 绑定地址
    pub bind_address: String,   // 默认 "0.0.0.0"
    /// 启用 HTTPS
    pub https_enabled: bool,    // 默认 false
    /// 自定义 SSL 证书路径
    pub ssl_cert_path: Option<String>,
    /// 自定义 SSL 密钥路径
    pub ssl_key_path: Option<String>,
}
```

---

## 3. TypeScript 类型生成

使用 `#[typeshare]` 属性自动生成 TypeScript 类型：

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct VideoConfig {
    pub device: Option<String>,
    pub width: u32,
    pub height: u32,
}
```

生成的 TypeScript：

```typescript
export interface VideoConfig {
    device?: string;
    width: number;
    height: number;
}
```

生成命令：

```bash
./scripts/generate-types.sh
# 或
typeshare src --lang=typescript --output-file=web/src/types/generated.ts
```

---

## 4. API 端点

所有配置端点均需要 **Admin** 权限，采用 RESTful 设计，按功能域分离。

### 4.1 全局配置

| 端点 | 方法 | 权限 | 描述 |
|------|------|------|------|
| `/api/config` | GET | Admin | 获取完整配置（敏感信息已过滤） |
| `/api/config` | POST | Admin | 更新完整配置（已废弃，请使用按域 PATCH） |

### 4.2 分域配置端点

| 端点 | 方法 | 权限 | 描述 |
|------|------|------|------|
| `/api/config/video` | GET | Admin | 获取视频配置 |
| `/api/config/video` | PATCH | Admin | 更新视频配置（部分更新） |
| `/api/config/stream` | GET | Admin | 获取流配置 |
| `/api/config/stream` | PATCH | Admin | 更新流配置（部分更新） |
| `/api/config/hid` | GET | Admin | 获取 HID 配置 |
| `/api/config/hid` | PATCH | Admin | 更新 HID 配置（部分更新） |
| `/api/config/msd` | GET | Admin | 获取 MSD 配置 |
| `/api/config/msd` | PATCH | Admin | 更新 MSD 配置（部分更新） |
| `/api/config/atx` | GET | Admin | 获取 ATX 配置 |
| `/api/config/atx` | PATCH | Admin | 更新 ATX 配置（部分更新） |
| `/api/config/audio` | GET | Admin | 获取音频配置 |
| `/api/config/audio` | PATCH | Admin | 更新音频配置（部分更新） |
| `/api/config/web` | GET | Admin | 获取 Web 服务器配置 |
| `/api/config/web` | PATCH | Admin | 更新 Web 服务器配置（部分更新） |

### 4.3 RustDesk 配置端点

| 端点 | 方法 | 权限 | 描述 |
|------|------|------|------|
| `/api/config/rustdesk` | GET | Admin | 获取 RustDesk 配置 |
| `/api/config/rustdesk` | PATCH | Admin | 更新 RustDesk 配置 |
| `/api/config/rustdesk/status` | GET | Admin | 获取 RustDesk 服务状态 |
| `/api/config/rustdesk/password` | GET | Admin | 获取设备密码 |
| `/api/config/rustdesk/regenerate-id` | POST | Admin | 重新生成设备 ID |
| `/api/config/rustdesk/regenerate-password` | POST | Admin | 重新生成设备密码 |

### 4.4 请求/响应示例

#### 获取视频配置

```bash
GET /api/config/video
```

响应：
```json
{
    "device": "/dev/video0",
    "format": "MJPEG",
    "width": 1920,
    "height": 1080,
    "fps": 30,
    "quality": 80
}
```

#### 部分更新视频配置

```bash
PATCH /api/config/video
Content-Type: application/json

{
    "width": 1280,
    "height": 720,
    "fps": 60
}
```

响应：更新后的完整 VideoConfig
```json
{
    "device": "/dev/video0",
    "format": "MJPEG",
    "width": 1280,
    "height": 720,
    "fps": 60,
    "quality": 80
}
```

**注意**：
- 所有 PATCH 请求都支持部分更新，只需要提供要修改的字段
- 未提供的字段保持原有值不变
- 更新后返回完整的配置对象
- 配置变更会自动触发相关组件重载

---

## 5. 配置变更通知

ConfigStore 提供 broadcast channel 用于配置变更通知：

```rust
/// 配置变更事件
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key: String,
}

// 订阅配置变更
let mut rx = config_store.subscribe();

// 监听变更事件
while let Ok(change) = rx.recv().await {
    println!("配置 {} 已更新", change.key);
    // 重载相关组件
}
```

**工作流程**：
1. 调用 `config_store.set()` 或 `config_store.update()`
2. 配置写入数据库（持久化）
3. 原子性更新内存缓存（ArcSwap）
4. 发送 `ConfigChange` 事件到 broadcast channel
5. 各组件的订阅者接收事件并执行重载逻辑

**组件重载示例**：
```rust
// VideoStreamManager 监听配置变更
let mut config_rx = config_store.subscribe();
tokio::spawn(async move {
    while let Ok(change) = config_rx.recv().await {
        if change.key == "app_config" {
            video_manager.reload().await;
        }
    }
});
```

---

## 6. 数据库结构

ConfigStore 使用 SQLite 存储配置和其他系统数据：

### 6.1 配置表

```sql
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

配置以 JSON 格式存储：
```sql
-- 应用配置
key: 'app_config'
value: '{"initialized": true, "video": {...}, "hid": {...}, ...}'
```

### 6.2 用户表

```sql
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 6.3 会话表

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    data TEXT
);
```

### 6.4 API 令牌表

```sql
CREATE TABLE IF NOT EXISTS api_tokens (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    permissions TEXT NOT NULL,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used TEXT
);
```

**存储特点**：
- 所有配置存储在单个 JSON 文本中（`app_config` key）
- 每次配置更新都更新整个 JSON，简化事务处理
- 使用 `ON CONFLICT` 实现 upsert 操作
- 连接池大小为 2（1 读 + 1 写），适合嵌入式环境

---

## 7. 使用示例

### 7.1 基本用法

```rust
use crate::config::ConfigStore;
use std::path::Path;

// 创建配置存储
let config_store = ConfigStore::new(Path::new("./data/config.db")).await?;

// 获取配置（无锁，零拷贝）
let config = config_store.get();
println!("视频设备: {:?}", config.video.device);
println!("是否已初始化: {}", config.initialized);

// 检查是否已初始化
if !config_store.is_initialized() {
    println!("系统尚未初始化，请完成初始设置");
}
```

### 7.2 更新配置

```rust
// 方式 1: 使用闭包更新（推荐）
config_store.update(|config| {
    config.video.width = 1280;
    config.video.height = 720;
    config.video.fps = 60;
}).await?;

// 方式 2: 整体替换
let mut new_config = (*config_store.get()).clone();
new_config.stream.mode = StreamMode::WebRTC;
new_config.stream.encoder = EncoderType::Rkmpp;
config_store.set(new_config).await?;
```

### 7.3 订阅配置变更

```rust
// 在组件中监听配置变更
let config_store = state.config.clone();
let mut rx = config_store.subscribe();

tokio::spawn(async move {
    while let Ok(change) = rx.recv().await {
        tracing::info!("配置 {} 已变更", change.key);

        // 重新加载配置
        let config = config_store.get();

        // 执行重载逻辑
        if let Err(e) = reload_component(&config).await {
            tracing::error!("重载组件失败: {}", e);
        }
    }
});
```

### 7.4 在 Handler 中使用

```rust
use axum::{extract::State, Json};
use std::sync::Arc;

use crate::config::VideoConfig;
use crate::state::AppState;

// 获取视频配置
pub async fn get_video_config(
    State(state): State<Arc<AppState>>
) -> Json<VideoConfig> {
    let config = state.config.get();
    Json(config.video.clone())
}

// 更新视频配置
pub async fn update_video_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<VideoConfig>,
) -> Result<Json<VideoConfig>> {
    // 更新配置
    state.config.update(|config| {
        config.video = update;
    }).await?;

    // 返回更新后的配置
    let config = state.config.get();
    Ok(Json(config.video.clone()))
}
```

### 7.5 访问数据库连接池

```rust
// ConfigStore 还提供数据库连接池访问
// 用于用户管理、会话管理等功能

let pool = config_store.pool();

// 查询用户
let user: Option<User> = sqlx::query_as(
    "SELECT * FROM users WHERE username = ?"
)
.bind(username)
.fetch_optional(pool)
.await?;
```

---

## 8. 默认配置

系统首次运行时会自动创建默认配置：

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            initialized: false,  // 需要通过初始设置向导完成
            auth: AuthConfig {
                session_timeout_secs: 86400,  // 24小时
                totp_enabled: false,
                totp_secret: None,
            },
            video: VideoConfig {
                device: None,      // 自动检测
                format: None,      // 自动检测或使用 MJPEG
                width: 1920,
                height: 1080,
                fps: 30,
                quality: 80,
            },
            stream: StreamConfig {
                mode: StreamMode::Mjpeg,
                encoder: EncoderType::Auto,
                bitrate_preset: BitratePreset::Balanced,
                stun_server: Some("stun:stun.l.google.com:19302".to_string()),
                turn_server: None,
                turn_username: None,
                turn_password: None,
                auto_pause_enabled: false,
                auto_pause_delay_secs: 10,
                client_timeout_secs: 30,
            },
            hid: HidConfig {
                backend: HidBackend::None,  // 需要用户手动启用
                otg_keyboard: "/dev/hidg0".to_string(),
                otg_mouse: "/dev/hidg1".to_string(),
                otg_udc: None,              // 自动检测
                otg_descriptor: OtgDescriptorConfig::default(),
                ch9329_port: "/dev/ttyUSB0".to_string(),
                ch9329_baudrate: 9600,
                mouse_absolute: true,
            },
            msd: MsdConfig {
                enabled: true,
                images_path: "./data/msd/images".to_string(),
                drive_path: "./data/msd/ventoy.img".to_string(),
                virtual_drive_size_mb: 16384,  // 16GB
            },
            atx: AtxConfig {
                enabled: false,  // 需要用户配置硬件绑定
                power: AtxKeyConfig::default(),
                reset: AtxKeyConfig::default(),
                led: AtxLedConfig::default(),
                wol_interface: String::new(),  // 自动检测
            },
            audio: AudioConfig {
                enabled: false,
                device: "default".to_string(),
                quality: "balanced".to_string(),
            },
            web: WebConfig {
                http_port: 8080,
                https_port: 8443,
                bind_address: "0.0.0.0".to_string(),
                https_enabled: false,
                ssl_cert_path: None,
                ssl_key_path: None,
            },
            extensions: ExtensionsConfig::default(),
            rustdesk: RustDeskConfig::default(),
        }
    }
}
```

**配置初始化流程**：
1. 用户首次访问 Web UI，系统检测到 `initialized = false`
2. 重定向到初始设置向导（`/setup`）
3. 用户设置管理员账户、选择视频设备等
4. 完成设置后，`initialized` 设为 `true`
5. 后续可通过设置页面（`/settings`）修改各项配置
