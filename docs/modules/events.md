# Events 模块文档

## 1. 模块概述

Events 模块提供事件总线功能，用于模块间通信和状态广播。

### 1.1 主要功能

- 事件发布/订阅
- 多订阅者广播
- WebSocket 事件推送
- 状态变更通知

### 1.2 文件结构

```
src/events/
└── mod.rs              # EventBus 实现
```

---

## 2. 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Event System                                        │
└─────────────────────────────────────────────────────────────────────────────┘

┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│     Video      │ │      HID       │ │     Audio      │
│    Module      │ │    Module      │ │    Module      │
└───────┬────────┘ └───────┬────────┘ └───────┬────────┘
        │                  │                  │
        │   publish()      │   publish()      │   publish()
        └──────────────────┼──────────────────┘
                           │
                           ▼
                ┌─────────────────────┐
                │      EventBus       │
                │ (broadcast channel) │
                └──────────┬──────────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         │   subscribe()   │   subscribe()   │
         ▼                 ▼                 ▼
┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│   WebSocket    │ │   DeviceInfo   │ │   Internal     │
│   Handler      │ │  Broadcaster   │ │    Tasks       │
└────────────────┘ └────────────────┘ └────────────────┘
```

---

## 3. 核心组件

### 3.1 EventBus

```rust
pub struct EventBus {
    /// 广播发送器
    tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    /// 创建事件总线
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// 发布事件
    pub fn publish(&self, event: SystemEvent) {
        let _ = self.tx.send(event);
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }
}
```

### 3.2 SystemEvent

```rust
#[derive(Clone, Debug, Serialize)]
pub enum SystemEvent {
    // 视频事件
    StreamStateChanged {
        state: String,
        device: Option<String>,
        resolution: Option<Resolution>,
        fps: Option<f32>,
    },

    VideoDeviceChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },

    // HID 事件
    HidStateChanged {
        backend: String,
        initialized: bool,
        keyboard_connected: bool,
        mouse_connected: bool,
        mouse_mode: String,
        error: Option<String>,
    },

    // MSD 事件
    MsdStateChanged {
        mode: String,
        connected: bool,
        image: Option<String>,
        error: Option<String>,
    },

    MsdDownloadProgress {
        download_id: String,
        downloaded: u64,
        total: u64,
        speed: u64,
    },

    // ATX 事件
    AtxStateChanged {
        power_on: bool,
        last_action: Option<String>,
        error: Option<String>,
    },

    // 音频事件
    AudioStateChanged {
        enabled: bool,
        streaming: bool,
        device: Option<String>,
        error: Option<String>,
    },

    // 配置事件
    ConfigChanged {
        section: String,
    },

    // 设备信息汇总
    DeviceInfo {
        video: VideoInfo,
        hid: HidInfo,
        msd: MsdInfo,
        atx: AtxInfo,
        audio: AudioInfo,
    },

    // 系统错误
    SystemError {
        module: String,
        severity: String,
        message: String,
    },

    // RustDesk 事件
    RustDeskStatusChanged {
        status: String,
        device_id: Option<String>,
        error: Option<String>,
    },

    RustDeskConnectionOpened {
        connection_id: String,
        peer_id: String,
    },

    RustDeskConnectionClosed {
        connection_id: String,
        peer_id: String,
        reason: String,
    },
}
```

---

## 4. 设备信息广播器

在 `main.rs` 中启动的后台任务：

```rust
pub fn spawn_device_info_broadcaster(
    state: Arc<AppState>,
    events: Arc<EventBus>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = events.subscribe();
        let mut debounce = tokio::time::interval(Duration::from_millis(100));
        let mut pending = false;

        loop {
            tokio::select! {
                // 收到事件
                result = rx.recv() => {
                    if result.is_ok() {
                        pending = true;
                    }
                }

                // 防抖定时器
                _ = debounce.tick() => {
                    if pending {
                        pending = false;
                        // 收集设备信息
                        let device_info = state.get_device_info().await;
                        // 广播
                        events.publish(SystemEvent::DeviceInfo(device_info));
                    }
                }
            }
        }
    })
}
```

---

## 5. WebSocket 事件推送

```rust
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.events.subscribe();

    loop {
        tokio::select! {
            // 发送事件给客户端
            result = rx.recv() => {
                if let Ok(event) = result {
                    let json = serde_json::to_string(&event).unwrap();
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }

            // 接收客户端消息
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
```

---

## 6. 使用示例

### 6.1 发布事件

```rust
// 视频模块发布状态变更
events.publish(SystemEvent::StreamStateChanged {
    state: "streaming".to_string(),
    device: Some("/dev/video0".to_string()),
    resolution: Some(Resolution { width: 1920, height: 1080 }),
    fps: Some(30.0),
});

// HID 模块发布状态变更
events.publish(SystemEvent::HidStateChanged {
    backend: "otg".to_string(),
    initialized: true,
    keyboard_connected: true,
    mouse_connected: true,
    mouse_mode: "absolute".to_string(),
    error: None,
});
```

### 6.2 订阅事件

```rust
let mut rx = events.subscribe();

loop {
    match rx.recv().await {
        Ok(SystemEvent::StreamStateChanged { state, .. }) => {
            println!("Stream state: {}", state);
        }
        Ok(SystemEvent::HidStateChanged { backend, .. }) => {
            println!("HID backend: {}", backend);
        }
        Err(_) => break,
    }
}
```

---

## 7. 前端事件处理

```typescript
// 连接 WebSocket
const ws = new WebSocket('/api/ws');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);

    switch (data.type) {
        case 'StreamStateChanged':
            updateStreamStatus(data);
            break;
        case 'HidStateChanged':
            updateHidStatus(data);
            break;
        case 'MsdStateChanged':
            updateMsdStatus(data);
            break;
        case 'DeviceInfo':
            updateAllDevices(data);
            break;
    }
};
```

---

## 8. 最佳实践

### 8.1 事件粒度

- 使用细粒度事件便于精确更新
- DeviceInfo 用于初始化和定期同步

### 8.2 防抖

- 使用 100ms 防抖避免事件风暴
- 合并多个快速变更

### 8.3 错误处理

- 发布失败静默忽略 (fire-and-forget)
- 订阅者断开自动清理
