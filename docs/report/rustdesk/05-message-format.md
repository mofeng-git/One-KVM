# 消息格式定义

## 概述

RustDesk 使用 Protocol Buffers (protobuf) 定义所有网络消息格式。主要有两个 proto 文件：

- `rendezvous.proto` - Rendezvous/Relay 服务器通信消息
- `message.proto` - 客户端之间通信消息

## Rendezvous 消息 (rendezvous.proto)

### 顶层消息

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

### 注册相关

```protobuf
// Peer 注册
message RegisterPeer {
  string id = 1;      // Peer ID
  int32 serial = 2;   // 配置序列号
}

message RegisterPeerResponse {
  bool request_pk = 2;  // 是否需要注册公钥
}

// 公钥注册
message RegisterPk {
  string id = 1;       // Peer ID
  bytes uuid = 2;      // 设备 UUID
  bytes pk = 3;        // Ed25519 公钥
  string old_id = 4;   // 旧 ID
}

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
  int32 keep_alive = 2;
}
```

### 连接协调相关

```protobuf
// 连接类型
enum ConnType {
  DEFAULT_CONN = 0;
  FILE_TRANSFER = 1;
  PORT_FORWARD = 2;
  RDP = 3;
  VIEW_CAMERA = 4;
}

// NAT 类型
enum NatType {
  UNKNOWN_NAT = 0;
  ASYMMETRIC = 1;   // 可打洞
  SYMMETRIC = 2;    // 需要中转
}

// Punch Hole 请求
message PunchHoleRequest {
  string id = 1;           // 目标 Peer ID
  NatType nat_type = 2;
  string licence_key = 3;
  ConnType conn_type = 4;
  string token = 5;
  string version = 6;
}

// Punch Hole 响应
message PunchHoleResponse {
  bytes socket_addr = 1;      // 目标地址
  bytes pk = 2;               // 公钥（已签名）
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
    bool is_local = 6;
  }
  string other_failure = 7;
  int32 feedback = 8;
}

// 服务器转发给被控端
message PunchHole {
  bytes socket_addr = 1;      // 控制端地址
  string relay_server = 2;
  NatType nat_type = 3;
}

// 被控端发送给服务器
message PunchHoleSent {
  bytes socket_addr = 1;
  string id = 2;
  string relay_server = 3;
  NatType nat_type = 4;
  string version = 5;
}
```

### Relay 相关

```protobuf
// Relay 请求
message RequestRelay {
  string id = 1;
  string uuid = 2;          // 配对 UUID
  bytes socket_addr = 3;
  string relay_server = 4;
  bool secure = 5;
  string licence_key = 6;
  ConnType conn_type = 7;
  string token = 8;
}

// Relay 响应
message RelayResponse {
  bytes socket_addr = 1;
  string uuid = 2;
  string relay_server = 3;
  oneof union {
    string id = 4;
    bytes pk = 5;
  }
  string refuse_reason = 6;
  string version = 7;
  int32 feedback = 9;
}
```

## 会话消息 (message.proto)

### 顶层消息

```protobuf
message Message {
  oneof union {
    SignedId signed_id = 3;
    PublicKey public_key = 4;
    TestDelay test_delay = 5;
    VideoFrame video_frame = 6;
    LoginRequest login_request = 7;
    LoginResponse login_response = 8;
    Hash hash = 9;
    MouseEvent mouse_event = 10;
    AudioFrame audio_frame = 11;
    CursorData cursor_data = 12;
    CursorPosition cursor_position = 13;
    uint64 cursor_id = 14;
    KeyEvent key_event = 15;
    Clipboard clipboard = 16;
    FileAction file_action = 17;
    FileResponse file_response = 18;
    Misc misc = 19;
    Cliprdr cliprdr = 20;
    MessageBox message_box = 21;
    SwitchSidesResponse switch_sides_response = 22;
    VoiceCallRequest voice_call_request = 23;
    VoiceCallResponse voice_call_response = 24;
    PeerInfo peer_info = 25;
    PointerDeviceEvent pointer_device_event = 26;
    Auth2FA auth_2fa = 27;
    MultiClipboards multi_clipboards = 28;
  }
}
```

### 认证相关

