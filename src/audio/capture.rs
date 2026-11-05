//! Platform-neutral capture lifecycle and PCM frame types.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, info};

use crate::error::Result;
use crate::utils::LogThrottler;

#[cfg(unix)]
#[path = "capture_linux.rs"]
mod imp;

#[cfg(windows)]
#[path = "capture_windows.rs"]
mod imp;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub buffer_frames: u32,
    pub period_frames: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            sample_rate: 48_000,
            channels: 2,
            buffer_frames: 4096,
            period_frames: 960,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: Bytes,
    pub sample_rate: u32,
    pub channels: u32,
}

impl AudioFrame {
    pub fn new_interleaved(data: Bytes, channels: u32, sample_rate: u32) -> Self {
        Self {
            data,
            sample_rate,
            channels,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Stopped,
    Starting,
    Running,
    Error,
}

pub struct AudioCapturer {
    config: AudioConfig,
    state: watch::Sender<CaptureState>,
    state_rx: watch::Receiver<CaptureState>,
    frame_tx: broadcast::Sender<AudioFrame>,
    stop_flag: Arc<AtomicBool>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    lifecycle: Mutex<()>,
    log_throttler: LogThrottler,
}

impl AudioCapturer {
    pub fn new(config: AudioConfig) -> Self {
        let (state, state_rx) = watch::channel(CaptureState::Stopped);
        let (frame_tx, _) = broadcast::channel(16);

        Self {
            config,
            state,
            state_rx,
            frame_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            task: Mutex::new(None),
            lifecycle: Mutex::new(()),
            log_throttler: LogThrottler::with_secs(5),
        }
    }

    pub fn state(&self) -> CaptureState {
        *self.state_rx.borrow()
    }

    pub fn state_watch(&self) -> watch::Receiver<CaptureState> {
        self.state_rx.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
        self.frame_tx.subscribe()
    }

    pub async fn start(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        if matches!(self.state(), CaptureState::Starting | CaptureState::Running) {
            return Ok(());
        }

        if let Some(previous) = self.task.lock().await.take() {
            let _ = previous.await;
        }

        debug!(
            "Starting audio capture on {} at {}Hz {}ch",
            self.config.device_name, self.config.sample_rate, self.config.channels
        );

        self.stop_flag.store(false, Ordering::Release);
        let _ = self.state.send(CaptureState::Starting);

        let config = self.config.clone();
        let state = self.state.clone();
        let frame_tx = self.frame_tx.clone();
        let stop_flag = Arc::clone(&self.stop_flag);
        let log_throttler = self.log_throttler.clone();

        let task = tokio::task::spawn_blocking(move || {
            match imp::run_capture(&config, &state, &frame_tx, &stop_flag, &log_throttler) {
                Ok(()) => {
                    let _ = state.send(CaptureState::Stopped);
                }
                Err(error) => {
                    crate::error_throttled!(
                        log_throttler,
                        "capture_error",
                        "Audio capture error: {}",
                        error
                    );
                    let _ = state.send(CaptureState::Error);
                }
            }
        });
        *self.task.lock().await = Some(task);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_flag.store(true, Ordering::Release);

        if let Some(task) = self.task.lock().await.take() {
            let _ = task.await;
        }

        let _ = self.state.send(CaptureState::Stopped);
        info!("Audio capture stopped");
        Ok(())
    }
}
