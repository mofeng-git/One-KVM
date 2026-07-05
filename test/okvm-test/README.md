# One-KVM 自动化产测工具

工具由两部分组成：

- `okvm_testctl.py`：Windows 本地控制端。负责 SSH 操作、One-KVM HTTP API 调用、MJPEG/WebRTC 客户端、HID 检测、MSD 检测、ATX API 检测和报告输出。`run` 子命令只支持原生 Windows，不支持 WSL/Linux。
- `agent/`：Windows 被控机配套程序。使用 Go 编写，编译为单文件 `.exe`。

## 编译 Windows 配套程序

在 Windows PowerShell 安装 Go 后执行：

```powershell
cd test/okvm-test/agent
New-Item -ItemType Directory -Force ..\bin
go build -o ..\bin\okvm-win-agent-amd64.exe .
```

编译产物输出到：

```text
test/okvm-test/bin/okvm-win-agent-amd64.exe
```

将该 exe 放到被控 Windows 机器上，在开始产测前运行：

```powershell
.\okvm-win-agent-amd64.exe -listen 0.0.0.0:8765
```

Windows 配套程序会监听所有网卡地址，本地控制端通过 `--agent-host <Windows测试机IP>` 主动连接它。配套程序默认隐藏测试窗口，只有视频延迟、HID 等需要采集画面/输入时才显示。

## 安装本地控制端依赖

Windows 控制端使用 PowerShell：

```powershell
cd test/okvm-test
py -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
python -m playwright install chrome
```

控制端固定使用 Google Chrome (`chrome`) 跑 WebRTC/H.265 测试，以便使用 Windows 核显/系统解码。所有 Chrome 启动都会加入 `--enable-features=WebRtcAllowH265Receive` 和 `--force-fieldtrials=WebRTC-Video-H26xPacketBuffer/Enabled`。

## 针对当前测试机运行

PowerShell：

```powershell
cd test/okvm-test
$env:OKVM_SSH_PASSWORD="1234"
python okvm_testctl.py run `
  --target 192.168.137.67 `
  --ssh-user root `
  --ssh-password-prompt `
  --reset `
  --agent-host <Windows测试机IP> `
  --agent-port 8765
```

默认流程：

1. 通过 SSH 停止 `one-kvm`。
2. 备份 `/etc/one-kvm/one-kvm.db`。
3. 删除当前数据库，实现强制重置。
4. 启动 `one-kvm`。
5. 采集控制端到目标机的 TCP connect 和 HTTP `/health` 往返延迟，调用初始化接口创建测试账号，登录后发现设备并应用测试配置。
6. 启用 OTG MSD 后，默认把本仓库的 Ventoy 资源同步到目标机 `/etc/one-kvm/ventoy`，并重启一次 `one-kvm` 让资源初始化生效。
7. 执行视频输入/输出、每个视频输入的 MJPEG 端到端画面延迟、HDMI 画面确认、颜色偏差、HID、MSD、ATX API 和性能检测。
8. 输出中文完整报告 `reports/<run-id>/report-YYYYMMDD-HHMMSS.md` 和面向用户的迷你摘要 `reports/<run-id>/user-report-YYYYMMDD-HHMMSS.md`，截图、日志和采集帧保存在 `reports/<run-id>/evidence/`。

终端结果默认使用颜色区分：绿色为 `PASS`，黄色为 `WARN/SKIP`，红色为 `FAIL`。可通过 `--no-color` 或 `NO_COLOR=1` 关闭颜色。

## 视频测试规则

控制端会先通过 SSH 执行 `lsusb -t`，再结合 `/api/devices` 自动选择视频输入：

- USB2.0 采集卡：测试 `1080p30 MJPEG`，再切换 `1080p YUYV` 并选择该分辨率最高帧率；如果没有 1080p YUYV，才退到不超过 1080p 的最高分辨率。
- USB3.0 采集卡：测试 `1080p60 MJPEG`，再切换 `1080p YUYV` 并选择该分辨率最高帧率；如果没有 1080p YUYV，才退到不超过 1080p 的最高分辨率。
- CSI/MIPI：只测试一套 `1080p60 NV12`，不做输入格式切换。

每个输入配置都会跑三种输出：

- MJPEG/HTTP
- H.264 WebRTC
- H.265 WebRTC

默认每个视频输出模式采样 30 秒；可通过 `--sample-seconds <秒数>` 覆盖。

MJPEG/HTTP 测试时，控制端会让 Windows agent 输出默认 60fps 的全屏动态画面，避免静态画面触发 MJPEG “无变化不发帧”策略导致 fps 误判；可通过 `--mjpeg-motion-fps <fps>` 覆盖。

如果浏览器或编码器不支持 H.265，会记录为 `SKIP`。

默认情况下，视频链路只要实际 fps 大于 0 就视为功能成功；报告保留实际 fps、RTT 和 jitter。需要把低帧率作为失败时，加 `--strict-performance`。

## HDMI 采集与视频延迟测试

连接 Windows agent 后，控制端会让 Windows 配套程序打开全屏纯色窗口，然后通过 One-KVM 的 MJPEG/HTTP 或浏览器 WebRTC 解码画面抓帧检测：

