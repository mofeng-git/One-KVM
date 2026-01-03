# HID 模块文档

## 1. 模块概述

HID (Human Interface Device) 模块负责将键盘和鼠标事件转发到目标计算机，是 One-KVM 实现远程控制的核心模块。

### 1.1 主要功能

- 键盘事件处理 (按键、修饰键)
- 鼠标事件处理 (移动、点击、滚轮)
- 多媒体键支持 (Consumer Control)
- 支持绝对和相对鼠标模式
- 多后端支持 (OTG、CH9329)
- WebSocket 和 DataChannel 输入
- 自动错误恢复和健康监控

### 1.2 USB Endpoint 使用

OTG 模式下的 endpoint 分配：

| 功能 | IN 端点 | OUT 端点 | 说明 |
|------|---------|----------|------|
| Keyboard | 1 | 1 | 带 LED 反馈 |
| MouseRelative | 1 | 0 | 相对鼠标 |
| MouseAbsolute | 1 | 0 | 绝对鼠标 |
| ConsumerControl | 1 | 0 | 多媒体键 |
| **HID 总计** | **4** | **1** | |
| MSD | 1 | 1 | 大容量存储 |
| **全部总计** | **5** | **2** | 兼容 6 endpoint 设备 |

> 注：EP0 (控制端点) 独立于数据端点，不计入上述统计。

### 1.3 文件结构

```
src/hid/
├── mod.rs              # HidController 主控制器
├── backend.rs          # HidBackend trait 和 HidBackendType
├── otg.rs              # OTG USB Gadget 后端实现
├── ch9329.rs           # CH9329 串口 HID 控制器后端
├── consumer.rs         # Consumer Control usage codes
├── keymap.rs           # JS keyCode 到 USB HID 的转换表
├── types.rs            # 事件类型定义 (KeyboardEvent, MouseEvent等)
├── monitor.rs          # 健康监视器 (HidHealthMonitor)
├── datachannel.rs      # DataChannel 二进制协议解析
└── websocket.rs        # WebSocket 二进制协议适配
```

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          HID Architecture                                    │
└─────────────────────────────────────────────────────────────────────────────┘

           Browser Input Events
                   │
         ┌─────────┴─────────┐
         │                   │
         ▼                   ▼
┌─────────────────┐  ┌─────────────────┐
│   WebSocket     │  │  DataChannel    │
│   Handler       │  │   Handler       │
│ (websocket.rs)  │  │(datachannel.rs) │
└────────┬────────┘  └────────┬────────┘
         │                    │
         └──────────┬─────────┘
                    │
                    ▼
         ┌─────────────────────┐
         │   HidController     │
         │     (mod.rs)        │
         │  - send_keyboard()  │
         │  - send_mouse()     │
         │  - send_consumer()  │
         │  - monitor (health) │
         └──────────┬──────────┘
                    │
         ┌──────────┼──────────┐
         │          │          │
         ▼          ▼          ▼
┌─────────────┐ ┌──────────┐ ┌──────────┐
│  OTG Backend│ │  CH9329  │ │   None   │
│  (otg.rs)   │ │ Backend  │ │ (dummy)  │
└──────┬──────┘ └────┬─────┘ └──────────┘
       │             │
       ▼             ▼
┌─────────────┐ ┌──────────┐
│ /dev/hidg*  │ │ Serial   │
│ USB Gadget  │ │ Port     │
└─────────────┘ └──────────┘
       │             │
       └──────┬──────┘
              │
              ▼
      ┌─────────────┐
      │  Target PC  │
      └─────────────┘
```

### 2.2 后端选择

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Backend Selection                                     │
└─────────────────────────────────────────────────────────────────────────────┘

HidBackendType::Otg
    │
    ├── 检查 OtgService 是否可用
    │
    ├── 请求 HID 函数 (4个设备, 共4个IN端点, 1个OUT端点)
    │   ├── /dev/hidg0 (键盘, 1 IN, 1 OUT for LED)
    │   ├── /dev/hidg1 (相对鼠标, 1 IN)
    │   ├── /dev/hidg2 (绝对鼠标, 1 IN)
    │   └── /dev/hidg3 (Consumer Control, 1 IN)
    │
    └── 创建 OtgBackend (从 HidDevicePaths)

HidBackendType::Ch9329 { port, baud_rate }
    │
    ├── 打开串口设备
    │
    ├── 初始化 CH9329 芯片
    │
    └── 创建 Ch9329Backend

HidBackendType::None
    │
    └── 不创建后端 (HID 功能禁用)
```

