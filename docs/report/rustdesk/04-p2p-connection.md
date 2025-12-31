# P2P 连接流程

## 概述

RustDesk 优先尝试建立 P2P 直连，只有在直连失败时才使用 Relay 中转。P2P 连接支持多种方式：
- TCP 打洞
- UDP 打洞（KCP）
- 局域网直连
- IPv6 直连

## 连接决策流程

```
                              开始连接
                                 │
                                 ▼
                         ┌──────────────┐
                         │ 是否强制 Relay？│
                         └──────┬───────┘
                           是   │   否
                     ┌─────────┴─────────┐
                     ▼                   ▼
              使用 Relay           检查 NAT 类型
                                        │
                         ┌──────────────┴──────────────┐
                         │                             │
                         ▼                             ▼
                  双方都是对称 NAT？            有一方是可穿透 NAT
                         │                             │
                    是   │                             │
                         ▼                             ▼
                  使用 Relay                  尝试 P2P 连接
                                                      │
                                        ┌─────────────┴─────────────┐
                                        │                           │
                                        ▼                           ▼
                                 同一局域网？                  不同网络
                                        │                           │
                                   是   │                           │
                                        ▼                           ▼
                               局域网直连                      尝试打洞
                                                                   │
                                                    ┌──────────────┴──────────────┐
                                                    │                             │
                                                    ▼                             ▼
                                            TCP 打洞成功？                  UDP 打洞成功？
                                                    │                             │
                                               是   │   否                   是   │   否
                                                    ▼    │                        ▼    │
                                            TCP P2P 连接  └───────────► KCP P2P 连接   │
                                                                                       ▼
                                                                               使用 Relay
```

## 客户端连接入口

```rust
// rustdesk/src/client.rs:188-230
impl Client {
    pub async fn start(
        peer: &str,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
    ) -> ResultType<...> {
        // 检查是否为 IP 直连
        if hbb_common::is_ip_str(peer) {
            return connect_tcp_local(check_port(peer, RELAY_PORT + 1), None, CONNECT_TIMEOUT).await;
        }

        // 检查是否为域名:端口格式
        if hbb_common::is_domain_port_str(peer) {
            return connect_tcp_local(peer, None, CONNECT_TIMEOUT).await;
        }

        // 通过 Rendezvous Server 连接
        let (rendezvous_server, servers, _) = crate::get_rendezvous_server(1_000).await;
        Self::_start_inner(peer, key, token, conn_type, interface, rendezvous_server, servers).await
    }
}
```

## 被控端处理连接请求

### 处理 PunchHole 消息

```rust
// rustdesk/src/rendezvous_mediator.rs:554-619
async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&ph.socket_addr);
    let relay_server = self.get_relay_server(ph.relay_server);

    // 判断是否需要 Relay
    if ph.nat_type.enum_value() == Ok(NatType::SYMMETRIC)
        || Config::get_nat_type() == NatType::SYMMETRIC as i32
        || relay
    {
        // 使用 Relay
        let uuid = Uuid::new_v4().to_string();
        return self.create_relay(ph.socket_addr, relay_server, uuid, server, true, true).await;
    }

    // 尝试 UDP 打洞
    if ph.udp_port > 0 {
        peer_addr.set_port(ph.udp_port as u16);
        self.punch_udp_hole(peer_addr, server, msg_punch).await?;
        return Ok(());
    }

    // 尝试 TCP 打洞
    log::debug!("Punch tcp hole to {:?}", peer_addr);
    let socket = {
        let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
        let local_addr = socket.local_addr();
        // 关键步骤：尝试连接对方，让 NAT 建立映射
        allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
        socket
    };

    // 发送 PunchHoleSent 告知 Rendezvous Server
    let mut msg_out = Message::new();
    msg_out.set_punch_hole_sent(PunchHoleSent {
        socket_addr: ph.socket_addr,
        id: Config::get_id(),
        relay_server,
        nat_type: nat_type.into(),
        version: crate::VERSION.to_owned(),
    });
    socket.send_raw(msg_out.write_to_bytes()?).await?;

    // 接受控制端连接
    crate::accept_connection(server.clone(), socket, peer_addr, true).await;
    Ok(())
}
```

