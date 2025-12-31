# OTG 模块文档

## 1. 模块概述

OTG (On-The-Go) 模块负责管理 Linux USB Gadget，为 HID 和 MSD 功能提供统一的 USB 设备管理。

### 1.1 主要功能

- USB Gadget 生命周期管理
- HID 函数配置 (键盘、鼠标)
- MSD 函数配置 (虚拟存储)
- ConfigFS 操作
- UDC 绑定/解绑

### 1.2 文件结构

```
src/otg/
├── mod.rs              # 模块导出
├── service.rs          # OtgService (17KB)
├── manager.rs          # OtgGadgetManager (12KB)
├── hid.rs              # HID Function (7KB)
├── msd.rs              # MSD Function (14KB)
├── configfs.rs         # ConfigFS 操作 (4KB)
├── endpoint.rs         # 端点分配 (2KB)
└── report_desc.rs      # HID 报告描述符 (6KB)
```

---

## 2. 架构设计

### 2.1 设计目标

解决 HID 和 MSD 共享同一个 USB Gadget 的所有权问题：

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OTG Ownership Model                                  │
└─────────────────────────────────────────────────────────────────────────────┘

              ┌─────────────────┐
              │   OtgService    │ ◄── 唯一所有者
              │  (service.rs)   │
              └────────┬────────┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
         ▼             ▼             ▼
   enable_hid()   enable_msd()   状态查询
         │             │
         └──────┬──────┘
                │
                ▼
         ┌─────────────────┐
         │OtgGadgetManager │
         │  (manager.rs)   │
         └────────┬────────┘
                  │
    ┌─────────────┼─────────────┐
    │             │             │
    ▼             ▼             ▼
┌───────┐   ┌───────┐   ┌───────┐
│ HID   │   │ MSD   │   │ UDC   │
│ Func  │   │ Func  │   │ Bind  │
└───────┘   └───────┘   └───────┘
```

### 2.2 ConfigFS 结构

```
/sys/kernel/config/usb_gadget/one-kvm/
├── idVendor                    # 0x05ac (Apple)
├── idProduct                   # 0x0001
├── bcdDevice                   # 0x0100
├── bcdUSB                      # 0x0200
├── bMaxPacketSize0             # 64
│
├── strings/
│   └── 0x409/                  # English
│       ├── manufacturer        # "One-KVM"
│       ├── product             # "KVM Device"
│       └── serialnumber        # UUID
│
├── configs/
│   └── c.1/
│       ├── MaxPower            # 500
│       ├── strings/
│       │   └── 0x409/
│       │       └── configuration  # "Config 1"
│       └── (function symlinks)
│
├── functions/
│   ├── hid.usb0/               # 键盘
│   │   ├── protocol            # 1 (keyboard)
│   │   ├── subclass            # 1 (boot)
│   │   ├── report_length       # 8
│   │   └── report_desc         # (binary)
│   │
│   ├── hid.usb1/               # 相对鼠标
│   │   ├── protocol            # 2 (mouse)
│   │   ├── subclass            # 1 (boot)
│   │   ├── report_length       # 4
│   │   └── report_desc         # (binary)
│   │
│   ├── hid.usb2/               # 绝对鼠标
│   │   ├── protocol            # 2 (mouse)
│   │   ├── subclass            # 0 (none)
│   │   ├── report_length       # 6
│   │   └── report_desc         # (binary)
│   │
│   └── mass_storage.usb0/      # 虚拟存储
│       ├── stall               # 1
│       └── lun.0/
│           ├── cdrom           # 1 (ISO mode)
│           ├── ro              # 1 (read-only)
│           ├── removable       # 1
│           ├── nofua           # 1
│           └── file            # /path/to/image.iso
│
└── UDC                         # UDC 设备名
```

---

## 3. 核心组件

### 3.1 OtgService (service.rs)

OTG 服务主类，提供统一的 USB Gadget 管理接口。

```rust
pub struct OtgService {
    /// Gadget 管理器
    manager: Arc<Mutex<OtgGadgetManager>>,

    /// 当前状态
    state: Arc<RwLock<OtgServiceState>>,

    /// HID 函数句柄
    hid_function: Arc<RwLock<Option<HidFunction>>>,

    /// MSD 函数句柄
    msd_function: Arc<RwLock<Option<MsdFunction>>>,