---

## 3. 核心组件

### 3.1 HidController (mod.rs)

HID 控制器主类，统一管理所有 HID 操作。

```rust
pub struct HidController {
    /// OTG Service reference (only used when backend is OTG)
    otg_service: Option<Arc<OtgService>>,

    /// Active backend
    backend: Arc<RwLock<Option<Box<dyn HidBackend>>>>,

    /// Backend type (mutable for reload)
    backend_type: RwLock<HidBackendType>,

    /// Event bus for broadcasting state changes (optional)
    events: RwLock<Option<Arc<EventBus>>>,

    /// Health monitor for error tracking and recovery
    monitor: Arc<HidHealthMonitor>,
}

impl HidController {
    /// Create a new HID controller with specified backend
    pub fn new(backend_type: HidBackendType, otg_service: Option<Arc<OtgService>>) -> Self;

    /// Set event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>);

    /// Initialize the HID backend
    pub async fn init(&self) -> Result<()>;

    /// Shutdown the HID backend and release resources
    pub async fn shutdown(&self) -> Result<()>;

    /// Send keyboard event
    pub async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()>;

    /// Send mouse event
    pub async fn send_mouse(&self, event: MouseEvent) -> Result<()>;

    /// Send consumer control event (multimedia keys)
    pub async fn send_consumer(&self, event: ConsumerEvent) -> Result<()>;

    /// Reset all keys (release all pressed keys)
    pub async fn reset(&self) -> Result<()>;

    /// Check if backend is available
    pub async fn is_available(&self) -> bool;

    /// Get backend type
    pub async fn backend_type(&self) -> HidBackendType;

    /// Get backend info
    pub async fn info(&self) -> Option<HidInfo>;

    /// Get current state as SystemEvent
    pub async fn current_state_event(&self) -> SystemEvent;

    /// Get the health monitor reference
    pub fn monitor(&self) -> &Arc<HidHealthMonitor>;

    /// Get current health status
    pub async fn health_status(&self) -> HidHealthStatus;

    /// Check if the HID backend is healthy
    pub async fn is_healthy(&self) -> bool;

    /// Reload the HID backend with new type
    pub async fn reload(&self, new_backend_type: HidBackendType) -> Result<()>;
}

pub struct HidInfo {
    /// Backend name
    pub name: &'static str,
    /// Whether backend is initialized
    pub initialized: bool,
    /// Whether absolute mouse positioning is supported
    pub supports_absolute_mouse: bool,
    /// Screen resolution for absolute mouse
    pub screen_resolution: Option<(u32, u32)>,
}
```

### 3.2 HidBackend Trait (backend.rs)

```rust
#[async_trait]
pub trait HidBackend: Send + Sync {
    /// Get backend name
    fn name(&self) -> &'static str;

    /// Initialize the backend
    async fn init(&self) -> Result<()>;

    /// Send a keyboard event
    async fn send_keyboard(&self, event: KeyboardEvent) -> Result<()>;

    /// Send a mouse event
    async fn send_mouse(&self, event: MouseEvent) -> Result<()>;

    /// Send a consumer control event (multimedia keys)
    async fn send_consumer(&self, event: ConsumerEvent) -> Result<()>;

    /// Reset all inputs (release all keys/buttons)
    async fn reset(&self) -> Result<()>;

    /// Shutdown the backend
    async fn shutdown(&self) -> Result<()>;

    /// Check if backend supports absolute mouse positioning
    fn supports_absolute_mouse(&self) -> bool;

    /// Get screen resolution (for absolute mouse)
    fn screen_resolution(&self) -> Option<(u32, u32)>;

    /// Set screen resolution (for absolute mouse)
    fn set_screen_resolution(&mut self, width: u32, height: u32);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HidBackendType {
    /// USB OTG gadget mode
    Otg,

    /// CH9329 serial HID controller
    Ch9329 {
        port: String,
        baud_rate: u32,
    },

    /// No HID backend (disabled)
    None,
}

impl HidBackendType {
    /// Check if OTG backend is available on this system
    pub fn otg_available() -> bool;

    /// Detect the best available backend
    pub fn detect() -> Self;

    /// Get backend name as string
    pub fn name_str(&self) -> &str;
}
```