```protobuf
// ID 和公钥
message IdPk {
  string id = 1;
  bytes pk = 2;
}

// 密钥交换
message PublicKey {
  bytes asymmetric_value = 1;  // X25519 公钥
  bytes symmetric_value = 2;   // 加密的对称密钥
}

// 签名的 ID
message SignedId {
  bytes id = 1;  // 签名的 IdPk
}

// 密码哈希挑战
message Hash {
  string salt = 1;
  string challenge = 2;
}

// 登录请求
message LoginRequest {
  string username = 1;
  bytes password = 2;         // 加密的密码
  string my_id = 4;
  string my_name = 5;
  OptionMessage option = 6;
  oneof union {
    FileTransfer file_transfer = 7;
    PortForward port_forward = 8;
    ViewCamera view_camera = 15;
  }
  bool video_ack_required = 9;
  uint64 session_id = 10;
  string version = 11;
  OSLogin os_login = 12;
  string my_platform = 13;
  bytes hwid = 14;
}

// 登录响应
message LoginResponse {
  oneof union {
    string error = 1;
    PeerInfo peer_info = 2;
  }
  bool enable_trusted_devices = 3;
}

// 2FA 认证
message Auth2FA {
  string code = 1;
  bytes hwid = 2;
}
```

### 视频相关

```protobuf
// 编码后的视频帧
message EncodedVideoFrame {
  bytes data = 1;
  bool key = 2;     // 是否关键帧
  int64 pts = 3;    // 时间戳
}

message EncodedVideoFrames {
  repeated EncodedVideoFrame frames = 1;
}

// 视频帧
message VideoFrame {
  oneof union {
    EncodedVideoFrames vp9s = 6;
    RGB rgb = 7;
    YUV yuv = 8;
    EncodedVideoFrames h264s = 10;
    EncodedVideoFrames h265s = 11;
    EncodedVideoFrames vp8s = 12;
    EncodedVideoFrames av1s = 13;
  }
  int32 display = 14;  // 显示器索引
}

// 显示信息
message DisplayInfo {
  sint32 x = 1;
  sint32 y = 2;
  int32 width = 3;
  int32 height = 4;
  string name = 5;
  bool online = 6;
  bool cursor_embedded = 7;
  Resolution original_resolution = 8;
  double scale = 9;
}
```

### 输入相关

```protobuf
// 鼠标事件
message MouseEvent {
  int32 mask = 1;      // 按钮掩码
  sint32 x = 2;
  sint32 y = 3;
  repeated ControlKey modifiers = 4;
}

// 键盘事件
message KeyEvent {
  bool down = 1;       // 按下/释放
  bool press = 2;      // 单击
  oneof union {
    ControlKey control_key = 3;
    uint32 chr = 4;         // 字符码
    uint32 unicode = 5;     // Unicode
    string seq = 6;         // 字符序列
    uint32 win2win_hotkey = 7;
  }
  repeated ControlKey modifiers = 8;
  KeyboardMode mode = 9;
}

// 键盘模式
enum KeyboardMode {
  Legacy = 0;
  Map = 1;
  Translate = 2;
  Auto = 3;
}

// 控制键枚举（部分）
enum ControlKey {
  Unknown = 0;
  Alt = 1;
  Backspace = 2;
  CapsLock = 3;
  Control = 4;
  Delete = 5;
  // ... 更多按键
  CtrlAltDel = 100;
  LockScreen = 101;
}
```

### 音频相关

```protobuf
// 音频格式
message AudioFormat {
  uint32 sample_rate = 1;
  uint32 channels = 2;
}

// 音频帧
message AudioFrame {
  bytes data = 1;  // Opus 编码数据
}
```

### 剪贴板相关

```protobuf
// 剪贴板格式
enum ClipboardFormat {
  Text = 0;
  Rtf = 1;
  Html = 2;
  ImageRgba = 21;
  ImagePng = 22;
  ImageSvg = 23;
  Special = 31;
}

// 剪贴板内容
message Clipboard {
  bool compress = 1;
  bytes content = 2;
  int32 width = 3;
  int32 height = 4;
  ClipboardFormat format = 5;
  string special_name = 6;
}

message MultiClipboards {
  repeated Clipboard clipboards = 1;
}
```

