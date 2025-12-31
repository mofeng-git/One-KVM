# RustDesk 协议 vs One-KVM 实现对比分析

本文档对比分析 RustDesk 原始协议与 One-KVM 的实现差异。

## 1. 概述

One-KVM 作为 IP-KVM 解决方案，只实现了 RustDesk 协议的**被控端（Controlled）** 功能，不实现控制端（Controller）功能。这是设计决策，因为 KVM 设备只需要接收远程控制，不需要控制其他设备。

### 架构差异

| 方面 | RustDesk 原版 | One-KVM |
|------|---------------|---------|
| 角色 | 双向（控制端+被控端） | 单向（仅被控端） |
| 连接方式 | P2P + Relay | 仅 Relay (TCP) |
| NAT 穿透 | UDP/TCP 打洞 + TURN | 不支持 |
| 传输协议 | UDP/TCP | 仅 TCP |

## 2. 已实现功能

### 2.1 Rendezvous 协议 (hbbs 通信)

| 消息类型 | 实现状态 | 备注 |
|----------|----------|------|
| RegisterPeer | ✅ 已实现 | 注册设备到服务器 |
| RegisterPeerResponse | ✅ 已实现 | 处理注册响应 |
| RegisterPk | ✅ 已实现 | 注册公钥 |
| RegisterPkResponse | ✅ 已实现 | 处理公钥注册响应 |
| PunchHoleSent | ✅ 已实现 | 响应打洞请求 |
| FetchLocalAddr | ✅ 已实现 | 获取本地地址 |
| LocalAddr | ✅ 已实现 | 返回本地地址 |
| RequestRelay | ✅ 已实现 | 请求中继连接 |
| RelayResponse | ✅ 已实现 | 处理中继响应 |
| ConfigUpdate | ✅ 已实现 | 接收配置更新 |

**实现文件**: `src/rustdesk/rendezvous.rs` (~829 行)

```rust
// 核心结构
pub struct RendezvousMediator {
    config: RustDeskConfig,
    key_pair: KeyPair,
    signing_key: SigningKeyPair,
    socket: UdpSocket,
    status: Arc<RwLock<RendezvousStatus>>,
    // ...
}
```

### 2.2 连接协议 (客户端连接)

| 消息类型 | 实现状态 | 备注 |
|----------|----------|------|
| SignedId | ✅ 已实现 | 签名身份验证 |
| PublicKey | ✅ 已实现 | 公钥交换 |
| Hash | ✅ 已实现 | 哈希挑战响应 |
| LoginRequest | ✅ 已实现 | 登录认证 |
| LoginResponse | ✅ 已实现 | 登录响应 |
| TestDelay | ✅ 已实现 | 延迟测试 |
| VideoFrame | ✅ 已实现 | 视频帧发送 |
| AudioFrame | ✅ 已实现 | 音频帧发送 |
| CursorData | ✅ 已实现 | 光标图像 |
| CursorPosition | ✅ 已实现 | 光标位置 |
| MouseEvent | ✅ 已实现 | 鼠标事件接收 |
| KeyEvent | ✅ 已实现 | 键盘事件接收 |

**实现文件**: `src/rustdesk/connection.rs` (~1349 行)

```rust
// 连接状态机
pub enum ConnectionState {
    WaitingForSignedId,
    WaitingForPublicKey,
    WaitingForHash,
    WaitingForLogin,
    Authenticated,
    Streaming,
}
```

### 2.3 加密模块

| 功能 | 实现状态 | 备注 |
|------|----------|------|
| Curve25519 密钥对 | ✅ 已实现 | 用于加密 |
| Ed25519 签名密钥对 | ✅ 已实现 | 用于签名 |
| Ed25519 → Curve25519 转换 | ✅ 已实现 | 密钥派生 |
| XSalsa20-Poly1305 | ✅ 已实现 | 会话加密 (secretbox) |
| 密码哈希 | ✅ 已实现 | 单重/双重 SHA256 |
| 会话密钥协商 | ✅ 已实现 | 对称密钥派生 |

**实现文件**: `src/rustdesk/crypto.rs` (~468 行)

```rust
// 密钥对结构
pub struct KeyPair {
    secret_key: [u8; 32],  // Curve25519 私钥
    public_key: [u8; 32],  // Curve25519 公钥
}

pub struct SigningKeyPair {
    secret_key: [u8; 64],  // Ed25519 私钥
    public_key: [u8; 32],  // Ed25519 公钥
}
```

### 2.4 视频/音频流