### 3.3 OTG 后端 (otg.rs)

通过 Linux USB OTG gadget 模拟 HID 设备。

```rust
pub struct OtgBackend {
    /// Keyboard device path (/dev/hidg0)
    keyboard_path: PathBuf,
    /// Relative mouse device path (/dev/hidg1)
    mouse_rel_path: PathBuf,
    /// Absolute mouse device path (/dev/hidg2)
    mouse_abs_path: PathBuf,
    /// Consumer control device path (/dev/hidg3)
    consumer_path: PathBuf,

    /// Keyboard device file
    keyboard_dev: Mutex<Option<File>>,
    /// Relative mouse device file
    mouse_rel_dev: Mutex<Option<File>>,
    /// Absolute mouse device file
    mouse_abs_dev: Mutex<Option<File>>,
    /// Consumer control device file
    consumer_dev: Mutex<Option<File>>,

    /// Current keyboard state
    keyboard_state: Mutex<KeyboardReport>,
    /// Current mouse button state
    mouse_buttons: AtomicU8,

    /// Last known LED state
    led_state: RwLock<LedState>,
    /// Screen resolution for absolute mouse
    screen_resolution: RwLock<Option<(u32, u32)>>,

    /// UDC name for state checking
    udc_name: RwLock<Option<String>>,
    /// Whether the device is currently online
    online: AtomicBool,

    /// Error tracking (for log throttling)
    last_error_log: Mutex<Instant>,
    error_count: AtomicU8,
    eagain_count: AtomicU8,
}

impl OtgBackend {
    /// Create OTG backend from device paths provided by OtgService
    pub fn from_handles(paths: HidDevicePaths) -> Result<Self>;

    /// Set the UDC name for state checking
    pub fn set_udc_name(&self, udc: &str);

    /// Check if the UDC is in "configured" state
    pub fn is_udc_configured(&self) -> bool;

    /// Check if device is online
    pub fn is_online(&self) -> bool;

    /// Read keyboard LED state (non-blocking)
    pub fn read_led_state(&self) -> Result<Option<LedState>>;

    /// Get last known LED state
    pub fn led_state(&self) -> LedState;
}

/// Keyboard LED state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LedState {
    pub num_lock: bool,
    pub caps_lock: bool,
    pub scroll_lock: bool,
    pub compose: bool,
    pub kana: bool,
}

impl LedState {
    /// Create from raw byte
    pub fn from_byte(b: u8) -> Self;

    /// Convert to raw byte
    pub fn to_byte(&self) -> u8;
}
```

#### HID 报告格式

```rust
/// 键盘报告 (8 字节)
#[derive(Debug, Clone, Default)]
pub struct KeyboardReport {
    pub modifiers: u8,    // Ctrl, Shift, Alt, Meta
    pub reserved: u8,     // 保留
    pub keys: [u8; 6],    // 最多 6 个按键 USB HID code
}

impl KeyboardReport {
    /// Convert to bytes for USB HID
    pub fn to_bytes(&self) -> [u8; 8];

    /// Add a key to the report
    pub fn add_key(&mut self, key: u8) -> bool;

    /// Remove a key from the report
    pub fn remove_key(&mut self, key: u8);

    /// Clear all keys
    pub fn clear(&mut self);
}

/// 鼠标报告 (相对模式 4 字节, 绝对模式 6 字节)
#[derive(Debug, Clone, Default)]
pub struct MouseReport {
    pub buttons: u8,      // 按钮状态
    pub x: i8,            // X 移动 (-127 ~ 127) for relative
    pub y: i8,            // Y 移动 (-127 ~ 127) for relative
    pub wheel: i8,        // 滚轮 (-127 ~ 127)
}

impl MouseReport {
    /// Convert to bytes for USB HID (relative mouse)
    pub fn to_bytes_relative(&self) -> [u8; 4];

    /// Convert to bytes for USB HID (absolute mouse)
    pub fn to_bytes_absolute(&self, x: u16, y: u16) -> [u8; 6];
}
```