    /// 请求计数器 (lock-free)
    pending_requests: AtomicU8,
}

impl OtgService {
    /// 创建服务
    pub fn new() -> Result<Self>;

    /// 启用 HID 功能
    pub async fn enable_hid(&self) -> Result<HidDevicePaths>;

    /// 禁用 HID 功能
    pub async fn disable_hid(&self) -> Result<()>;

    /// 启用 MSD 功能
    pub async fn enable_msd(&self) -> Result<MsdFunction>;

    /// 禁用 MSD 功能
    pub async fn disable_msd(&self) -> Result<()>;

    /// 获取状态
    pub fn state(&self) -> OtgServiceState;

    /// 检查 HID 是否启用
    pub fn is_hid_enabled(&self) -> bool;

    /// 检查 MSD 是否启用
    pub fn is_msd_enabled(&self) -> bool;
}

pub struct OtgServiceState {
    /// Gadget 是否激活
    pub gadget_active: bool,

    /// HID 是否启用
    pub hid_enabled: bool,

    /// MSD 是否启用
    pub msd_enabled: bool,

    /// HID 设备路径
    pub hid_paths: Option<HidDevicePaths>,

    /// 错误信息
    pub error: Option<String>,
}

pub struct HidDevicePaths {
    pub keyboard: PathBuf,        // /dev/hidg0
    pub mouse_relative: PathBuf,  // /dev/hidg1
    pub mouse_absolute: PathBuf,  // /dev/hidg2
}
```

### 3.2 OtgGadgetManager (manager.rs)

Gadget 生命周期管理器。

```rust
pub struct OtgGadgetManager {
    /// Gadget 路径
    gadget_path: PathBuf,

    /// UDC 设备名
    udc_name: Option<String>,

    /// 是否已创建
    created: bool,

    /// 是否已绑定
    bound: bool,

    /// 端点分配器
    endpoint_allocator: EndpointAllocator,
}

impl OtgGadgetManager {
    /// 创建管理器
    pub fn new() -> Result<Self>;

    /// 创建 Gadget
    pub fn create_gadget(&mut self, config: &GadgetConfig) -> Result<()>;

    /// 销毁 Gadget
    pub fn destroy_gadget(&mut self) -> Result<()>;

    /// 绑定 UDC
    pub fn bind_udc(&mut self) -> Result<()>;

    /// 解绑 UDC
    pub fn unbind_udc(&mut self) -> Result<()>;

    /// 添加函数
    pub fn add_function(&mut self, func: &dyn GadgetFunction) -> Result<()>;

    /// 移除函数
    pub fn remove_function(&mut self, func: &dyn GadgetFunction) -> Result<()>;

    /// 链接函数到配置
    pub fn link_function(&self, func: &dyn GadgetFunction) -> Result<()>;

    /// 取消链接函数
    pub fn unlink_function(&self, func: &dyn GadgetFunction) -> Result<()>;

    /// 检测可用 UDC
    fn detect_udc() -> Result<String>;
}

pub struct GadgetConfig {
    pub name: String,            // "one-kvm"
    pub vendor_id: u16,          // 0x05ac
    pub product_id: u16,         // 0x0001
    pub manufacturer: String,    // "One-KVM"
    pub product: String,         // "KVM Device"
    pub serial: String,          // UUID
}
```

### 3.3 HID Function (hid.rs)

```rust
pub struct HidFunction {
    /// 键盘函数
    keyboard: HidFunctionConfig,

    /// 相对鼠标函数
    mouse_relative: HidFunctionConfig,

    /// 绝对鼠标函数
    mouse_absolute: HidFunctionConfig,
}

pub struct HidFunctionConfig {
    /// 函数名
    pub name: String,            // "hid.usb0"

    /// 协议
    pub protocol: u8,            // 1=keyboard, 2=mouse

    /// 子类
    pub subclass: u8,            // 1=boot, 0=none

    /// 报告长度
    pub report_length: u8,

    /// 报告描述符
    pub report_desc: Vec<u8>,
}

impl HidFunction {
    /// 创建 HID 函数
    pub fn new() -> Self;

    /// 获取键盘报告描述符
    pub fn keyboard_report_desc() -> Vec<u8>;

    /// 获取相对鼠标报告描述符
    pub fn mouse_relative_report_desc() -> Vec<u8>;

