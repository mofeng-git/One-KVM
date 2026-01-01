//! MJPEG stream handler
//!
//! Manages video frame distribution and per-client statistics.

use arc_swap::ArcSwap;
use parking_lot::Mutex as ParkingMutex;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::video::encoder::JpegEncoder;
use crate::video::encoder::traits::{Encoder, EncoderConfig};
use crate::video::format::PixelFormat;
use crate::video::VideoFrame;

/// Client ID type (UUID string)
pub type ClientId = String;

/// Per-client session information
#[derive(Debug, Clone)]
pub struct ClientSession {
    /// Unique client ID
    pub id: ClientId,
    /// Connection timestamp
    pub connected_at: Instant,
    /// Last activity timestamp (frame sent)
    pub last_activity: Instant,
    /// Frames sent to this client
    pub frames_sent: u64,
    /// FPS calculator (1-second rolling window)
    pub fps_calculator: FpsCalculator,
}

impl ClientSession {
    /// Create a new client session
    pub fn new(id: ClientId) -> Self {
        let now = Instant::now();
        Self {
            id,
            connected_at: now,
            last_activity: now,
            frames_sent: 0,
            fps_calculator: FpsCalculator::new(),
        }
    }

    /// Get connection duration
    pub fn connected_duration(&self) -> Duration {
        self.last_activity.duration_since(self.connected_at)
    }

    /// Get idle duration
    pub fn idle_duration(&self) -> Duration {
        Instant::now().duration_since(self.last_activity)
    }
}

/// Rolling window FPS calculator
#[derive(Debug, Clone)]
pub struct FpsCalculator {
    /// Frame timestamps in last window
    frame_times: VecDeque<Instant>,
    /// Window duration (default 1 second)
    window: Duration,
    /// Cached count of frames in current window (optimization to avoid O(n) filtering)
    count_in_window: usize,
}

impl FpsCalculator {
    /// Create a new FPS calculator with 1-second window
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(120), // Max 120fps tracking
            window: Duration::from_secs(1),
            count_in_window: 0,
        }
    }

    /// Record a frame sent
    pub fn record_frame(&mut self) {
        let now = Instant::now();
        self.frame_times.push_back(now);

        // Remove frames outside window and maintain count
        let cutoff = now - self.window;
        while let Some(&oldest) = self.frame_times.front() {
            if oldest < cutoff {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }

        // Update cached count
        self.count_in_window = self.frame_times.len();
    }

    /// Calculate current FPS (frames in last 1 second window)
    pub fn current_fps(&self) -> u32 {
        // Return cached count instead of filtering entire deque (O(1) instead of O(n))
        self.count_in_window as u32
    }
}

impl Default for FpsCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Auto-pause configuration
#[derive(Debug, Clone)]
pub struct AutoPauseConfig {
    /// Enable auto-pause when no clients
    pub enabled: bool,
    /// Delay before pausing (default 10s)
    pub shutdown_delay_secs: u64,
    /// Client timeout for cleanup (default 30s)
    pub client_timeout_secs: u64,
}

impl Default for AutoPauseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shutdown_delay_secs: 10,
            client_timeout_secs: 30,
        }
    }
}

/// MJPEG stream handler
/// Manages video frame distribution to HTTP clients
pub struct MjpegStreamHandler {
    /// Current frame (latest) - using ArcSwap for lock-free reads
    current_frame: ArcSwap<Option<VideoFrame>>,
    /// Frame update notification
    frame_notify: broadcast::Sender<()>,
    /// Whether stream is online
    online: AtomicBool,
    /// Frame sequence counter
    sequence: AtomicU64,
    /// Per-client sessions (ClientId -> ClientSession)
    /// Use parking_lot::RwLock for better performance
    clients: ParkingRwLock<HashMap<ClientId, ClientSession>>,
    /// Auto-pause configuration
    auto_pause_config: ParkingRwLock<AutoPauseConfig>,
    /// Last frame timestamp
    last_frame_ts: ParkingRwLock<Option<Instant>>,
    /// Dropped same frames count
    dropped_same_frames: AtomicU64,
    /// Maximum consecutive same frames to drop (0 = disabled)
    max_drop_same_frames: AtomicU64,
    /// JPEG encoder for non-JPEG input formats
    jpeg_encoder: ParkingMutex<Option<JpegEncoder>>,
}

