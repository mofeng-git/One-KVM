# Relay 服务器协议

## 概述

Relay Server（hbbr）是 RustDesk 的数据中转服务器，当 P2P 连接无法建立时（如双方都在 Symmetric NAT 后面），所有通信数据通过 Relay Server 转发。

## 服务器架构

### 监听端口

| 端口 | 协议 | 用途 |
|------|------|------|
| 21117 | TCP | 主要中转端口 |
| 21119 | WebSocket | Web 客户端支持 |

### 核心配置

```rust
// rustdesk-server/src/relay_server.rs:40-46
static DOWNGRADE_THRESHOLD_100: AtomicUsize = AtomicUsize::new(66);        // 0.66
static DOWNGRADE_START_CHECK: AtomicUsize = AtomicUsize::new(1_800_000);   // 30分钟 (ms)
static LIMIT_SPEED: AtomicUsize = AtomicUsize::new(32 * 1024 * 1024);      // 32 Mb/s
static TOTAL_BANDWIDTH: AtomicUsize = AtomicUsize::new(1024 * 1024 * 1024);// 1024 Mb/s
static SINGLE_BANDWIDTH: AtomicUsize = AtomicUsize::new(128 * 1024 * 1024);// 128 Mb/s
const BLACKLIST_FILE: &str = "blacklist.txt";
const BLOCKLIST_FILE: &str = "blocklist.txt";
```

## 连接配对机制

### 配对原理

Relay Server 使用 UUID 来配对两个客户端的连接：

1. 第一个客户端连接并发送 `RequestRelay` 消息（包含 UUID）
2. 服务器将该连接存储在等待队列中
3. 第二个客户端使用相同的 UUID 连接
4. 服务器将两个连接配对，开始转发数据

### 配对流程

```rust
// rustdesk-server/src/relay_server.rs:425-462
async fn make_pair_(stream: impl StreamTrait, addr: SocketAddr, key: &str, limiter: Limiter) {
    let mut stream = stream;
    if let Ok(Some(Ok(bytes))) = timeout(30_000, stream.recv()).await {
        if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
            if let Some(rendezvous_message::Union::RequestRelay(rf)) = msg_in.union {
                // 验证许可证密钥
                if !key.is_empty() && rf.licence_key != key {
                    log::warn!("Relay authentication failed from {}", addr);
                    return;
                }

                if !rf.uuid.is_empty() {
                    // 尝试查找配对
                    let mut peer = PEERS.lock().await.remove(&rf.uuid);
                    if let Some(peer) = peer.as_mut() {
                        // 找到配对，开始中转
                        log::info!("Relay request {} got paired", rf.uuid);
                        relay(addr, &mut stream, peer, limiter).await;
                    } else {
                        // 没找到，存储等待配对
                        log::info!("New relay request {} from {}", rf.uuid, addr);
                        PEERS.lock().await.insert(rf.uuid.clone(), Box::new(stream));
                        sleep(30.).await;  // 等待 30 秒
                        PEERS.lock().await.remove(&rf.uuid);  // 超时移除
                    }
                }
            }
        }
    }
}
```

## 数据转发

### 转发逻辑

```rust
// rustdesk-server/src/relay_server.rs:464-566
async fn relay(
    addr: SocketAddr,
    stream: &mut impl StreamTrait,
    peer: &mut Box<dyn StreamTrait>,
    total_limiter: Limiter,
) -> ResultType<()> {
    let limiter = <Limiter>::new(SINGLE_BANDWIDTH.load(Ordering::SeqCst) as f64);
    let blacklist_limiter = <Limiter>::new(LIMIT_SPEED.load(Ordering::SeqCst) as _);

    loop {
        tokio::select! {
            // 从 peer 接收数据，发送给 stream
            res = peer.recv() => {
                if let Some(Ok(bytes)) = res {
                    // 带宽限制
                    if blacked || downgrade {
                        blacklist_limiter.consume(bytes.len() * 8).await;
                    } else {
                        limiter.consume(bytes.len() * 8).await;
                    }
                    total_limiter.consume(bytes.len() * 8).await;
                    stream.send_raw(bytes.into()).await?;
                } else {
                    break;
                }
            },
            // 从 stream 接收数据，发送给 peer
            res = stream.recv() => {
                if let Some(Ok(bytes)) = res {
                    // 带宽限制
                    limiter.consume(bytes.len() * 8).await;
                    total_limiter.consume(bytes.len() * 8).await;
                    peer.send_raw(bytes.into()).await?;
                } else {
                    break;
                }
            },
            _ = timer.tick() => {
                // 超时检测
                if last_recv_time.elapsed().as_secs() > 30 {
                    bail!("Timeout");
                }
            }
        }

        // 降级检测
        if elapsed > DOWNGRADE_START_CHECK && total > elapsed * downgrade_threshold {
            downgrade = true;
            log::info!("Downgrade {}, exceed threshold", id);
        }
    }
    Ok(())
}
```

