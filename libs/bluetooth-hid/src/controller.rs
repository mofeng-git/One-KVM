//! Exclusive controller ownership. No BLE switching or power cycling.
use std::{
    fs::{File, OpenOptions},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;
pub struct Controller {
    _lock: File,
    adapter: String,
    old_class: (u8, u8),
    old_connectable: bool,
}
async fn run_once(adapter: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("btmgmt");
    cmd.arg("--index")
        .arg(adapter.trim_start_matches("hci"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Install bluez/btmgmt: {e}"))?;
    // BlueZ 5.55's btmgmt exits on stdin EOF, even with a command argument.
    // Keep a pipe open until it finishes; systemd normally supplies /dev/null.
    let stdin = child.stdin.take();
    let result = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .map_err(|_| "btmgmt timeout")?
        .map_err(|e| format!("Install bluez/btmgmt: {e}"))?;
    drop(stdin);
    let text = String::from_utf8_lossy(&result.stdout).into_owned();
    if !result.status.success()
        || !result.stderr.is_empty()
        || text.to_lowercase().contains("failed")
    {
        return Err(format!(
            "btmgmt {}: {text} {}",
            args.join(" "),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(text)
}
async fn run(adapter: &str, args: &[&str]) -> Result<String, String> {
    // BlueZ updates the EIR/class asynchronously after registering SDP.
    // Kernel management rejects a simultaneous class change with Busy.
    for attempt in 0..5 {
        match run_once(adapter, args).await {
            Err(error) if error.contains("(Busy)") && attempt < 4 => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            result => return result,
        }
    }
    unreachable!()
}
impl Controller {
    pub async fn acquire(adapter: &str) -> Result<Self, String> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(format!("/run/one-kvm-bluetooth-{adapter}.lock"))
            .map_err(|e| e.to_string())?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Bluetooth adapter already owned by One-KVM".into());
        }
        let info = run(adapter, &["info"]).await?;
        let settings = info
            .lines()
            .find_map(|l| l.trim().strip_prefix("current settings:"))
            .ok_or("Cannot read Bluetooth settings")?;
        if !settings.split_whitespace().any(|s| s == "br/edr") {
            return Err(
                "Classic Bluetooth disabled; enable BR/EDR with btmgmt before using HID".into(),
            );
        }
        let class = info
            .split_whitespace()
            .skip_while(|s| *s != "class")
            .nth(1)
            .ok_or("Missing Bluetooth class")?;
        let class =
            u32::from_str_radix(class.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
        Ok(Self {
            _lock: lock,
            adapter: adapter.into(),
            old_class: (((class >> 8) & 0x1f) as u8, (class & 0xfc) as u8),
            old_connectable: settings.split_whitespace().any(|s| s == "connectable"),
        })
    }
    pub async fn configure(&self) -> Result<(), String> {
        run(&self.adapter, &["class", "5", "192"]).await?;
        run(&self.adapter, &["connectable", "on"]).await?;
        Ok(())
    }
    pub async fn restore(&self) -> Result<(), String> {
        let mut errors = vec![];
        if let Err(e) = run(
            &self.adapter,
            &[
                "class",
                &self.old_class.0.to_string(),
                &self.old_class.1.to_string(),
            ],
        )
        .await
        {
            errors.push(e);
        }
        if let Err(e) = run(
            &self.adapter,
            &[
                "connectable",
                if self.old_connectable { "on" } else { "off" },
            ],
        )
        .await
        {
            errors.push(e);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}
