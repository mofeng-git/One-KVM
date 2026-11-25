# One-KVM Plugin Developer Guide

[简体中文](README.md)

A One-KVM plugin is a standalone program with a `manifest.json` file. The manifest tells One-KVM what the plugin is called, where it appears, and which settings and buttons to show. The program does the work.

In this guide, you'll build a greeting demo with a text setting and a button that counts clicks. Then you'll package and install it like any other plugin.

## 1. Create a project

You'll need Rust, Cargo, Python 3, and a Bash shell. Start in the One-KVM repository root. These commands create a separate project outside the repository.

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

## 2. Write the plugin

Replace `src/main.rs` with this code.

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

`run` connects the plugin to One-KVM. The host calls `status` regularly to refresh the page and calls `action` when someone clicks a button. To stop the plugin, the host closes standard input and the SDK calls `shutdown`. You can use that method to stop any background tasks you started.

## 3. Write the manifest

Create `manifest.json` in the root of the demo project.

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

- `abi` is the protocol version the plugin shares with the host. Use `1` for the current protocol. Put your plugin's release version in `version`.
- `id` identifies the plugin. Start with a lowercase letter and use lowercase letters, digits, hyphens, and underscores. Keep it to 64 characters or fewer. Reuse the same ID for upgrades.
- Fill in `name`, `version`, and `author`. The plugin list shows all three. Use a non-empty string for `author`.
- Match the keys in the manifest to your code. Here, the configuration key `message` maps to `Config.message`, and the status key `clicks` comes from `status`. The button ID is the value passed to `action`.

### Menus and translations

Set `menu` to `"remote_access"` to put the plugin under **Remote Access**. That menu is visible when at least one installed plugin uses it. Use `"third_party_access"` for **Third-Party Access** or `"console_toolbar"` for the console toolbar. You can also use the ID of an existing settings page.

The demo creates a custom menu. One-KVM adds **Plugin Examples** or **插件示例** to the settings sidebar and shows the plugin card on that page. Plugins with the same menu ID share one page. The menu stays visible when a plugin is stopped. It disappears when the last plugin using that ID is uninstalled.

Names, descriptions, and labels can be plain strings. To provide translations, include both `zh-CN` and `en-US` as shown above. If the current locale has no translation, One-KVM looks for a match in the same language and then tries English.

## 4. Build, package, and install

Run these commands from the demo project root. They build a package for testing on your own machine.

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

The packaging script uses `--target` to fill in `os` and `arch`. For another device, build for that device and pass the same target to the packaging script. The executable also needs to match the device's runtime environment, such as musl on OpenWrt.

1. Open **Settings → Plugins** and upload the `.onepkg` file.
2. Open **Plugin Examples** in the sidebar. Edit the greeting and click **Start**.
3. Check that the new greeting appears. Click **Count a click** and wait for the count to update.
4. Click **Stop**, then start the plugin again. The demo keeps its count in memory, so it resets to zero.

Installing another package with the same ID upgrades the existing plugin and keeps its saved settings and data. Before releasing an upgrade, try installing it over a copy with existing settings.

## Building your next plugin

| What you want to do | Where to start |
| --- | --- |
| Add text fields, switches, or dropdowns | Add entries to `config_schema`. Common types are `text`, `password`, `textarea`, `integer`, `boolean`, `select`, and `string_list`. Each default value must have the right type. |
| Keep data across restarts | Write it under `client.data`. Upgrades keep this directory, and uninstalling clears it. Read settings with `client.read_config()`. When settings change, a running plugin restarts and reads the new values. |
| Receive video or audio, or send keyboard and mouse input | Declare what you need in `capabilities` and use the [SDK's `Client`](src/runtime.rs). For example, video subscriptions need `video`, and keyboard or mouse input needs `hid`. |
| Wrap an existing program | Use the SDK's `embedded` feature to manage the child process. See the [GOSTC plugin](../../plugins/gostc/src/main.rs) for an example. |
| Let users download files from the plugin | Set `"files": true`, write files to `client.data/files/`, and return an array under the `files` status key. Each entry has a `name` and can include a `display_name`. |
| Save files directly to virtual media storage | Declare the `images` capability and call `client.images_dir()` for the current path. To let users download or delete the files from the plugin UI, set both `"files": true` and `"files_in_images": true` and list the files your plugin owns. See the [AirFerry plugin](../../plugins/airferry/src/main.rs). |

### Troubleshooting

- **Upload fails.** Check the ABI, author, target OS, and architecture. Make sure the manifest is valid JSON and each default value matches its declared type.
- **The settings page is missing.** Check `menu` in the manifest. Toolbar plugins appear in the console.
- **The plugin keeps restarting.** Open the logs on its plugin card. One-KVM collects SDK errors and the plugin's standard output and standard error.
- **A status value or button is missing.** Declare the status key in `status_fields` and the button ID in `actions`. Buttons are available while the plugin is running.

See the [manifest types](src/lib.rs) and the [existing plugins](../../plugins/) for more details.
