use arc_swap::ArcSwap;
use parking_lot::Mutex as ParkingMutex;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Generation token paired with `client_id` so [`unregister_client`] ignores stale drops.
pub type ClientGeneration = u64;

use crate::video::encoder::traits::{Encoder, EncoderConfig};
use crate::video::encoder::JpegEncoder;
use crate::video::format::PixelFormat;
use crate::video::VideoFrame;

pub type ClientId = String;

#[derive(Debug, Clone)]
pub struct ClientSession {
    pub id: ClientId,
    pub generation: ClientGeneration,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub frames_sent: u64,
    pub fps_calculator: FpsCalculator,
}

impl ClientSession {
    pub fn new(id: ClientId, generation: ClientGeneration) -> Self {
        let now = Instant::now();
        Self {
            id,
            generation,
            connected_at: now,
            last_activity: now,
            frames_sent: 0,
            fps_calculator: FpsCalculator::new(),
        }
    }

    pub fn connected_elapsed(&self) -> Duration {
        self.connected_at.elapsed()
    }
}

#[derive(Debug, Clone)]
pub struct FpsCalculator {
    frame_times: VecDeque<Instant>,
    window: Duration,
}

impl FpsCalculator {
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(120),
            window: Duration::from_secs(1),
        }
    }

    pub fn record_frame(&mut self) {
        let now = Instant::now();
        self.frame_times.push_back(now);
        self.prune(now);
    }

    /// Rolling-window FPS sample count (~1s).
    pub fn current_fps(&mut self) -> u32 {
        self.prune(Instant::now());
        self.frame_times.len() as u32
    }

    fn prune(&mut self, now: Instant) {
        let cutoff = now - self.window;
        while matches!(self.frame_times.front(), Some(&t) if t < cutoff) {
            self.frame_times.pop_front();
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoPauseConfig {
    pub enabled: bool,
    pub shutdown_delay_secs: u64,
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

pub struct MjpegStreamHandler {
    current_frame: ArcSwap<Option<VideoFrame>>,
    frame_notify: broadcast::Sender<()>,
    online: AtomicBool,
    sequence: AtomicU64,
    clients: ParkingRwLock<HashMap<ClientId, ClientSession>>,
    next_generation: AtomicU64,
    auto_pause_config: ParkingRwLock<AutoPauseConfig>,
    last_frame_ts: ParkingRwLock<Option<Instant>>,
    dropped_same_frames: AtomicU64,
    max_drop_same_frames: AtomicU64,
    jpeg_encoder: ParkingMutex<Option<JpegEncoder>>,
    jpeg_quality: AtomicU64,
}

impl MjpegStreamHandler {
    pub fn new() -> Self {
        Self::with_drop_limit(100)
    }

    pub fn with_drop_limit(max_drop: u64) -> Self {
        let (frame_notify, _) = broadcast::channel(16);
        Self {
            current_frame: ArcSwap::from_pointee(None),
            frame_notify,
            online: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            clients: ParkingRwLock::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
            jpeg_encoder: ParkingMutex::new(None),
            auto_pause_config: ParkingRwLock::new(AutoPauseConfig::default()),
            last_frame_ts: ParkingRwLock::new(None),
            dropped_same_frames: AtomicU64::new(0),
            max_drop_same_frames: AtomicU64::new(max_drop),
            jpeg_quality: AtomicU64::new(80),
        }
    }

    pub fn set_jpeg_quality(&self, quality: u8) {
        let clamped = quality.clamp(1, 100) as u64;
        self.jpeg_quality.store(clamped, Ordering::Relaxed);
    }

    pub fn update_frame(&self, frame: VideoFrame) {
        let has_clients = !self.clients.read().is_empty();
        if !has_clients {
            self.dropped_same_frames.store(0, Ordering::Relaxed);
            self.sequence.fetch_add(1, Ordering::Relaxed);
            self.online.store(frame.online, Ordering::SeqCst);
            *self.last_frame_ts.write() = Some(Instant::now());

            if frame.format.is_compressed() {
                self.current_frame.store(Arc::new(Some(frame)));
            } else {
                self.current_frame.store(Arc::new(None));
            }
            return;
        }

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

        let max_drop = self.max_drop_same_frames.load(Ordering::Relaxed);
        if max_drop > 0 && frame.online {
            let current = self.current_frame.load();
            if let Some(ref prev_frame) = **current {
                let dropped_count = self.dropped_same_frames.load(Ordering::Relaxed);

                if dropped_count < max_drop && frames_are_identical(prev_frame, &frame) {
                    let last_ts = *self.last_frame_ts.read();
                    let should_force_send = if let Some(ts) = last_ts {
                        ts.elapsed() >= Duration::from_secs(1)
                    } else {
                        false
                    };

                    if !should_force_send {
                        self.dropped_same_frames.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                }
            }
        }

        self.dropped_same_frames.store(0, Ordering::Relaxed);

        self.sequence.fetch_add(1, Ordering::Relaxed);
        self.online.store(frame.online, Ordering::SeqCst);
        *self.last_frame_ts.write() = Some(Instant::now());
        self.current_frame.store(Arc::new(Some(frame)));

        let _ = self.frame_notify.send(());
    }

    fn encode_to_jpeg(&self, frame: &VideoFrame) -> Result<VideoFrame, String> {
        let resolution = frame.resolution;
        let sequence = self.sequence.load(Ordering::Relaxed);
        let desired_quality = self.jpeg_quality.load(Ordering::Relaxed) as u32;

        let mut encoder_guard = self.jpeg_encoder.lock();
        let encoder = encoder_guard.get_or_insert_with(|| {
            let config = EncoderConfig::jpeg(resolution, desired_quality);
            match JpegEncoder::new(config) {
                Ok(enc) => {
                    debug!(
                        "Created JPEG encoder for MJPEG stream: {}x{} (q={})",
                        resolution.width, resolution.height, desired_quality
                    );
                    enc
                }
                Err(e) => {
                    warn!("Failed to create JPEG encoder: {}", e);
                    panic!("Failed to create JPEG encoder");
                }
            }
        });

        if encoder.config().resolution != resolution {
            debug!(
                "Resolution changed, recreating JPEG encoder: {}x{}",
                resolution.width, resolution.height
            );
            let config = EncoderConfig::jpeg(resolution, desired_quality);
            *encoder =
                JpegEncoder::new(config).map_err(|e| format!("Failed to create encoder: {}", e))?;
        } else if encoder.config().quality != desired_quality {
            if let Err(e) = encoder.set_quality(desired_quality) {
                warn!("Failed to set JPEG quality: {}, recreating encoder", e);
                let config = EncoderConfig::jpeg(resolution, desired_quality);
                *encoder = JpegEncoder::new(config)
                    .map_err(|e| format!("Failed to create encoder: {}", e))?;
            }
        }

        let encoded = match frame.format {
            PixelFormat::Yuyv => encoder
                .encode_yuyv(frame.data(), sequence)
                .map_err(|e| format!("YUYV encode failed: {}", e))?,
            PixelFormat::Yvyu => encoder
                .encode_yvyu(frame.data(), sequence)
                .map_err(|e| format!("YVYU encode failed: {}", e))?,
            PixelFormat::Nv12 => encoder
                .encode_nv12(frame.data(), sequence)
                .map_err(|e| format!("NV12 encode failed: {}", e))?,
            PixelFormat::Nv16 => encoder
                .encode_nv16(frame.data(), sequence)
                .map_err(|e| format!("NV16 encode failed: {}", e))?,
            PixelFormat::Nv24 => encoder
                .encode_nv24(frame.data(), sequence)
                .map_err(|e| format!("NV24 encode failed: {}", e))?,
            PixelFormat::Rgb24 => encoder
                .encode_rgb(frame.data(), sequence)
                .map_err(|e| format!("RGB encode failed: {}", e))?,
            PixelFormat::Bgr24 => encoder
                .encode_bgr(frame.data(), sequence)
                .map_err(|e| format!("BGR encode failed: {}", e))?,
            _ => {
                return Err(format!(
                    "Unsupported format for JPEG encoding: {}",
                    frame.format
                ));
            }
        };

        Ok(VideoFrame::new(
            encoded.data,
            resolution,
            PixelFormat::Mjpeg,
            0,
            sequence,
        ))
    }

    pub fn set_offline(&self) {
        self.online.store(false, Ordering::SeqCst);
        let _ = self.frame_notify.send(());
    }

    pub fn set_online(&self) {
        self.online.store(true, Ordering::SeqCst);
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::SeqCst)
    }

    pub fn client_count(&self) -> u64 {
        self.clients.read().len() as u64
    }

    /// Connects `client_id`; return value must be passed to [`unregister_client`].
    pub fn register_client(&self, client_id: ClientId) -> ClientGeneration {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let session = ClientSession::new(client_id.clone(), generation);
        self.clients.write().insert(client_id.clone(), session);
        info!(
            "Client {} connected (total: {})",
            client_id,
            self.client_count()
        );
        generation
    }

    pub fn unregister_client(&self, client_id: &str, expected_generation: ClientGeneration) {
        let mut clients = self.clients.write();
        match clients.get(client_id) {
            Some(session) if session.generation == expected_generation => {}
            _ => return,
        }
        if let Some(session) = clients.remove(client_id) {
            let duration = session.connected_elapsed();
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

    pub fn record_frame_sent(&self, client_id: &str) {
        if let Some(session) = self.clients.write().get_mut(client_id) {
            session.last_activity = Instant::now();
            session.frames_sent += 1;
            session.fps_calculator.record_frame();
        }
    }

    pub fn get_clients_stat(&self) -> HashMap<String, crate::events::types::ClientStats> {
        // write() because `current_fps()` mutates the underlying VecDeque
        // to prune stale samples. Held for ~microseconds, called once per
        // second by the stats broadcaster.
        self.clients
            .write()
            .iter_mut()
            .map(|(id, session)| {
                (
                    id.clone(),
                    crate::events::types::ClientStats {
                        id: id.clone(),
                        fps: session.fps_calculator.current_fps(),
                        connected_secs: session.connected_elapsed().as_secs(),
                    },
                )
            })
            .collect()
    }

    pub fn auto_pause_config(&self) -> AutoPauseConfig {
        self.auto_pause_config.read().clone()
    }

    pub fn set_auto_pause_config(&self, config: AutoPauseConfig) {
        info!(
            "Auto-pause config updated: enabled={}, delay={}s, timeout={}s",
            config.enabled, config.shutdown_delay_secs, config.client_timeout_secs
        );
        *self.auto_pause_config.write() = config;
    }

    pub fn current_frame(&self) -> Option<VideoFrame> {
        (**self.current_frame.load()).clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.frame_notify.subscribe()
    }

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
        self.set_offline();
    }
}

pub struct ClientGuard {
    client_id: ClientId,
    generation: ClientGeneration,
    handler: Arc<MjpegStreamHandler>,
}

impl ClientGuard {
    pub fn new(client_id: ClientId, handler: Arc<MjpegStreamHandler>) -> Self {
        let generation = handler.register_client(client_id.clone());
        Self {
            client_id,
            generation,
            handler,
        }
    }

    pub fn id(&self) -> &ClientId {
        &self.client_id
    }
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.handler
            .unregister_client(&self.client_id, self.generation);
    }
}

impl MjpegStreamHandler {
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

                {
                    let clients = handler.clients.read();
                    for (id, session) in clients.iter() {
                        if now.duration_since(session.last_activity) > timeout {
                            stale.push(id.clone());
                        }
                    }
                }

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

fn frames_are_identical(a: &VideoFrame, b: &VideoFrame) -> bool {
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

    let a_data = a.data();
    let b_data = b.data();
    let len = a_data.len();

    if len <= 256 {
        return a_data == b_data;
    }

    const SAMPLE: usize = 16;
    debug_assert!(len == b_data.len());

    if a_data[..SAMPLE] != b_data[..SAMPLE] {
        return false;
    }
    if a_data[len - SAMPLE..] != b_data[len - SAMPLE..] {
        return false;
    }

    let quarter = len / 4;
    let quarter_start = quarter.saturating_sub(SAMPLE / 2);
    if a_data[quarter_start..quarter_start + SAMPLE]
        != b_data[quarter_start..quarter_start + SAMPLE]
    {
        return false;
    }
    let mid = len / 2;
    let mid_start = mid.saturating_sub(SAMPLE / 2);
    if a_data[mid_start..mid_start + SAMPLE] != b_data[mid_start..mid_start + SAMPLE] {
        return false;
    }

    a.get_hash() == b.get_hash()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::{format::Resolution, PixelFormat};
    use bytes::Bytes;

    #[tokio::test]
    async fn test_stream_handler() {
        let handler = MjpegStreamHandler::new();
        assert!(!handler.is_online());
        assert_eq!(handler.client_count(), 0);

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

        assert_eq!(calc.current_fps(), 0);

        calc.record_frame();
        calc.record_frame();
        calc.record_frame();

        assert_eq!(calc.current_fps(), 3);
        assert_eq!(calc.frame_times.len(), 3);
    }

    #[test]
    fn test_fps_calculator_decays_without_new_frames() {
        let mut calc = FpsCalculator::new();
        calc.window = Duration::from_millis(50);

        calc.record_frame();
        calc.record_frame();
        assert_eq!(calc.current_fps(), 2);

        std::thread::sleep(Duration::from_millis(80));

        assert_eq!(calc.current_fps(), 0);
        assert!(calc.frame_times.is_empty());
    }

    #[test]
    fn test_client_guard_generation_isolation() {
        let handler = Arc::new(MjpegStreamHandler::new());
        let id = "shared-id".to_string();

        let stale = ClientGuard::new(id.clone(), handler.clone());
        let stale_gen = stale.generation;

        let fresh = ClientGuard::new(id.clone(), handler.clone());
        assert_ne!(stale_gen, fresh.generation);
        assert_eq!(handler.client_count(), 1);

        drop(stale);
        assert_eq!(handler.client_count(), 1);

        drop(fresh);
        assert_eq!(handler.client_count(), 0);
    }
}