- `hdmi_identity`：确认 HDMI 采集画面确实跟随被控 Windows 画面变化，至少验证红、绿、蓝三种高饱和颜色。
- `hdmi_color_range`：采样画面中心区域，输出期望 RGB、实测 RGB 均值、标准差、平均误差和最大通道误差。
- `video_latency_<输入>_mjpeg` / `video_latency_<输入>_h264` / `video_latency_<输入>_h265`：每个被选中的视频输入 case 都会分别执行 MJPEG、H.264、H.265 延迟测试。Windows agent 延迟切换纯色，Windows 控制端从 MJPEG 帧或 Chrome 解码后的 WebRTC video 帧中检测首次颜色变化，输出端到端视觉延迟 p50、p95、max。延迟测试不保存命中帧截图。

纯色采样不会再用第一帧直接判定；控制端会在超时时间内持续拉取 MJPEG，等待目标颜色出现。若始终未匹配，会保存最佳接近帧，并在 Markdown 报告中写出实测 RGB、未命中数量和采样帧数。

默认颜色误差阈值：

- 平均 RGB 误差 `<=30`：`PASS`
- 平均 RGB 误差 `<=60`：`WARN`
- 平均 RGB 误差 `>60`：`FAIL`

默认视频视觉延迟阈值只用于基本功能判定：

- p95 `<=3000ms`：`PASS`
- p95 `>3000ms` 或未检测到目标颜色：`FAIL`

相关开关：

- `--no-hdmi-tests`：跳过 HDMI 画面、颜色和视频视觉延迟测试。
- `--hdmi-capture-timeout <秒>`：单个纯色采样等待目标颜色的最长时间，默认 8 秒。
- `--hdmi-match-frames <帧数>`：目标颜色需要连续匹配的帧数，默认 1；静态 MJPEG 场景建议保持默认。
- `--hdmi-color-warn <level>` / `--hdmi-color-fail <level>`：调整颜色偏差阈值。
- `--video-latency-trials <次数>`：调整切色延迟测试次数，默认 5 次。旧名 `--hdmi-latency-trials` 仍兼容。
- `--video-latency-fail-ms <ms>`：调整基本功能阈值，默认 3000ms。旧名 `--hdmi-latency-fail-ms` 仍兼容。

WebRTC 性能表仍保留连接 RTT/jitter；真实画面级延迟会额外按 MJPEG、H.264、H.265 逐输入输出。

## 网页截图证据

默认保留关键过程网页截图到 `reports/<run-id>/evidence/screenshots/`：

- 登录后控制台首页。
- MJPEG 模式网页截图。
- H.264 WebRTC 模式网页截图。
- H.265 WebRTC 尝试网页截图。
- HID/MSD/ATX 测试后的控制台状态。

视频模式截图会等待页面脱离“等待首帧/连接中”等过渡状态后再保存；H.265 不支持等真实失败页面会直接保留作为证据。等待时间可用 `--screenshot-wait-ms <ms>` 调整，默认 6000ms。

如需跳过网页截图，可加 `--no-screenshots`。

## HID 测试

HID 测试依赖 Windows agent。流程如下：

1. `/hid/status` 确认 HID 后端可用。
2. 键盘矩阵覆盖全部字母、数字、F1-F12，以及不影响系统运行的 Ctrl/Shift + 功能键组合。
3. 鼠标矩阵覆盖绝对坐标移动和相对移动。
4. 键盘鼠标矩阵完成后，使用 F8 做 HID 延迟采样，输出 p50、p95、max。

HID 延迟默认 5 次采样，可通过 `--hid-latency-trials <次数>` 调整。默认 p95 `<=80ms` 为 `PASS`，`<=200ms` 为 `WARN`，更高为 `FAIL`；阈值可用 `--hid-latency-warn-ms <ms>` 和 `--hid-latency-fail-ms <ms>` 调整。

## MSD 测试

MSD 测试依赖 OTG。流程如下：

1. 确认目标机 `/etc/one-kvm/ventoy` 存在 `boot.img`、`core.img`、`ventoy.disk.img`。
2. 如果资源缺失，默认从 `libs/ventoy-img-rs/resources` 解压并通过 SSH/SFTP 同步到目标机。
3. 启用 MSD 后重启 `one-kvm`，确保 Ventoy 资源在服务进程中初始化。
4. 通过 `/api/msd/drive/init` 创建小型虚拟盘。
5. 通过 `/api/msd/connect {"mode":"drive"}` 连接到 Windows。
6. Windows agent 等待新盘符出现。
7. Windows agent 写入测试文件、同步到虚拟盘、优先用未缓存读取读回并校验 SHA-256，同时输出简单写入/读取速度。若 Windows/驱动不支持未缓存读取，会退回缓存读取并在报告中标为“仅校验”，不作为真实读盘速度。
8. 控制端断开 MSD，Windows agent 确认盘符消失。

相关开关：

- `--ventoy-resources-dir <dir>`：指定本地 Ventoy 资源目录。
- `--no-ventoy-sync`：不向目标机同步 Ventoy 资源。
- `--no-msd-restart-after-enable`：启用 MSD 后不重启 `one-kvm`。

## ATX 测试

默认不执行真实电源按键动作，因为当前测试环境不会连接真实 ATX 设备。

默认只验证：

- `/api/atx/status`
- `/api/config/atx`
- `/api/atx/wol`

## 注意事项

- `--reset` 会删除 One-KVM 当前 SQLite 数据库，虽然会先备份，但这是破坏性操作。
- 旧数据库不会自动恢复，备份保存在目标机 `/etc/one-kvm/test-backups/<run-id>/`。
- 编译出的 `.exe` 和测试报告默认被 `.gitignore` 忽略。