### 处理 FetchLocalAddr（局域网连接）

```rust
// rustdesk/src/rendezvous_mediator.rs:481-552
async fn handle_intranet(&self, fla: FetchLocalAddr, server: ServerPtr) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&fla.socket_addr);
    let relay_server = self.get_relay_server(fla.relay_server.clone());

    // 尝试局域网直连
    if is_ipv4(&self.addr) && !relay && !config::is_disable_tcp_listen() {
        if let Err(err) = self.handle_intranet_(fla.clone(), server.clone(), relay_server.clone()).await {
            log::debug!("Failed to handle intranet: {:?}, will try relay", err);
        } else {
            return Ok(());
        }
    }

    // 局域网直连失败，使用 Relay
    let uuid = Uuid::new_v4().to_string();
    self.create_relay(fla.socket_addr, relay_server, uuid, server, true, true).await
}

async fn handle_intranet_(&self, fla: FetchLocalAddr, server: ServerPtr, relay_server: String) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&fla.socket_addr);
    let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let local_addr = socket.local_addr();

    // 发送本地地址给 Rendezvous Server
    let mut msg_out = Message::new();
    msg_out.set_local_addr(LocalAddr {
        id: Config::get_id(),
        socket_addr: AddrMangle::encode(peer_addr).into(),
        local_addr: AddrMangle::encode(local_addr).into(),
        relay_server,
        version: crate::VERSION.to_owned(),
    });
    socket.send_raw(msg_out.write_to_bytes()?).await?;

    // 接受连接
    crate::accept_connection(server.clone(), socket, peer_addr, true).await;
    Ok(())
}
```

## UDP 打洞 (KCP)

### 打洞原理

UDP 打洞利用 NAT 的端口映射特性：

