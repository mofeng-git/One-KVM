//! Built-in shell over WebSocket, backed by a pseudo-terminal.
//!
//! This replaces the external `ttyd` dependency for the web terminal. The
//! server owns the PTY and the shell process; the browser only renders bytes.
//!
//! Wire protocol
//! -------------
//! Client -> server:
//! - text frames are JSON commands: `{"type":"input","data":"..."}` and
//!   `{"type":"resize","cols":120,"rows":32}`
//! - binary frames are treated as raw input bytes
//!
//! Server -> client:
//! - binary frames contain raw terminal output
//! - text frames contain JSON control messages: `ready`, `exit`, `error`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::config::validate_shell;
use crate::state::AppState;

/// Maximum number of concurrent terminal sessions across the whole server.
const MAX_SESSIONS: usize = 8;
/// Reject oversized client frames before they reach the PTY.
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;

static ACTIVE_SESSIONS: AtomicUsize = AtomicUsize::new(0);

struct SessionGuard;

impl Drop for SessionGuard {
    fn drop(&mut self) {
        ACTIVE_SESSIONS.fetch_sub(1, Ordering::SeqCst);
    }
}

fn acquire_session() -> Option<SessionGuard> {
    let mut current = ACTIVE_SESSIONS.load(Ordering::SeqCst);
    loop {
        if current >= MAX_SESSIONS {
            return None;
        }
        match ACTIVE_SESSIONS.compare_exchange_weak(
            current,
            current + 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => return Some(SessionGuard),
            Err(actual) => current = actual,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct TerminalQuery {
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
    Ping,
}

pub async fn shell_ws_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TerminalQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let config = state.config.get();

    if !config.terminal.enabled {
        return (StatusCode::FORBIDDEN, "Web terminal is disabled").into_response();
    }

    let shell = config.terminal.shell.clone();
    drop(config);

    if let Err(error) = validate_shell(&shell) {
        tracing::warn!(%error, "Rejected built-in terminal connection");
        return (StatusCode::SERVICE_UNAVAILABLE, error).into_response();
    }

    let Some(guard) = acquire_session() else {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many terminal sessions").into_response();
    };

    let cols = query.cols.unwrap_or(DEFAULT_COLS).clamp(1, 1000);
    let rows = query.rows.unwrap_or(DEFAULT_ROWS).clamp(1, 1000);

    ws.max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| async move {
            let _guard = guard;
            run_shell_session(socket, shell, cols, rows).await;
        })
}

async fn run_shell_session(socket: WebSocket, shell: String, cols: u16, rows: u16) {
    let session = match PtySession::spawn(&shell, cols, rows) {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, "Failed to start built-in shell");
            send_shell_error(socket, format!("Failed to start shell: {error}")).await;
            return;
        }
    };

    tracing::info!(shell = %shell, cols, rows, pid = session.child.id().unwrap_or_default(), "Built-in shell session started");

    let PtySession {
        master_fd,
        reader,
        writer,
        mut child,
    } = session;

    let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    tokio::spawn(read_pty(reader, output_tx));

    let (mut sender, mut receiver) = socket.split();
    let mut writer = writer;

    let _ = send_control(&mut sender, json!({ "type": "ready" })).await;

    loop {
        tokio::select! {
            message = receiver.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if !handle_client_command(&text, master_fd, &mut writer).await {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        if writer.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            chunk = output_rx.recv() => {
                match chunk {
                    Some(data) => {
                        if sender.send(Message::Binary(data.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            status = child.wait() => {
                let code = status.ok().and_then(|status| status.code());
                let _ = send_control(&mut sender, json!({ "type": "exit", "code": code })).await;
                break;
            }
        }
    }

    // The client may have disconnected while the shell was still running. Make
    // sure the process does not outlive the WebSocket.
    let pid = child.id().unwrap_or_default();
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;

    tracing::info!(pid, "Built-in shell session ended");
}

async fn read_pty(mut reader: tokio::fs::File, output_tx: tokio::sync::mpsc::Sender<Vec<u8>>) {
    let mut buffer = vec![0u8; 8192];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => {
                if output_tx.send(buffer[..read].to_vec()).await.is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            // EIO is the normal way a Linux PTY master reports that the slave
            // side (the shell) has gone away.
            Err(error) if error.raw_os_error() == Some(libc::EIO) => break,
            Err(_) => break,
        }
    }
}

async fn handle_client_command(
    text: &str,
    master_fd: std::os::fd::RawFd,
    writer: &mut tokio::fs::File,
) -> bool {
    let command: ClientCommand = match serde_json::from_str(text) {
        Ok(command) => command,
        // Ignore malformed frames instead of tearing down an interactive
        // session; the browser side only sends JSON control messages.
        Err(_) => return true,
    };

    match command {
        ClientCommand::Input { data } => writer.write_all(data.as_bytes()).await.is_ok(),
        ClientCommand::Resize { cols, rows } => {
            resize_pty(master_fd, cols, rows);
            true
        }
        ClientCommand::Ping => true,
    }
}

async fn send_control(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    value: serde_json::Value,
) -> Result<(), axum::Error> {
    sender.send(Message::Text(value.to_string().into())).await
}

async fn send_shell_error(socket: WebSocket, message: String) {
    let (mut sender, _receiver) = socket.split();
    let _ = send_control(&mut sender, json!({ "type": "error", "message": message })).await;
    let _ = sender.send(Message::Close(None)).await;
}

fn resize_pty(master_fd: std::os::fd::RawFd, cols: u16, rows: u16) {
    let size = libc::winsize {
        ws_row: rows.max(1),
        ws_col: cols.max(1),
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // SAFETY: `master_fd` is owned by the live `PtySession` and is not closed
    // while this function runs. The kernel only reads the passed winsize.
    unsafe {
        libc::ioctl(master_fd, libc::TIOCSWINSZ, &size);
    }
}

struct PtySession {
    master_fd: std::os::fd::RawFd,
    reader: tokio::fs::File,
    writer: tokio::fs::File,
    child: tokio::process::Child,
}

impl PtySession {
    fn spawn(shell: &str, cols: u16, rows: u16) -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;

        let winsize = nix::pty::Winsize {
            ws_row: rows.max(1),
            ws_col: cols.max(1),
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let openpty = nix::pty::openpty(Some(&winsize), None).map_err(nix_error)?;
        let master = openpty.master;
        let slave = openpty.slave;

        set_close_on_exec(&master)?;
        set_close_on_exec(&slave)?;

        let slave_fd = slave.as_raw_fd();
        let stdin = std::process::Stdio::from(slave.try_clone()?);
        let stdout = std::process::Stdio::from(slave.try_clone()?);
        let stderr = std::process::Stdio::from(slave.try_clone()?);

        let mut parts = shell.split_whitespace();
        let program = parts.next().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Shell path is empty")
        })?;

        let mut command = tokio::process::Command::new(program);
        command
            .args(parts)
            .stdin(stdin)
            .stdout(stdout)
            .stderr(stderr)
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .kill_on_drop(true);

        if let Some(home) = home_dir() {
            command.current_dir(home);
        }

        // SAFETY: `pre_exec` runs in the forked child before `exec`. `setsid`
        // and `ioctl(TIOCSCTTY)` are async-signal-safe and only operate on the
        // slave PTY passed below. Returning an error aborts the exec safely.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command.spawn()?;
        drop(slave);

        let master_fd = master.as_raw_fd();
        let std_master = std::fs::File::from(master);
        let std_reader = std_master.try_clone()?;

        Ok(Self {
            master_fd,
            reader: tokio::fs::File::from_std(std_reader),
            writer: tokio::fs::File::from_std(std_master),
            child,
        })
    }
}

fn set_close_on_exec(fd: &std::os::fd::OwnedFd) -> std::io::Result<()> {
    nix::fcntl::fcntl(
        fd,
        nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
    )
    .map(|_| ())
    .map_err(nix_error)
}

fn nix_error(error: nix::errno::Errno) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, error.to_string())
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_client_commands() {
        let input: ClientCommand =
            serde_json::from_str(r#"{"type":"input","data":"ls\n"}"#).unwrap();
        assert!(matches!(input, ClientCommand::Input { data } if data == "ls\n"));

        let resize: ClientCommand =
            serde_json::from_str(r#"{"type":"resize","cols":120,"rows":40}"#).unwrap();
        assert!(matches!(
            resize,
            ClientCommand::Resize {
                cols: 120,
                rows: 40
            }
        ));

        assert!(serde_json::from_str::<ClientCommand>(r#"{"type":"ping"}"#).is_ok());
        assert!(serde_json::from_str::<ClientCommand>("not json").is_err());
    }

    #[tokio::test]
    async fn pty_runs_a_shell_and_returns_output() {
        let session = PtySession::spawn("/bin/sh", 80, 24).expect("spawn shell");
        let PtySession {
            reader,
            writer,
            mut child,
            ..
        } = session;
        let mut reader = reader;
        let mut writer = writer;

        writer
            .write_all(b"echo one-kvm-pty\n")
            .await
            .expect("write");

        let mut collected = Vec::new();
        let mut buffer = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

        while tokio::time::Instant::now() < deadline {
            let read = tokio::time::timeout(
                std::time::Duration::from_millis(500),
                reader.read(&mut buffer),
            )
            .await;

            if let Ok(Ok(read)) = read {
                collected.extend_from_slice(&buffer[..read]);
                if String::from_utf8_lossy(&collected).contains("one-kvm-pty") {
                    break;
                }
            }
        }

        assert!(
            String::from_utf8_lossy(&collected).contains("one-kvm-pty"),
            "shell output was: {:?}",
            String::from_utf8_lossy(&collected)
        );

        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
