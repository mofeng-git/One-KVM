# MSD 模块文档

## 1. 模块概述

MSD (Mass Storage Device) 模块提供虚拟存储设备功能，允许将 ISO/IMG 镜像作为 USB 存储设备挂载到目标计算机。

### 1.1 主要功能

- ISO/IMG 镜像挂载
- 镜像下载管理
- Ventoy 多 ISO 启动盘
- 热插拔支持
- 下载进度追踪

### 1.2 文件结构

```
src/msd/
├── mod.rs              # 模块导出
├── controller.rs       # MsdController (20KB)
├── image.rs            # 镜像管理 (21KB)
├── ventoy_drive.rs     # Ventoy 驱动 (24KB)
├── monitor.rs          # 健康监视 (9KB)
└── types.rs            # 类型定义 (6KB)
```

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          MSD Architecture                                    │
└─────────────────────────────────────────────────────────────────────────────┘

                    Web API
                       │
                       ▼
              ┌─────────────────┐
              │  MsdController  │
              │ (controller.rs) │
              └────────┬────────┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
         ▼             ▼             ▼
┌─────────────┐ ┌───────────┐ ┌───────────┐
│   Image     │ │  Ventoy   │ │    OTG    │
│  Manager    │ │   Drive   │ │  Service  │
│ (image.rs)  │ │(ventoy.rs)│ │           │
└──────┬──────┘ └─────┬─────┘ └─────┬─────┘
       │              │             │
       ▼              ▼             ▼
┌─────────────┐ ┌───────────┐ ┌───────────┐
│  /data/     │ │  exFAT    │ │  MSD      │
│  images/    │ │  Drive    │ │ Function  │
└─────────────┘ └───────────┘ └───────────┘
                                    │
                                    ▼
                            ┌───────────────┐
                            │  Target PC    │
                            │  (USB Drive)  │
                            └───────────────┘
```

### 2.2 MSD 模式

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            MSD Modes                                         │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  Image Mode                                                                  │
│  ┌───────────┐                                                              │
│  │ ISO/IMG   │ ──► MSD LUN ──► Target PC sees single drive                 │
│  │ File      │                                                              │
│  └───────────┘                                                              │
│  特点:                                                                       │
│  - 单个镜像文件                                                              │
│  - 直接挂载                                                                  │
│  - 适合系统安装                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  Ventoy Mode                                                                 │
│  ┌───────────┐                                                              │
│  │  ISO 1    │                                                              │
│  ├───────────┤      ┌───────────┐                                          │
│  │  ISO 2    │ ──►  │  Ventoy   │ ──► Target PC sees bootable drive       │
│  ├───────────┤      │  Drive    │     with ISO selection menu              │
│  │  ISO 3    │      └───────────┘                                          │
│  └───────────┘                                                              │
│  特点:                                                                       │
│  - 多个 ISO 文件                                                             │
│  - exFAT 文件系统                                                            │
│  - 启动菜单选择                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 核心组件

### 3.1 MsdController (controller.rs)

MSD 控制器主类。

```rust
pub struct MsdController {
    /// 当前状态
    state: Arc<RwLock<MsdState>>,

    /// 镜像管理器
    image_manager: Arc<ImageManager>,

    /// Ventoy 驱动器
    ventoy_drive: Arc<RwLock<Option<VentoyDrive>>>,

    /// OTG 服务
    otg_service: Arc<OtgService>,

    /// MSD 函数句柄
    msd_function: Arc<RwLock<Option<MsdFunction>>>,

    /// 事件总线
    events: Arc<EventBus>,

    /// 数据目录
    data_dir: PathBuf,
}

impl MsdController {
    /// 创建控制器
    pub async fn new(
        otg_service: Arc<OtgService>,
        data_dir: PathBuf,
        events: Arc<EventBus>,
    ) -> Result<Arc<Self>>;

    /// 获取状态
    pub fn state(&self) -> MsdState;

    /// 连接 MSD
    pub async fn connect(&self) -> Result<()>;

    /// 断开 MSD
    pub async fn disconnect(&self) -> Result<()>;

    /// 切换到镜像模式
    pub async fn set_image(&self, image_id: &str) -> Result<()>;

    /// 切换到 Ventoy 模式
    pub async fn set_ventoy(&self) -> Result<()>;

    /// 清除当前挂载
    pub async fn clear(&self) -> Result<()>;

    /// 列出镜像
    pub fn list_images(&self) -> Vec<ImageInfo>;

    /// 上传镜像
    pub async fn upload_image(&self, name: &str, data: Bytes) -> Result<ImageInfo>;

