# NAT 穿透技术

## 概述

RustDesk 实现了多种 NAT 穿透技术，以在不同网络环境下建立 P2P 连接：

- NAT 类型检测
- UDP 打洞
- TCP 打洞
- Relay 中转（作为后备）

## NAT 类型

### 分类

```protobuf
enum NatType {
  UNKNOWN_NAT = 0;   // 未知
  ASYMMETRIC = 1;    // 非对称 NAT (Cone NAT) - 可打洞
  SYMMETRIC = 2;     // 对称 NAT - 通常需要 Relay
}
```

### NAT 类型说明

| 类型 | 描述 | 可打洞 |
|------|------|--------|
| Full Cone | 外部端口固定，任何外部主机可访问 | ✅ 最容易 |
| Restricted Cone | 外部端口固定，仅允许曾发送过数据的 IP | ✅ 容易 |
| Port Restricted Cone | 外部端口固定，仅允许曾发送过数据的 IP:Port | ✅ 可能 |
| Symmetric | 每个目标地址使用不同外部端口 | ❌ 困难 |

## NAT 类型检测

### 检测原理

RustDesk 使用双端口检测法：

1. 客户端向 Rendezvous Server 的主端口 (21116) 发送 TestNatRequest
2. 同时向 NAT 测试端口 (21115) 发送 TestNatRequest
3. 比较两次响应中观测到的源端口

```
客户端                    Rendezvous Server
   │                           │
   │   TestNatRequest ────────►│ Port 21116
   │                           │
   │   TestNatRequest ────────►│ Port 21115
   │                           │
   │◄──────── TestNatResponse  │ (包含观测到的源端口)
   │                           │
   │                           │
   │ 比较两次源端口             │
   │ 相同 → ASYMMETRIC         │
   │ 不同 → SYMMETRIC           │
```

### 实现代码

**客户端发送检测请求：**

```rust
// rustdesk/src/lib.rs
pub fn test_nat_type() {
    tokio::spawn(async move {
        let rendezvous_server = Config::get_rendezvous_servers().first().cloned();
        if let Some(host) = rendezvous_server {
            // 连接主端口
            let host = check_port(&host, RENDEZVOUS_PORT);

            // 连接 NAT 测试端口
            let host2 = crate::increase_port(&host, -1);

            // 发送测试请求
            let mut msg = RendezvousMessage::new();
            msg.set_test_nat_request(TestNatRequest {
                serial: Config::get_serial(),
            });

            // 收集两次响应的端口
            let port1 = send_and_get_port(&host, &msg).await;
            let port2 = send_and_get_port(&host2, &msg).await;

            // 判断 NAT 类型
            let nat_type = if port1 == port2 {
                NatType::ASYMMETRIC  // 可打洞
            } else {
                NatType::SYMMETRIC   // 需要 Relay
            };

            Config::set_nat_type(nat_type as i32);
        }
    });
}
```

**服务器响应：**

```rust
// rustdesk-server/src/rendezvous_server.rs:1080-1087
Some(rendezvous_message::Union::TestNatRequest(_)) => {
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_test_nat_response(TestNatResponse {
        port: addr.port() as _,  // 返回观测到的源端口
        ..Default::default()
    });
    stream.send(&msg_out).await.ok();
}
```

## UDP 打洞

### 原理

UDP 打洞利用 NAT 的端口映射机制：

```
    A (内网)                                             B (内网)
       │                                                    │
       │ ──► NAT_A ──► Internet ──► NAT_B ──► (丢弃)        │
       │                                                    │
       │                                                    │
       │ (NAT_A 创建了映射 A:port → A_ext:port_a)           │
       │                                                    │
       │                                                    │
       │ (丢弃) ◄── NAT_A ◄── Internet ◄── NAT_B ◄── │
       │                                                    │
       │                                                    │
       │ (NAT_B 创建了映射 B:port → B_ext:port_b)           │
       │                                                    │
       │ ──► NAT_A ──► Internet ──► NAT_B ──► │
       │ (NAT_A 的映射存在，包被转发)                        │
       │                                                    │
       │ ◄── NAT_A ◄── Internet ◄── NAT_B ◄── │
       │ (NAT_B 的映射存在，包被转发)                        │
       │                                                    │
       │ ◄───────── 双向通信建立 ──────────► │
```

