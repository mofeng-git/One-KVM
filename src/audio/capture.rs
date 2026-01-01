//! ALSA audio capture implementation

use alsa::pcm::{Access, Format, Frames, HwParams, State, IO};
use alsa::{Direction, ValueOr, PCM};
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, error, info, warn};

use super::device::AudioDeviceInfo;
use crate::error::{AppError, Result};

/// Audio capture configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// ALSA device name (e.g., "hw:0,0" or "default")
    pub device_name: String,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u32,
    /// Samples per frame (for Opus, typically 480 for 10ms at 48kHz)
    pub frame_size: u32,
    /// Buffer size in frames
    pub buffer_frames: u32,
    /// Period size in frames
    pub period_frames: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: "default".to_string(),
            sample_rate: 48000,
            channels: 2,
            frame_size: 960, // 20ms at 48kHz (good for Opus)
            buffer_frames: 4096,
            period_frames: 960,
        }
    }
}

impl AudioConfig {
    /// Create config for a specific device
    pub fn for_device(device: &AudioDeviceInfo) -> Self {
        let sample_rate = if device.sample_rates.contains(&48000) {
            48000
        } else {
            *device.sample_rates.first().unwrap_or(&48000)
        };

        let channels = if device.channels.contains(&2) {
            2
        } else {
            *device.channels.first().unwrap_or(&2)
        };

        Self {
            device_name: device.name.clone(),
            sample_rate,
            channels,
            frame_size: sample_rate / 50, // 20ms
            ..Default::default()
        }
    }

    /// Bytes per sample (16-bit signed)
    pub fn bytes_per_sample(&self) -> u32 {
        2 * self.channels
    }

    /// Bytes per frame
    pub fn bytes_per_frame(&self) -> usize {
        (self.frame_size * self.bytes_per_sample()) as usize
    }
}

/// Audio frame data
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Raw PCM data (S16LE interleaved)
    pub data: Bytes,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Number of samples per channel
    pub samples: u32,
    /// Frame sequence number
    pub sequence: u64,
    /// Capture timestamp
    pub timestamp: Instant,
}

impl AudioFrame {
    pub fn new(data: Bytes, config: &AudioConfig, sequence: u64) -> Self {
        Self {
            samples: data.len() as u32 / config.bytes_per_sample(),
            data,
            sample_rate: config.sample_rate,
            channels: config.channels,
            sequence,
            timestamp: Instant::now(),
        }
    }
}

/// Audio capture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Stopped,
    Running,
    Error,
}

/// Audio capture statistics
#[derive(Debug, Clone, Default)]
pub struct AudioStats {
    pub frames_captured: u64,
    pub frames_dropped: u64,
    pub buffer_overruns: u64,
    pub current_latency_ms: f32,
}

/// ALSA audio capturer
pub struct AudioCapturer {
    config: AudioConfig,
    state: Arc<watch::Sender<CaptureState>>,
    state_rx: watch::Receiver<CaptureState>,
    stats: Arc<Mutex<AudioStats>>,
    frame_tx: broadcast::Sender<AudioFrame>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
    capture_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AudioCapturer {
    /// Create a new audio capturer
    pub fn new(config: AudioConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(CaptureState::Stopped);
        let (frame_tx, _) = broadcast::channel(16); // Buffer size 16 for low latency

        Self {
            config,
            state: Arc::new(state_tx),
            state_rx,
            stats: Arc::new(Mutex::new(AudioStats::default())),
            frame_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
            capture_handle: Mutex::new(None),
        }
    }

    /// Get current state
    pub fn state(&self) -> CaptureState {
        *self.state_rx.borrow()
    }

    /// Subscribe to state changes
    pub fn state_watch(&self) -> watch::Receiver<CaptureState> {
        self.state_rx.clone()
    }

    /// Subscribe to audio frames
    pub fn subscribe(&self) -> broadcast::Receiver<AudioFrame> {
        self.frame_tx.subscribe()
    }

    /// Get statistics
    pub async fn stats(&self) -> AudioStats {
        self.stats.lock().await.clone()
    }

    /// Start capturing
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
        let stats = self.stats.clone();
        let frame_tx = self.frame_tx.clone();
        let stop_flag = self.stop_flag.clone();
        let sequence = self.sequence.clone();

        let handle = tokio::task::spawn_blocking(move || {
            capture_loop(config, state, stats, frame_tx, stop_flag, sequence);
        });

        *self.capture_handle.lock().await = Some(handle);
        Ok(())
    }

    /// Stop capturing
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping audio capture");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.lock().await.take() {
            let _ = handle.await;
        }

        let _ = self.state.send(CaptureState::Stopped);
        Ok(())
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.state() == CaptureState::Running
    }
}

/// Main capture loop
fn capture_loop(
    config: AudioConfig,
    state: Arc<watch::Sender<CaptureState>>,
    stats: Arc<Mutex<AudioStats>>,
    frame_tx: broadcast::Sender<AudioFrame>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
) {
    let result = run_capture(&config, &state, &stats, &frame_tx, &stop_flag, &sequence);

    if let Err(e) = result {
        error!("Audio capture error: {}", e);
        let _ = state.send(CaptureState::Error);
    } else {
        let _ = state.send(CaptureState::Stopped);
    }
}

