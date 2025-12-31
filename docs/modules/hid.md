# HID 模块文档

## 1. 模块概述

HID (Human Interface Device) 模块负责将键盘和鼠标事件转发到目标计算机，是 One-KVM 实现远程控制的核心模块。

### 1.1 主要功能

- 键盘事件处理 (按键、修饰键)
- 鼠标事件处理 (移动、点击、滚轮)
- 支持绝对和相对鼠标模式
- 多后端支持 (OTG、CH9329)
- WebSocket 和 DataChannel 输入

### 1.2 文件结构

```
src/hid/
├── mod.rs              # HidController (16KB)
├── backend.rs          # 后端抽象
├── otg.rs              # OTG 后端 (33KB)
├── ch9329.rs           # CH9329 串口后端 (46KB)
├── keymap.rs           # 按键映射 (14KB)
├── types.rs            # 类型定义
├── monitor.rs          # 健康监视 (14KB)
├── datachannel.rs      # DataChannel 适配 (8KB)
└── websocket.rs        # WebSocket 适配 (6KB)
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
         │  - select_backend() │
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
    ├── 请求 HID 函数 (3个设备)
    │   ├── /dev/hidg0 (键盘)
    │   ├── /dev/hidg1 (相对鼠标)
    │   └── /dev/hidg2 (绝对鼠标)
    │
    └── 创建 OtgHidBackend

HidBackendType::Ch9329 { port, baud_rate }
    │
    ├── 打开串口设备
    │
    ├── 初始化 CH9329 芯片
    │
    └── 创建 Ch9329HidBackend

HidBackendType::None
    │
    └── 创建空后端 (丢弃所有事件)
```

---

## 3. 核心组件

### 3.1 HidController (mod.rs)

HID 控制器主类，统一管理所有 HID 操作。

```rust
pub struct HidController {
    /// 当前后端
    backend: Arc<RwLock<Box<dyn HidBackend>>>,

    /// 后端类型
    backend_type: Arc<RwLock<HidBackendType>>,

    /// OTG 服务引用
    otg_service: Arc<OtgService>,

    /// 健康监视器
    monitor: Arc<HidHealthMonitor>,

    /// 配置
    config: Arc<RwLock<HidConfig>>,

    /// 事件总线
    events: Arc<EventBus>,

    /// 鼠标模式
    mouse_mode: Arc<RwLock<MouseMode>>,
}

impl HidController {
    /// 初始化控制器
    pub async fn init(
        otg_service: Arc<OtgService>,
        config: &HidConfig,
        events: Arc<EventBus>,
    ) -> Result<Arc<Self>>;

    /// 发送键盘事件
    pub async fn send_keyboard(&self, event: &KeyboardEvent) -> Result<()>;

    /// 发送鼠标事件
    pub async fn send_mouse(&self, event: &MouseEvent) -> Result<()>;

    /// 设置鼠标模式
    pub fn set_mouse_mode(&self, mode: MouseMode);

    /// 获取鼠标模式
    pub fn get_mouse_mode(&self) -> MouseMode;

    /// 重新加载配置
    pub async fn reload(&self, config: &HidConfig) -> Result<()>;

    /// 重置 HID 状态
    pub async fn reset(&self) -> Result<()>;

    /// 获取状态信息
    pub fn info(&self) -> HidInfo;
}

pub struct HidInfo {
    pub backend: String,
    pub initialized: bool,
    pub keyboard_connected: bool,
    pub mouse_connected: bool,
    pub mouse_mode: MouseMode,
    pub error: Option<String>,
}
```

### 3.2 HidBackend Trait (backend.rs)

```rust
#[async_trait]
pub trait HidBackend: Send + Sync {
    /// 发送键盘事件
    async fn send_keyboard(&self, event: &KeyboardEvent) -> Result<()>;

    /// 发送鼠标事件
    async fn send_mouse(&self, event: &MouseEvent, mode: MouseMode) -> Result<()>;

    /// 重置状态
    async fn reset(&self) -> Result<()>;

    /// 获取后端信息
    fn info(&self) -> HidBackendInfo;

    /// 检查连接状态
    fn is_connected(&self) -> bool;
}

pub struct HidBackendInfo {
    pub name: String,
    pub backend_type: HidBackendType,
    pub keyboard_connected: bool,
    pub mouse_connected: bool,
}

#[derive(Clone, Debug)]
pub enum HidBackendType {
    /// USB OTG gadget 模式
    Otg,

    /// CH9329 串口 HID 控制器
    Ch9329 {
        port: String,
        baud_rate: u32,
    },

    /// 禁用 HID
    None,
}
```