### 原始模式

当两端都支持原始模式时，跳过 protobuf 解析以提高性能：

```rust
// rustdesk-server/src/relay_server.rs:440-444
if !stream.is_ws() && !peer.is_ws() {
    peer.set_raw();
    stream.set_raw();
    log::info!("Both are raw");
}
```

## 带宽控制

### 多级限速

1. **总带宽限制**：整个服务器的总带宽
2. **单连接限制**：每个中转连接的带宽
3. **黑名单限速**：对黑名单 IP 的特殊限制

### 降级机制

当连接持续占用高带宽时，会触发降级：

```rust
// 条件：
// 1. 连接时间 > DOWNGRADE_START_CHECK (30分钟)
// 2. 平均带宽 > SINGLE_BANDWIDTH * 0.66
// 降级后使用 LIMIT_SPEED (32 Mb/s) 限速
if elapsed > DOWNGRADE_START_CHECK.load(Ordering::SeqCst)
    && !downgrade
    && total > elapsed * downgrade_threshold
{
    downgrade = true;
}
```

## 安全控制

### 黑名单

用于限速特定 IP：

```
# blacklist.txt
192.168.1.100
10.0.0.50
```

### 封锁名单

用于完全拒绝特定 IP：

```
# blocklist.txt
1.2.3.4
5.6.7.8
```

### 运行时管理命令

通过本地 TCP 连接（仅限 localhost）发送命令：

```rust
// rustdesk-server/src/relay_server.rs:152-324
match fds.next() {
    Some("h") => // 帮助
    Some("blacklist-add" | "ba") => // 添加黑名单
    Some("blacklist-remove" | "br") => // 移除黑名单
    Some("blacklist" | "b") => // 查看黑名单
    Some("blocklist-add" | "Ba") => // 添加封锁名单
    Some("blocklist-remove" | "Br") => // 移除封锁名单
    Some("blocklist" | "B") => // 查看封锁名单
    Some("downgrade-threshold" | "dt") => // 设置降级阈值
    Some("downgrade-start-check" | "t") => // 设置降级检测时间
    Some("limit-speed" | "ls") => // 设置限速
    Some("total-bandwidth" | "tb") => // 设置总带宽
    Some("single-bandwidth" | "sb") => // 设置单连接带宽
    Some("usage" | "u") => // 查看使用统计
}
```

## 协议消息

### RequestRelay

用于建立中转连接的请求消息：

```protobuf
message RequestRelay {
  string id = 1;            // 目标 Peer ID
  string uuid = 2;          // 连接 UUID（配对用）
  bytes socket_addr = 3;    // 本端地址
  string relay_server = 4;  // Relay 服务器
  bool secure = 5;          // 是否加密
  string licence_key = 6;   // 许可证密钥
  ConnType conn_type = 7;   // 连接类型
  string token = 8;         // 认证令牌
}
```

## 时序图

### 中转连接建立

```
客户端 A                  Relay Server                  客户端 B
   │                          │                            │
   │   RequestRelay(uuid)     │                            │
   ├─────────────────────────►│                            │
   │                          │                            │
   │                          │  (存储等待配对)              │
   │                          │                            │
   │                          │        RequestRelay(uuid)  │
   │                          │◄───────────────────────────┤
   │                          │                            │
   │                          │  (配对成功)                 │
   │                          │                            │
   │  ◄────────── 数据转发 ─────────────────────────────────►│
   │                          │                            │
```

### 数据转发

```
客户端 A           Relay Server           客户端 B
   │                    │                    │
   │ ────[数据]───────► │                    │
   │                    │ ────[数据]───────► │
   │                    │                    │
   │                    │ ◄───[数据]──────── │
   │ ◄───[数据]──────── │                    │
   │                    │                    │
```

## 性能优化

### 零拷贝

使用 `Bytes` 类型减少内存拷贝：

```rust
async fn send_raw(&mut self, bytes: Bytes) -> ResultType<()>;
```

### WebSocket 支持

支持 WebSocket 协议以穿越防火墙：

```rust
#[async_trait]
impl StreamTrait for tokio_tungstenite::WebSocketStream<TcpStream> {
    async fn recv(&mut self) -> Option<Result<BytesMut, Error>> {
        if let Some(msg) = self.next().await {
            match msg {
                Ok(tungstenite::Message::Binary(bytes)) => {
                    Some(Ok(bytes[..].into()))
                }
                // ...
            }
        }
    }
}
```

## 监控指标

服务器跟踪以下指标：

| 指标 | 说明 |
|------|------|
| elapsed | 连接持续时间 (ms) |
| total | 总传输数据量 (bit) |
| highest | 最高瞬时速率 (kb/s) |
| speed | 当前速率 (kb/s) |

通过 `usage` 命令查看：

```
192.168.1.100:12345: 3600s 1024.00MB 50000kb/s 45000kb/s 42000kb/s
```
