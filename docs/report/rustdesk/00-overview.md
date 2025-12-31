# RustDesk 通信协议技术报告

## 概述

本报告详细分析 RustDesk 远程桌面软件的客户端与服务器之间的通信协议，包括 Rendezvous 服务器（hbbs）、Relay 服务器（hbbr）以及客户端之间的 P2P 连接机制。

## 文档结构

| 文档 | 内容 |
|------|------|
| [01-architecture.md](01-architecture.md) | 整体架构设计 |
| [02-rendezvous-protocol.md](02-rendezvous-protocol.md) | Rendezvous 服务器协议 |
| [03-relay-protocol.md](03-relay-protocol.md) | Relay 服务器协议 |
| [04-p2p-connection.md](04-p2p-connection.md) | P2P 连接流程 |
| [05-message-format.md](05-message-format.md) | 消息格式定义 |
| [06-encryption.md](06-encryption.md) | 加密机制 |
| [07-nat-traversal.md](07-nat-traversal.md) | NAT 穿透技术 |
| [08-onekvm-comparison.md](08-onekvm-comparison.md) | **One-KVM 实现对比分析** |

## 核心组件

### 1. Rendezvous Server (hbbs)
- **功能**: ID 注册、Peer 发现、NAT 类型检测、连接协调
- **端口**: 21116 (TCP/UDP), 21115 (NAT 测试), 21118 (WebSocket)
- **源文件**: `rustdesk-server/src/rendezvous_server.rs`

### 2. Relay Server (hbbr)
- **功能**: 当 P2P 连接失败时提供数据中转
- **端口**: 21117 (TCP), 21119 (WebSocket)
- **源文件**: `rustdesk-server/src/relay_server.rs`

### 3. 客户端 (RustDesk)
- **功能**: 远程桌面控制、文件传输、屏幕共享
- **核心模块**:
  - `rendezvous_mediator.rs` - 与 Rendezvous 服务器通信
  - `client.rs` - 客户端连接逻辑
  - `server/connection.rs` - 被控端连接处理

## 协议栈

```
┌─────────────────────────────────────────┐
│            Application Layer            │
│   (Video/Audio/Keyboard/Mouse/File)     │
├─────────────────────────────────────────┤
│            Message Layer                │
│         (Protobuf Messages)             │
├─────────────────────────────────────────┤
│           Security Layer                │
│    (Sodium: X25519 + ChaCha20)          │
├─────────────────────────────────────────┤
│           Transport Layer               │
│   (TCP/UDP/WebSocket/KCP)               │
└─────────────────────────────────────────┘
```

## 关键技术特点

1. **混合连接模式**: 优先尝试 P2P 直连，失败后自动切换到 Relay 中转
2. **多协议支持**: TCP、UDP、WebSocket、KCP
3. **端到端加密**: 使用 libsodium 实现的 X25519 密钥交换和 ChaCha20-Poly1305 对称加密
4. **NAT 穿透**: 支持 UDP 打洞和 TCP 打洞技术
5. **服务器签名**: 可选的服务器公钥签名验证，防止中间人攻击

## 版本信息

- 分析基于 RustDesk 最新版本源码
- Protocol Buffer 版本: proto3
- 加密库: libsodium (sodiumoxide)