| 编码格式 | 实现状态 | 备注 |
|----------|----------|------|
| H.264 | ✅ 已实现 | 主要格式 |
| H.265/HEVC | ✅ 已实现 | 高效编码 |
| VP8 | ✅ 已实现 | WebRTC 兼容 |
| VP9 | ✅ 已实现 | 高质量 |
| AV1 | ✅ 已实现 | 新一代编码 |
| Opus 音频 | ✅ 已实现 | 低延迟音频 |

**实现文件**: `src/rustdesk/frame_adapters.rs` (~316 行)

### 2.5 HID 事件

| 功能 | 实现状态 | 备注 |
|------|----------|------|
| 鼠标移动 | ✅ 已实现 | 绝对/相对坐标 |
| 鼠标按键 | ✅ 已实现 | 左/中/右键 |
| 鼠标滚轮 | ✅ 已实现 | 垂直滚动 |
| 键盘按键 | ✅ 已实现 | 按下/释放 |
| 控制键映射 | ✅ 已实现 | ControlKey → USB HID |
| X11 键码映射 | ✅ 已实现 | X11 → USB HID |

**实现文件**: `src/rustdesk/hid_adapter.rs` (~386 行)

### 2.6 协议帧编码

| 功能 | 实现状态 | 备注 |
|------|----------|------|
| BytesCodec | ✅ 已实现 | 变长帧编码 |
| 1-4 字节头 | ✅ 已实现 | 根据长度自动选择 |
| 最大 1GB 消息 | ✅ 已实现 | 与原版一致 |

**实现文件**: `src/rustdesk/bytes_codec.rs` (~253 行)

## 3. 未实现功能

### 3.1 NAT 穿透相关

| 功能 | 原因 |
|------|------|
| UDP 打洞 | One-KVM 仅使用 TCP 中继 |
| TCP 打洞 | 同上 |
| STUN/TURN | 不需要 NAT 类型检测 |
| TestNat | 同上 |
| P2P 直连 | 设计简化，仅支持中继 |

### 3.2 客户端发起功能

| 功能 | 原因 |
|------|------|
| PunchHole (发起) | KVM 只接收连接 |
| RelayRequest | 同上 |
| ConnectPeer | 同上 |
| OnlineRequest | 不需要查询其他设备 |

### 3.3 文件传输

| 功能 | 原因 |
|------|------|
| FileTransfer | 超出 KVM 功能范围 |
| FileAction | 同上 |
| FileResponse | 同上 |
| FileTransferBlock | 同上 |

### 3.4 高级功能

| 功能 | 原因 |
|------|------|
| 剪贴板同步 | 超出 KVM 功能范围 |
| 多显示器切换 | One-KVM 使用单一视频源 |
| 虚拟显示器 | 不适用 |
| 端口转发 | 超出 KVM 功能范围 |
| 语音通话 | 不需要 |
| RDP 输入 | 不需要 |
| 插件系统 | 不支持 |
| 软件更新 | One-KVM 有自己的更新机制 |

### 3.5 权限协商

| 功能 | 原因 |
|------|------|
| Option 消息 | One-KVM 假设完全控制权限 |
| 权限请求 | 同上 |
| PermissionInfo | 同上 |

## 4. 实现差异

### 4.1 连接模式

**RustDesk 原版:**
```
客户端 ──UDP打洞──> 被控端 (P2P 优先)
       └──Relay──> 被控端 (回退)
```

**One-KVM:**
```
RustDesk客户端 ──TCP中继──> hbbr服务器 ──> One-KVM设备
```

One-KVM 只支持 TCP 中继连接，不支持 P2P 直连。这简化了实现，但可能增加延迟。

### 4.2 会话加密

**RustDesk 原版:**
- 支持 ChaCha20-Poly1305 (流式)
- 支持 XSalsa20-Poly1305 (secretbox)
- 动态协商加密方式

**One-KVM:**
- 仅支持 XSalsa20-Poly1305 (secretbox)
- 使用序列号作为 nonce

```rust
// One-KVM 的加密实现
fn encrypt_message(&mut self, plaintext: &[u8]) -> Vec<u8> {
    let nonce = make_nonce(&self.send_nonce);
    self.send_nonce = self.send_nonce.wrapping_add(1);
    secretbox::seal(plaintext, &nonce, &self.session_key)
}
```

### 4.3 视频流方向

**RustDesk 原版:**
- 双向视频流（可控制和被控制）
- 远程桌面捕获

**One-KVM:**
- 单向视频流（仅发送）
- 从 V4L2 设备捕获
- 集成到 One-KVM 的 VideoStreamManager

```rust
// One-KVM 视频流集成
pub async fn start_video_stream(&self, state: &AppState) {
    let stream_manager = &state.video_stream_manager;
    // 从 One-KVM 的视频管理器获取帧
}
```

