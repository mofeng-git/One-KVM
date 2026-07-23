use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use crate::error::{AppError, Result};

/// Kill aplay after this much idle time (no incoming audio frames).
const IDLE_CLOSE_TIMEOUT_MS: u64 = 2000;

/// Configuration for the UAC playback stream.
#[derive(Debug, Clone)]
pub struct UacPlaybackConfig {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for UacPlaybackConfig {
    fn default() -> Self {
        Self {
            device_name: crate::otg::uac::find_uac_pcm_device()
                .unwrap_or_else(crate::otg::uac::uac_pcm_device),
            sample_rate: 48000,
            channels: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UacPcmFrame {
    pub data: Vec<u8>,
    pub duration_ms: u32,
}

/// Writes PCM to the UAC gadget via `aplay` subprocess — same
/// mechanism as the successful manual test: `ffmpeg | aplay hw:0,0`.
#[derive(Clone)]
pub struct UacPlaybackWriter {
    pcm_sender: mpsc::Sender<UacPcmFrame>,
    stop_tx: watch::Sender<bool>,
}

impl UacPlaybackWriter {
    pub fn start(config: UacPlaybackConfig) -> Result<Self> {
        let (pcm_sender, pcm_receiver) = mpsc::channel::<UacPcmFrame>(64);
        let (stop_tx, stop_rx) = watch::channel(false);

        let device = config.device_name;
        let rate = config.sample_rate;
        let ch = config.channels;

        let thread_device = device.clone();
        std::thread::Builder::new()
            .name("uac-aplay".into())
            .spawn(move || {
                Self::playback_loop(&thread_device, rate, ch, pcm_receiver, stop_rx);
                info!("UAC aplay thread stopped");
            })
            .map_err(|e| AppError::Internal(format!("spawn: {e}")))?;

        info!("UAC aplay writer started on {device}");
        Ok(Self { pcm_sender, stop_tx })
    }

    pub async fn write(&self, frame: UacPcmFrame) -> Result<()> {
        self.pcm_sender.send(frame).await
            .map_err(|_| AppError::Internal("UAC channel closed".into()))
    }

    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }

    // ── internals ──────────────────────────────────────────

    fn spawn_aplay(device: &str, rate: u32, ch: u16) -> Option<(Child, Box<dyn Write + Send>)> {
        let mut cmd = Command::new("aplay");
        cmd.arg("-D").arg(device)
            .arg("-f").arg("S16_LE")
            .arg("-r").arg(rate.to_string())
            .arg("-c").arg(ch.to_string())
            .arg("-")                         // stdin
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());        // → journalctl

        match cmd.spawn() {
            Ok(mut child) => {
                let stdin = child.stdin.take()?;
                info!("aplay spawned pid={}", child.id());
                Some((child, Box::new(stdin)))
            }
            Err(e) => {
                warn!("aplay spawn failed: {e}");
                None
            }
        }
    }

    fn kill_aplay(mut child: Child, stdin: Box<dyn Write + Send>) {
        drop(stdin); // close pipe → EOF for aplay
        let _ = child.wait();
    }

    fn playback_loop(
        device: &str,
        rate: u32,
        ch: u16,
        mut receiver: mpsc::Receiver<UacPcmFrame>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let idle_timeout = Duration::from_millis(IDLE_CLOSE_TIMEOUT_MS);
        let mut aplay: Option<(Child, Box<dyn Write + Send>)> = None;
        let mut last_write = std::time::Instant::now();
        let mut frame_count: u64 = 0;
        let mut byte_count: u64 = 0;

        loop {
            // ── wait for frame ──────────────────────────
            let need_timeout = aplay.is_some()
                && last_write.elapsed() >= idle_timeout;
            let deadline = if need_timeout || aplay.is_none() {
                Some(std::time::Instant::now() + Duration::from_millis(200))
            } else {
                None
            };

            let frame = loop {
                if *stop_rx.borrow() { break None; }
                match receiver.try_recv() {
                    Ok(f) => break Some(f),
                    Err(mpsc::error::TryRecvError::Disconnected) => break None,
                    Err(mpsc::error::TryRecvError::Empty) => {}
                }
                if let Some(dl) = deadline {
                    if std::time::Instant::now() >= dl {
                        break None;
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            };

            if *stop_rx.borrow() {
                break;
            }

            match frame {
                Some(f) => {
                    last_write = std::time::Instant::now();

                    // Ensure aplay is alive
                    if aplay.is_none() {
                        aplay = Self::spawn_aplay(device, rate, ch);
                    }

                    if let Some((child, stdin)) = aplay.as_mut() {
                        // Check child health
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                warn!("aplay died: {status}");
                                aplay = Self::spawn_aplay(device, rate, ch);
                                if aplay.is_none() { continue; }
                            }
                            Ok(None) => {} // alive
                            Err(e) => {
                                warn!("aplay wait error: {e}");
                                aplay = None;
                                continue;
                            }
                        }
                    }

                    if let Some((child, stdin)) = aplay.as_mut() {
                        match stdin.write_all(&f.data) {
                            Ok(()) => {
                                let _ = stdin.flush();
                                frame_count += 1;
                                byte_count += f.data.len() as u64;
                            }
                            Err(e) => {
                                warn!("aplay write error: {e}");
                                // aplay died — reap and restart
                                if let Some((c, s)) = aplay.take() {
                                    Self::kill_aplay(c, s);
                                }
                            }
                        }
                    }
                }
                None => {
                    // Timeout — kill aplay
                    if let Some((c, s)) = aplay.take() {
                        Self::kill_aplay(c, s);
                    }
                    if *stop_rx.borrow() {
                        break;
                    }
                }
            }
        }

        // Cleanup
        if let Some((c, s)) = aplay.take() {
            Self::kill_aplay(c, s);
        }
    }
}
