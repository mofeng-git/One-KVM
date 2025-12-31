# ATX 模块文档

## 1. 模块概述

ATX 模块提供电源控制功能，通过 GPIO 或 USB 继电器控制目标计算机的电源和重置按钮。

### 1.1 主要功能

- 电源按钮控制
- 重置按钮控制
- 电源 LED 状态监视
- Wake-on-LAN 支持
- 多后端支持 (GPIO/USB 继电器)

### 1.2 文件结构

```
src/atx/
├── mod.rs              # 模块导出
├── controller.rs       # AtxController (11KB)
├── executor.rs         # 动作执行器 (10KB)
├── types.rs            # 类型定义 (7KB)
├── led.rs              # LED 监视 (5KB)
└── wol.rs              # Wake-on-LAN (5KB)
```

---

## 2. 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          ATX Architecture                                    │
└─────────────────────────────────────────────────────────────────────────────┘

                Web API
                   │
                   ▼
         ┌─────────────────┐
         │  AtxController  │
         │ (controller.rs) │
         └────────┬────────┘
                  │
    ┌─────────────┼─────────────┐
    │             │             │
    ▼             ▼             ▼
┌────────┐  ┌────────┐    ┌────────┐
│ Power  │  │ Reset  │    │  LED   │
│Executor│  │Executor│    │Monitor │
└───┬────┘  └───┬────┘    └───┬────┘
    │           │             │
    ▼           ▼             ▼
┌────────┐  ┌────────┐    ┌────────┐
│ GPIO   │  │ GPIO   │    │ GPIO   │
│ or USB │  │ or USB │    │  Input │
│ Relay  │  │ Relay  │    │        │
└───┬────┘  └───┬────┘    └───┬────┘
    │           │             │
    └───────────┼─────────────┘
                │
                ▼
        ┌───────────────┐
        │   Target PC   │
        │ (ATX Header)  │
        └───────────────┘
```

---

## 3. 核心组件

### 3.1 AtxController (controller.rs)

```rust
pub struct AtxController {
    /// 电源按钮配置
    power: Arc<AtxButton>,

    /// 重置按钮配置
    reset: Arc<AtxButton>,

    /// LED 监视器
    led_monitor: Arc<RwLock<Option<LedMonitor>>>,

    /// WoL 控制器
    wol: Arc<RwLock<Option<WolController>>>,

    /// 当前状态
    state: Arc<RwLock<AtxState>>,

    /// 事件总线
    events: Arc<EventBus>,
}

impl AtxController {
    /// 创建控制器
    pub fn new(config: &AtxConfig, events: Arc<EventBus>) -> Result<Self>;

    /// 短按电源按钮 (开机/正常关机)
    pub async fn power_short_press(&self) -> Result<()>;

    /// 长按电源按钮 (强制关机)
    pub async fn power_long_press(&self) -> Result<()>;

    /// 按重置按钮
    pub async fn reset_press(&self) -> Result<()>;

    /// 获取电源状态
    pub fn power_state(&self) -> PowerState;

    /// 发送 WoL 魔术包
    pub async fn wake_on_lan(&self, mac: &str) -> Result<()>;

    /// 获取状态
    pub fn state(&self) -> AtxState;

    /// 重新加载配置
    pub async fn reload(&self, config: &AtxConfig) -> Result<()>;
}

pub struct AtxState {
    /// 是否可用
    pub available: bool,

    /// 电源是否开启
    pub power_on: bool,

    /// 最后操作时间
    pub last_action: Option<DateTime<Utc>>,

    /// 错误信息
    pub error: Option<String>,
}

pub enum PowerState {
    On,
    Off,
    Unknown,
}
```

### 3.2 AtxButton (executor.rs)

```rust
pub struct AtxButton {
    /// 按钮名称
    name: String,

    /// 驱动类型
    driver: AtxDriverType,

    /// GPIO 句柄
    gpio: Option<LineHandle>,

    /// USB 继电器句柄
    relay: Option<UsbRelay>,

    /// 配置
    config: AtxKeyConfig,
}

impl AtxButton {
    /// 创建按钮
    pub fn new(name: &str, config: &AtxKeyConfig) -> Result<Self>;

    /// 短按 (100ms)
    pub async fn short_press(&self) -> Result<()>;

    /// 长按 (3000ms)
    pub async fn long_press(&self) -> Result<()>;

    /// 自定义按压时间
    pub async fn press(&self, duration: Duration) -> Result<()>;

    /// 设置输出状态
    fn set_output(&self, high: bool) -> Result<()>;
}

pub enum AtxDriverType {
    /// GPIO 直连
    Gpio,

    /// USB 继电器
    UsbRelay,

    /// 禁用
    None,
}
```

### 3.3 LedMonitor (led.rs)

```rust
pub struct LedMonitor {
    /// GPIO 引脚
    pin: u32,

    /// GPIO 句柄
    line: LineHandle,

    /// 当前状态
    state: Arc<AtomicBool>,

    /// 监视任务
    monitor_task: Option<JoinHandle<()>>,
}

impl LedMonitor {
    /// 创建监视器
    pub fn new(config: &AtxLedConfig) -> Result<Self>;

    /// 启动监视
    pub fn start(&mut self, events: Arc<EventBus>) -> Result<()>;