    /// 获取绝对鼠标报告描述符
    pub fn mouse_absolute_report_desc() -> Vec<u8>;
}

impl GadgetFunction for HidFunction {
    fn name(&self) -> &str;
    fn function_type(&self) -> &str;  // "hid"
    fn configure(&self, path: &Path) -> Result<()>;
}
```

### 3.4 MSD Function (msd.rs)

```rust
pub struct MsdFunction {
    /// 函数名
    name: String,

    /// LUN 配置
    luns: Vec<MsdLun>,
}

pub struct MsdLun {
    /// LUN 编号
    pub lun_id: u8,

    /// 镜像文件路径
    pub file: Option<PathBuf>,

    /// 是否 CD-ROM 模式
    pub cdrom: bool,

    /// 是否只读
    pub readonly: bool,

    /// 是否可移除
    pub removable: bool,
}

impl MsdFunction {
    /// 创建 MSD 函数
    pub fn new() -> Self;

    /// 设置镜像文件
    pub fn set_image(&mut self, path: &Path, cdrom: bool) -> Result<()>;

    /// 清除镜像
    pub fn clear_image(&mut self) -> Result<()>;

    /// 弹出介质
    pub fn eject(&mut self) -> Result<()>;
}

impl GadgetFunction for MsdFunction {
    fn name(&self) -> &str;
    fn function_type(&self) -> &str;  // "mass_storage"
    fn configure(&self, path: &Path) -> Result<()>;
}
```

### 3.5 ConfigFS 操作 (configfs.rs)

```rust
pub struct ConfigFs;

impl ConfigFs {
    /// ConfigFS 根路径
    const ROOT: &'static str = "/sys/kernel/config/usb_gadget";

    /// 创建目录
    pub fn mkdir(path: &Path) -> Result<()>;

    /// 删除目录
    pub fn rmdir(path: &Path) -> Result<()>;

    /// 写入文件
    pub fn write_file(path: &Path, content: &str) -> Result<()>;

    /// 写入二进制文件
    pub fn write_binary(path: &Path, data: &[u8]) -> Result<()>;

    /// 读取文件
    pub fn read_file(path: &Path) -> Result<String>;

    /// 创建符号链接
    pub fn symlink(target: &Path, link: &Path) -> Result<()>;

    /// 删除符号链接
    pub fn unlink(path: &Path) -> Result<()>;

    /// 列出目录
    pub fn list_dir(path: &Path) -> Result<Vec<String>>;
}
```

### 3.6 端点分配 (endpoint.rs)

```rust
pub struct EndpointAllocator {
    /// 已使用的端点
    used_endpoints: HashSet<u8>,

    /// 最大端点数
    max_endpoints: u8,
}

impl EndpointAllocator {
    /// 创建分配器
    pub fn new(max_endpoints: u8) -> Self;

    /// 分配端点
    pub fn allocate(&mut self, count: u8) -> Result<Vec<u8>>;

    /// 释放端点
    pub fn release(&mut self, endpoints: &[u8]);

    /// 检查可用端点数
    pub fn available(&self) -> u8;
}
```

### 3.7 报告描述符 (report_desc.rs)

```rust
pub struct ReportDescriptor;

impl ReportDescriptor {
    /// 标准键盘报告描述符
    pub fn keyboard() -> Vec<u8> {
        vec![
            0x05, 0x01,        // Usage Page (Generic Desktop)
            0x09, 0x06,        // Usage (Keyboard)
            0xA1, 0x01,        // Collection (Application)
            0x05, 0x07,        //   Usage Page (Key Codes)
            0x19, 0xE0,        //   Usage Minimum (224)
            0x29, 0xE7,        //   Usage Maximum (231)
            0x15, 0x00,        //   Logical Minimum (0)
            0x25, 0x01,        //   Logical Maximum (1)
            0x75, 0x01,        //   Report Size (1)
            0x95, 0x08,        //   Report Count (8)
            0x81, 0x02,        //   Input (Data, Variable, Absolute)
            0x95, 0x01,        //   Report Count (1)
            0x75, 0x08,        //   Report Size (8)
            0x81, 0x01,        //   Input (Constant)
            0x95, 0x06,        //   Report Count (6)
            0x75, 0x08,        //   Report Size (8)
            0x15, 0x00,        //   Logical Minimum (0)
            0x25, 0x65,        //   Logical Maximum (101)
            0x05, 0x07,        //   Usage Page (Key Codes)
            0x19, 0x00,        //   Usage Minimum (0)
            0x29, 0x65,        //   Usage Maximum (101)
            0x81, 0x00,        //   Input (Data, Array)
            0xC0,              // End Collection
        ]
    }

