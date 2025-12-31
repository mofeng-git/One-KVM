# RustDesk 架构设计

## 系统架构图

```
                                    ┌──────────────────────┐
                                    │   Rendezvous Server  │
                                    │       (hbbs)         │
                                    │  Port: 21116         │
                                    └──────────┬───────────┘
                                               │
                    ┌──────────────────────────┼──────────────────────────┐
                    │                          │                          │
                    ▼                          ▼                          ▼
            ┌───────────────┐          ┌───────────────┐          ┌───────────────┐
            │   Client A    │          │   Client B    │          │   Client C    │
            │   (控制端)     │          │   (被控端)     │          │   (被控端)     │
            └───────┬───────┘          └───────┬───────┘          └───────────────┘
                    │                          │
                    │    P2P Connection        │
                    │◄────────────────────────►│
                    │                          │
                    │   (如果 P2P 失败)         │
                    │          │               │
                    │          ▼               │
                    │  ┌───────────────┐       │
                    └─►│  Relay Server │◄──────┘
                       │    (hbbr)     │
                       │  Port: 21117  │
                       └───────────────┘
```

## 服务器组件详解

### Rendezvous Server (hbbs)

**监听端口：**
| 端口 | 协议 | 用途 |
|------|------|------|
| 21116 | TCP | 主要通信端口，处理 punch hole 请求 |
| 21116 | UDP | Peer 注册、NAT 类型检测 |
| 21115 | TCP | NAT 测试专用端口 |
| 21118 | WebSocket | Web 客户端支持 |

**核心数据结构：**

```rust
// rustdesk-server/src/rendezvous_server.rs:64-83
pub struct RendezvousServer {
    tcp_punch: Arc<Mutex<HashMap<SocketAddr, Sink>>>,  // TCP punch hole 连接
    pm: PeerMap,                                        // Peer 映射表
    tx: Sender,                                         // 消息发送通道
    relay_servers: Arc<RelayServers>,                  // 可用 Relay 服务器列表
    relay_servers0: Arc<RelayServers>,                 // 原始 Relay 服务器列表
    rendezvous_servers: Arc<Vec<String>>,              // Rendezvous 服务器列表
    inner: Arc<Inner>,                                  // 内部配置
}

struct Inner {
    serial: i32,                      // 配置序列号
    version: String,                  // 软件版本
    software_url: String,             // 软件更新 URL
    mask: Option<Ipv4Network>,        // LAN 掩码
    local_ip: String,                 // 本地 IP
    sk: Option<sign::SecretKey>,      // 服务器签名密钥
}
```

**Peer 数据结构：**

```rust
// rustdesk-server/src/peer.rs:32-42
pub struct Peer {
    pub socket_addr: SocketAddr,   // 最后注册的地址
    pub last_reg_time: Instant,    // 最后注册时间
    pub guid: Vec<u8>,             // 数据库 GUID
    pub uuid: Bytes,               // 设备 UUID
    pub pk: Bytes,                 // 公钥
    pub info: PeerInfo,            // Peer 信息
    pub reg_pk: (u32, Instant),    // 注册频率限制
}
```

### Relay Server (hbbr)

**监听端口：**
| 端口 | 协议 | 用途 |
|------|------|------|
| 21117 | TCP | 主要中转端口 |
| 21119 | WebSocket | Web 客户端支持 |

**核心特性：**

```rust
// rustdesk-server/src/relay_server.rs:40-44
static DOWNGRADE_THRESHOLD_100: AtomicUsize = AtomicUsize::new(66);        // 降级阈值
static DOWNGRADE_START_CHECK: AtomicUsize = AtomicUsize::new(1_800_000);   // 检测开始时间(ms)
static LIMIT_SPEED: AtomicUsize = AtomicUsize::new(32 * 1024 * 1024);      // 限速(bit/s)
static TOTAL_BANDWIDTH: AtomicUsize = AtomicUsize::new(1024 * 1024 * 1024);// 总带宽
static SINGLE_BANDWIDTH: AtomicUsize = AtomicUsize::new(128 * 1024 * 1024);// 单连接带宽
```

## 客户端架构

### 核心模块

```
rustdesk/src/
├── rendezvous_mediator.rs  # Rendezvous 服务器通信
├── client.rs               # 控制端核心逻辑
├── server/
│   ├── mod.rs              # 被控端服务
│   ├── connection.rs       # 连接处理
│   ├── video_service.rs    # 视频服务
│   ├── audio_service.rs    # 音频服务
│   └── input_service.rs    # 输入服务
├── common.rs               # 通用函数（加密、解密）
└── platform/               # 平台特定代码
```

### RendezvousMediator

```rust
// rustdesk/src/rendezvous_mediator.rs:44-50
pub struct RendezvousMediator {
    addr: TargetAddr<'static>,   // 服务器地址
    host: String,                // 服务器主机名
    host_prefix: String,         // 主机前缀
    keep_alive: i32,             // 保活间隔
}
```

**两种连接模式：**

1. **UDP 模式** (默认):
   - 用于 Peer 注册和心跳
   - 更低延迟
   - 可能被某些防火墙阻止

2. **TCP 模式**:
   - 用于代理环境
   - WebSocket 模式
   - 更可靠

## 连接流程概述

### 被控端启动流程

```
1. 生成设备 ID 和密钥对
2. 连接 Rendezvous Server
3. 发送 RegisterPeer 消息
4. 如果需要，发送 RegisterPk 注册公钥
5. 定期发送心跳保持在线状态
6. 等待 PunchHole 或 RequestRelay 请求
```

### 控制端连接流程

```
1. 输入目标设备 ID
2. 连接 Rendezvous Server
3. 发送 PunchHoleRequest 消息
4. 根据响应决定连接方式:
   a. 直连 (P2P): 使用 PunchHole 信息尝试打洞
   b. 局域网: 使用 LocalAddr 信息直连
   c. 中转: 通过 Relay Server 连接
5. 建立安全加密通道
6. 发送 LoginRequest 进行身份验证
7. 开始远程控制会话
```

## 数据流

### 视频流

```
被控端                              控制端
  │                                  │
  │  VideoFrame (H264/VP9/...)       │
  ├─────────────────────────────────►│
  │                                  │
  │  加密 → 传输 → 解密 → 解码 → 显示 │
```

### 输入流

```
控制端                              被控端
  │                                  │
  │  MouseEvent/KeyEvent             │
  ├─────────────────────────────────►│
  │                                  │
  │  加密 → 传输 → 解密 → 模拟输入    │
```

## 高可用设计

### 多服务器支持

- 客户端可配置多个 Rendezvous Server
- 自动选择延迟最低的服务器
- 连接失败时自动切换备用服务器

### Relay Server 选择

- 支持配置多个 Relay Server
- 轮询算法分配负载
- 定期检查 Relay Server 可用性

### 重连机制

```rust
// 连接超时和重试参数
const REG_INTERVAL: i64 = 12_000;      // 注册间隔 12 秒
const REG_TIMEOUT: i32 = 30_000;       // 注册超时 30 秒
const CONNECT_TIMEOUT: u64 = 18_000;   // 连接超时 18 秒
```