1. A 向 Rendezvous Server 注册，NAT 创建映射 `A_internal:port1 → A_external:port2`
2. B 同样注册，创建映射 `B_internal:port3 → B_external:port4`
3. A 向 B 的外部地址发送 UDP 包，A 的 NAT 创建到 B 的映射
4. B 向 A 的外部地址发送 UDP 包，B 的 NAT 创建到 A 的映射
5. 如果 NAT 不是 Symmetric 类型，双方的包可以到达对方

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
    let (socket, addr) = new_direct_udp_for(&self.host).await?;
    let data = msg_out.write_to_bytes()?;

    // 发送到 Rendezvous Server
    socket.send_to(&data, addr).await?;

    // 多次尝试发送以增加成功率
    let socket_cloned = socket.clone();
    tokio::spawn(async move {
        for _ in 0..2 {
            let tm = (hbb_common::time_based_rand() % 20 + 10) as f32 / 1000.;
            hbb_common::sleep(tm).await;
            socket.send_to(&data, addr).await.ok();
        }
    });

    // 等待对方连接
    udp_nat_listen(socket_cloned.clone(), peer_addr, peer_addr, server).await?;
    Ok(())
}
```

### KCP 协议

RustDesk 在 UDP 上使用 KCP 协议提供可靠传输：

```rust
// rustdesk/src/rendezvous_mediator.rs:824-851
async fn udp_nat_listen(
    socket: Arc<tokio::net::UdpSocket>,
    peer_addr: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
) -> ResultType<()> {
    socket.connect(peer_addr).await?;

    // 执行 UDP 打洞
    let res = crate::punch_udp(socket.clone(), true).await?;

    // 建立 KCP 流
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

## TCP 打洞

### 原理

TCP 打洞比 UDP 更难，因为 TCP 需要三次握手。基本思路：

1. A 和 B 都尝试同时向对方发起连接
2. 第一个 SYN 包会被对方的 NAT 丢弃（因为没有映射）
3. 但这个 SYN 包会在 A 的 NAT 上创建映射
4. 当 B 的 SYN 包到达 A 的 NAT 时，由于已有映射，会被转发给 A
5. 连接建立

### 实现

```rust
// rustdesk/src/rendezvous_mediator.rs:604-617
log::debug!("Punch tcp hole to {:?}", peer_addr);
let mut socket = {
    let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let local_addr = socket.local_addr();
    // 关键：使用相同的本地地址尝试连接对方
    // 这会在 NAT 上创建映射，使对方的连接请求能够到达
    allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
    socket
};
```

## Relay 连接

当 P2P 失败时，使用 Relay：

```rust
// rustdesk/src/rendezvous_mediator.rs:434-479
async fn create_relay(
    &self,
    socket_addr: Vec<u8>,
    relay_server: String,
    uuid: String,
    server: ServerPtr,
    secure: bool,
    initiate: bool,
) -> ResultType<()> {
    let peer_addr = AddrMangle::decode(&socket_addr);
    log::info!(
        "create_relay requested from {:?}, relay_server: {}, uuid: {}, secure: {}",
        peer_addr, relay_server, uuid, secure,
    );

    // 连接 Rendezvous Server 发送 RelayResponse
    let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
    let mut msg_out = Message::new();
    let mut rr = RelayResponse {
        socket_addr: socket_addr.into(),
        version: crate::VERSION.to_owned(),
        ..Default::default()
    };
    if initiate {
        rr.uuid = uuid.clone();
        rr.relay_server = relay_server.clone();
        rr.set_id(Config::get_id());
    }
    msg_out.set_relay_response(rr);
    socket.send(&msg_out).await?;

    // 连接 Relay Server
    crate::create_relay_connection(
        server,
        relay_server,
        uuid,
        peer_addr,
        secure,
        is_ipv4(&self.addr),
    ).await;
    Ok(())
}
```

## IPv6 支持

RustDesk 优先尝试 IPv6 连接：

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

## 连接状态机

```
          ┌─────────────────────────────────────────┐
          │                                         │
          ▼                                         │
    ┌───────────┐                              ┌────┴────┐
    │ 等待连接   │──────PunchHoleRequest──────►│正在连接  │
    └───────────┘                              └────┬────┘
                                                   │
                    ┌──────────────────────────────┼──────────────────────────────┐
                    │                              │                              │
                    ▼                              ▼                              ▼
             ┌────────────┐                ┌─────────────┐                ┌─────────────┐
             │ P2P TCP    │                │ P2P UDP/KCP │                │   Relay     │
             │ 连接中     │                │ 连接中      │                │   连接中     │
             └─────┬──────┘                └──────┬──────┘                └──────┬──────┘
                   │                              │                              │
          成功     │   失败                成功   │   失败                 成功   │   失败
                   │    │                        │    │                         │    │
                   ▼    │                        ▼    │                         ▼    │
            ┌──────────┐│                 ┌──────────┐│                  ┌──────────┐│
            │已连接     ││                 │已连接     ││                  │已连接     ││
            │(直连)    ││                 │(UDP)     ││                  │(中转)    ││
            └──────────┘│                 └──────────┘│                  └──────────┘│
                        │                             │                              │
                        └──────────────►尝试 Relay◄───┘                              │
                                            │                                        │
                                            └────────────────────────────────────────┘
```

## 直接连接模式

用户可以配置允许直接 TCP 连接（不经过 Rendezvous Server）：

```rust
// rustdesk/src/rendezvous_mediator.rs:727-792
async fn direct_server(server: ServerPtr) {
    let mut listener = None;
    let mut port = get_direct_port();  // 默认 21118

    loop {
        let disabled = !option2bool(OPTION_DIRECT_SERVER, &Config::get_option(OPTION_DIRECT_SERVER));

        if !disabled && listener.is_none() {
            match hbb_common::tcp::listen_any(port as _).await {
                Ok(l) => {
                    listener = Some(l);
                    log::info!("Direct server listening on: {:?}", l.local_addr());
                }
                Err(err) => {
                    log::error!("Failed to start direct server: {}", err);
                }
            }
        }

        if let Some(l) = listener.as_mut() {
            if let Ok(Ok((stream, addr))) = hbb_common::timeout(1000, l.accept()).await {
                stream.set_nodelay(true).ok();
                log::info!("direct access from {}", addr);
                let server = server.clone();
                tokio::spawn(async move {
                    crate::server::create_tcp_connection(server, stream, addr, false).await
                });
            }
        }
    }
}
```
