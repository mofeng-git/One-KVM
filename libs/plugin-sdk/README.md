# One-KVM 插件开发者指南

[English](README.en.md)

One-KVM 插件就是一个独立程序，加上一份 `manifest.json`。清单告诉 One-KVM 插件叫什么、放在哪个菜单、有哪些设置和按钮。程序负责真正的工作。

下面做一个「问候示例」插件。它有一个可编辑的问候语，还有一个按钮，用来记录你点了几次。写完后，你可以把它当成普通插件安装。

## 1. 创建项目

需要 Rust、Cargo 和 Python 3。以下命令使用 Bash。先进入 One-KVM 仓库根目录，示例项目会建在仓库外，可以独立构建。

```sh
repo="$(pwd)"
demo_dir="$(mktemp -d)"
cargo new --bin "$demo_dir/hello-plugin"
cd "$demo_dir/hello-plugin"

cargo add one-kvm-plugin-sdk --path "$repo/libs/plugin-sdk" --features runtime
cargo add anyhow serde_json
cargo add serde --features derive
cargo add tokio --features macros,rt-multi-thread
```

## 2. 写插件程序

把 `src/main.rs` 换成下面的内容。

```rust
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{bail, Result};
use one_kvm_plugin_sdk::{runtime::{run, Service}, Status};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Config {
    message: String,
}

struct Hello {
    message: String,
    clicks: AtomicU32,
}

impl Service for Hello {
    async fn status(&self) -> Result<Status> {
        Ok(Status {
            fields: [
                ("message".into(), json!(self.message)),
                ("clicks".into(), json!(self.clicks.load(Ordering::Relaxed))),
            ]
            .into_iter()
            .collect(),
            ..Status::default()
        })
    }

    async fn action(&self, id: &str) -> Result<Value> {
        if id != "count_click" {
            bail!("Unknown action: {id}");
        }

        self.clicks.fetch_add(1, Ordering::Relaxed);
        Ok(Value::Null)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config: Config = client.read_config().await?;
        Ok(Hello {
            message: config.message,
            clicks: AtomicU32::new(0),
        })
    })
    .await
}
```

`run` 会和 One-KVM 建立连接。宿主定期调用 `status` 刷新页面，点击按钮时调用 `action`。插件停止时，宿主会关闭标准输入，SDK 会调用 `shutdown`。如果插件自己开了后台任务，可以在这里收尾。

## 3. 写插件清单

在示例项目根目录创建 `manifest.json`。

```json
{
  "abi": 1,
  "id": "hello-plugin",
  "name": { "zh-CN": "问候示例", "en-US": "Greeting Demo" },
  "version": "0.1.0",
  "author": "Your Name",
  "description": {
    "zh-CN": "显示一条问候语，记录按钮点击次数。",
    "en-US": "Show a greeting and count button clicks."
  },
  "menu": {
    "id": "plugin-examples",
    "label": { "zh-CN": "插件示例", "en-US": "Plugin Examples" }
  },
  "config_schema": [
    {
      "key": "message",
      "label": { "zh-CN": "问候语", "en-US": "Greeting" },
      "kind": "text",
      "default": "你好，One-KVM！"
    }
  ],
  "status_fields": [
    { "key": "message", "label": { "zh-CN": "当前问候", "en-US": "Current greeting" } },
    { "key": "clicks", "label": { "zh-CN": "点击次数", "en-US": "Clicks" } }
  ],
  "actions": [
    { "id": "count_click", "label": { "zh-CN": "计数加一", "en-US": "Count a click" } }
  ]
}
```

注意以下几处。

- `abi` 是宿主和插件沟通的协议版本。当前填 `1`。插件自己的发布版本填在 `version`。
- `id` 是插件身份。用小写字母开头，之后可以用小写字母、数字、短横线和下划线，最多 64 个字符。升级时沿用同一个 ID。
- `name`、`version` 和 `author` 都要填写。插件列表会显示这三项。`author` 填一个非空字符串。
- 清单里的配置键和状态键要与代码一致。比如 `message` 对应 `Config.message`，`clicks` 对应 `status` 返回的键。按钮的 `id` 对应 `action` 收到的 ID。