### 实现

**被控端打洞：**

```rust
// rustdesk/src/rendezvous_mediator.rs:621-642
async fn punch_udp_hole(
    &self,
    peer_addr: SocketAddr,
    server: ServerPtr,
    msg_punch: PunchHoleSent,
) -> ResultType<()> {
    let mut msg_out = Message::new();
    msg_out.set_punch_hole_sent(msg_punch);

    // 创建 UDP socket
    let (socket, addr) = new_direct_udp_for(&self.host).await?;
    let data = msg_out.write_to_bytes()?;

    // 发送到 Rendezvous Server（会转发给控制端）
    socket.send_to(&data, addr).await?;

    // 多次发送以增加成功率
    let socket_cloned = socket.clone();
    tokio::spawn(async move {
        for _ in 0..2 {
            let tm = (hbb_common::time_based_rand() % 20 + 10) as f32 / 1000.;
            hbb_common::sleep(tm).await;
            socket.send_to(&data, addr).await.ok();
        }
    });

    // 等待对方连接
    udp_nat_listen(socket_cloned, peer_addr, peer_addr, server).await?;
    Ok(())
}
```

**UDP 监听和 KCP 建立：**

```rust
// rustdesk/src/rendezvous_mediator.rs:824-851
async fn udp_nat_listen(
    socket: Arc<tokio::net::UdpSocket>,
    peer_addr: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
) -> ResultType<()> {
    // 连接到对方地址
    socket.connect(peer_addr).await?;

    // 执行 UDP 打洞
    let res = crate::punch_udp(socket.clone(), true).await?;

    // 建立 KCP 可靠传输层
    let stream = crate::kcp_stream::KcpStream::accept(
        socket,
        Duration::from_millis(CONNECT_TIMEOUT as _),
        res,
    ).await?;

    // 创建连接
    crate::server::create_tcp_connection(server, stream.1, peer_addr_v4, true).await?;
    Ok(())
}
```

### KCP 协议

RustDesk 在 UDP 上使用 KCP 提供可靠传输，KCP 特点：

- 更激进的重传策略
- 更低的延迟
- 可配置的可靠性级别

## TCP 打洞

### 原理

TCP 打洞比 UDP 困难，因为 TCP 需要三次握手。技巧是让双方同时发起连接：

```
    A                       NAT_A     NAT_B                       B
    │                         │         │                         │
    │ ─── SYN ───────────────►│─────────│────► (丢弃，无映射)     │
    │                         │         │                         │
    │ (NAT_A 创建到 B 的映射)  │         │                         │
    │                         │         │                         │
    │ (丢弃，无映射) ◄─────────│─────────│◄─── SYN ───────────── │
    │                         │         │                         │
    │                         │         │ (NAT_B 创建到 A 的映射)  │
    │                         │         │                         │
    │ ─── SYN ───────────────►│─────────│────► SYN ───────────► │
    │                         │         │ (映射存在，转发成功)     │
    │                         │         │                         │
    │ ◄─── SYN+ACK ──────────│─────────│◄─── SYN+ACK ───────── │
    │                         │         │                         │
    │ ─── ACK ───────────────►│─────────│────► ACK ───────────► │
    │                         │         │                         │
    │ ◄─────────── 连接建立 ─────────────────────────────────────►│
```

### 实现

```rust
// rustdesk/src/rendezvous_mediator.rs:604-617
log::debug!("Punch tcp hole to {:?}", peer_addr);
let mut socket = {
    // 1. 先连接 Rendezvous Server 获取本地地址
    let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let local_addr = socket.local_addr();

    // 2. 用相同的本地地址尝试连接对方
    // 这会在 NAT 上创建映射
    // 虽然连接会失败，但映射已建立
    allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);

    socket
};

// 3. 发送 PunchHoleSent 通知服务器
// 服务器会转发给控制端
let mut msg_out = Message::new();
msg_out.set_punch_hole_sent(msg_punch);
socket.send_raw(msg_out.write_to_bytes()?).await?;

// 4. 等待控制端连接
// 由于已有映射，控制端的连接可以成功
crate::accept_connection(server.clone(), socket, peer_addr, true).await;
```

## 局域网直连

### 检测同一局域网