### 文件传输相关

```protobuf
// 文件操作
message FileAction {
  oneof union {
    ReadDir read_dir = 1;
    FileTransferSendRequest send = 2;
    FileTransferReceiveRequest receive = 3;
    FileDirCreate create = 4;
    FileRemoveDir remove_dir = 5;
    FileRemoveFile remove_file = 6;
    ReadAllFiles all_files = 7;
    FileTransferCancel cancel = 8;
    FileTransferSendConfirmRequest send_confirm = 9;
    FileRename rename = 10;
    ReadEmptyDirs read_empty_dirs = 11;
  }
}

// 文件响应
message FileResponse {
  oneof union {
    FileDirectory dir = 1;
    FileTransferBlock block = 2;
    FileTransferError error = 3;
    FileTransferDone done = 4;
    FileTransferDigest digest = 5;
    ReadEmptyDirsResponse empty_dirs = 6;
  }
}

// 文件传输块
message FileTransferBlock {
  int32 id = 1;
  sint32 file_num = 2;
  bytes data = 3;
  bool compressed = 4;
  uint32 blk_id = 5;
}

// 文件条目
message FileEntry {
  FileType entry_type = 1;
  string name = 2;
  bool is_hidden = 3;
  uint64 size = 4;
  uint64 modified_time = 5;
}
```

### 杂项消息

```protobuf
message Misc {
  oneof union {
    ChatMessage chat_message = 4;
    SwitchDisplay switch_display = 5;
    PermissionInfo permission_info = 6;
    OptionMessage option = 7;
    AudioFormat audio_format = 8;
    string close_reason = 9;
    bool refresh_video = 10;
    bool video_received = 12;
    BackNotification back_notification = 13;
    bool restart_remote_device = 14;
    // ... 更多选项
  }
}

// Peer 信息
message PeerInfo {
  string username = 1;
  string hostname = 2;
  string platform = 3;
  repeated DisplayInfo displays = 4;
  int32 current_display = 5;
  bool sas_enabled = 6;
  string version = 7;
  Features features = 9;
  SupportedEncoding encoding = 10;
  SupportedResolutions resolutions = 11;
  string platform_additions = 12;
  WindowsSessions windows_sessions = 13;
}

// 选项消息
message OptionMessage {
  enum BoolOption {
    NotSet = 0;
    No = 1;
    Yes = 2;
  }
  ImageQuality image_quality = 1;
  BoolOption lock_after_session_end = 2;
  BoolOption show_remote_cursor = 3;
  BoolOption privacy_mode = 4;
  BoolOption block_input = 5;
  int32 custom_image_quality = 6;
  BoolOption disable_audio = 7;
  BoolOption disable_clipboard = 8;
  BoolOption enable_file_transfer = 9;
  SupportedDecoding supported_decoding = 10;
  int32 custom_fps = 11;
  // ... 更多选项
}
```

## 消息编码

### 长度前缀

TCP 传输时使用长度前缀编码：

```rust
// hbb_common/src/bytes_codec.rs
pub struct BytesCodec {
    state: DecodeState,
    raw: bool,
}

impl Decoder for BytesCodec {
    type Item = BytesMut;
    type Error = std::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, Self::Error> {
        if self.raw {
            // 原始模式：直接返回数据
            if buf.is_empty() {
                Ok(None)
            } else {
                Ok(Some(buf.split()))
            }
        } else {
            // 标准模式：4 字节长度前缀 + 数据
            match self.state {
                DecodeState::Head => {
                    if buf.len() < 4 {
                        return Ok(None);
                    }
                    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                    buf.advance(4);
                    self.state = DecodeState::Data(len);
                    self.decode(buf)
                }
                DecodeState::Data(len) => {
                    if buf.len() < len {
                        return Ok(None);
                    }
                    let data = buf.split_to(len);
                    self.state = DecodeState::Head;
                    Ok(Some(data))
                }
            }
        }
    }
}
```

### 加密模式

当启用加密时，消息结构为：

```
┌─────────────┬─────────────┬─────────────────────────┐
│  Length(4)  │  Nonce(8)   │  Encrypted Data(N)      │
└─────────────┴─────────────┴─────────────────────────┘
```
