//! V4L2 video capture implementation
//!
//! Provides async video capture using memory-mapped buffers.

use bytes::Bytes;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, error, info, warn};
use v4l::buffer::Type as BufferType;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::capture::Parameters;
use v4l::video::Capture;
use v4l::Format;

use super::format::{PixelFormat, Resolution};
use super::frame::VideoFrame;
use crate::error::{AppError, Result};

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
    /// Total frames captured
    pub frames_captured: u64,
    /// Frames dropped (invalid/too small)
    pub frames_dropped: u64,
    /// Current FPS (calculated)
    pub current_fps: f32,
    /// Average frame size in bytes
    pub avg_frame_size: usize,
    /// Capture errors
    pub errors: u64,
    /// Last frame timestamp
    pub last_frame_ts: Option<Instant>,
    /// Whether signal is present
    pub signal_present: bool,
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
    frame_tx: broadcast::Sender<VideoFrame>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
    capture_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Last error that occurred (device path, reason)
    last_error: Arc<parking_lot::RwLock<Option<(String, String)>>>,
}

impl VideoCapturer {
    /// Create a new video capturer
    pub fn new(config: CaptureConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(CaptureState::Stopped);
        let (frame_tx, _) = broadcast::channel(4); // Reduced from 64 for lower latency

        Self {
            config,
            state: Arc::new(state_tx),
            state_rx,
            stats: Arc::new(Mutex::new(CaptureStats::default())),
            frame_tx,
            stop_flag: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU64::new(0)),
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

    /// Subscribe to frames
    pub fn subscribe(&self) -> broadcast::Receiver<VideoFrame> {
        self.frame_tx.subscribe()
    }

    /// Get frame sender (for sharing with other components like WebRTC)
    pub fn frame_sender(&self) -> broadcast::Sender<VideoFrame> {
        self.frame_tx.clone()
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
        let frame_tx = self.frame_tx.clone();
        let stop_flag = self.stop_flag.clone();
        let sequence = self.sequence.clone();
        let last_error = self.last_error.clone();

        let handle = tokio::task::spawn_blocking(move || {
            capture_loop(config, state, stats, frame_tx, stop_flag, sequence, last_error);
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
    frame_tx: broadcast::Sender<VideoFrame>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
    error_holder: Arc<parking_lot::RwLock<Option<(String, String)>>>,
) {
    let result = run_capture(
        &config,
        &state,
        &stats,
        &frame_tx,
        &stop_flag,
        &sequence,
    );

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
    frame_tx: &broadcast::Sender<VideoFrame>,
    stop_flag: &AtomicBool,
    sequence: &AtomicU64,
) -> Result<()> {
    // Retry logic for device busy errors
    const MAX_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 200;

    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        if stop_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Open device
        let device = match Device::with_path(&config.device_path) {
            Ok(d) => d,
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

        // Set format
        let format = Format::new(
            config.resolution.width,
            config.resolution.height,
            config.format.to_fourcc(),
        );

        let actual_format = match device.set_format(&format) {
            Ok(f) => f,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("busy") || err_str.contains("resource") {
                    warn!(
                        "Device busy on set_format attempt {}/{}, retrying in {}ms...",
                        attempt + 1,
                        MAX_RETRIES,
                        RETRY_DELAY_MS
                    );
                    std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    last_error = Some(AppError::VideoError(format!("Failed to set format: {}", e)));
                    continue;
                }
                return Err(AppError::VideoError(format!("Failed to set format: {}", e)));
            }
        };

        // Device opened and format set successfully - proceed with capture
        return run_capture_inner(
            config,
            state,
            stats,
            frame_tx,
            stop_flag,
            sequence,
            device,
            actual_format,
        );
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
    frame_tx: &broadcast::Sender<VideoFrame>,
    stop_flag: &AtomicBool,
    sequence: &AtomicU64,
    device: Device,
    actual_format: Format,
) -> Result<()> {
    info!(
        "Capture format: {}x{} {:?} stride={}",
        actual_format.width, actual_format.height, actual_format.fourcc, actual_format.stride
    );

    let resolution = Resolution::new(actual_format.width, actual_format.height);
    let pixel_format = PixelFormat::from_fourcc(actual_format.fourcc).unwrap_or(config.format);

    // Try to set hardware FPS (V4L2 VIDIOC_S_PARM)
    if config.fps > 0 {
        match device.set_params(&Parameters::with_fps(config.fps)) {
            Ok(actual_params) => {
                // Extract actual FPS from returned interval (numerator/denominator)
                let actual_hw_fps = if actual_params.interval.numerator > 0 {
                    actual_params.interval.denominator / actual_params.interval.numerator
                } else {
                    0
                };

                if actual_hw_fps == config.fps {
                    info!("Hardware FPS set successfully: {} fps", actual_hw_fps);
                } else if actual_hw_fps > 0 {
                    info!(
                        "Hardware FPS coerced: requested {} fps, got {} fps",
                        config.fps, actual_hw_fps
                    );
                } else {
                    warn!("Hardware FPS setting returned invalid interval");
                }
            }
            Err(e) => {
                warn!("Failed to set hardware FPS: {}", e);
            }
        }
    }

    // Create stream with mmap buffers
    let mut stream =
        MmapStream::with_buffers(&device, BufferType::VideoCapture, config.buffer_count)
            .map_err(|e| AppError::VideoError(format!("Failed to create stream: {}", e)))?;

    let _ = state.send(CaptureState::Running);
    info!("Capture started");

    // FPS calculation variables
    let mut fps_frame_count = 0u64;
    let mut fps_window_start = Instant::now();
    let fps_window_duration = Duration::from_secs(1);

    // Main capture loop
    while !stop_flag.load(Ordering::Relaxed) {
        // Try to capture a frame
        let (buf, meta) = match stream.next() {
            Ok(frame_data) => frame_data,
            Err(e) => {
                if e.kind() == io::ErrorKind::TimedOut {
                    warn!("Capture timeout - no signal?");
                    let _ = state.send(CaptureState::NoSignal);

                    // Update stats
                    if let Ok(mut s) = stats.try_lock() {
                        s.signal_present = false;
                    }

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

                error!("Capture error: {}", e);
                if let Ok(mut s) = stats.try_lock() {
                    s.errors += 1;
                }
                continue;
            }
        };

        // Use actual bytes used, not buffer size
        let frame_size = meta.bytesused as usize;

        // Validate frame
        if frame_size < MIN_FRAME_SIZE {
            debug!("Dropping small frame: {} bytes (bytesused={})", frame_size, meta.bytesused);
            if let Ok(mut s) = stats.try_lock() {
                s.frames_dropped += 1;
            }
            continue;
        }

        // For JPEG formats, validate header
        if pixel_format.is_compressed() && !is_valid_jpeg(&buf[..frame_size]) {
            debug!("Dropping invalid JPEG frame (size={})", frame_size);
            if let Ok(mut s) = stats.try_lock() {
                s.frames_dropped += 1;
            }
            continue;
        }

        // Create frame with actual data size
        let seq = sequence.fetch_add(1, Ordering::Relaxed);
        let frame = VideoFrame::new(
            Bytes::copy_from_slice(&buf[..frame_size]),
            resolution,
            pixel_format,
            actual_format.stride,
            seq,
        );

        // Update state if was no signal
        if *state.borrow() == CaptureState::NoSignal {
            let _ = state.send(CaptureState::Running);
        }

        // Send frame to subscribers
        let receiver_count = frame_tx.receiver_count();
        if receiver_count > 0 {
            if let Err(e) = frame_tx.send(frame) {
                debug!("No active receivers for frame: {}", e);
            }
        } else if seq % 60 == 0 {
            // Log every 60 frames (about 1 second at 60fps) when no receivers
            debug!("No receivers for video frames (receiver_count=0)");
        }

        // Update stats
        if let Ok(mut s) = stats.try_lock() {
            s.frames_captured += 1;
            s.signal_present = true;
            s.last_frame_ts = Some(Instant::now());

            // Update FPS calculation
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
    }

    info!("Capture stopped");
    Ok(())
}

/// Validate JPEG frame data
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
    pub async fn grab(
        &self,
        resolution: Resolution,
        format: PixelFormat,
    ) -> Result<VideoFrame> {
        let device_path = self.device_path.clone();

        tokio::task::spawn_blocking(move || {
            grab_single_frame(&device_path, resolution, format)
        })
        .await
        .map_err(|e| AppError::VideoError(format!("Grab task failed: {}", e)))?
    }
}

fn grab_single_frame(
    device_path: &Path,
    resolution: Resolution,
    format: PixelFormat,
) -> Result<VideoFrame> {
    let device = Device::with_path(device_path).map_err(|e| {
        AppError::VideoError(format!("Failed to open device: {}", e))
    })?;

    let fmt = Format::new(resolution.width, resolution.height, format.to_fourcc());
    let actual = device.set_format(&fmt).map_err(|e| {
        AppError::VideoError(format!("Failed to set format: {}", e))
    })?;

    let mut stream = MmapStream::with_buffers(&device, BufferType::VideoCapture, 2)
        .map_err(|e| AppError::VideoError(format!("Failed to create stream: {}", e)))?;

    // Try to get a valid frame (skip first few which might be bad)
    for attempt in 0..5 {
        match stream.next() {
            Ok((buf, _meta)) => {
                if buf.len() >= MIN_FRAME_SIZE {
                    let actual_format =
                        PixelFormat::from_fourcc(actual.fourcc).unwrap_or(format);

                    return Ok(VideoFrame::new(
                        Bytes::copy_from_slice(buf),
                        Resolution::new(actual.width, actual.height),
                        actual_format,
                        actual.stride,
                        0,
                    ));
                }
            }
            Err(e) => {
                if attempt == 4 {
                    return Err(AppError::VideoError(format!(
                        "Failed to grab frame: {}",
                        e
                    )));
                }
            }
        }
    }

    Err(AppError::VideoError("Failed to capture valid frame".to_string()))
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
