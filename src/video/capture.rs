//! V4L2 video capture implementation
//!
//! Provides async video capture using memory-mapped buffers.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex};
use tracing::{debug, error, info, warn};

use super::format::{PixelFormat, Resolution};
use super::frame::VideoFrame;
use crate::error::{AppError, Result};
use crate::utils::LogThrottler;
use crate::video::v4l2r_capture::V4l2rCaptureStream;

/// Default number of capture buffers (reduced from 4 to 2 for lower latency)
const DEFAULT_BUFFER_COUNT: u32 = 2;
/// Default capture timeout in seconds
const DEFAULT_TIMEOUT: u64 = 2;
/// Minimum valid frame size (bytes)
const MIN_FRAME_SIZE: usize = 128;

/// Video capturer configuration
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Device path
    pub device_path: PathBuf,
    /// Desired resolution
    pub resolution: Resolution,
    /// Desired pixel format
    pub format: PixelFormat,
    /// Desired frame rate (0 = max available)
    pub fps: u32,
    /// Number of capture buffers
    pub buffer_count: u32,
    /// Capture timeout
    pub timeout: Duration,
    /// JPEG quality (1-100, for MJPEG sources with hardware quality control)
    pub jpeg_quality: u8,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            device_path: PathBuf::from("/dev/video0"),
            resolution: Resolution::HD1080,
            format: PixelFormat::Mjpeg,
            fps: 30,
            buffer_count: DEFAULT_BUFFER_COUNT,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT),
            jpeg_quality: 80,
        }
    }
}

impl CaptureConfig {
    /// Create config for a specific device
    pub fn for_device(path: impl AsRef<Path>) -> Self {
        Self {
            device_path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Set resolution
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.resolution = Resolution::new(width, height);
        self
    }

    /// Set format
    pub fn with_format(mut self, format: PixelFormat) -> Self {
        self.format = format;
        self
    }

    /// Set frame rate
    pub fn with_fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }
}

/// Capture statistics
#[derive(Debug, Clone, Default)]
pub struct CaptureStats {
    /// Current FPS (calculated)
    pub current_fps: f32,
}

/// Video capturer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Not started
    Stopped,
    /// Starting (initializing device)
    Starting,
    /// Running and capturing
    Running,
    /// No signal from source
    NoSignal,
    /// Error occurred
    Error,
    /// Device was lost (disconnected)
    DeviceLost,
}

/// Async video capturer
pub struct VideoCapturer {
    config: CaptureConfig,
    state: Arc<watch::Sender<CaptureState>>,
    state_rx: watch::Receiver<CaptureState>,
    stats: Arc<Mutex<CaptureStats>>,
    stop_flag: Arc<AtomicBool>,
    capture_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Last error that occurred (device path, reason)
    last_error: Arc<parking_lot::RwLock<Option<(String, String)>>>,
}