### 菜单和中英文

把 `menu` 写成 `"remote_access"`，插件就会出现在「远程访问」。没有插件使用这个菜单时，侧栏会隐藏它。`"third_party_access"` 对应「第三方接入」，`"console_toolbar"` 对应控制台工具栏。也可以指定现有设置页的 ID。

示例用的是自定义菜单。One-KVM 会在设置侧栏添加「插件示例」或「Plugin Examples」，并在对应页面显示插件卡片。使用同一个菜单 ID 的插件会放在同一页。卸载最后一个使用该 ID 的插件后，菜单会消失。停用插件时，菜单仍会保留，方便修改配置。

名称、描述和标签可以写成普通字符串。需要翻译时，像示例一样同时写 `zh-CN` 和 `en-US`。缺少当前语言时，页面会尝试同语种的文字，再尝试英文。

## 4. 打包并安装

在示例项目根目录运行下面的命令。这里在本机编译，包也用于本机测试。

```sh
target="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release --target "$target"

binary="target/$target/release/hello-plugin"
case "$target" in *windows*) binary="$binary.exe" ;; esac

python3 "$repo/scripts/pack-plugin.py" \
  --manifest manifest.json \
  --binary "$binary" \
  --target "$target" \
  --output "hello-plugin-$target.onepkg"
```

打包脚本根据 `--target` 填写 `os` 和 `arch`。如果插件要在另一台设备上运行，先用那台设备对应的目标编译，并在打包时传入同一个目标。可执行文件需要兼容设备的运行环境，例如 OpenWrt 上的 musl。

1. 打开 **设置 → 插件**，上传生成的 `.onepkg`。
2. 打开侧栏的 **插件示例**，修改问候语，然后点 **启动**。
3. 确认页面显示新的问候语。点 **计数加一**，等状态刷新后查看次数。
4. 点 **停止**，再启动一次。示例的点击次数保存在内存里，重启后会归零。

同一个 ID 的包再次安装会升级原插件，并保留已保存的配置和数据。发布升级包前，试一次带着旧配置升级的流程。

## 做下一个插件时

| 想做什么 | 从哪里开始 |
| --- | --- |
| 加输入框、开关和下拉框 | 在 `config_schema` 加字段。常用类型有 `text`、`password`、`textarea`、`integer`、`boolean`、`select` 和 `string_list`。每个默认值都要和类型匹配。 |
| 保存重启后还要用的数据 | 写到 `client.data` 下。升级时保留，卸载时清理。配置用 `client.read_config()` 读取。保存新配置时，正在运行的插件会重启并重新读取配置。 |
| 接收视频、音频或控制键鼠 | 在 `capabilities` 中声明需要的能力，然后调用 [SDK 的 `Client`](src/runtime.rs)。例如订阅视频需要 `video`，发送键鼠需要 `hid`。 |
| 把现成的程序做成插件 | 用 SDK 的 `embedded` 功能管理子进程。可以参考 [GOSTC 插件](../../plugins/gostc/src/main.rs)。 |
| 让用户下载插件生成的文件 | 设置 `"files": true`，把文件放进 `client.data/files/`，并在状态的 `files` 中返回文件列表。每项包含 `name`，也可以带 `display_name`。参考 [AirFerry 插件](../../plugins/airferry/src/main.rs)。 |

### 遇到问题先看这里

- **上传失败时**，核对 ABI、作者、目标系统和架构。确认清单是合法的 JSON，字段默认值和声明的类型一致。
- **装了却没看到配置页时**，查看清单的 `menu`。工具栏插件的入口在控制台。
- **点击启动后一直重试时**，打开插件卡片里的日志。宿主会收集 SDK 的错误和插件的标准输出、标准错误。
- **页面缺少状态或按钮时**，确认状态键写在 `status_fields`，按钮 ID 写在 `actions`。按钮在插件运行时才可点击。

更多细节可以看 [清单类型](src/lib.rs)、[现有插件](../../plugins/)和 [One-KVM 插件说明](../../docs/plugins.md)。
