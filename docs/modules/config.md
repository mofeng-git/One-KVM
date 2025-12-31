# Config 模块文档

## 1. 模块概述

Config 模块提供配置管理功能，所有配置存储在 SQLite 数据库中。

### 1.1 主要功能

- SQLite 配置存储
- 类型安全的配置结构
- 热重载支持
- TypeScript 类型生成

### 1.2 文件结构

```
src/config/
├── mod.rs              # 模块导出
├── schema.rs           # 配置结构定义 (12KB)
└── store.rs            # SQLite 存储 (8KB)
```

---

## 2. 核心组件

### 2.1 ConfigStore (store.rs)

```rust
pub struct ConfigStore {
    db: Pool<Sqlite>,
}

impl ConfigStore {
    /// 创建存储
    pub async fn new(db_path: &Path) -> Result<Self>;

    /// 获取完整配置
    pub async fn get_config(&self) -> Result<AppConfig>;

    /// 更新配置
    pub async fn update_config(&self, config: &AppConfig) -> Result<()>;

    /// 获取单个配置项
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;

    /// 设置单个配置项
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;

    /// 删除配置项
    pub async fn delete(&self, key: &str) -> Result<()>;

    /// 重置为默认
    pub async fn reset_to_default(&self) -> Result<()>;
}
```

### 2.2 AppConfig (schema.rs)

```rust
#[derive(Serialize, Deserialize, Default)]
#[typeshare]
pub struct AppConfig {
    /// 视频配置
    pub video: VideoConfig,

    /// 流配置
    pub stream: StreamConfig,

    /// HID 配置
    pub hid: HidConfig,

    /// MSD 配置
    pub msd: MsdConfig,

    /// ATX 配置
    pub atx: AtxConfig,

    /// 音频配置
    pub audio: AudioConfig,

    /// 认证配置
    pub auth: AuthConfig,

    /// Web 配置
    pub web: WebConfig,

    /// RustDesk 配置
    pub rustdesk: RustDeskConfig,

    /// 扩展配置
    pub extensions: ExtensionsConfig,
}
```

### 2.3 各模块配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct VideoConfig {
    pub device: Option<String>,
    pub format: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
}

#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct StreamConfig {
    pub mode: StreamMode,
    pub bitrate_kbps: u32,
    pub gop_size: u32,
    pub encoder: EncoderType,
    pub stun_server: Option<String>,
    pub turn_server: Option<String>,
    pub turn_username: Option<String>,
    pub turn_password: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct HidConfig {
    pub backend: HidBackendType,
    pub ch9329_device: Option<String>,
    pub ch9329_baud_rate: Option<u32>,
    pub default_mouse_mode: MouseMode,
}

// ... 其他配置结构
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

| 端点 | 方法 | 权限 | 描述 |
|------|------|------|------|
| `/api/config` | GET | Admin | 获取完整配置 |
| `/api/config` | PATCH | Admin | 更新配置 |
| `/api/config/video` | GET | Admin | 获取视频配置 |
| `/api/config/video` | PATCH | Admin | 更新视频配置 |
| `/api/config/stream` | GET | Admin | 获取流配置 |
| `/api/config/stream` | PATCH | Admin | 更新流配置 |
| `/api/config/hid` | GET | Admin | 获取 HID 配置 |
| `/api/config/hid` | PATCH | Admin | 更新 HID 配置 |
| `/api/config/reset` | POST | Admin | 重置为默认 |

### 响应格式

```json
// GET /api/config/video
{
    "device": "/dev/video0",
    "format": "MJPEG",
    "width": 1920,
    "height": 1080,
    "fps": 30,
    "quality": 80
}

// PATCH /api/config/video
// Request:
{
    "width": 1280,
    "height": 720
}

// Response: 更新后的完整配置
```

---

## 5. 配置热重载

配置更改后自动重载相关组件：

```rust
// 更新配置
config_store.update_config(&new_config).await?;

// 发布配置变更事件
events.publish(SystemEvent::ConfigChanged {
    section: "video".to_string(),
});

// 各组件监听事件并重载
// VideoStreamManager::on_config_changed()
// HidController::reload()
// etc.
```

---

## 6. 数据库结构

```sql
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

配置以 JSON 格式存储：

```
key: "app_config"
value: { "video": {...}, "hid": {...}, ... }
```

---

## 7. 使用示例

```rust
// 获取配置
let config = config_store.get_config().await?;
println!("Video device: {:?}", config.video.device);

// 更新配置
let mut config = config_store.get_config().await?;
config.video.width = 1280;
config.video.height = 720;
config_store.update_config(&config).await?;

// 获取单个配置项
let video: Option<VideoConfig> = config_store.get("video").await?;

// 设置单个配置项
config_store.set("video", &video_config).await?;
```

---

## 8. 默认配置

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            video: VideoConfig {
                device: None,
                format: None,
                width: 1920,
                height: 1080,
                fps: 30,
                quality: 80,
            },
            stream: StreamConfig {
                mode: StreamMode::Mjpeg,
                bitrate_kbps: 2000,
                gop_size: 60,
                encoder: EncoderType::H264,
                ..Default::default()
            },
            // ...
        }
    }
}
```
