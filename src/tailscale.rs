//! Tailscale 集成。
//!
//! 与其它扩展（ttyd/frpc/easytier）不同，Tailscale 需要一个常驻守护进程
//! `tailscaled`，并通过 `tailscale` CLI 控制。所以这里直接委托系统的
//! tailscaled.service，one-kvm 只负责：开关、填认证信息、读状态。

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::extensions::TailscaleConfig;

pub const BIN: &str = "/usr/bin/tailscale";
const SOCKET: &str = "/run/tailscale/tailscaled.sock";

#[derive(Debug, Clone, Serialize, Default)]
pub struct TailscaleStatus {
    /// 二进制是否存在（镜像里是否带了 tailscale）
    pub installed: bool,
    /// BackendState 是否为 Running
    pub running: bool,
    /// BackendState：NoState/NeedsLogin/Starting/Running/Stopped…
    pub state: String,
    /// 本机 tailnet IP（100.x.y.z）
    pub ip: Option<String>,
    pub hostname: Option<String>,
    /// 未认证时，需要用户点开的登录链接
    pub login_url: Option<String>,
}

static LOGIN_URL: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn login_url_cell() -> &'static Mutex<Option<String>> {
    LOGIN_URL.get_or_init(|| Mutex::new(None))
}

fn set_login_url(value: Option<String>) {
    if let Ok(mut guard) = login_url_cell().lock() {
        *guard = value;
    }
}

fn login_url() -> Option<String> {
    login_url_cell().lock().ok().and_then(|guard| guard.clone())
}

pub fn installed() -> bool {
    std::path::Path::new(BIN).exists()
}

async fn run(args: &[&str]) -> Result<String, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(BIN).args(args).output(),
    )
    .await
    .map_err(|_| "tailscale 命令超时".to_string())?
    .map_err(|error| format!("执行 tailscale 失败: {error}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

pub async fn status() -> TailscaleStatus {
    let mut status = TailscaleStatus {
        installed: installed(),
        state: "unknown".to_string(),
        ..Default::default()
    };

    if !status.installed {
        status.state = "not_installed".to_string();
        return status;
    }

    match run(&["status", "--json"]).await {
        Ok(json) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                status.state = value
                    .get("BackendState")
                    .and_then(|item| item.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                status.running = status.state == "Running";
                status.hostname = value
                    .pointer("/Self/HostName")
                    .and_then(|item| item.as_str())
                    .map(str::to_string);
                status.ip = value
                    .pointer("/Self/TailscaleIPs/0")
                    .and_then(|item| item.as_str())
                    .map(str::to_string);
            }
            if status.state == "NeedsLogin" {
                status.login_url = login_url();
            }
        }
        Err(error) => {
            if error.contains("failed to connect to local tailscaled") {
                status.state = "daemon_stopped".to_string();
            } else {
                status.state = "error".to_string();
                tracing::warn!("读取 Tailscale 状态失败: {error}");
            }
        }
    }

    status
}

/// 后台跑 `tailscale up`（无 authkey 时），从 stderr 里抓登录链接。
async fn spawn_login(args: Vec<String>) {
    set_login_url(None);

    let mut child = match Command::new(BIN)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!("启动 tailscale up 失败: {error}");
            return;
        }
    };

    let stderr = child.stderr.take();
    tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(url) = line.split_whitespace().find(|part| part.starts_with("https://")) {
                    tracing::info!("Tailscale 登录链接: {url}");
                    set_login_url(Some(url.trim_end_matches('.').to_string()));
                }
            }
        }
        // tailscale up 认证完成后会自行退出
        let _ = child.wait().await;
    });
}

pub async fn apply(config: &TailscaleConfig) -> Result<(), String> {
    if !installed() {
        return Err(format!("未找到 {BIN}，请先在镜像/系统里安装 tailscale"));
    }

    if !config.enabled {
        // 只下线，不停止守护进程：停掉后连状态都读不到，开关就会来回抽风
        let _ = run(&["down"]).await;
        set_login_url(None);
        return Ok(());
    }

    // 拉起守护进程
    let output = Command::new("systemctl")
        .args(["enable", "--now", "tailscaled"])
        .output()
        .await
        .map_err(|error| format!("启动 tailscaled 失败: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "启动 tailscaled 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    for _ in 0..40 {
        if std::path::Path::new(SOCKET).exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let mut args = vec![
        "up".to_string(),
        "--accept-routes".to_string(),
        "--reset".to_string(),
    ];

    if !config.hostname.trim().is_empty() {
        args.push("--hostname".to_string());
        args.push(config.hostname.trim().to_string());
    }
    if !config.login_server.trim().is_empty() {
        args.push("--login-server".to_string());
        args.push(config.login_server.trim().to_string());
    }

    if !config.auth_key.trim().is_empty() {
        args.push("--authkey".to_string());
        args.push(config.auth_key.trim().to_string());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run(&refs).await?;
        set_login_url(None);
    } else {
        spawn_login(args).await;
    }

    Ok(())
}

pub async fn login_url_public() -> Option<String> {
    login_url()
}