```rust
// rustdesk-server/src/rendezvous_server.rs:721-728
let same_intranet: bool = !ws
    && (peer_is_lan && is_lan || {
        match (peer_addr, addr) {
            (SocketAddr::V4(a), SocketAddr::V4(b)) => a.ip() == b.ip(),
            (SocketAddr::V6(a), SocketAddr::V6(b)) => a.ip() == b.ip(),
            _ => false,
        }
    });
```

### 局域网连接流程

```
控制端              Rendezvous Server              被控端
   │                       │                          │
   │ PunchHoleRequest ────►│                          │
   │                       │                          │
   │                       │ (检测到同一局域网)         │
   │                       │                          │
   │                       │    FetchLocalAddr ──────►│
   │                       │                          │
   │                       │◄────── LocalAddr ────────│
   │                       │   (包含被控端内网地址)     │
   │                       │                          │
   │◄─ PunchHoleResponse ──│                          │
   │  (is_local=true)      │                          │
   │  (socket_addr=内网地址)│                          │
   │                       │                          │
   │ ─────────── 直接连接内网地址 ────────────────────►│
```

## IPv6 支持

IPv6 通常不需要 NAT 穿透，但 RustDesk 仍支持 IPv6 打洞以处理有状态防火墙：

```rust
// rustdesk/src/rendezvous_mediator.rs:808-822
async fn start_ipv6(
    peer_addr_v6: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
) -> bytes::Bytes {
    crate::test_ipv6().await;
    if let Some((socket, local_addr_v6)) = crate::get_ipv6_socket().await {
        let server = server.clone();
        tokio::spawn(async move {
            allow_err!(udp_nat_listen(socket.clone(), peer_addr_v6, peer_addr_v4, server).await);
        });
        return local_addr_v6;
    }
    Default::default()
}
```

## 连接策略决策树

```
                         开始连接
                            │
                            ▼
                   ┌───────────────┐
                   │  NAT 类型检测  │
                   └───────┬───────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
           ▼               ▼               ▼
      ASYMMETRIC       UNKNOWN        SYMMETRIC
           │               │               │
           ▼               ▼               │
     ┌──────────┐    ┌──────────┐         │
     │ 尝试 UDP  │    │ 尝试 TCP  │         │
     │ 打洞     │    │ 打洞     │         │
     └────┬─────┘    └────┬─────┘         │
          │               │               │
     成功  │  失败    成功  │  失败         │
          ▼    │          ▼    │          │
     ┌────────┐│     ┌────────┐│          │
     │UDP P2P ││     │TCP P2P ││          │
     └────────┘│     └────────┘│          │
               │               │          │
               └───────┬───────┘          │
                       │                  │
                       ▼                  │
               ┌───────────────┐          │
               │   使用 Relay   │◄─────────┘
               └───────────────┘
```

## 性能优化

### 多路径尝试

RustDesk 同时尝试多种连接方式，选择最快成功的：

```rust
// rustdesk/src/client.rs:342-364
let mut connect_futures = Vec::new();

// 同时尝试 UDP 和 TCP
if udp.0.is_some() {
    connect_futures.push(Self::_start_inner(..., udp).boxed());
}
connect_futures.push(Self::_start_inner(..., (None, None)).boxed());

// 使用 select_ok 选择第一个成功的
match select_ok(connect_futures).await {
    Ok(conn) => Ok(conn),
    Err(e) => Err(e),
}
```

### 超时控制

```rust
const CONNECT_TIMEOUT: u64 = 18_000;  // 18 秒
const REG_TIMEOUT: i32 = 30_000;      // 30 秒

// 连接超时处理
if let Ok(Ok((stream, addr))) = timeout(CONNECT_TIMEOUT, socket.accept()).await {
    // 连接成功
} else {
    // 超时，尝试其他方式
}
```

## 常见问题和解决方案

| 问题 | 原因 | 解决方案 |
|------|------|----------|
| 双 Symmetric NAT | 两端都是对称 NAT | 使用 Relay |
| 防火墙阻止 UDP | 企业防火墙 | 使用 TCP 或 WebSocket |
| 端口预测失败 | NAT 端口分配不规律 | 多次尝试或使用 Relay |
| IPv6 不通 | ISP 或防火墙问题 | 回退到 IPv4 |
