# 加密机制

## 概述

RustDesk 使用 libsodium (sodiumoxide) 库实现端到端加密，主要包含：

- **Ed25519**: 用于身份签名和验证
- **X25519**: 用于密钥交换
- **ChaCha20-Poly1305**: 用于对称加密

## 密钥类型

### 1. 身份密钥对 (Ed25519)

用于 Peer 身份认证和签名：

```rust
// 生成密钥对
use sodiumoxide::crypto::sign;
let (pk, sk) = sign::gen_keypair();
// pk: sign::PublicKey (32 bytes)
// sk: sign::SecretKey (64 bytes)
```

### 2. 服务器签名密钥

Rendezvous Server 可以配置签名密钥，用于签名 Peer 公钥：

```rust
// rustdesk-server/src/rendezvous_server.rs:1185-1210
fn get_server_sk(key: &str) -> (String, Option<sign::SecretKey>) {
    let mut out_sk = None;
    let mut key = key.to_owned();

    // 如果是 base64 编码的私钥
    if let Ok(sk) = base64::decode(&key) {
        if sk.len() == sign::SECRETKEYBYTES {
            log::info!("The key is a crypto private key");
            key = base64::encode(&sk[(sign::SECRETKEYBYTES / 2)..]);  // 公钥部分
            let mut tmp = [0u8; sign::SECRETKEYBYTES];
            tmp[..].copy_from_slice(&sk);
            out_sk = Some(sign::SecretKey(tmp));
        }
    }

    // 如果是占位符，生成新密钥对
    if key.is_empty() || key == "-" || key == "_" {
        let (pk, sk) = crate::common::gen_sk(0);
        out_sk = sk;
        if !key.is_empty() {
            key = pk;
        }
    }

    if !key.is_empty() {
        log::info!("Key: {}", key);
    }
    (key, out_sk)
}
```

### 3. 会话密钥 (X25519 + ChaCha20)

用于客户端之间的加密通信：

```rust
// hbb_common/src/tcp.rs:27-28
#[derive(Clone)]
pub struct Encrypt(pub Key, pub u64, pub u64);
// Key: secretbox::Key (32 bytes)
// u64: 发送计数器
// u64: 接收计数器
```

## 密钥交换流程

### 1. 身份验证

客户端首先交换签名的身份：

```protobuf
message IdPk {
  string id = 1;   // Peer ID
  bytes pk = 2;    // Ed25519 公钥
}

message SignedId {
  bytes id = 1;    // 签名的 IdPk (by server or self)
}
```

### 2. X25519 密钥交换

使用 X25519 ECDH 生成共享密钥：

```rust
// 生成临时密钥对
use sodiumoxide::crypto::box_;
let (our_pk, our_sk) = box_::gen_keypair();

// 计算共享密钥
let shared_secret = box_::curve25519xsalsa20poly1305::scalarmult(&our_sk, &their_pk);

// 派生对称密钥
let symmetric_key = secretbox::Key::from_slice(&shared_secret[..32]).unwrap();
```

### 3. 对称密钥消息

```protobuf
message PublicKey {
  bytes asymmetric_value = 1;  // X25519 公钥
  bytes symmetric_value = 2;   // 加密的对称密钥（用于额外安全）
}
```

## 会话加密

### 加密实现

```rust
// hbb_common/src/tcp.rs
impl Encrypt {
    pub fn new(key: Key) -> Self {
        Self(key, 0, 0)  // 初始化计数器为 0
    }

    // 加密
    pub fn enc(&mut self, data: &[u8]) -> Vec<u8> {
        self.1 += 1;  // 递增发送计数器
        let nonce = self.get_nonce(self.1);
        let encrypted = secretbox::seal(data, &nonce, &self.0);

        // 格式: nonce (8 bytes) + encrypted data
        let mut result = Vec::with_capacity(8 + encrypted.len());
        result.extend_from_slice(&self.1.to_le_bytes());
        result.extend_from_slice(&encrypted);
        result
    }

    // 解密
    pub fn dec(&mut self, data: &mut BytesMut) -> io::Result<()> {
        if data.len() < 8 + secretbox::MACBYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "too short"));
        }

        // 提取 nonce
        let counter = u64::from_le_bytes(data[..8].try_into().unwrap());

        // 防重放攻击检查
        if counter <= self.2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "replay attack"));
        }
        self.2 = counter;

        let nonce = self.get_nonce(counter);
        let plaintext = secretbox::open(&data[8..], &nonce, &self.0)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "decrypt failed"))?;

        data.clear();
        data.extend_from_slice(&plaintext);
        Ok(())
    }

    fn get_nonce(&self, counter: u64) -> Nonce {
        let mut nonce = [0u8; 24];
        nonce[..8].copy_from_slice(&counter.to_le_bytes());
        Nonce(nonce)
    }
}
```