    /// 从 URL 下载镜像
    pub async fn download_image(&self, url: &str) -> Result<String>;

    /// 删除镜像
    pub async fn delete_image(&self, image_id: &str) -> Result<()>;

    /// 获取下载进度
    pub fn get_download_progress(&self, download_id: &str) -> Option<DownloadProgress>;
}

pub struct MsdState {
    /// 是否可用
    pub available: bool,

    /// 当前模式
    pub mode: MsdMode,

    /// 是否已连接
    pub connected: bool,

    /// 当前镜像信息
    pub current_image: Option<ImageInfo>,

    /// 驱动器信息
    pub drive_info: Option<DriveInfo>,

    /// 错误信息
    pub error: Option<String>,
}

pub enum MsdMode {
    /// 未激活
    None,

    /// 单镜像模式
    Image,

    /// Ventoy 模式
    Drive,
}
```

### 3.2 ImageManager (image.rs)

镜像文件管理器。

```rust
pub struct ImageManager {
    /// 镜像目录
    images_dir: PathBuf,

    /// 镜像列表缓存
    images: RwLock<HashMap<String, ImageInfo>>,

    /// 下载任务
    downloads: RwLock<HashMap<String, DownloadTask>>,

    /// HTTP 客户端
    http_client: reqwest::Client,
}

impl ImageManager {
    /// 创建管理器
    pub fn new(images_dir: PathBuf) -> Result<Self>;

    /// 扫描镜像目录
    pub fn scan_images(&self) -> Result<Vec<ImageInfo>>;

    /// 获取镜像信息
    pub fn get_image(&self, id: &str) -> Option<ImageInfo>;

    /// 添加镜像
    pub async fn add_image(&self, name: &str, data: Bytes) -> Result<ImageInfo>;

    /// 删除镜像
    pub fn delete_image(&self, id: &str) -> Result<()>;

    /// 开始下载
    pub async fn start_download(&self, url: &str) -> Result<String>;

    /// 取消下载
    pub fn cancel_download(&self, download_id: &str) -> Result<()>;

    /// 获取下载进度
    pub fn get_download_progress(&self, download_id: &str) -> Option<DownloadProgress>;

    /// 验证镜像文件
    fn validate_image(path: &Path) -> Result<ImageFormat>;
}

pub struct ImageInfo {
    /// 唯一 ID
    pub id: String,

    /// 文件名
    pub name: String,

    /// 文件大小
    pub size: u64,

    /// 格式
    pub format: ImageFormat,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 下载状态
    pub download_status: Option<DownloadStatus>,
}

pub enum ImageFormat {
    /// ISO 光盘镜像
    Iso,

    /// 原始磁盘镜像
    Img,

    /// 未知格式
    Unknown,
}

pub struct DownloadProgress {
    /// 已下载字节
    pub downloaded: u64,

    /// 总字节数
    pub total: u64,

    /// 下载速度 (bytes/sec)
    pub speed: u64,

    /// 预计剩余时间
    pub eta_secs: u64,

    /// 状态
    pub status: DownloadStatus,
}

pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed(String),
    Cancelled,
}
```

### 3.3 VentoyDrive (ventoy_drive.rs)

Ventoy 可启动驱动器管理。

```rust
pub struct VentoyDrive {
    /// 驱动器路径
    drive_path: PathBuf,

    /// 镜像路径
    images: Vec<PathBuf>,

    /// 容量
    capacity: u64,

    /// 已用空间
    used: u64,
}

impl VentoyDrive {
    /// 创建 Ventoy 驱动器
    pub fn create(drive_path: PathBuf, capacity: u64) -> Result<Self>;

    /// 添加 ISO
    pub fn add_iso(&mut self, iso_path: &Path) -> Result<()>;

    /// 移除 ISO
    pub fn remove_iso(&mut self, name: &str) -> Result<()>;

    /// 列出 ISO
    pub fn list_isos(&self) -> Vec<String>;

    /// 获取驱动器信息
    pub fn info(&self) -> DriveInfo;

    /// 获取驱动器路径
    pub fn path(&self) -> &Path;
}

pub struct DriveInfo {
    /// 容量
    pub capacity: u64,

    /// 已用空间
    pub used: u64,

    /// 可用空间
    pub available: u64,

    /// ISO 列表
    pub isos: Vec<String>,
}
```

---

## 4. 类型定义

### 4.1 MSD 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct MsdConfig {
    /// 是否启用 MSD
    pub enabled: bool,

    /// 镜像目录
    pub images_dir: Option<String>,

    /// 默认模式
    pub default_mode: MsdMode,

    /// Ventoy 容量 (MB)
    pub ventoy_capacity_mb: u32,
}

impl Default for MsdConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            images_dir: None,
            default_mode: MsdMode::None,
            ventoy_capacity_mb: 4096,  // 4GB
        }
    }
}
```