    /// 相对鼠标报告描述符
    pub fn mouse_relative() -> Vec<u8>;

    /// 绝对鼠标报告描述符
    pub fn mouse_absolute() -> Vec<u8>;
}
```

---

## 4. 生命周期管理

### 4.1 初始化流程

```
OtgService::new()
    │
    ├── 检测 UDC 设备
    │   └── 读取 /sys/class/udc/
    │
    ├── 创建 OtgGadgetManager
    │
    └── 初始化状态

enable_hid()
    │
    ├── 检查 Gadget 是否存在
    │   └── 如不存在，创建 Gadget
    │
    ├── 创建 HID 函数
    │   ├── hid.usb0 (键盘)
    │   ├── hid.usb1 (相对鼠标)
    │   └── hid.usb2 (绝对鼠标)
    │
    ├── 配置函数
    │   └── 写入报告描述符
    │
    ├── 链接函数到配置
    │
    ├── 绑定 UDC (如未绑定)
    │
    └── 等待设备节点出现
        └── /dev/hidg0, hidg1, hidg2
```

### 4.2 清理流程

```
disable_hid()
    │
    ├── 检查是否有其他函数使用
    │
    ├── 如果只有 HID，解绑 UDC
    │
    ├── 取消链接 HID 函数
    │
    └── 删除 HID 函数目录

disable_msd()
    │
    ├── 同上...
    │
    └── 如果没有任何函数，销毁 Gadget
```

---

## 5. 配置

### 5.1 OTG 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct OtgConfig {
    /// 是否启用 OTG
    pub enabled: bool,

    /// 厂商 ID
    pub vendor_id: u16,

    /// 产品 ID
    pub product_id: u16,

    /// 厂商名称
    pub manufacturer: String,

    /// 产品名称
    pub product: String,
}

impl Default for OtgConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            vendor_id: 0x05ac,  // Apple
            product_id: 0x0001,
            manufacturer: "One-KVM".to_string(),
            product: "KVM Device".to_string(),
        }
    }
}
```

---

## 6. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum OtgError {
    #[error("No UDC device found")]
    NoUdcDevice,

    #[error("Gadget already exists")]
    GadgetExists,

    #[error("Gadget not found")]
    GadgetNotFound,

    #[error("Function already exists: {0}")]
    FunctionExists(String),

    #[error("UDC busy")]
    UdcBusy,

    #[error("ConfigFS error: {0}")]
    ConfigFsError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Device node not found: {0}")]
    DeviceNodeNotFound(String),
}
```

---

## 7. 使用示例

### 7.1 启用 HID

```rust
let otg = OtgService::new()?;

// 启用 HID
let paths = otg.enable_hid().await?;
println!("Keyboard: {:?}", paths.keyboard);
println!("Mouse relative: {:?}", paths.mouse_relative);
println!("Mouse absolute: {:?}", paths.mouse_absolute);

// 使用设备...

// 禁用 HID
otg.disable_hid().await?;
```

### 7.2 启用 MSD

```rust
let otg = OtgService::new()?;

// 启用 MSD
let mut msd = otg.enable_msd().await?;

// 挂载 ISO
msd.set_image(Path::new("/data/ubuntu.iso"), true)?;

// 弹出
msd.eject()?;

// 禁用 MSD
otg.disable_msd().await?;
```

---

## 8. 常见问题

### Q: 找不到 UDC 设备?

1. 检查内核是否支持 USB Gadget
2. 加载必要的内核模块:
   ```bash
   modprobe libcomposite
   modprobe usb_f_hid
   modprobe usb_f_mass_storage
   ```
3. 检查 `/sys/class/udc/` 目录

### Q: 权限错误?

1. 以 root 运行
2. 或配置 udev 规则

### Q: 设备节点不出现?

1. 检查 UDC 是否正确绑定
2. 查看 `dmesg` 日志
3. 检查 ConfigFS 配置

### Q: 目标 PC 不识别?

1. 检查 USB 线缆
2. 检查报告描述符
3. 使用 `lsusb` 确认设备