### 消息格式

加密后的消息结构：

```
┌──────────────────┬─────────────────────────────────────────┐
│   Counter (8B)   │   Encrypted Data + MAC (N+16 bytes)     │
└──────────────────┴─────────────────────────────────────────┘
```

## 密码验证

### 挑战-响应机制

被控端生成随机盐和挑战，控制端计算哈希响应：

```protobuf
message Hash {
  string salt = 1;       // 随机盐
  string challenge = 2;  // 随机挑战
}
```

### 密码处理

```rust
// 客户端计算密码哈希
fn get_password_hash(password: &str, salt: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    hasher.finalize().to_vec()
}

// 发送加密的密码（使用对称密钥加密）
fn encrypt_password(password_hash: &[u8], symmetric_key: &Key) -> Vec<u8> {
    secretbox::seal(password_hash, &nonce, symmetric_key)
}
```

## 服务器公钥验证

### 签名验证

如果 Rendezvous Server 配置了密钥，会签名 Peer 公钥：

```rust
// 服务器签名 IdPk
let signed_id_pk = sign::sign(
    &IdPk { id, pk, ..Default::default() }
        .write_to_bytes()?,
    &server_sk,
);

// 客户端验证
fn verify_server_signature(signed_pk: &[u8], server_pk: &sign::PublicKey) -> Option<IdPk> {
    if let Ok(verified) = sign::verify(signed_pk, server_pk) {
        return IdPk::parse_from_bytes(&verified).ok();
    }
    None
}
```

### 客户端获取服务器公钥

```rust
pub async fn get_rs_pk(id: &str) -> ResultType<(String, sign::PublicKey)> {
    // 从配置或 Rendezvous Server 获取公钥
    let key = Config::get_option("key");
    if !key.is_empty() {
        if let Ok(pk) = base64::decode(&key) {
            if pk.len() == sign::PUBLICKEYBYTES {
                return Ok((key, sign::PublicKey::from_slice(&pk).unwrap()));
            }
        }
    }
    // ... 从服务器获取
}
```

## TCP 连接加密

### 安全 TCP 握手

```rust
// rustdesk/src/common.rs
pub async fn secure_tcp(conn: &mut Stream, key: &str) -> ResultType<()> {
    // 1. 生成临时 X25519 密钥对
    let (our_pk, our_sk) = box_::gen_keypair();

    // 2. 发送我们的公钥
    let mut msg = Message::new();
    msg.set_public_key(PublicKey {
        asymmetric_value: our_pk.0.to_vec().into(),
        ..Default::default()
    });
    conn.send(&msg).await?;

    // 3. 接收对方公钥
    let msg = conn.next_timeout(CONNECT_TIMEOUT).await?
        .ok_or_else(|| anyhow!("timeout"))?;
    let their_pk = msg.get_public_key();

    // 4. 计算共享密钥
    let shared = box_::curve25519xsalsa20poly1305::scalarmult(
        &our_sk,
        &box_::PublicKey::from_slice(&their_pk.asymmetric_value)?,
    );

    // 5. 设置加密
    conn.set_key(secretbox::Key::from_slice(&shared[..32]).unwrap());
    Ok(())
}
```

## 安全特性

### 1. 前向保密

每个会话使用临时密钥对，即使长期密钥泄露，历史会话仍然安全。

### 2. 重放攻击防护

使用递增计数器作为 nonce 的一部分，拒绝旧的或重复的消息。

### 3. 中间人攻击防护

- 服务器签名 Peer 公钥
- 可配置服务器公钥验证

### 4. 密码暴力破解防护

- 使用盐和多次哈希
- 服务器端限流

## 加密算法参数

| 算法 | 密钥大小 | Nonce 大小 | MAC 大小 |
|------|----------|------------|----------|
| Ed25519 | 64 bytes (private), 32 bytes (public) | N/A | 64 bytes |
| X25519 | 32 bytes | N/A | N/A |
| ChaCha20-Poly1305 | 32 bytes | 24 bytes | 16 bytes |

## 密钥生命周期

```
┌─────────────────────────────────────────────────────────────┐
│                     长期密钥 (Ed25519)                        │
│  ┌─────────────────┐                                        │
│  │ 设备首次启动时生成 │                                        │
│  │ 存储在配置文件中  │                                        │
│  └─────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     会话密钥 (X25519)                        │
│  ┌─────────────────┐    ┌─────────────────┐                 │
│  │ 每次连接时生成    │───►│ 用于密钥协商     │                 │
│  │ 临时密钥对       │    │ 派生对称密钥     │                 │
│  └─────────────────┘    └─────────────────┘                 │
│                              │                              │
│                              ▼                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              对称密钥 (ChaCha20-Poly1305)            │   │
│  │              用于会话中的所有消息加密                   │   │
│  │              会话结束时销毁                            │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```