    /// 停止监视
    pub fn stop(&mut self);

    /// 获取当前状态
    pub fn state(&self) -> bool;
}
```

### 3.4 WolController (wol.rs)

```rust
pub struct WolController {
    /// 网络接口
    interface: String,

    /// 广播地址
    broadcast_addr: SocketAddr,
}

impl WolController {
    /// 创建控制器
    pub fn new(interface: Option<&str>) -> Result<Self>;

    /// 发送 WoL 魔术包
    pub async fn wake(&self, mac: &str) -> Result<()>;

    /// 构建魔术包
    fn build_magic_packet(mac: &[u8; 6]) -> [u8; 102];

    /// 解析 MAC 地址
    fn parse_mac(mac: &str) -> Result<[u8; 6]>;
}
```

---

## 4. 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct AtxConfig {
    /// 是否启用
    pub enabled: bool,

    /// 电源按钮配置
    pub power: AtxKeyConfig,

    /// 重置按钮配置
    pub reset: AtxKeyConfig,

    /// LED 监视配置
    pub led: AtxLedConfig,

    /// WoL 配置
    pub wol: WolConfig,
}

#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct AtxKeyConfig {
    /// 驱动类型
    pub driver: AtxDriverType,

    /// GPIO 芯片 (如 /dev/gpiochip0)
    pub gpio_chip: Option<String>,

    /// GPIO 引脚号
    pub gpio_pin: Option<u32>,

    /// USB 继电器设备
    pub relay_device: Option<String>,

    /// 继电器通道
    pub relay_channel: Option<u8>,

    /// 激活电平
    pub active_level: ActiveLevel,
}

#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct AtxLedConfig {
    /// 是否启用
    pub enabled: bool,

    /// GPIO 芯片
    pub gpio_chip: Option<String>,

    /// GPIO 引脚号
    pub gpio_pin: Option<u32>,

    /// 激活电平
    pub active_level: ActiveLevel,
}

pub enum ActiveLevel {
    High,
    Low,
}

impl Default for AtxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            power: AtxKeyConfig::default(),
            reset: AtxKeyConfig::default(),
            led: AtxLedConfig::default(),
            wol: WolConfig::default(),
        }
    }
}
```

---

## 5. API 端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/atx/status` | GET | 获取 ATX 状态 |
| `/api/atx/power/short` | POST | 短按电源 |
| `/api/atx/power/long` | POST | 长按电源 |
| `/api/atx/reset` | POST | 按重置 |
| `/api/atx/wol` | POST | 发送 WoL |

### 响应格式

```json
// GET /api/atx/status
{
    "available": true,
    "power_on": true,
    "last_action": "2024-01-15T10:30:00Z",
    "error": null
}

// POST /api/atx/wol
// Request: { "mac": "00:11:22:33:44:55" }
{
    "success": true
}
```

---

## 6. 硬件连接

### 6.1 GPIO 直连

```
One-KVM Device                 Target PC
┌─────────────┐               ┌─────────────┐
│   GPIO Pin  │───────────────│ Power SW    │
│   (Output)  │               │             │
└─────────────┘               └─────────────┘

接线说明:
- GPIO 引脚连接到 ATX 电源按钮
- 使用光耦或继电器隔离 (推荐)
- 注意电平匹配
```

### 6.2 USB 继电器

```
One-KVM Device                 USB Relay                Target PC
┌─────────────┐               ┌─────────────┐          ┌─────────────┐
│    USB      │───────────────│   Relay     │──────────│ Power SW    │
│             │               │             │          │             │
└─────────────┘               └─────────────┘          └─────────────┘

优点:
- 完全隔离
- 无需担心电平问题
- 更安全
```

---

## 7. 事件

```rust
pub enum SystemEvent {
    AtxStateChanged {
        power_on: bool,
        last_action: Option<String>,
        error: Option<String>,
    },

    AtxActionPerformed {
        action: String,  // "power_short" | "power_long" | "reset" | "wol"
        success: bool,
    },
}
```

---

## 8. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum AtxError {
    #[error("ATX not available")]
    NotAvailable,

    #[error("GPIO error: {0}")]
    GpioError(String),

    #[error("Relay error: {0}")]
    RelayError(String),

    #[error("WoL error: {0}")]
    WolError(String),

    #[error("Invalid MAC address: {0}")]
    InvalidMac(String),

    #[error("Operation in progress")]
    Busy,
}
```

---

## 9. 使用示例

```rust
let atx = AtxController::new(&config, events)?;

// 开机
atx.power_short_press().await?;

// 检查状态
tokio::time::sleep(Duration::from_secs(5)).await;
if atx.power_state() == PowerState::On {
    println!("PC is now on");
}

// 强制关机
atx.power_long_press().await?;

// 重置
atx.reset_press().await?;

// Wake-on-LAN
atx.wake_on_lan("00:11:22:33:44:55").await?;
```

---

## 10. 常见问题

### Q: GPIO 无法控制?

1. 检查引脚配置
2. 检查权限 (`/dev/gpiochip*`)
3. 检查接线

### Q: LED 状态不正确?

1. 检查 active_level 配置
2. 检查 GPIO 输入模式

### Q: WoL 不工作?

1. 检查目标 PC BIOS 设置
2. 检查网卡支持
3. 检查网络广播