impl MjpegStreamHandler {
    /// Create a new MJPEG stream handler
    pub fn new() -> Self {
        Self::with_drop_limit(100) // Default: drop up to 100 same frames
    }

    /// Create handler with custom drop limit
    pub fn with_drop_limit(max_drop: u64) -> Self {
        let (frame_notify, _) = broadcast::channel(16); // Buffer size 16 for low latency
        Self {
            current_frame: ArcSwap::from_pointee(None),
            frame_notify,
            online: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            clients: ParkingRwLock::new(HashMap::new()),
            jpeg_encoder: ParkingMutex::new(None),
            auto_pause_config: ParkingRwLock::new(AutoPauseConfig::default()),
            last_frame_ts: ParkingRwLock::new(None),
            dropped_same_frames: AtomicU64::new(0),
            max_drop_same_frames: AtomicU64::new(max_drop),
        }
    }

    /// Update current frame
    pub fn update_frame(&self, frame: VideoFrame) {
        // If frame is not JPEG, encode it
        let frame = if !frame.format.is_compressed() {
            match self.encode_to_jpeg(&frame) {
                Ok(jpeg_frame) => jpeg_frame,
                Err(e) => {
                    warn!("Failed to encode frame to JPEG: {}", e);
                    return;
                }
            }
        } else {
            frame
        };

        // Frame deduplication (ustreamer-style)
        // Check if this frame is identical to the previous one
        let max_drop = self.max_drop_same_frames.load(Ordering::Relaxed);
        if max_drop > 0 && frame.online {
            let current = self.current_frame.load();
            if let Some(ref prev_frame) = **current {
                let dropped_count = self.dropped_same_frames.load(Ordering::Relaxed);

                // Check if we should drop this frame
                if dropped_count < max_drop && frames_are_identical(prev_frame, &frame) {
                    // Check last frame timestamp to ensure minimum 1fps
                    let last_ts = *self.last_frame_ts.read();
                    let should_force_send = if let Some(ts) = last_ts {
                        ts.elapsed() >= Duration::from_secs(1)
                    } else {
                        false
                    };

                    if !should_force_send {
                        // Drop this duplicate frame
                        self.dropped_same_frames.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    // If more than 1 second since last frame, force send even if identical
                }
            }
        }

        // Frame is different or limit reached or forced by 1fps guarantee, update
        self.dropped_same_frames.store(0, Ordering::Relaxed);

        self.sequence.fetch_add(1, Ordering::Relaxed);
        self.online.store(true, Ordering::SeqCst);
        *self.last_frame_ts.write() = Some(Instant::now());
        self.current_frame.store(Arc::new(Some(frame)));

        // Notify waiting clients
        let _ = self.frame_notify.send(());
    }

    /// Encode a non-JPEG frame to JPEG
    fn encode_to_jpeg(&self, frame: &VideoFrame) -> Result<VideoFrame, String> {
        let resolution = frame.resolution;
        let sequence = self.sequence.load(Ordering::Relaxed);

        // Get or create encoder
        let mut encoder_guard = self.jpeg_encoder.lock();
        let encoder = encoder_guard.get_or_insert_with(|| {
            let config = EncoderConfig::jpeg(resolution, 85);
            match JpegEncoder::new(config) {
                Ok(enc) => {
                    debug!("Created JPEG encoder for MJPEG stream: {}x{}", resolution.width, resolution.height);
                    enc
                }
                Err(e) => {
                    warn!("Failed to create JPEG encoder: {}, using default", e);
                    // Try with default config
                    JpegEncoder::new(EncoderConfig::jpeg(resolution, 85))
                        .expect("Failed to create default JPEG encoder")
                }
            }
        });

        // Check if resolution changed
        if encoder.config().resolution != resolution {
            debug!("Resolution changed, recreating JPEG encoder: {}x{}", resolution.width, resolution.height);
            let config = EncoderConfig::jpeg(resolution, 85);
            *encoder = JpegEncoder::new(config).map_err(|e| format!("Failed to create encoder: {}", e))?;
        }

        // Encode based on input format
        let encoded = match frame.format {
            PixelFormat::Yuyv => {
                encoder.encode_yuyv(frame.data(), sequence)
                    .map_err(|e| format!("YUYV encode failed: {}", e))?
            }
            PixelFormat::Nv12 => {
                encoder.encode_nv12(frame.data(), sequence)
                    .map_err(|e| format!("NV12 encode failed: {}", e))?
            }
            PixelFormat::Rgb24 => {
                encoder.encode_rgb(frame.data(), sequence)
                    .map_err(|e| format!("RGB encode failed: {}", e))?
            }
            PixelFormat::Bgr24 => {
                encoder.encode_bgr(frame.data(), sequence)
                    .map_err(|e| format!("BGR encode failed: {}", e))?
            }
            _ => {
                return Err(format!("Unsupported format for JPEG encoding: {}", frame.format));
            }
        };

        // Create new VideoFrame with JPEG data
        Ok(VideoFrame::from_vec(
            encoded.data.to_vec(),
            resolution,
            PixelFormat::Mjpeg,
            0, // stride not relevant for JPEG
            sequence,
        ))
    }

    /// Set stream offline
    pub fn set_offline(&self) {
        self.online.store(false, Ordering::SeqCst);
        let _ = self.frame_notify.send(());
    }

    /// Set stream online (called when streaming starts)
    pub fn set_online(&self) {
        self.online.store(true, Ordering::SeqCst);
    }

    /// Check if stream is online
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }

