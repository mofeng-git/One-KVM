# Auth 模块文档

## 1. 模块概述

Auth 模块提供用户认证和会话管理功能。

### 1.1 主要功能

- 用户管理
- 密码哈希 (Argon2)
- 会话管理
- 认证中间件
- 权限控制

### 1.2 文件结构

```
src/auth/
├── mod.rs              # 模块导出
├── user.rs             # 用户管理 (5KB)
├── session.rs          # 会话管理 (4KB)
├── password.rs         # 密码哈希 (1KB)
└── middleware.rs       # 中间件 (4KB)
```

---

## 2. 核心组件

### 2.1 UserStore (user.rs)

```rust
pub struct UserStore {
    db: Pool<Sqlite>,
}

impl UserStore {
    /// 创建存储
    pub async fn new(db: Pool<Sqlite>) -> Result<Self>;

    /// 创建用户
    pub async fn create_user(&self, user: &CreateUser) -> Result<User>;

    /// 获取用户
    pub async fn get_user(&self, id: &str) -> Result<Option<User>>;

    /// 按用户名获取
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>>;

    /// 更新用户
    pub async fn update_user(&self, id: &str, update: &UpdateUser) -> Result<()>;

    /// 删除用户
    pub async fn delete_user(&self, id: &str) -> Result<()>;

    /// 列出用户
    pub async fn list_users(&self) -> Result<Vec<User>>;

    /// 验证密码
    pub async fn verify_password(&self, username: &str, password: &str) -> Result<Option<User>>;

    /// 更新密码
    pub async fn update_password(&self, id: &str, new_password: &str) -> Result<()>;

    /// 检查是否需要初始化
    pub async fn needs_setup(&self) -> Result<bool>;
}

pub struct User {
    pub id: String,
    pub username: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

pub enum UserRole {
    Admin,
    User,
}

pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub role: UserRole,
}
```

### 2.2 SessionStore (session.rs)

```rust
pub struct SessionStore {
    /// 会话映射
    sessions: RwLock<HashMap<String, Session>>,

    /// 会话超时
    timeout: Duration,
}

impl SessionStore {
    /// 创建存储
    pub fn new(timeout: Duration) -> Self;

    /// 创建会话
    pub fn create_session(&self, user: &User) -> String;

    /// 获取会话
    pub fn get_session(&self, token: &str) -> Option<Session>;

    /// 删除会话
    pub fn delete_session(&self, token: &str);

    /// 清理过期会话
    pub fn cleanup_expired(&self);

    /// 刷新会话
    pub fn refresh_session(&self, token: &str) -> bool;
}

pub struct Session {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub created_at: Instant,
    pub last_active: Instant,
}
```

### 2.3 密码哈希 (password.rs)

```rust
/// 哈希密码
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

/// 验证密码
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
```

### 2.4 认证中间件 (middleware.rs)

```rust
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    request: Request,
    next: Next,
) -> Response {
    // 获取 session token
    let token = cookies
        .get("session_id")
        .map(|c| c.value().to_string());

    // 验证会话
    let session = token
        .and_then(|t| state.sessions.get_session(&t));

    if let Some(session) = session {
        // 将用户信息注入请求
        let mut request = request;
        request.extensions_mut().insert(session);
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

pub async fn admin_middleware(
    session: Extension<Session>,
    request: Request,
    next: Next,
) -> Response {
    if session.role == UserRole::Admin {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}
```

---

## 3. API 端点

| 端点 | 方法 | 权限 | 描述 |
|------|------|------|------|
| `/api/auth/login` | POST | Public | 登录 |
| `/api/auth/logout` | POST | User | 登出 |
| `/api/auth/check` | GET | User | 检查认证 |
| `/api/auth/password` | POST | User | 修改密码 |
| `/api/users` | GET | Admin | 列出用户 |
| `/api/users` | POST | Admin | 创建用户 |
| `/api/users/:id` | DELETE | Admin | 删除用户 |
| `/api/setup/init` | POST | Public | 初始化设置 |

### 请求/响应格式

```json
// POST /api/auth/login
// Request:
{
    "username": "admin",
    "password": "password123"
}

// Response:
{
    "user": {
        "id": "uuid",
        "username": "admin",
        "role": "admin"
    }
}

// GET /api/auth/check
{
    "authenticated": true,
    "user": {
        "id": "uuid",
        "username": "admin",
        "role": "admin"
    }
}
```

---

## 4. 配置

```rust
#[derive(Serialize, Deserialize)]
#[typeshare]
pub struct AuthConfig {
    /// 会话超时 (秒)
    pub session_timeout_secs: u64,

    /// 是否启用认证
    pub enabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_timeout_secs: 86400,  // 24 小时
            enabled: true,
        }
    }
}
```

---

## 5. 安全特性

### 5.1 密码存储

- Argon2id 哈希
- 随机盐值
- 不可逆

### 5.2 会话安全

- 随机 token (UUID v4)
- HTTPOnly Cookie
- 会话超时
- 自动清理

### 5.3 权限控制

- 两级权限: Admin / User
- 中间件检查
- 敏感操作需 Admin

---

## 6. 使用示例

```rust
// 创建用户
let user = users.create_user(&CreateUser {
    username: "admin".to_string(),
    password: "password123".to_string(),
    role: UserRole::Admin,
}).await?;

// 验证密码
if let Some(user) = users.verify_password("admin", "password123").await? {
    // 创建会话
    let token = sessions.create_session(&user);

    // 设置 Cookie
    cookies.add(Cookie::build("session_id", token)
        .http_only(true)
        .path("/")
        .finish());
}

// 获取会话
if let Some(session) = sessions.get_session(&token) {
    println!("User: {}", session.username);
}
```

---

## 7. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("User already exists")]
    UserExists,

    #[error("Session expired")]
    SessionExpired,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Setup required")]
    SetupRequired,
}
```