impl VideoCapturer {
    /// Create a new video capturer
    pub fn new(config: CaptureConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(CaptureState::Stopped);

        Self {
            config,
            state: Arc::new(state_tx),
            state_rx,
            stats: Arc::new(Mutex::new(CaptureStats::default())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            capture_handle: Mutex::new(None),
            last_error: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Get current capture state
    pub fn state(&self) -> CaptureState {
        *self.state_rx.borrow()
    }

    /// Subscribe to state changes
    pub fn state_watch(&self) -> watch::Receiver<CaptureState> {
        self.state_rx.clone()
    }

    /// Get last error (device path, reason)
    pub fn last_error(&self) -> Option<(String, String)> {
        self.last_error.read().clone()
    }

    /// Clear last error
    pub fn clear_error(&self) {
        *self.last_error.write() = None;
    }

    /// Get capture statistics
    pub async fn stats(&self) -> CaptureStats {
        self.stats.lock().await.clone()
    }

    /// Get config
    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    /// Start capturing in background
    pub async fn start(&self) -> Result<()> {
        let current_state = self.state();
        // Already running or starting - nothing to do
        if current_state == CaptureState::Running || current_state == CaptureState::Starting {
            return Ok(());
        }

        info!(
            "Starting capture on {:?} at {}x{} {}",
            self.config.device_path,
            self.config.resolution.width,
            self.config.resolution.height,
            self.config.format
        );

        // Set Starting state immediately to prevent concurrent start attempts
        let _ = self.state.send(CaptureState::Starting);

        // Clear any previous error
        *self.last_error.write() = None;

        self.stop_flag.store(false, Ordering::SeqCst);

        let config = self.config.clone();
        let state = self.state.clone();
        let stats = self.stats.clone();
        let stop_flag = self.stop_flag.clone();
        let last_error = self.last_error.clone();

        let handle = tokio::task::spawn_blocking(move || {
            capture_loop(config, state, stats, stop_flag, last_error);
        });

        *self.capture_handle.lock().await = Some(handle);

        Ok(())
    }

    /// Stop capturing
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping capture");
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.capture_handle.lock().await.take() {
            let _ = handle.await;
        }

        let _ = self.state.send(CaptureState::Stopped);
        Ok(())
    }

    /// Check if capturing
    pub fn is_running(&self) -> bool {
        self.state() == CaptureState::Running
    }

    /// Get the latest frame (if any receivers would get it)
    pub fn latest_frame(&self) -> Option<VideoFrame> {
        // This is a bit tricky with broadcast - we'd need to track internally
        // For now, callers should use subscribe()
        None
    }
}

/// Main capture loop (runs in blocking thread)
fn capture_loop(
    config: CaptureConfig,
    state: Arc<watch::Sender<CaptureState>>,
    stats: Arc<Mutex<CaptureStats>>,
    stop_flag: Arc<AtomicBool>,
    error_holder: Arc<parking_lot::RwLock<Option<(String, String)>>>,
) {
    let result = run_capture(&config, &state, &stats, &stop_flag);

    match result {
        Ok(_) => {
            let _ = state.send(CaptureState::Stopped);
        }
        Err(AppError::VideoDeviceLost { device, reason }) => {
            error!("Video device lost: {} - {}", device, reason);
            // Store the error for recovery handling
            *error_holder.write() = Some((device, reason));
            let _ = state.send(CaptureState::DeviceLost);
        }
        Err(e) => {
            error!("Capture error: {}", e);
            let _ = state.send(CaptureState::Error);
        }
    }
}

fn run_capture(
    config: &CaptureConfig,
    state: &watch::Sender<CaptureState>,
    stats: &Arc<Mutex<CaptureStats>>,
    stop_flag: &AtomicBool,
) -> Result<()> {
    // Retry logic for device busy errors
    const MAX_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 200;

    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        let stream = match V4l2rCaptureStream::open(
            &config.device_path,
            config.resolution,
            config.format,
            config.fps,
            config.buffer_count,
            config.timeout,
        ) {
            Ok(stream) => stream,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("busy") || err_str.contains("resource") {
                    warn!(
                        "Device busy on attempt {}/{}, retrying in {}ms...",
                        attempt + 1,
                        MAX_RETRIES,
                        RETRY_DELAY_MS
                    );
                    std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    last_error = Some(AppError::VideoError(format!(
                        "Failed to open device {:?}: {}",
                        config.device_path, e
                    )));
                    continue;
                }
                return Err(AppError::VideoError(format!(
                    "Failed to open device {:?}: {}",
                    config.device_path, e
                )));
            }
        };

        return run_capture_inner(config, state, stats, stop_flag, stream);
    }

    // All retries exhausted
    Err(last_error.unwrap_or_else(|| {
        AppError::VideoError("Failed to open device after all retries".to_string())
    }))
}