fn run_capture(
    config: &AudioConfig,
    state: &watch::Sender<CaptureState>,
    stats: &Arc<Mutex<AudioStats>>,
    frame_tx: &broadcast::Sender<AudioFrame>,
    stop_flag: &AtomicBool,
    sequence: &AtomicU64,
) -> Result<()> {
    // Open ALSA device
    let pcm = PCM::new(&config.device_name, Direction::Capture, false).map_err(|e| {
        AppError::AudioError(format!(
            "Failed to open audio device {}: {}",
            config.device_name, e
        ))
    })?;

    // Configure hardware parameters
    {
        let hwp = HwParams::any(&pcm).map_err(|e| {
            AppError::AudioError(format!("Failed to get HwParams: {}", e))
        })?;

        hwp.set_channels(config.channels).map_err(|e| {
            AppError::AudioError(format!("Failed to set channels: {}", e))
        })?;

        hwp.set_rate(config.sample_rate, ValueOr::Nearest).map_err(|e| {
            AppError::AudioError(format!("Failed to set sample rate: {}", e))
        })?;

        hwp.set_format(Format::s16()).map_err(|e| {
            AppError::AudioError(format!("Failed to set format: {}", e))
        })?;

        hwp.set_access(Access::RWInterleaved).map_err(|e| {
            AppError::AudioError(format!("Failed to set access: {}", e))
        })?;

        hwp.set_buffer_size_near(config.buffer_frames as Frames).map_err(|e| {
            AppError::AudioError(format!("Failed to set buffer size: {}", e))
        })?;

        hwp.set_period_size_near(config.period_frames as Frames, ValueOr::Nearest)
            .map_err(|e| AppError::AudioError(format!("Failed to set period size: {}", e)))?;

        pcm.hw_params(&hwp).map_err(|e| {
            AppError::AudioError(format!("Failed to apply hw params: {}", e))
        })?;
    }

    // Get actual configuration
    let actual_rate = pcm.hw_params_current()
        .map(|h| h.get_rate().unwrap_or(config.sample_rate))
        .unwrap_or(config.sample_rate);

    info!(
        "Audio capture configured: {}Hz {}ch (requested {}Hz)",
        actual_rate, config.channels, config.sample_rate
    );

    // Prepare for capture
    pcm.prepare().map_err(|e| {
        AppError::AudioError(format!("Failed to prepare PCM: {}", e))
    })?;

    let _ = state.send(CaptureState::Running);

    // Allocate buffer - use u8 directly for zero-copy
    let frame_bytes = config.bytes_per_frame();
    let mut buffer = vec![0u8; frame_bytes];

    // Capture loop
    while !stop_flag.load(Ordering::Relaxed) {
        // Check PCM state
        match pcm.state() {
            State::XRun => {
                warn!("Audio buffer overrun, recovering");
                if let Ok(mut s) = stats.try_lock() {
                    s.buffer_overruns += 1;
                }
                let _ = pcm.prepare();
                continue;
            }
            State::Suspended => {
                warn!("Audio device suspended, recovering");
                let _ = pcm.resume();
                continue;
            }
            _ => {}
        }

        // Get IO handle and read audio data directly as bytes
        // Note: Use io() instead of io_checked() because USB audio devices
        // typically don't support mmap, which io_checked() requires
        let io: IO<u8> = pcm.io_bytes();

        match io.readi(&mut buffer) {
            Ok(frames_read) => {
                if frames_read == 0 {
                    continue;
                }

                // Calculate actual byte count
                let byte_count = frames_read * config.channels as usize * 2;

                // Directly use the buffer slice (already in correct byte format)
                let seq = sequence.fetch_add(1, Ordering::Relaxed);
                let frame = AudioFrame::new(
                    Bytes::copy_from_slice(&buffer[..byte_count]),
                    config,
                    seq,
                );

                // Send to subscribers
                if frame_tx.receiver_count() > 0 {
                    if let Err(e) = frame_tx.send(frame) {
                        debug!("No audio receivers: {}", e);
                    }
                }

                // Update stats
                if let Ok(mut s) = stats.try_lock() {
                    s.frames_captured += 1;
                }
            }
            Err(e) => {
                // Check for buffer overrun (EPIPE = 32 on Linux)
                let desc = e.to_string();
                if desc.contains("EPIPE") || desc.contains("Broken pipe") {
                    // Buffer overrun
                    warn!("Audio buffer overrun");
                    if let Ok(mut s) = stats.try_lock() {
                        s.buffer_overruns += 1;
                    }
                    let _ = pcm.prepare();
                } else {
                    error!("Audio read error: {}", e);
                    if let Ok(mut s) = stats.try_lock() {
                        s.frames_dropped += 1;
                    }
                }
            }
        }
    }

    info!("Audio capture stopped");
    Ok(())
}