    /// Get current client count
    pub fn client_count(&self) -> u64 {
        self.clients.read().len() as u64
    }

    /// Register a new client
    pub fn register_client(&self, client_id: ClientId) {
        let session = ClientSession::new(client_id.clone());
        self.clients.write().insert(client_id.clone(), session);
        info!("Client {} connected (total: {})", client_id, self.client_count());
    }

    /// Unregister a client
    pub fn unregister_client(&self, client_id: &str) {
        if let Some(session) = self.clients.write().remove(client_id) {
            let duration = session.connected_duration();
            let duration_secs = duration.as_secs_f32();
            let avg_fps = if duration_secs > 0.1 {
                session.frames_sent as f32 / duration_secs
            } else {
                0.0
            };
            info!(
                "Client {} disconnected after {:.1}s ({} frames, {:.1} avg FPS)",
                client_id, duration_secs, session.frames_sent, avg_fps
            );
        }
    }

    /// Record frame sent to a specific client
    pub fn record_frame_sent(&self, client_id: &str) {
        if let Some(session) = self.clients.write().get_mut(client_id) {
            session.last_activity = Instant::now();
            session.frames_sent += 1;
            session.fps_calculator.record_frame();
        }
    }

    /// Get per-client statistics
    pub fn get_clients_stat(&self) -> HashMap<String, crate::events::types::ClientStats> {
        self.clients
            .read()
            .iter()
            .map(|(id, session)| {
                (
                    id.clone(),
                    crate::events::types::ClientStats {
                        id: id.clone(),
                        fps: session.fps_calculator.current_fps(),
                        connected_secs: session.connected_duration().as_secs(),
                    },
                )
            })
            .collect()
    }

    /// Get auto-pause configuration
    pub fn auto_pause_config(&self) -> AutoPauseConfig {
        self.auto_pause_config.read().clone()
    }

    /// Update auto-pause configuration
    pub fn set_auto_pause_config(&self, config: AutoPauseConfig) {
        let config_clone = config.clone();
        *self.auto_pause_config.write() = config;
        info!(
            "Auto-pause config updated: enabled={}, delay={}s, timeout={}s",
            config_clone.enabled, config_clone.shutdown_delay_secs, config_clone.client_timeout_secs
        );
    }