/// Inner capture function after device is successfully opened
fn run_capture_inner(
    config: &CaptureConfig,
    state: &watch::Sender<CaptureState>,
    stats: &Arc<Mutex<CaptureStats>>,
    stop_flag: &AtomicBool,
    mut stream: V4l2rCaptureStream,
) -> Result<()> {
    let resolution = stream.resolution();
    let pixel_format = stream.format();
    let stride = stream.stride();
    info!(
        "Capture format: {}x{} {:?} stride={}",
        resolution.width, resolution.height, pixel_format, stride
    );

    let _ = state.send(CaptureState::Running);
    info!("Capture started");

    // FPS calculation variables
    let mut fps_frame_count = 0u64;
    let mut fps_window_start = Instant::now();
    let fps_window_duration = Duration::from_secs(1);
    let mut scratch = Vec::new();
    let capture_error_throttler = LogThrottler::with_secs(5);
    let mut suppressed_capture_errors: HashMap<String, u64> = HashMap::new();

    let classify_capture_error = |err: &std::io::Error| -> String {
        let message = err.to_string();
        if message.contains("dqbuf failed") && message.contains("EINVAL") {
            "capture_dqbuf_einval".to_string()
        } else if message.contains("dqbuf failed") {
            "capture_dqbuf".to_string()
        } else {
            format!("capture_{:?}", err.kind())
        }
    };

    // Main capture loop
    while !stop_flag.load(Ordering::Relaxed) {
        let meta = match stream.next_into(&mut scratch) {
            Ok(meta) => meta,
            Err(e) => {
                if e.kind() == io::ErrorKind::TimedOut {
                    warn!("Capture timeout - no signal?");
                    let _ = state.send(CaptureState::NoSignal);

                    // Wait a bit before retrying
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                // Check for device loss errors
                let is_device_lost = match e.raw_os_error() {
                    Some(6) => true,   // ENXIO - No such device or address
                    Some(19) => true,  // ENODEV - No such device
                    Some(5) => true,   // EIO - I/O error (device removed)
                    Some(32) => true,  // EPIPE - Broken pipe
                    Some(108) => true, // ESHUTDOWN - Transport endpoint shutdown
                    _ => false,
                };

                if is_device_lost {
                    let device_path = config.device_path.display().to_string();
                    error!("Video device lost: {} - {}", device_path, e);
                    return Err(AppError::VideoDeviceLost {
                        device: device_path,
                        reason: e.to_string(),
                    });
                }

                let key = classify_capture_error(&e);
                if capture_error_throttler.should_log(&key) {
                    let suppressed = suppressed_capture_errors.remove(&key).unwrap_or(0);
                    if suppressed > 0 {
                        error!("Capture error: {} (suppressed {} repeats)", e, suppressed);
                    } else {
                        error!("Capture error: {}", e);
                    }
                } else {
                    let counter = suppressed_capture_errors.entry(key).or_insert(0);
                    *counter = counter.saturating_add(1);
                }
                continue;
            }
        };

        // Use actual bytes used, not buffer size
        let frame_size = meta.bytes_used;

        // Validate frame
        if frame_size < MIN_FRAME_SIZE {
            debug!(
                "Dropping small frame: {} bytes (bytesused={})",
                frame_size, meta.bytes_used
            );
            continue;
        }

        // Update state if was no signal
        if *state.borrow() == CaptureState::NoSignal {
            let _ = state.send(CaptureState::Running);
        }

        // Update FPS calculation
        if let Ok(mut s) = stats.try_lock() {
            fps_frame_count += 1;
            let elapsed = fps_window_start.elapsed();

            if elapsed >= fps_window_duration {
                // Calculate FPS from the completed window
                s.current_fps = (fps_frame_count as f32 / elapsed.as_secs_f32()).max(0.0);
                // Reset for next window
                fps_frame_count = 0;
                fps_window_start = Instant::now();
            } else if elapsed.as_millis() > 100 && fps_frame_count > 0 {
                // Provide partial estimate if we have at least 100ms of data
                s.current_fps = (fps_frame_count as f32 / elapsed.as_secs_f32()).max(0.0);
            }
        }

        if *state.borrow() == CaptureState::NoSignal {
            let _ = state.send(CaptureState::Running);
        }
    }

    info!("Capture stopped");
    Ok(())
}

/// Validate JPEG frame data
#[cfg(test)]
fn is_valid_jpeg(data: &[u8]) -> bool {
    if data.len() < 125 {
        return false;
    }

    // Check start marker (0xFFD8)
    let start_marker = ((data[0] as u16) << 8) | data[1] as u16;
    if start_marker != 0xFFD8 {
        return false;
    }

    // Check end marker
    let end = data.len();
    let end_marker = ((data[end - 2] as u16) << 8) | data[end - 1] as u16;

    // Valid end markers: 0xFFD9, 0xD900, 0x0000 (padded)
    matches!(end_marker, 0xFFD9 | 0xD900 | 0x0000)
}

/// Frame grabber for one-shot capture
pub struct FrameGrabber {
    device_path: PathBuf,
}

impl FrameGrabber {
    /// Create a new frame grabber
    pub fn new(device_path: impl AsRef<Path>) -> Self {
        Self {
            device_path: device_path.as_ref().to_path_buf(),
        }
    }

    /// Capture a single frame
    pub async fn grab(&self, resolution: Resolution, format: PixelFormat) -> Result<VideoFrame> {
        let device_path = self.device_path.clone();

        tokio::task::spawn_blocking(move || grab_single_frame(&device_path, resolution, format))
            .await
            .map_err(|e| AppError::VideoError(format!("Grab task failed: {}", e)))?
    }
}

fn grab_single_frame(
    device_path: &Path,
    resolution: Resolution,
    format: PixelFormat,
) -> Result<VideoFrame> {
    let mut stream = V4l2rCaptureStream::open(
        device_path,
        resolution,
        format,
        0,
        2,
        Duration::from_secs(DEFAULT_TIMEOUT),
    )?;
    let actual_resolution = stream.resolution();
    let actual_format = stream.format();
    let actual_stride = stream.stride();
    let mut scratch = Vec::new();

    // Try to get a valid frame (skip first few which might be bad)
    for attempt in 0..5 {
        match stream.next_into(&mut scratch) {
            Ok(meta) => {
                if meta.bytes_used >= MIN_FRAME_SIZE {
                    return Ok(VideoFrame::new(
                        Bytes::copy_from_slice(&scratch[..meta.bytes_used]),
                        actual_resolution,
                        actual_format,
                        actual_stride,
                        0,
                    ));
                }
            }
            Err(e) if attempt == 4 => {
                return Err(AppError::VideoError(format!("Failed to grab frame: {}", e)));
            }
            Err(_) => {}
        }
    }

    Err(AppError::VideoError(
        "Failed to capture valid frame".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_jpeg() {
        // Valid JPEG header and footer
        let mut data = vec![0xFF, 0xD8]; // SOI
        data.extend(vec![0u8; 200]); // Content
        data.extend([0xFF, 0xD9]); // EOI

        assert!(is_valid_jpeg(&data));

        // Invalid - too small
        assert!(!is_valid_jpeg(&[0xFF, 0xD8, 0xFF, 0xD9]));

        // Invalid - wrong header
        let mut bad = vec![0x00, 0x00];
        bad.extend(vec![0u8; 200]);
        assert!(!is_valid_jpeg(&bad));
    }
}
