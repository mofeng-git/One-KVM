use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::boot::parse_mkisofs_boot_args;
use crate::error::{AppError, MsdErrorCode, Result};

const XORRISO_TIMEOUT: Duration = Duration::from_secs(3600);
const MOUNT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_PROCESS_LOG_LINES: usize = 20;
const MAX_PROCESS_LINE_BYTES: usize = 2048;
pub(super) const EL_TORITO_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MountType {
    Udf,
    Iso9660,
}

impl MountType {
    fn as_fstype(self) -> &'static str {
        match self {
            Self::Udf => "udf",
            Self::Iso9660 => "iso9660",
        }
    }
}

pub(super) struct MountGuard {
    path: PathBuf,
    mounted: bool,
}

impl MountGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            mounted: false,
        }
    }

    pub(super) fn mark_mounted(&mut self) {
        self.mounted = true;
    }

    pub(super) async fn unmount(mut self) {
        if self.mounted {
            self.mounted = false;
            if !umount(&self.path).await {
                let _ = Command::new("umount")
                    .arg("-l")
                    .arg(&self.path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;
            }
        }
        let _ = tokio::fs::remove_dir(&self.path).await;
    }
}

impl Drop for MountGuard {
    fn drop(&mut self) {
        if self.mounted {
            self.mounted = false;
            let _ = std::process::Command::new("umount")
                .arg("-l")
                .arg(&self.path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = std::fs::remove_dir(&self.path);
    }
}

async fn umount(path: &Path) -> bool {
    matches!(
        Command::new("umount")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await,
        Ok(status) if status.success()
    )
}

pub(super) async fn mount_image(
    source: &Path,
    target: &Path,
    mount_type: MountType,
    cancel: &CancellationToken,
) -> Result<()> {
    if mount_type == MountType::Udf {
        let _ = Command::new("modprobe")
            .arg("udf")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }

    if let Err(error) = run_mount(source, target, Some(mount_type.as_fstype()), cancel).await {
        if cancel.is_cancelled() {
            return Err(cancelled_error());
        }
        warn!(%error, "Typed mount failed, retrying auto-detection");
    } else {
        return Ok(());
    }

    run_mount(source, target, None, cancel).await
}

async fn run_mount(
    source: &Path,
    target: &Path,
    fstype: Option<&str>,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut command = Command::new("mount");
    if let Some(fstype) = fstype {
        command.arg("-t").arg(fstype);
    }
    command
        .args(["-o", "loop,ro,nosuid,nodev,noexec"])
        .arg(source)
        .arg(target);

    let output = run_simple(command, cancel, MOUNT_TIMEOUT).await?;
    if output.status.success() {
        Ok(())
    } else {
        warn!(
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "Failed to mount ISO for conversion"
        );
        Err(MsdErrorCode::MsdIsoMountFailed.into())
    }
}

pub(super) async fn catalog_boot_options(source: &Path, cancel: &CancellationToken) -> Vec<String> {
    let mut command = Command::new("xorriso");
    command
        .arg("-indev")
        .arg(source)
        .args(["-report_el_torito", "as_mkisofs"]);

    match run_simple(command, cancel, EL_TORITO_TIMEOUT).await {
        Ok(output) if output.status.success() => {
            parse_mkisofs_boot_args(&String::from_utf8_lossy(&output.stdout))
        }
        Ok(output) => {
            warn!(
                stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                "Failed to read the El Torito catalog"
            );
            Vec::new()
        }
        Err(error) => {
            warn!(%error, "Failed to read the El Torito catalog");
            Vec::new()
        }
    }
}

pub(super) struct ProcessResult {
    pub(super) status: std::process::ExitStatus,
    pub(super) log_tail: Vec<String>,
}

pub(super) async fn run_simple(
    mut command: Command,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<std::process::Output> {
    command
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().map_err(|error| {
        warn!(%error, "Failed to spawn ISO conversion helper");
        AppError::from(MsdErrorCode::MsdOperationFailed)
    })?;

    tokio::select! {
        _ = cancel.cancelled() => Err(cancelled_error()),
        result = tokio::time::timeout(timeout, child.wait_with_output()) => match result {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(error)) => {
                warn!(%error, "ISO conversion helper failed to run");
                Err(MsdErrorCode::MsdOperationFailed.into())
            }
            Err(_) => {
                warn!("ISO conversion helper timed out");
                Err(MsdErrorCode::MsdOperationFailed.into())
            }
        }
    }
}

pub(super) async fn run_xorriso(
    mut command: Command,
    cancel: &CancellationToken,
) -> Result<ProcessResult> {
    command
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        warn!(%error, "Failed to spawn xorriso");
        AppError::from(MsdErrorCode::MsdIsoConversionFailed)
    })?;
    let Some(mut stderr) = child.stderr.take() else {
        return Err(MsdErrorCode::MsdIsoConversionFailed.into());
    };

    let deadline = tokio::time::sleep(XORRISO_TIMEOUT);
    tokio::pin!(deadline);

    let mut log_tail = VecDeque::with_capacity(MAX_PROCESS_LOG_LINES);
    let mut line = Vec::new();
    let mut buffer = [0u8; 4096];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(cancelled_error());
            }
            _ = &mut deadline => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                warn!("xorriso timed out");
                return Err(MsdErrorCode::MsdIsoConversionFailed.into());
            }
            read = stderr.read(&mut buffer) => match read {
                Ok(0) => break,
                Ok(count) => {
                    for &byte in &buffer[..count] {
                        if byte == b'\n' || byte == b'\r' {
                            push_process_line(&mut log_tail, &mut line);
                        } else if line.len() < MAX_PROCESS_LINE_BYTES {
                            line.push(byte);
                        }
                    }
                }
                Err(error) => {
                    warn!(%error, "Failed to read xorriso output");
                    break;
                }
            }
        }
    }

    let status = child.wait().await.map_err(|error| {
        warn!(%error, "Failed to wait for xorriso");
        AppError::from(MsdErrorCode::MsdIsoConversionFailed)
    })?;

    Ok(ProcessResult {
        status,
        log_tail: log_tail.into_iter().collect(),
    })
}

fn push_process_line(log_tail: &mut VecDeque<String>, line: &mut Vec<u8>) {
    if line.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(line).trim().to_string();
    line.clear();
    if text.is_empty() || text.contains("xorriso : UPDATE :") {
        return;
    }
    if log_tail.len() >= MAX_PROCESS_LOG_LINES {
        log_tail.pop_front();
    }
    log_tail.push_back(text);
}

/// Count regular files in the mounted source tree, capped at the ISO shell
/// threshold. Symlinks are not followed.

pub(super) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

pub(super) fn cancelled_error() -> AppError {
    MsdErrorCode::MsdIsoConversionFailed.into()
}