### 3.3 OTG 后端 (otg.rs)

通过 Linux USB OTG gadget 模拟 HID 设备。

```rust
pub struct OtgHidBackend {
    /// HID 设备路径
    paths: HidDevicePaths,

    /// 键盘设备文件
    keyboard_fd: RwLock<Option<File>>,

    /// 相对鼠标设备文件
    mouse_rel_fd: RwLock<Option<File>>,

    /// 绝对鼠标设备文件
    mouse_abs_fd: RwLock<Option<File>>,

    /// 当前键盘状态
    keyboard_state: Mutex<KeyboardState>,

    /// OTG 服务引用
    otg_service: Arc<OtgService>,
}

impl OtgHidBackend {
    /// 创建 OTG 后端
    pub async fn new(otg_service: Arc<OtgService>) -> Result<Self>;

    /// 打开 HID 设备
    async fn open_devices(&self) -> Result<()>;

    /// 关闭 HID 设备
    async fn close_devices(&self);

    /// 写入键盘报告
    fn write_keyboard_report(&self, report: &KeyboardReport) -> Result<()>;

    /// 写入鼠标报告
    fn write_mouse_report(&self, report: &[u8], absolute: bool) -> Result<()>;
}

pub struct HidDevicePaths {
    pub keyboard: PathBuf,       // /dev/hidg0
    pub mouse_relative: PathBuf, // /dev/hidg1
    pub mouse_absolute: PathBuf, // /dev/hidg2
}
```

#### HID 报告格式

```rust
/// 键盘报告 (8 字节)
#[repr(C, packed)]
pub struct KeyboardReport {
    pub modifiers: u8,    // Ctrl, Shift, Alt, GUI
    pub reserved: u8,     // 保留
    pub keys: [u8; 6],    // 最多 6 个按键 scancode
}

/// 相对鼠标报告 (4 字节)
#[repr(C, packed)]
pub struct MouseRelativeReport {
    pub buttons: u8,      // 按钮状态
    pub x: i8,            // X 移动 (-127 ~ 127)
    pub y: i8,            // Y 移动 (-127 ~ 127)
    pub wheel: i8,        // 滚轮 (-127 ~ 127)
}

/// 绝对鼠标报告 (6 字节)
#[repr(C, packed)]
pub struct MouseAbsoluteReport {
    pub buttons: u8,      // 按钮状态
    pub x: u16,           // X 坐标 (0 ~ 32767)
    pub y: u16,           // Y 坐标 (0 ~ 32767)
    pub wheel: i8,        // 滚轮 (-127 ~ 127)
}
```

### 3.4 CH9329 后端 (ch9329.rs)

通过 CH9329 芯片（串口转 HID）实现 HID 功能。

```rust
pub struct Ch9329HidBackend {
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

impl Ch9329HidBackend {
    /// 创建 CH9329 后端
    pub fn new(device: &str, baud_rate: u32) -> Result<Self>;

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
pub struct KeyboardEvent {
    /// 按下的键列表
    pub keys: Vec<KeyCode>,

    /// 修饰键状态
    pub modifiers: KeyboardModifiers,
}

#[derive(Default)]
pub struct KeyboardModifiers {
    pub left_ctrl: bool,
    pub left_shift: bool,
    pub left_alt: bool,
    pub left_gui: bool,
    pub right_ctrl: bool,
    pub right_shift: bool,
    pub right_alt: bool,
    pub right_gui: bool,
}

impl KeyboardModifiers {
    /// 转换为 USB HID 修饰符字节
    pub fn to_byte(&self) -> u8 {
        let mut byte = 0u8;
        if self.left_ctrl { byte |= 0x01; }
        if self.left_shift { byte |= 0x02; }
        if self.left_alt { byte |= 0x04; }
        if self.left_gui { byte |= 0x08; }
        if self.right_ctrl { byte |= 0x10; }
        if self.right_shift { byte |= 0x20; }
        if self.right_alt { byte |= 0x40; }
        if self.right_gui { byte |= 0x80; }
        byte
    }
}
```

### 4.2 鼠标事件 (types.rs)