#### 错误恢复机制

OTG 后端实现了基于 PiKVM 和 JetKVM 的自动错误恢复：

- **EAGAIN (errno 11)**: 资源暂时不可用 - 使用 `poll()` 等待设备可写，超时后静默丢弃
- **ESHUTDOWN (errno 108)**: 传输端点关闭 - 关闭设备句柄，下次操作时自动重新打开
- **Write Timeout**: 使用 500ms 超时 (`HID_WRITE_TIMEOUT_MS`)，超时后静默丢弃数据
- **日志限流**: 防止大量错误日志泛滥

### 3.4 CH9329 后端 (ch9329.rs)

通过 CH9329 芯片（串口转 HID）实现 HID 功能。

```rust
pub struct Ch9329Backend {
    /// 串口设备
    port: Mutex<Box<dyn SerialPort>>,

    /// 设备路径
    device_path: String,

    /// 波特率
    baud_rate: u32,

    /// 当前键盘状态
    keyboard_state: Mutex<KeyboardState>,

    /// 连接状态
    connected: AtomicBool,
}

impl Ch9329Backend {
    /// 创建 CH9329 后端
    pub fn with_baud_rate(device: &str, baud_rate: u32) -> Result<Self>;

    /// 发送命令
    fn send_command(&self, cmd: &[u8]) -> Result<Vec<u8>>;

    /// 发送键盘数据包
    fn send_keyboard_packet(&self, report: &KeyboardReport) -> Result<()>;

    /// 发送鼠标数据包
    fn send_mouse_packet(&self, report: &[u8], absolute: bool) -> Result<()>;
}
```

#### CH9329 协议

```
帧格式:
┌──────┬──────┬──────┬──────────┬──────────┬──────┐
│ HEAD │ ADDR │ CMD  │ LEN      │ DATA     │ SUM  │
│ 0x57 │ 0xAB │ 0xXX │ data_len │ payload  │ csum │
└──────┴──────┴──────┴──────────┴──────────┴──────┘

命令码:
0x02 - 发送键盘数据
0x04 - 发送绝对鼠标数据
0x05 - 发送相对鼠标数据
0x0E - 获取芯片信息
```

---

## 4. 事件类型

### 4.1 键盘事件 (types.rs)

```rust
/// Keyboard event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyEventType {
    Down,  // 按键按下
    Up,    // 按键释放
}

/// Keyboard modifier flags
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardModifiers {
    pub left_ctrl: bool,
    pub left_shift: bool,
    pub left_alt: bool,
    pub left_meta: bool,
    pub right_ctrl: bool,
    pub right_shift: bool,
    pub right_alt: bool,
    pub right_meta: bool,
}

impl KeyboardModifiers {
    /// Convert to USB HID modifier byte
    pub fn to_hid_byte(&self) -> u8;

    /// Create from USB HID modifier byte
    pub fn from_hid_byte(byte: u8) -> Self;

    /// Check if any modifier is active
    pub fn any(&self) -> bool;
}

/// Keyboard event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardEvent {
    /// Event type (down/up)
    #[serde(rename = "type")]
    pub event_type: KeyEventType,

    /// Key code (USB HID usage code or JavaScript keyCode)
    pub key: u8,

    /// Modifier keys state
    #[serde(default)]
    pub modifiers: KeyboardModifiers,

    /// If true, key is already USB HID code (skip js_to_usb conversion)
    #[serde(default)]
    pub is_usb_hid: bool,
}
```

### 4.2 鼠标事件 (types.rs)

```rust
/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButton {
    /// Convert to USB HID button bit
    pub fn to_hid_bit(&self) -> u8;
}

/// Mouse event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseEventType {
    Move,     // 相对移动
    MoveAbs,  // 绝对位置
    Down,     // 按钮按下
    Up,       // 按钮释放
    Scroll,   // 滚轮滚动
}

/// Mouse event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    /// Event type
    #[serde(rename = "type")]
    pub event_type: MouseEventType,

    /// X coordinate or delta
    #[serde(default)]
    pub x: i32,

    /// Y coordinate or delta
    #[serde(default)]
    pub y: i32,

    /// Button (for down/up events)
    #[serde(default)]
    pub button: Option<MouseButton>,

    /// Scroll delta (for scroll events)
    #[serde(default)]
    pub scroll: i8,
}
```

