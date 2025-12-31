# Rendezvous 服务器协议

## 概述

Rendezvous Server（hbbs）是 RustDesk 的核心协调服务器，负责：
- Peer ID 注册和发现
- 公钥存储和分发
- NAT 类型检测
- P2P 连接协调（打洞辅助）
- Relay Server 分配

## 协议消息定义

所有消息使用 Protocol Buffers 定义在 `protos/rendezvous.proto`：

```protobuf
message RendezvousMessage {
  oneof union {
    RegisterPeer register_peer = 6;
    RegisterPeerResponse register_peer_response = 7;
    PunchHoleRequest punch_hole_request = 8;
    PunchHole punch_hole = 9;
    PunchHoleSent punch_hole_sent = 10;
    PunchHoleResponse punch_hole_response = 11;
    FetchLocalAddr fetch_local_addr = 12;
    LocalAddr local_addr = 13;
    ConfigUpdate configure_update = 14;
    RegisterPk register_pk = 15;
    RegisterPkResponse register_pk_response = 16;
    SoftwareUpdate software_update = 17;
    RequestRelay request_relay = 18;
    RelayResponse relay_response = 19;
    TestNatRequest test_nat_request = 20;
    TestNatResponse test_nat_response = 21;
    PeerDiscovery peer_discovery = 22;
    OnlineRequest online_request = 23;
    OnlineResponse online_response = 24;
    KeyExchange key_exchange = 25;
    HealthCheck hc = 26;
  }
}
```

## 核心流程

### 1. Peer 注册流程

**客户端 → 服务器：RegisterPeer**

```protobuf
message RegisterPeer {
  string id = 1;      // Peer ID (如 "123456789")
  int32 serial = 2;   // 配置序列号
}
```

**服务器处理逻辑：**

```rust
// rustdesk-server/src/rendezvous_server.rs:318-333
Some(rendezvous_message::Union::RegisterPeer(rp)) => {
    if !rp.id.is_empty() {
        log::trace!("New peer registered: {:?} {:?}", &rp.id, &addr);
        self.update_addr(rp.id, addr, socket).await?;
        // 如果服务器配置更新，发送 ConfigUpdate
        if self.inner.serial > rp.serial {
            let mut msg_out = RendezvousMessage::new();
            msg_out.set_configure_update(ConfigUpdate {
                serial: self.inner.serial,
                rendezvous_servers: (*self.rendezvous_servers).clone(),
                ..Default::default()
            });
            socket.send(&msg_out, addr).await?;
        }
    }
}
```

**服务器 → 客户端：RegisterPeerResponse**

```protobuf
message RegisterPeerResponse {
  bool request_pk = 2;  // 是否需要注册公钥
}
```

### 2. 公钥注册流程

当服务器检测到 Peer 的公钥为空或 IP 变化时，会请求注册公钥。

**客户端 → 服务器：RegisterPk**

```protobuf
message RegisterPk {
  string id = 1;       // Peer ID
  bytes uuid = 2;      // 设备 UUID
  bytes pk = 3;        // Ed25519 公钥
  string old_id = 4;   // 旧 ID（如果更换）
}
```

**服务器处理逻辑：**

```rust
// rustdesk-server/src/rendezvous_server.rs:334-418
Some(rendezvous_message::Union::RegisterPk(rk)) => {
    // 验证 UUID 和公钥
    if rk.uuid.is_empty() || rk.pk.is_empty() {
        return Ok(());
    }
    let id = rk.id;
    let ip = addr.ip().to_string();

    // ID 长度检查
    if id.len() < 6 {
        return send_rk_res(socket, addr, UUID_MISMATCH).await;
    }

    // IP 封锁检查
    if !self.check_ip_blocker(&ip, &id).await {
        return send_rk_res(socket, addr, TOO_FREQUENT).await;
    }

    // UUID 匹配验证
    let peer = self.pm.get_or(&id).await;
    // ... UUID 验证逻辑 ...

    // 更新数据库
    if changed {
        self.pm.update_pk(id, peer, addr, rk.uuid, rk.pk, ip).await;
    }

    // 发送成功响应
    msg_out.set_register_pk_response(RegisterPkResponse {
        result: register_pk_response::Result::OK.into(),
        ..Default::default()
    });
}
```

**服务器 → 客户端：RegisterPkResponse**