```rust
pub struct MouseEvent {
    /// 按钮
    pub button: Option<MouseButton>,

    /// 事件类型
    pub event_type: MouseEventType,

    /// 相对移动 X
    pub dx: i16,

    /// 相对移动 Y
    pub dy: i16,

    /// 绝对位置 X (0-32767)
    pub x: u32,

    /// 绝对位置 Y (0-32767)
    pub y: u32,

    /// 滚轮移动
    pub wheel: i8,
}

pub enum MouseButton {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

pub enum MouseEventType {
    Press,
    Release,
    Move,
    Wheel,
}

pub enum MouseMode {
    /// 相对模式 (用于普通操作)
    Relative,

    /// 绝对模式 (用于 BIOS/精确定位)
    Absolute,
}
```

---

## 5. 按键映射

### 5.1 KeyCode 枚举 (keymap.rs)

```rust
pub enum KeyCode {
    // 字母键
    KeyA, KeyB, KeyC, /* ... */ KeyZ,

    // 数字键
    Digit1, Digit2, /* ... */ Digit0,

    // 功能键
    F1, F2, /* ... */ F12,

    // 控制键
    Escape, Tab, CapsLock, Space, Enter, Backspace,
    Insert, Delete, Home, End, PageUp, PageDown,

    // 方向键
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,

    // 修饰键
    ShiftLeft, ShiftRight,
    ControlLeft, ControlRight,
    AltLeft, AltRight,
    MetaLeft, MetaRight,

    // 小键盘
    Numpad0, Numpad1, /* ... */ Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide,
    NumpadEnter, NumpadDecimal, NumLock,

    // 其他
    PrintScreen, ScrollLock, Pause,
    /* ... */
}

impl KeyCode {
    /// 转换为 USB HID scancode
    pub fn to_scancode(&self) -> u8;

    /// 从 JavaScript keyCode 转换
    pub fn from_js_code(code: &str) -> Option<Self>;

    /// 是否为修饰键
    pub fn is_modifier(&self) -> bool;
}
```

### 5.2 JavaScript 键码映射

```javascript
// 前端发送的格式
{
    "type": "keyboard",
    "keys": ["KeyA", "KeyB"],
    "modifiers": {
        "ctrl": false,
        "shift": true,
        "alt": false,
        "meta": false
    }
}
```

---

## 6. 输入处理器

### 6.1 WebSocket Handler (websocket.rs)

```rust
pub struct WsHidHandler {
    hid: Arc<HidController>,
}

impl WsHidHandler {
    pub fn new(hid: Arc<HidController>) -> Self;

    /// 处理 WebSocket 消息
    pub async fn handle_message(&self, msg: &str) -> Result<()> {
        let event: HidMessage = serde_json::from_str(msg)?;

        match event {
            HidMessage::Keyboard(kb) => {
                self.hid.send_keyboard(&kb).await?;
            }
            HidMessage::Mouse(mouse) => {
                self.hid.send_mouse(&mouse).await?;
            }
            HidMessage::SetMouseMode(mode) => {
                self.hid.set_mouse_mode(mode);
            }
        }

        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum HidMessage {
    #[serde(rename = "keyboard")]
    Keyboard(KeyboardEvent),

    #[serde(rename = "mouse")]
    Mouse(MouseEvent),

    #[serde(rename = "mouse_mode")]
    SetMouseMode(MouseMode),
}
```

### 6.2 DataChannel Handler (datachannel.rs)

用于 WebRTC 模式下的 HID 事件处理。

```rust
pub struct HidDataChannelHandler {
    hid: Arc<HidController>,
}

impl HidDataChannelHandler {
    pub fn new(hid: Arc<HidController>) -> Self;

    /// 处理 DataChannel 消息
    pub async fn handle_message(&self, data: &[u8]) -> Result<()>;

    /// 创建 DataChannel 配置
    pub fn datachannel_config() -> RTCDataChannelInit;
}
```

---

## 7. 健康监视

### 7.1 HidHealthMonitor (monitor.rs)

```rust
pub struct HidHealthMonitor {
    /// 错误计数
    error_count: AtomicU32,

    /// 连续错误计数
    consecutive_errors: AtomicU32,

    /// 最后错误时间
    last_error: RwLock<Option<Instant>>,

    /// 最后错误消息
    last_error_msg: RwLock<Option<String>>,

    /// 重试配置
    config: MonitorConfig,
}

impl HidHealthMonitor {
    /// 记录错误
    pub fn record_error(&self, error: &str);

    /// 记录成功
    pub fn record_success(&self);

    /// 是否应该重试
    pub fn should_retry(&self) -> bool;

    /// 是否需要重新初始化
    pub fn needs_reinit(&self) -> bool;

    /// 获取健康状态
    pub fn health_status(&self) -> HealthStatus;
}

pub enum HealthStatus {
    Healthy,
    Degraded { error_rate: f32 },
    Unhealthy { consecutive_errors: u32 },
}
```