### 4.3 Consumer Control 事件 (types.rs)

```rust
/// Consumer control event (multimedia keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerEvent {
    /// USB HID Consumer Control Usage Code
    pub usage: u16,
}

// 常用 Usage Codes (定义在 consumer.rs)
pub mod usage {
    pub const PLAY_PAUSE: u16 = 0x00CD;
    pub const STOP: u16 = 0x00B7;
    pub const NEXT_TRACK: u16 = 0x00B5;
    pub const PREV_TRACK: u16 = 0x00B6;
    pub const MUTE: u16 = 0x00E2;
    pub const VOLUME_UP: u16 = 0x00E9;
    pub const VOLUME_DOWN: u16 = 0x00EA;
}
```

---

## 5. 按键映射

### 5.1 按键转换 (keymap.rs)

模块使用固定大小的查找表 (256 字节) 实现 JavaScript keyCode 到 USB HID usage code 的 O(1) 转换。

```rust
/// Convert JavaScript keyCode to USB HID keyCode
pub fn js_to_usb(js_code: u8) -> Option<u8>;

/// Check if a key code is a modifier key
pub fn is_modifier_key(usb_code: u8) -> bool;

/// Get modifier bit for a modifier key
pub fn modifier_bit(usb_code: u8) -> Option<u8>;

// USB HID key codes 定义在 usb 子模块
pub mod usb {
    pub const KEY_A: u8 = 0x04;
    pub const KEY_ENTER: u8 = 0x28;
    pub const KEY_LEFT_CTRL: u8 = 0xE0;
    // ... 等等
}

// JavaScript key codes 定义在 js 子模块
pub mod js {
    pub const KEY_A: u8 = 65;
    pub const KEY_ENTER: u8 = 13;
    // ... 等等
}
```

### 5.2 转换示例

```
JavaScript    →    USB HID
65 (KEY_A)    →    0x04
13 (ENTER)    →    0x28
37 (LEFT)     →    0x50
17 (CTRL)     →    0xE0
```

---

## 6. 输入处理器

### 6.1 WebSocket Handler (websocket.rs)

使用二进制协议 (与 DataChannel 格式相同):

```rust
/// Binary response codes
const RESP_OK: u8 = 0x00;
const RESP_ERR_HID_UNAVAILABLE: u8 = 0x01;
const RESP_ERR_INVALID_MESSAGE: u8 = 0x02;

/// WebSocket HID upgrade handler
pub async fn ws_hid_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>
) -> Response;

/// Handle HID WebSocket connection
async fn handle_hid_socket(socket: WebSocket, state: Arc<AppState>);

/// Handle binary HID message (same format as DataChannel)
async fn handle_binary_message(data: &[u8], state: &AppState) -> Result<(), String>;
```

### 6.2 DataChannel Handler (datachannel.rs)

用于 WebRTC 模式下的 HID 事件处理，使用二进制协议。

#### 二进制消息格式

```
消息类型常量:
MSG_KEYBOARD = 0x01  // 键盘事件
MSG_MOUSE    = 0x02  // 鼠标事件
MSG_CONSUMER = 0x03  // Consumer Control 事件

键盘消息 (4 字节):
┌──────────┬──────────┬──────────┬──────────┐
│ MSG_TYPE │ EVENT    │ KEY_CODE │ MODIFIER │
│  0x01    │ 0/1      │ JS code  │ bitmask  │
└──────────┴──────────┴──────────┴──────────┘
EVENT: 0=down, 1=up

鼠标消息 (7 字节):
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ MSG_TYPE │ EVENT    │ X (i16)  │ Y (i16)  │ BTN/SCRL │
│  0x02    │ 0-4      │ LE       │ LE       │ u8/i8    │
└──────────┴──────────┴──────────┴──────────┴──────────┘
EVENT: 0=move, 1=moveabs, 2=down, 3=up, 4=scroll

Consumer Control 消息 (3 字节):
┌──────────┬──────────────────────┐
│ MSG_TYPE │ USAGE CODE (u16 LE)  │
│  0x03    │ e.g. 0x00CD          │
└──────────┴──────────────────────┘
```