### 4.4 HID 事件处理

**RustDesk 原版:**
- 转发到远程系统的输入子系统
- 使用 enigo 或 uinput

**One-KVM:**
- 转发到 USB OTG/HID 设备
- 控制物理 KVM 目标机器

```rust
// One-KVM HID 适配
pub fn convert_mouse_event(event: &RustDeskMouseEvent) -> Option<OneKvmMouseEvent> {
    // 转换 RustDesk 鼠标事件到 One-KVM HID 事件
}

pub fn convert_key_event(event: &RustDeskKeyEvent) -> Option<OneKvmKeyEvent> {
    // 转换 RustDesk 键盘事件到 One-KVM HID 事件
}
```

### 4.5 配置管理

**RustDesk 原版:**
- 使用 TOML/JSON 配置文件
- 硬编码默认值

**One-KVM:**
- 集成到 SQLite 配置系统
- Web UI 管理
- 使用 typeshare 生成 TypeScript 类型

```rust
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDeskConfig {
    pub enabled: bool,
    pub rendezvous_server: String,
    pub device_id: String,
    // ...
}
```

### 4.6 设备 ID 生成

**RustDesk 原版:**
- 基于 MAC 地址和硬件信息
- 固定便携式 ID

**One-KVM:**
- 随机生成 9 位数字
- 存储在配置中

```rust
pub fn generate_device_id() -> String {
    let mut rng = rand::thread_rng();
    let id: u32 = rng.gen_range(100_000_000..999_999_999);
    id.to_string()
}
```

## 5. 协议兼容性

### 5.1 完全兼容

| 功能 | 说明 |
|------|------|
| Rendezvous 注册 | 可与官方 hbbs 服务器通信 |
| 中继连接 | 可通过官方 hbbr 服务器中继 |
| 加密握手 | 与 RustDesk 客户端兼容 |
| 视频编码 | 支持所有主流编码格式 |
| HID 事件 | 接收标准 RustDesk 输入事件 |

### 5.2 部分兼容

| 功能 | 说明 |
|------|------|
| 密码认证 | 仅支持设备密码，不支持一次性密码 |
| 会话加密 | 仅 XSalsa20-Poly1305 |

### 5.3 不兼容

| 功能 | 说明 |
|------|------|
| P2P 连接 | 客户端必须通过中继连接 |
| 文件传输 | 不支持 |
| 剪贴板 | 不支持 |

## 6. 代码结构对比

### RustDesk 原版结构

```
rustdesk/
├── libs/hbb_common/     # 公共库
│   ├── protos/          # Protobuf 定义
│   └── src/
├── src/
│   ├── server/          # 被控端服务
│   ├── client/          # 控制端
│   ├── ui/              # 用户界面
│   └── rendezvous_mediator.rs
```

### One-KVM 结构

```
src/rustdesk/
├── mod.rs               # 模块导出
├── config.rs            # 配置类型 (~164 行)
├── crypto.rs            # 加密模块 (~468 行)
├── bytes_codec.rs       # 帧编码 (~253 行)
├── protocol.rs          # 消息辅助 (~170 行)
├── rendezvous.rs        # Rendezvous 中介 (~829 行)
├── connection.rs        # 连接处理 (~1349 行)
├── hid_adapter.rs       # HID 转换 (~386 行)
└── frame_adapters.rs    # 视频/音频适配 (~316 行)
```

**总计**: ~3935 行代码

## 7. 总结

### 实现率统计

| 类别 | RustDesk 功能数 | One-KVM 实现数 | 实现率 |
|------|-----------------|----------------|--------|
| Rendezvous 协议 | 15+ | 10 | ~67% |
| 连接协议 | 30+ | 12 | ~40% |
| 加密功能 | 8 | 6 | 75% |
| 视频/音频 | 6 | 6 | 100% |
| HID 功能 | 6 | 6 | 100% |

### 设计理念

One-KVM 的 RustDesk 实现专注于 **IP-KVM 核心功能**:

1. **精简**: 只实现必要的被控端功能
2. **可靠**: 使用 TCP 中继保证连接稳定性
3. **集成**: 与 One-KVM 现有视频/HID 系统无缝集成
4. **安全**: 完整实现加密和认证机制

### 客户端兼容性

One-KVM 可与标准 RustDesk 客户端配合使用:
- RustDesk 桌面客户端 (Windows/macOS/Linux)
- RustDesk 移动客户端 (Android/iOS)
- RustDesk Web 客户端

只需确保:
1. 配置相同的 Rendezvous 服务器
2. 使用设备 ID 和密码连接
3. 客户端支持中继连接