---

## 8. 配置

### 8.1 HID 配置结构

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct HidConfig {
    /// 后端类型
    pub backend: HidBackendType,

    /// CH9329 设备路径 (如果使用 CH9329)
    pub ch9329_device: Option<String>,

    /// CH9329 波特率
    pub ch9329_baud_rate: Option<u32>,

    /// 默认鼠标模式
    pub default_mouse_mode: MouseMode,

    /// 鼠标灵敏度 (1-10)
    pub mouse_sensitivity: u8,

    /// 启用滚轮
    pub enable_wheel: bool,
}

impl Default for HidConfig {
    fn default() -> Self {
        Self {
            backend: HidBackendType::Otg,
            ch9329_device: None,
            ch9329_baud_rate: Some(9600),
            default_mouse_mode: MouseMode::Absolute,
            mouse_sensitivity: 5,
            enable_wheel: true,
        }
    }
}
```

---

## 9. API 端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/hid/status` | GET | 获取 HID 状态 |
| `/api/hid/reset` | POST | 重置 HID 状态 |
| `/api/hid/keyboard` | POST | 发送键盘事件 |
| `/api/hid/mouse` | POST | 发送鼠标事件 |
| `/api/hid/mouse/mode` | GET | 获取鼠标模式 |
| `/api/hid/mouse/mode` | POST | 设置鼠标模式 |

### 响应格式

```json
// GET /api/hid/status
{
    "backend": "otg",
    "initialized": true,
    "keyboard_connected": true,
    "mouse_connected": true,
    "mouse_mode": "absolute",
    "error": null
}
```

---

## 10. 事件

```rust
pub enum SystemEvent {
    HidStateChanged {
        backend: String,
        initialized: bool,
        keyboard_connected: bool,
        mouse_connected: bool,
        mouse_mode: String,
        error: Option<String>,
    },
}
```

---

## 11. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum HidError {
    #[error("Backend not initialized")]
    NotInitialized,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device busy: {0}")]
    DeviceBusy(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Serial port error: {0}")]
    SerialError(String),

    #[error("Invalid key code: {0}")]
    InvalidKeyCode(String),

    #[error("OTG service error: {0}")]
    OtgError(String),
}
```

---

## 12. 使用示例

### 12.1 初始化 HID 控制器

```rust
let otg_service = Arc::new(OtgService::new()?);
let events = Arc::new(EventBus::new());

let hid = HidController::init(
    otg_service,
    &HidConfig::default(),
    events,
).await?;
```

### 12.2 发送键盘事件

```rust
// 按下 Ctrl+C
hid.send_keyboard(&KeyboardEvent {
    keys: vec![KeyCode::KeyC],
    modifiers: KeyboardModifiers {
        left_ctrl: true,
        ..Default::default()
    },
}).await?;

// 释放所有键
hid.send_keyboard(&KeyboardEvent {
    keys: vec![],
    modifiers: KeyboardModifiers::default(),
}).await?;
```

### 12.3 发送鼠标事件

```rust
// 移动鼠标到绝对位置
hid.send_mouse(&MouseEvent {
    button: None,
    event_type: MouseEventType::Move,
    dx: 0,
    dy: 0,
    x: 16384,  // 屏幕中心
    y: 16384,
    wheel: 0,
}).await?;

// 点击左键
hid.send_mouse(&MouseEvent {
    button: Some(MouseButton::Left),
    event_type: MouseEventType::Press,
    ..Default::default()
}).await?;

hid.send_mouse(&MouseEvent {
    button: Some(MouseButton::Left),
    event_type: MouseEventType::Release,
    ..Default::default()
}).await?;
```

---

## 13. 常见问题

### Q: OTG 模式下键盘/鼠标不工作?

1. 检查 `/dev/hidg*` 设备是否存在
2. 检查 USB gadget 是否正确配置
3. 检查目标 PC 是否识别 USB 设备
4. 查看 `dmesg` 日志

### Q: CH9329 无法初始化?

1. 检查串口设备路径
2. 检查波特率设置
3. 使用 `minicom` 测试串口连接

### Q: 鼠标定位不准确?

1. 使用绝对鼠标模式
2. 校准屏幕分辨率
3. 检查缩放设置

### Q: 按键有延迟?

1. 检查网络延迟
2. 使用 WebRTC 模式
3. 减少中间代理