```rust
/// Parsed HID event from DataChannel
#[derive(Debug, Clone)]
pub enum HidChannelEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Consumer(ConsumerEvent),
}

/// Parse a binary HID message from DataChannel
pub fn parse_hid_message(data: &[u8]) -> Option<HidChannelEvent>;

/// Encode events to binary format (for sending to client if needed)
pub fn encode_keyboard_event(event: &KeyboardEvent) -> Vec<u8>;
pub fn encode_mouse_event(event: &MouseEvent) -> Vec<u8>;
```

---

## 7. 健康监视

### 7.1 HidHealthMonitor (monitor.rs)

```rust
/// HID health status
#[derive(Debug, Clone, PartialEq)]
pub enum HidHealthStatus {
    /// Device is healthy and operational
    Healthy,

    /// Device has an error, attempting recovery
    Error {
        reason: String,
        error_code: String,
        retry_count: u32,
    },

    /// Device is disconnected
    Disconnected,
}

/// HID health monitor configuration
#[derive(Debug, Clone)]
pub struct HidMonitorConfig {
    /// Health check interval in milliseconds
    pub check_interval_ms: u64,
    /// Retry interval when device is lost (milliseconds)
    pub retry_interval_ms: u64,
    /// Maximum retry attempts before giving up (0 = infinite)
    pub max_retries: u32,
    /// Log throttle interval in seconds
    pub log_throttle_secs: u64,
    /// Recovery cooldown in milliseconds
    pub recovery_cooldown_ms: u64,
}

/// HID health monitor
pub struct HidHealthMonitor {
    /// Current health status
    status: RwLock<HidHealthStatus>,
    /// Event bus for notifications
    events: RwLock<Option<Arc<EventBus>>>,
    /// Log throttler to prevent log flooding
    throttler: LogThrottler,
    /// Configuration
    config: HidMonitorConfig,
    /// Current retry count
    retry_count: AtomicU32,
    /// Last error code (for change detection)
    last_error_code: RwLock<Option<String>>,
    /// Last recovery timestamp (for cooldown)
    last_recovery_ms: AtomicU64,
}

impl HidHealthMonitor {
    /// Create a new HID health monitor
    pub fn new(config: HidMonitorConfig) -> Self;

    /// Create with default configuration
    pub fn with_defaults() -> Self;

    /// Set the event bus for broadcasting state changes
    pub async fn set_event_bus(&self, events: Arc<EventBus>);

    /// Report an error from HID operations
    pub async fn report_error(
        &self,
        backend: &str,
        device: Option<&str>,
        reason: &str,
        error_code: &str,
    );

    /// Report that a reconnection attempt is starting
    pub async fn report_reconnecting(&self, backend: &str);

    /// Report that the device has recovered
    pub async fn report_recovered(&self, backend: &str);

    /// Get the current health status
    pub async fn status(&self) -> HidHealthStatus;

    /// Get the current retry count
    pub fn retry_count(&self) -> u32;

    /// Check if the monitor is in an error state
    pub async fn is_error(&self) -> bool;

    /// Check if the monitor is healthy
    pub async fn is_healthy(&self) -> bool;

    /// Reset the monitor to healthy state
    pub async fn reset(&self);

    /// Check if we should continue retrying
    pub fn should_retry(&self) -> bool;

    /// Get the retry interval
    pub fn retry_interval(&self) -> Duration;
}
```

#### 错误处理流程

1. **报告错误**: `report_error()` - 更新状态、限流日志、发布事件
2. **重连通知**: `report_reconnecting()` - 每5次尝试发布一次事件
3. **恢复通知**: `report_recovered()` - 重置状态、发布恢复事件
4. **日志限流**: 5秒内不重复日志相同错误
5. **恢复冷却**: 恢复后1秒内抑制错误日志

---

## 8. 系统事件