```protobuf
message RegisterPkResponse {
  enum Result {
    OK = 0;
    UUID_MISMATCH = 2;
    ID_EXISTS = 3;
    TOO_FREQUENT = 4;
    INVALID_ID_FORMAT = 5;
    NOT_SUPPORT = 6;
    SERVER_ERROR = 7;
  }
  Result result = 1;
  int32 keep_alive = 2;  // 心跳间隔
}
```

### 3. Punch Hole 请求流程

当控制端要连接被控端时，首先发送 PunchHoleRequest。

**控制端 → 服务器：PunchHoleRequest**

```protobuf
message PunchHoleRequest {
  string id = 1;           // 目标 Peer ID
  NatType nat_type = 2;    // 请求方的 NAT 类型
  string licence_key = 3;  // 许可证密钥
  ConnType conn_type = 4;  // 连接类型
  string token = 5;        // 认证令牌
  string version = 6;      // 客户端版本
}

enum NatType {
  UNKNOWN_NAT = 0;
  ASYMMETRIC = 1;
  SYMMETRIC = 2;
}

enum ConnType {
  DEFAULT_CONN = 0;
  FILE_TRANSFER = 1;
  PORT_FORWARD = 2;
  RDP = 3;
  VIEW_CAMERA = 4;
}
```

**服务器处理逻辑：**

```rust
// rustdesk-server/src/rendezvous_server.rs:674-765
async fn handle_punch_hole_request(...) {
    // 1. 验证许可证密钥
    if !key.is_empty() && ph.licence_key != key {
        return Ok((PunchHoleResponse { failure: LICENSE_MISMATCH }, None));
    }

    // 2. 查找目标 Peer
    if let Some(peer) = self.pm.get(&id).await {
        let (elapsed, peer_addr) = peer.read().await;

        // 3. 检查在线状态
        if elapsed >= REG_TIMEOUT {
            return Ok((PunchHoleResponse { failure: OFFLINE }, None));
        }

        // 4. 判断是否同一局域网
        let same_intranet = (peer_is_lan && is_lan) ||
                            (peer_addr.ip() == addr.ip());

        if same_intranet {
            // 请求获取本地地址
            msg_out.set_fetch_local_addr(FetchLocalAddr {
                socket_addr: AddrMangle::encode(addr).into(),
                relay_server,
            });
        } else {
            // 发送 Punch Hole 请求给被控端
            msg_out.set_punch_hole(PunchHole {
                socket_addr: AddrMangle::encode(addr).into(),
                nat_type: ph.nat_type,
                relay_server,
            });
        }
        return Ok((msg_out, Some(peer_addr)));
    }

    // Peer 不存在
    Ok((PunchHoleResponse { failure: ID_NOT_EXIST }, None))
}
```

**服务器 → 被控端：PunchHole 或 FetchLocalAddr**

```protobuf
message PunchHole {
  bytes socket_addr = 1;    // 控制端地址（编码）
  string relay_server = 2;  // Relay 服务器地址
  NatType nat_type = 3;     // 控制端 NAT 类型
}

message FetchLocalAddr {
  bytes socket_addr = 1;    // 控制端地址（编码）
  string relay_server = 2;  // Relay 服务器地址
}
```

### 4. 被控端响应流程

**被控端 → 服务器：PunchHoleSent 或 LocalAddr**

```protobuf
message PunchHoleSent {
  bytes socket_addr = 1;    // 控制端地址
  string id = 2;            // 被控端 ID
  string relay_server = 3;  // Relay 服务器
  NatType nat_type = 4;     // 被控端 NAT 类型
  string version = 5;       // 客户端版本
}

message LocalAddr {
  bytes socket_addr = 1;    // 控制端地址
  bytes local_addr = 2;     // 被控端本地地址
  string relay_server = 3;  // Relay 服务器
  string id = 4;            // 被控端 ID
  string version = 5;       // 客户端版本
}
```

**服务器 → 控制端：PunchHoleResponse**

```protobuf
message PunchHoleResponse {
  bytes socket_addr = 1;    // 被控端地址
  bytes pk = 2;             // 被控端公钥（已签名）
  enum Failure {
    ID_NOT_EXIST = 0;
    OFFLINE = 2;
    LICENSE_MISMATCH = 3;
    LICENSE_OVERUSE = 4;
  }
  Failure failure = 3;
  string relay_server = 4;
  oneof union {
    NatType nat_type = 5;
    bool is_local = 6;      // 是否为局域网连接
  }
  string other_failure = 7;
  int32 feedback = 8;
}
```

