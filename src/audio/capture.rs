use alsa::pcm::{Access, Format, Frames, HwParams, State, IO};
use alsa::{Direction, ValueOr, PCM};
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, info};

use super::device::AudioDeviceInfo;
use crate::error::{AppError, Result};
use crate::utils::LogThrottler;
use crate::{error_throttled, warn_throttled};

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub frame_size: u32,
    pub buffer_frames: u32,
    pub period_frames: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            sample_rate: 48000,
            channels: 2,
            frame_size: 960,
            buffer_frames: 4096,
            period_frames: 960,
        }
    }
}

impl AudioConfig {
    pub fn for_device(device: &AudioDeviceInfo) -> Self {
        Self {
            device_name: device.name.clone(),
            ..Default::default()
        }
    }

    pub fn bytes_per_sample(&self) -> u32 {
        2 * self.channels
    }

    pub fn bytes_per_frame(&self) -> usize {
        (self.frame_size * self.bytes_per_sample()) as usize
    }
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub data: Bytes,
    pub sample_rate: u32,
    pub channels: u32,
    pub samples: u32,
    pub sequence: u64,
    pub timestamp: Instant,
}

impl AudioFrame {
    pub fn new_interleaved(data: Bytes, channels: u32, sample_rate: u32, sequence: u64) -> Self {
        let bps = 2 * channels;
        Self {
            samples: data.len() as u32 / bps,
            data,
            sample_rate,
            channels,
            sequence,
            timestamp: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Stopped,
    Running,
    Error,
}

pub struct AudioCapturer {
    config: AudioConfig,
    state: Arc<watch::Sender<CaptureState>>,
    state_rx: watch::Receiver<CaptureState>,
    frame_tx: broadcast::Sender<AudioFrame>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
    capture_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    log_throttler: LogThrottler,
}

impl AudioCapturer {
    pub fn new(config: AudioConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(CaptureState::Stopped);
        let (frame_tx, _) = broadcast::channel(16);

        Self {
            config,
            state: Arc::new(state_tx),
            state_rx,
            frame_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            capture_handle: Mutex::new(None),
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
        if self.state() == CaptureState::Running {
            return Ok(());
        }

        info!(
            "Starting audio capture on {} at {}Hz {}ch",
            self.config.device_name, self.config.sample_rate, self.config.channels
        );

        self.stop_flag.store(false, Ordering::SeqCst);

        let config = self.config.clone();
        let state = self.state.clone();
        let frame_tx = self.frame_tx.clone();
        let stop_flag = self.stop_flag.clone();
        let sequence = self.sequence.clone();
        let log_throttler = self.log_throttler.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let result = run_capture(
                &config,
                &state,
                &frame_tx,
                &stop_flag,
                &sequence,
                &log_throttler,
            );

            if let Err(e) = result {
                error_throttled!(log_throttler, "capture_error", "Audio capture error: {}", e);
                let _ = state.send(CaptureState::Error);
            } else {
                let _ = state.send(CaptureState::Stopped);
            }
        });

        *self.capture_handle.lock().await = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping audio capture");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.lock().await.take() {
            let _ = handle.await;
        }

        let _ = self.state.send(CaptureState::Stopped);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.state() == CaptureState::Running
    }
}