---

## 5. API 端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/msd/status` | GET | 获取 MSD 状态 |
| `/api/msd/connect` | POST | 连接 MSD |
| `/api/msd/disconnect` | POST | 断开 MSD |
| `/api/msd/images` | GET | 列出镜像 |
| `/api/msd/images` | POST | 上传镜像 |
| `/api/msd/images/:id` | DELETE | 删除镜像 |
| `/api/msd/images/download` | POST | 从 URL 下载 |
| `/api/msd/images/download/:id` | GET | 获取下载进度 |
| `/api/msd/images/download/:id` | DELETE | 取消下载 |
| `/api/msd/set-image` | POST | 设置当前镜像 |
| `/api/msd/set-ventoy` | POST | 设置 Ventoy 模式 |
| `/api/msd/clear` | POST | 清除挂载 |

### 响应格式

```json
// GET /api/msd/status
{
    "available": true,
    "mode": "image",
    "connected": true,
    "current_image": {
        "id": "abc123",
        "name": "ubuntu-22.04.iso",
        "size": 4700000000,
        "format": "iso"
    },
    "drive_info": null,
    "error": null
}

// GET /api/msd/images
{
    "images": [
        {
            "id": "abc123",
            "name": "ubuntu-22.04.iso",
            "size": 4700000000,
            "format": "iso",
            "created_at": "2024-01-15T10:30:00Z"
        }
    ]
}

// POST /api/msd/images/download
// Request: { "url": "https://example.com/image.iso" }
// Response: { "download_id": "xyz789" }

// GET /api/msd/images/download/xyz789
{
    "downloaded": 1234567890,
    "total": 4700000000,
    "speed": 12345678,
    "eta_secs": 280,
    "status": "downloading"
}
```

---

## 6. 事件

```rust
pub enum SystemEvent {
    MsdStateChanged {
        mode: MsdMode,
        connected: bool,
        image: Option<String>,
        error: Option<String>,
    },

    MsdDownloadProgress {
        download_id: String,
        progress: DownloadProgress,
    },

    MsdDownloadComplete {
        download_id: String,
        image_id: String,
        success: bool,
        error: Option<String>,
    },
}
```

---

## 7. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum MsdError {
    #[error("MSD not available")]
    NotAvailable,

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Not connected")]
    NotConnected,

    #[error("Image not found: {0}")]
    ImageNotFound(String),

    #[error("Invalid image format: {0}")]
    InvalidFormat(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Storage full")]
    StorageFull,

    #[error("OTG error: {0}")]
    OtgError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

---

## 8. 使用示例

### 8.1 挂载 ISO 镜像

```rust
let msd = MsdController::new(otg_service, data_dir, events).await?;

// 列出镜像
let images = msd.list_images();
println!("Available images: {:?}", images);

// 设置镜像
msd.set_image("abc123").await?;

// 连接到目标 PC
msd.connect().await?;

// 目标 PC 现在可以看到 USB 驱动器...

// 断开连接
msd.disconnect().await?;
```

### 8.2 从 URL 下载

```rust
// 开始下载
let download_id = msd.download_image("https://example.com/ubuntu.iso").await?;

// 监控进度
loop {
    if let Some(progress) = msd.get_download_progress(&download_id) {
        println!("Progress: {}%", progress.downloaded * 100 / progress.total);

        if matches!(progress.status, DownloadStatus::Completed) {
            break;
        }
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

### 8.3 使用 Ventoy 模式

```rust
// 切换到 Ventoy 模式
msd.set_ventoy().await?;

// 获取驱动器信息
let state = msd.state();
if let Some(drive_info) = state.drive_info {
    println!("Capacity: {} MB", drive_info.capacity / 1024 / 1024);
    println!("ISOs: {:?}", drive_info.isos);
}

// 连接
msd.connect().await?;
```

---

## 9. 常见问题

### Q: 镜像无法挂载?

1. 检查镜像文件完整性
2. 确认文件格式正确
3. 检查存储空间

### Q: 目标 PC 不识别?

1. 检查 USB 连接
2. 尝试重新连接
3. 查看目标 PC 的设备管理器

### Q: 下载速度慢?

1. 检查网络连接
2. 使用更近的镜像源
3. 检查磁盘 I/O

### Q: Ventoy 启动失败?

1. 检查目标 PC BIOS 设置
2. 尝试不同的启动模式
3. 确认 ISO 文件支持 Ventoy