### 5. Relay 请求流程

当 P2P 连接失败或 NAT 类型不支持打洞时，使用 Relay。

**客户端 → 服务器：RequestRelay**

```protobuf
message RequestRelay {
  string id = 1;            // 目标 Peer ID
  string uuid = 2;          // 连接 UUID（用于配对）
  bytes socket_addr = 3;    // 本端地址
  string relay_server = 4;  // 指定的 Relay 服务器
  bool secure = 5;          // 是否使用加密
  string licence_key = 6;   // 许可证密钥
  ConnType conn_type = 7;   // 连接类型
  string token = 8;         // 认证令牌
}
```

**服务器 → 客户端：RelayResponse**

```protobuf
message RelayResponse {
  bytes socket_addr = 1;    // 对端地址
  string uuid = 2;          // 连接 UUID
  string relay_server = 3;  // Relay 服务器地址
  oneof union {
    string id = 4;          // 对端 ID
    bytes pk = 5;           // 对端公钥
  }
  string refuse_reason = 6; // 拒绝原因
  string version = 7;       // 版本
  int32 feedback = 9;
}
```

## NAT 类型检测

**客户端 → 服务器：TestNatRequest**

```protobuf
message TestNatRequest {
  int32 serial = 1;  // 配置序列号
}
```

**服务器 → 客户端：TestNatResponse**

```protobuf
message TestNatResponse {
  int32 port = 1;           // 观测到的源端口
  ConfigUpdate cu = 2;      // 配置更新
}
```

NAT 检测原理：
1. 客户端同时向主端口（21116）和 NAT 测试端口（21115）发送请求
2. 比较两次响应中观测到的源端口
3. 如果端口一致，则为 ASYMMETRIC NAT（适合打洞）
4. 如果端口不一致，则为 SYMMETRIC NAT（需要 Relay）

## 地址编码

RustDesk 使用 `AddrMangle` 对 SocketAddr 进行编码：

```rust
// 编码示例
// IPv4: 4 bytes IP + 2 bytes port = 6 bytes
// IPv6: 16 bytes IP + 2 bytes port = 18 bytes
pub fn encode(addr: SocketAddr) -> Vec<u8>;
pub fn decode(bytes: &[u8]) -> SocketAddr;
```

## 安全机制

### 服务器签名

当服务器配置了私钥时，会对 Peer 的公钥进行签名：

```rust
// rustdesk-server/src/rendezvous_server.rs:1160-1182
async fn get_pk(&mut self, version: &str, id: String) -> Bytes {
    if version.is_empty() || self.inner.sk.is_none() {
        Bytes::new()
    } else {
        match self.pm.get(&id).await {
            Some(peer) => {
                let pk = peer.read().await.pk.clone();
                // 使用服务器私钥签名 IdPk
                sign::sign(
                    &IdPk { id, pk, ..Default::default() }
                        .write_to_bytes()
                        .unwrap_or_default(),
                    self.inner.sk.as_ref().unwrap(),
                ).into()
            }
            _ => Bytes::new(),
        }
    }
}
```

### IP 封锁

服务器实现了 IP 封锁机制防止滥用：

```rust
// rustdesk-server/src/rendezvous_server.rs:866-894
async fn check_ip_blocker(&self, ip: &str, id: &str) -> bool {
    let mut lock = IP_BLOCKER.lock().await;
    if let Some(old) = lock.get_mut(ip) {
        // 每秒请求超过 30 次则封锁
        if counter.0 > 30 {
            return false;
        }
        // 每天超过 300 个不同 ID 则封锁
        if counter.0.len() > 300 {
            return !is_new;
        }
    }
    true
}
```

## 时序图

### 完整连接建立流程

```
控制端              Rendezvous Server           被控端
  │                       │                       │
  │  PunchHoleRequest     │                       │
  ├──────────────────────►│                       │
  │                       │    PunchHole          │
  │                       ├──────────────────────►│
  │                       │                       │
  │                       │    PunchHoleSent      │
  │                       │◄──────────────────────┤
  │  PunchHoleResponse    │                       │
  │◄──────────────────────┤                       │
  │                       │                       │
  │  ─────────── P2P Connection ──────────────────│
  │◄─────────────────────────────────────────────►│
```