```rust
pub enum SystemEvent {
    /// HID state changed
    HidStateChanged {
        backend: String,
        initialized: bool,
        error: Option<String>,
        error_code: Option<String>,
    },

    /// HID device lost
    HidDeviceLost {
        backend: String,
        device: Option<String>,
        reason: String,
        error_code: String,
    },

    /// HID reconnecting
    HidReconnecting {
        backend: String,
        attempt: u32,
    },

    /// HID recovered
    HidRecovered {
        backend: String,
    },
}
```

---

## 9. 错误处理

```rust
pub enum AppError {
    /// HID backend error
    HidError {
        backend: String,
        reason: String,
        error_code: String,
    },

    // ... 其他错误类型
}
```

常见错误码:

- `enoent` - 设备文件不存在 (ENOENT)
- `epipe` - 管道断开 (EPIPE)
- `eshutdown` - 端点关闭 (ESHUTDOWN)
- `eagain` - 资源暂时不可用 (EAGAIN)
- `eagain_retry` - EAGAIN 重试中 (内部使用，不报告给监视器)
- `enxio` - 设备或地址不存在 (ENXIO)
- `enodev` - 设备不存在 (ENODEV)
- `eio` - I/O 错误 (EIO)
- `io_error` - 其他 I/O 错误
- `not_opened` - 设备未打开
- `init_failed` - 初始化失败

---

## 10. 使用示例

### 10.1 初始化 HID 控制器

```rust
// 创建 HID 控制器 (OTG 模式)
let hid = HidController::new(
    HidBackendType::Otg,
    Some(otg_service.clone())
);

// 设置事件总线
hid.set_event_bus(event_bus.clone()).await;

// 初始化后端
hid.init().await?;
```

### 10.2 发送键盘事件

```rust
// 按下 Ctrl+C
hid.send_keyboard(KeyboardEvent {
    event_type: KeyEventType::Down,
    key: 67, // JS keyCode for 'C'
    modifiers: KeyboardModifiers {
        left_ctrl: true,
        ..Default::default()
    },
    is_usb_hid: false,
}).await?;

// 释放 Ctrl+C
hid.send_keyboard(KeyboardEvent {
    event_type: KeyEventType::Up,
    key: 67,
    modifiers: KeyboardModifiers::default(),
    is_usb_hid: false,
}).await?;
```

### 10.3 发送鼠标事件

```rust
// 移动鼠标到绝对位置 (屏幕中心)
hid.send_mouse(MouseEvent {
    event_type: MouseEventType::MoveAbs,
    x: 16384,  // 0-32767 范围 (HID 标准)
    y: 16384,
    button: None,
    scroll: 0,
}).await?;

// 点击左键
hid.send_mouse(MouseEvent::button_down(MouseButton::Left)).await?;
hid.send_mouse(MouseEvent::button_up(MouseButton::Left)).await?;

// 相对移动
hid.send_mouse(MouseEvent::move_rel(10, -10)).await?;

// 滚轮滚动
hid.send_mouse(MouseEvent::scroll(-1)).await?;
```

### 10.4 发送多媒体键

```rust
use crate::hid::consumer::usage;

// 播放/暂停
hid.send_consumer(ConsumerEvent {
    usage: usage::PLAY_PAUSE,
}).await?;

// 音量增加
hid.send_consumer(ConsumerEvent {
    usage: usage::VOLUME_UP,
}).await?;
```

### 10.5 重新加载后端

```rust
// 切换到 CH9329 后端
hid.reload(HidBackendType::Ch9329 {
    port: "/dev/ttyUSB0".to_string(),
    baud_rate: 9600,
}).await?;
```

---

## 11. 常见问题

### Q: OTG 模式下键盘/鼠标不工作?

1. 检查 `/dev/hidg*` 设备是否存在: `ls -l /dev/hidg*`
2. 检查 USB gadget 是否正确配置: `ls /sys/kernel/config/usb_gadget/`
3. 检查 UDC 是否绑定: `cat /sys/kernel/config/usb_gadget/*/UDC`
4. 检查目标 PC 是否识别 USB 设备 (在目标 PC 上运行 `dmesg` 或查看设备管理器)
5. 查看 One-KVM 日志: `journalctl -u one-kvm -f`

### Q: CH9329 无法初始化?