fn run_capture(
    config: &AudioConfig,
    state: &watch::Sender<CaptureState>,
    frame_tx: &broadcast::Sender<AudioFrame>,
    stop_flag: &AtomicBool,
    sequence: &AtomicU64,
    log_throttler: &LogThrottler,
) -> Result<()> {
    let pcm = PCM::new(&config.device_name, Direction::Capture, false).map_err(|e| {
        AppError::AudioError(format!(
            "Failed to open audio device {}: {}",
            config.device_name, e
        ))
    })?;

    {
        let hwp = HwParams::any(&pcm)
            .map_err(|e| AppError::AudioError(format!("Failed to get HwParams: {}", e)))?;

        hwp.set_channels(config.channels)
            .map_err(|e| AppError::AudioError(format!("Failed to set channels: {}", e)))?;

        hwp.set_rate(config.sample_rate, ValueOr::Nearest)
            .map_err(|e| AppError::AudioError(format!("Failed to set sample rate: {}", e)))?;

        hwp.set_format(Format::s16())
            .map_err(|e| AppError::AudioError(format!("Failed to set format: {}", e)))?;

        hwp.set_access(Access::RWInterleaved)
            .map_err(|e| AppError::AudioError(format!("Failed to set access: {}", e)))?;

        hwp.set_buffer_size_near(config.buffer_frames as Frames)
            .map_err(|e| AppError::AudioError(format!("Failed to set buffer size: {}", e)))?;

        hwp.set_period_size_near(config.period_frames as Frames, ValueOr::Nearest)
            .map_err(|e| AppError::AudioError(format!("Failed to set period size: {}", e)))?;

        pcm.hw_params(&hwp)
            .map_err(|e| AppError::AudioError(format!("Failed to apply hw params: {}", e)))?;
    }

    let hw_now = pcm.hw_params_current().map_err(|e| {
        AppError::AudioError(format!("Failed to read hw_params after apply: {}", e))
    })?;
    let actual_rate = hw_now
        .get_rate()
        .map_err(|e| AppError::AudioError(format!("Failed to read sample rate: {}", e)))?;
    let actual_ch = hw_now
        .get_channels()
        .map_err(|e| AppError::AudioError(format!("Failed to read channels: {}", e)))?;
    if actual_rate != 48_000 {
        return Err(AppError::AudioError(format!(
            "Audio capture requires 48000 Hz; device is {} Hz",
            actual_rate
        )));
    }
    if actual_ch != 2 {
        return Err(AppError::AudioError(format!(
            "Audio capture requires 2 channels (stereo); device has {}",
            actual_ch
        )));
    }
    info!("Audio capture: 48000 Hz, 2 ch");

    pcm.prepare()
        .map_err(|e| AppError::AudioError(format!("Failed to prepare PCM: {}", e)))?;

    let _ = state.send(CaptureState::Running);

    let period_frames = pcm
        .hw_params_current()
        .ok()
        .and_then(|h| h.get_period_size().ok())
        .map(|f| f as usize)
        .unwrap_or(1024)
        .max(256);
    let buf_frames = period_frames.saturating_mul(4).max(2048);
    let bytes_per_frame = (config.channels as usize) * 2;
    let mut buffer = vec![0u8; buf_frames * bytes_per_frame];

    while !stop_flag.load(Ordering::Relaxed) {
        match pcm.state() {
            State::XRun => {
                warn_throttled!(log_throttler, "xrun", "Audio buffer overrun, recovering");
                let _ = pcm.prepare();
                continue;
            }
            State::Suspended => {
                warn_throttled!(
                    log_throttler,
                    "suspended",
                    "Audio device suspended, recovering"
                );
                let _ = pcm.resume();
                continue;
            }
            _ => {}
        }

        // io_bytes: USB capture often lacks mmap (io_checked requires it).
        let io: IO<u8> = pcm.io_bytes();

        match io.readi(&mut buffer) {
            Ok(frames_read) => {
                if frames_read == 0 {
                    continue;
                }

                let byte_count = frames_read * config.channels as usize * 2;

                let seq = sequence.fetch_add(1, Ordering::Relaxed);
                let frame = AudioFrame::new_interleaved(
                    Bytes::copy_from_slice(&buffer[..byte_count]),
                    config.channels,
                    48_000,
                    seq,
                );

                if frame_tx.receiver_count() > 0 {
                    if let Err(e) = frame_tx.send(frame) {
                        debug!("No audio receivers: {}", e);
                    }
                }
            }
            Err(e) => {
                let desc = e.to_string();
                if desc.contains("EPIPE") || desc.contains("Broken pipe") {
                    warn_throttled!(log_throttler, "buffer_overrun", "Audio buffer overrun");
                    let _ = pcm.prepare();
                } else if desc.contains("No such device") || desc.contains("ENODEV") {
                    error_throttled!(log_throttler, "no_device", "Audio read error: {}", e);
                } else {
                    error_throttled!(log_throttler, "read_error", "Audio read error: {}", e);
                }
            }
        }
    }

    info!("Audio capture stopped");
    Ok(())
}