    /// Get current frame (if any)
    pub fn current_frame(&self) -> Option<VideoFrame> {
        (**self.current_frame.load()).clone()
    }

    /// Subscribe to frame updates
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.frame_notify.subscribe()
    }

    /// Disconnect all clients (used during config changes)
    /// This clears the client list and sets the stream offline,
    /// which will cause all active MJPEG streams to terminate.
    pub fn disconnect_all_clients(&self) {
        let count = {
            let mut clients = self.clients.write();
            let count = clients.len();
            clients.clear();
            count
        };
        if count > 0 {
            info!("Disconnected all {} MJPEG clients for config change", count);
        }
        // Set offline to signal all streaming tasks to stop
        self.set_offline();
    }
}

impl Default for MjpegStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for client lifecycle management
/// Ensures cleanup even on panic or abrupt disconnection
pub struct ClientGuard {
    client_id: ClientId,
    handler: Arc<MjpegStreamHandler>,
}

impl ClientGuard {
    /// Create a new client guard
    pub fn new(client_id: ClientId, handler: Arc<MjpegStreamHandler>) -> Self {
        handler.register_client(client_id.clone());
        Self {
            client_id,
            handler,
        }
    }

    /// Get client ID
    pub fn id(&self) -> &ClientId {
        &self.client_id
    }
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.handler.unregister_client(&self.client_id);
    }
}

impl MjpegStreamHandler {
    /// Start stale client cleanup task
    /// Should be called once when handler is created
    pub fn start_cleanup_task(self: Arc<Self>) {
        let handler = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                let timeout_secs = handler.auto_pause_config().client_timeout_secs;
                let timeout = Duration::from_secs(timeout_secs);
                let now = Instant::now();
                let mut stale = Vec::new();

                // Find stale clients
                {
                    let clients = handler.clients.read();
                    for (id, session) in clients.iter() {
                        if now.duration_since(session.last_activity) > timeout {
                            stale.push(id.clone());
                        }
                    }
                }

                // Remove stale clients
                if !stale.is_empty() {
                    let mut clients = handler.clients.write();
                    for id in stale {
                        if let Some(session) = clients.remove(&id) {
                            warn!(
                                "Removed stale client {} (inactive for {:.1}s)",
                                id,
                                now.duration_since(session.last_activity).as_secs_f32()
                            );
                        }
                    }
                }
            }
        });
    }
}

/// Compare two frames for equality (hash-based, ustreamer-style)
/// Returns true if frames are identical in geometry and content
fn frames_are_identical(a: &VideoFrame, b: &VideoFrame) -> bool {
    // Quick checks first (geometry)
    if a.len() != b.len() {
        return false;
    }

    if a.resolution.width != b.resolution.width || a.resolution.height != b.resolution.height {
        return false;
    }

    if a.format != b.format {
        return false;
    }

    if a.stride != b.stride {
        return false;
    }

    if a.online != b.online {
        return false;
    }

    // Compare hashes instead of full binary data
    // Hash is computed once and cached in OnceLock for efficiency
    // This is much faster than binary comparison for large frames (1080p MJPEG)
    a.get_hash() == b.get_hash()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use crate::video::{format::Resolution, PixelFormat};

    #[tokio::test]
    async fn test_stream_handler() {
        let handler = MjpegStreamHandler::new();
        assert!(!handler.is_online());
        assert_eq!(handler.client_count(), 0);

        // Create a frame
        let _frame = VideoFrame::new(
            Bytes::from(vec![0xFF, 0xD8, 0x00, 0x00, 0xFF, 0xD9]),
            Resolution::VGA,
            PixelFormat::Mjpeg,
            0,
            1,
        );
    }

    #[test]
    fn test_fps_calculator() {
        let mut calc = FpsCalculator::new();

        // Initially empty
        assert_eq!(calc.current_fps(), 0);

        // Record some frames
        calc.record_frame();
        calc.record_frame();
        calc.record_frame();

        // Should have 3 frames in window
        assert!(calc.frame_times.len() == 3);
    }
}