1. 检查串口设备路径: `ls -l /dev/ttyUSB*`
2. 检查串口权限: `sudo chmod 666 /dev/ttyUSB0`
3. 检查波特率设置 (默认 9600)
4. 使用 `minicom` 或 `screen` 测试串口连接:
   ```bash
   minicom -D /dev/ttyUSB0 -b 9600
   ```

### Q: 鼠标定位不准确?

1. 使用绝对鼠标模式 (默认)
2. 确保前端发送的坐标在 0-32767 范围内
3. 检查前端是否正确处理屏幕缩放
4. 检查浏览器缩放级别 (应为 100%)

### Q: 按键有延迟?

1. 检查网络延迟: `ping <kvm-ip>`
2. 使用 WebRTC 模式 (DataChannel) 而不是 WebSocket
3. 减少网络跳数 (避免多层代理)
4. 检查服务器 CPU 负载

### Q: 频繁出现 ESHUTDOWN 错误?

这是正常现象，通常发生在：
- MSD (大容量存储) 设备挂载/卸载时
- USB 主机重新枚举设备时
- 目标 PC 进入休眠/唤醒时

OTG 后端会自动处理这些错误并重新打开设备，无需人工干预。

### Q: 如何查看 LED 状态 (Num Lock, Caps Lock)?

```rust
let led_state = otg_backend.led_state();
println!("Caps Lock: {}", led_state.caps_lock);
println!("Num Lock: {}", led_state.num_lock);
```

LED 状态会在键盘设备的 OUT endpoint 接收到数据时自动更新。

---

## 12. 性能优化

### 12.1 零拷贝写入

OTG 后端使用 `write_all()` 直接写入设备文件，避免额外的内存拷贝。

### 12.2 非阻塞 I/O

所有设备文件以 `O_NONBLOCK` 模式打开，配合 `poll()` 实现超时控制。

### 12.3 事件批处理

前端可以批量发送多个事件，后端逐个处理。对于鼠标移动，超时的帧会被静默丢弃。

### 12.4 日志限流

使用 `LogThrottler` 防止大量重复日志影响性能：
- HID 错误日志: 5秒限流
- 恢复后冷却: 1秒内不记录新错误

---

## 13. 安全考虑

### 13.1 设备权限

- OTG gadget 设备文件 (`/dev/hidg*`) 需要读写权限
- 通常需要 `root` 权限或添加用户到 `input` 组
- 建议使用 udev 规则自动设置权限

### 13.2 输入验证

- 所有来自前端的事件都经过验证
- 无效的按键码会被忽略或映射到默认值
- 鼠标坐标会被限制在有效范围内

### 13.3 资源限制

- 键盘报告最多支持 6 个同时按键 (USB HID 标准)
- 鼠标移动范围限制在 -127~127 (相对) 或 0~32767 (绝对)
- 超时的 HID 写入会被丢弃，不会无限等待

---

## 14. 调试技巧

### 14.1 启用详细日志

```bash
RUST_LOG=one_kvm::hid=debug ./one-kvm
```

### 14.2 监控 HID 设备

```bash
# 监控键盘事件
sudo cat /dev/hidg0 | hexdump -C

# 监控鼠标事件
sudo cat /dev/hidg1 | hexdump -C
```

### 14.3 检查 USB 枚举

在目标 PC 上:

```bash
# Linux
dmesg | grep -i hid
lsusb

# Windows
# 打开设备管理器 -> 人体学输入设备
```

### 14.4 测试 CH9329

```bash
# 发送测试命令 (获取版本)
echo -ne '\x57\xAB\x0E\x00\x0E' > /dev/ttyUSB0
```

---

## 15. 参考资料

- [USB HID Usage Tables 1.12](https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf)
- [Linux USB Gadget API](https://www.kernel.org/doc/html/latest/usb/gadget_configfs.html)
- [PiKVM HID Implementation](https://github.com/pikvm/kvmd/blob/master/kvmd/apps/otg/hid/)
- [JetKVM HID Write Timeout](https://github.com/jetkvm/jetkvm/blob/main/jetkvm/hid.c#L25)
- [CH9329 Datasheet](http://www.wch.cn/downloads/CH9329DS1_PDF.html)
