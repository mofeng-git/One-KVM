//! Shared Audio Pipeline for WebRTC
//!
//! This module provides a shared audio encoding pipeline that can serve
//! multiple WebRTC sessions with a single encoder instance.
//!
//! # Architecture
//!
//! ```text
//! AudioCapturer (ALSA)
//!        |
//!        v (broadcast::Receiver<AudioFrame>)
//! SharedAudioPipeline (single Opus encoder)
//!        |
//!        v (broadcast::Sender<OpusFrame>)
//!   ┌────┴────┬────────┬────────┐
//!   v         v        v        v
//! Session1  Session2  Session3  ...
//! (RTP)     (RTP)     (RTP)     (RTP)
//! ```
//!
//! # Key Features
//!
//! - **Single encoder**: All sessions share one Opus encoder
//! - **Broadcast distribution**: Encoded frames are broadcast to all subscribers
//! - **Dynamic bitrate**: Bitrate can be changed at runtime
//! - **Statistics**: Tracks encoding performance metrics

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

use super::capture::AudioFrame;
use super::encoder::{OpusConfig, OpusEncoder, OpusFrame};
use crate::error::{AppError, Result};

/// Shared audio pipeline configuration
#[derive(Debug, Clone)]
pub struct SharedAudioPipelineConfig {
    /// Sample rate (must match audio capture)
    pub sample_rate: u32,
    /// Number of channels (1 or 2)
    pub channels: u32,
    /// Target bitrate in bps
    pub bitrate: u32,
    /// Opus application mode
    pub application: OpusApplicationMode,
    /// Enable forward error correction
    pub fec: bool,
    /// Broadcast channel capacity
    pub channel_capacity: usize,
}

impl Default for SharedAudioPipelineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 64000,
            application: OpusApplicationMode::Audio,
            fec: true,
            channel_capacity: 16,  // Reduced from 64 for lower latency
        }
    }
}

impl SharedAudioPipelineConfig {
    /// Create config optimized for voice
    pub fn voice() -> Self {
        Self {
            bitrate: 32000,
            application: OpusApplicationMode::Voip,
            ..Default::default()
        }
    }

    /// Create config optimized for music/high quality
    pub fn high_quality() -> Self {
        Self {
            bitrate: 128000,
            application: OpusApplicationMode::Audio,
            ..Default::default()
        }
    }

    /// Convert to OpusConfig
    pub fn to_opus_config(&self) -> OpusConfig {
        OpusConfig {
            sample_rate: self.sample_rate,
            channels: self.channels,
            bitrate: self.bitrate,
            application: match self.application {
                OpusApplicationMode::Voip => super::encoder::OpusApplication::Voip,
                OpusApplicationMode::Audio => super::encoder::OpusApplication::Audio,
                OpusApplicationMode::LowDelay => super::encoder::OpusApplication::LowDelay,
            },
            fec: self.fec,
        }
    }
}

/// Opus application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusApplicationMode {
    /// Voice over IP - optimized for speech
    Voip,
    /// General audio - balanced quality
    Audio,
    /// Low delay mode - minimal latency
    LowDelay,
}

/// Shared audio pipeline statistics
#[derive(Debug, Clone, Default)]
pub struct SharedAudioPipelineStats {
    /// Frames received from audio capture
    pub frames_received: u64,
    /// Frames successfully encoded
    pub frames_encoded: u64,
    /// Frames dropped (encode errors)
    pub frames_dropped: u64,
    /// Total bytes encoded
    pub bytes_encoded: u64,
    /// Number of active subscribers
    pub subscribers: u64,
    /// Average encode time in milliseconds
    pub avg_encode_time_ms: f32,
    /// Current bitrate in bps
    pub current_bitrate: u32,
    /// Pipeline running time in seconds
    pub running_time_secs: f64,
}

/// Shared Audio Pipeline
///
/// Provides a single Opus encoder that serves multiple WebRTC sessions.
/// All sessions receive the same encoded audio stream via broadcast channel.
pub struct SharedAudioPipeline {
    /// Configuration
    config: RwLock<SharedAudioPipelineConfig>,
    /// Opus encoder (protected by mutex for encoding)
    encoder: Mutex<Option<OpusEncoder>>,
    /// Broadcast sender for encoded Opus frames
    opus_tx: broadcast::Sender<OpusFrame>,
    /// Running state
    running: AtomicBool,
    /// Statistics
    stats: Mutex<SharedAudioPipelineStats>,
    /// Start time for running time calculation
    start_time: RwLock<Option<Instant>>,
    /// Encode time accumulator for averaging
    encode_time_sum_us: AtomicU64,
    /// Encode count for averaging
    encode_count: AtomicU64,
    /// Stop signal (atomic for lock-free checking)
    stop_flag: AtomicBool,
    /// Encoding task handle
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl SharedAudioPipeline {
    /// Create a new shared audio pipeline
    pub fn new(config: SharedAudioPipelineConfig) -> Result<Arc<Self>> {
        let (opus_tx, _) = broadcast::channel(config.channel_capacity);

        Ok(Arc::new(Self {
            config: RwLock::new(config),
            encoder: Mutex::new(None),
            opus_tx,
            running: AtomicBool::new(false),
            stats: Mutex::new(SharedAudioPipelineStats::default()),
            start_time: RwLock::new(None),
            encode_time_sum_us: AtomicU64::new(0),
            encode_count: AtomicU64::new(0),
            stop_flag: AtomicBool::new(false),
            task_handle: Mutex::new(None),
        }))
    }

    /// Create with default configuration
    pub fn default_config() -> Result<Arc<Self>> {
        Self::new(SharedAudioPipelineConfig::default())
    }

    /// Start the audio encoding pipeline
    ///
    /// # Arguments
    /// * `audio_rx` - Receiver for raw audio frames from AudioCapturer
    pub async fn start(self: &Arc<Self>, audio_rx: broadcast::Receiver<AudioFrame>) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let config = self.config.read().await.clone();

        info!(
            "Starting shared audio pipeline: {}Hz {}ch {}bps",
            config.sample_rate, config.channels, config.bitrate
        );

        // Create encoder
        let opus_config = config.to_opus_config();
        let encoder = OpusEncoder::new(opus_config)?;
        *self.encoder.lock().await = Some(encoder);

        // Reset stats
        {
            let mut stats = self.stats.lock().await;
            *stats = SharedAudioPipelineStats::default();
            stats.current_bitrate = config.bitrate;
        }

        // Reset counters
        self.encode_time_sum_us.store(0, Ordering::SeqCst);
        self.encode_count.store(0, Ordering::SeqCst);
        *self.start_time.write().await = Some(Instant::now());
        self.stop_flag.store(false, Ordering::SeqCst);

        self.running.store(true, Ordering::SeqCst);

        // Start encoding task
        let pipeline = self.clone();
        let handle = tokio::spawn(async move {
            pipeline.encoding_task(audio_rx).await;
        });

        *self.task_handle.lock().await = Some(handle);

        info!("Shared audio pipeline started");
        Ok(())
    }

    /// Stop the audio encoding pipeline
    pub fn stop(&self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        info!("Stopping shared audio pipeline");

        // Signal stop (atomic, no lock needed)
        self.stop_flag.store(true, Ordering::SeqCst);

        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if pipeline is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Subscribe to encoded Opus frames
    pub fn subscribe(&self) -> broadcast::Receiver<OpusFrame> {
        self.opus_tx.subscribe()
    }

    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.opus_tx.receiver_count()
    }

    /// Get current statistics
    pub async fn stats(&self) -> SharedAudioPipelineStats {
        let mut stats = self.stats.lock().await.clone();
        stats.subscribers = self.subscriber_count() as u64;

        // Calculate average encode time
        let count = self.encode_count.load(Ordering::SeqCst);
        if count > 0 {
            let sum_us = self.encode_time_sum_us.load(Ordering::SeqCst);
            stats.avg_encode_time_ms = (sum_us as f64 / count as f64 / 1000.0) as f32;
        }

        // Calculate running time
        if let Some(start) = *self.start_time.read().await {
            stats.running_time_secs = start.elapsed().as_secs_f64();
        }

        stats
    }

    /// Set bitrate dynamically
    pub async fn set_bitrate(&self, bitrate: u32) -> Result<()> {
        // Update config
        self.config.write().await.bitrate = bitrate;

        // Update encoder if running
        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate)?;
        }

        // Update stats
        self.stats.lock().await.current_bitrate = bitrate;

        info!("Shared audio pipeline bitrate changed to {}bps", bitrate);
        Ok(())
    }

    /// Update configuration (requires restart)
    pub async fn update_config(&self, config: SharedAudioPipelineConfig) -> Result<()> {
        if self.is_running() {
            return Err(AppError::AudioError(
                "Cannot update config while pipeline is running".to_string(),
            ));
        }

        *self.config.write().await = config;
        Ok(())
    }

    /// Internal encoding task
    async fn encoding_task(self: Arc<Self>, mut audio_rx: broadcast::Receiver<AudioFrame>) {
        info!("Audio encoding task started");

        loop {
            // Check stop flag (atomic, no async lock needed)
            if self.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Receive audio frame with timeout
            let recv_result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                audio_rx.recv(),
            )
            .await;

            match recv_result {
                Ok(Ok(audio_frame)) => {
                    // Update received count
                    {
                        let mut stats = self.stats.lock().await;
                        stats.frames_received += 1;
                    }

                    // Encode frame
                    let encode_start = Instant::now();
                    let encode_result = {
                        let mut encoder_guard = self.encoder.lock().await;
                        if let Some(ref mut encoder) = *encoder_guard {
                            Some(encoder.encode_frame(&audio_frame))
                        } else {
                            None
                        }
                    };
                    let encode_time = encode_start.elapsed();

                    // Update encode time stats
                    self.encode_time_sum_us
                        .fetch_add(encode_time.as_micros() as u64, Ordering::SeqCst);
                    self.encode_count.fetch_add(1, Ordering::SeqCst);

                    match encode_result {
                        Some(Ok(opus_frame)) => {
                            // Update stats
                            {
                                let mut stats = self.stats.lock().await;
                                stats.frames_encoded += 1;
                                stats.bytes_encoded += opus_frame.data.len() as u64;
                            }

                            // Broadcast to subscribers
                            if self.opus_tx.receiver_count() > 0 {
                                if let Err(e) = self.opus_tx.send(opus_frame) {
                                    trace!("No audio subscribers: {}", e);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!("Opus encode error: {}", e);
                            let mut stats = self.stats.lock().await;
                            stats.frames_dropped += 1;
                        }
                        None => {
                            warn!("Encoder not available");
                            break;
                        }
                    }
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    info!("Audio source channel closed");
                    break;
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Audio pipeline lagged by {} frames", n);
                    let mut stats = self.stats.lock().await;
                    stats.frames_dropped += n;
                }
                Err(_) => {
                    // Timeout - check if still running
                    if !self.running.load(Ordering::SeqCst) {
                        break;
                    }
                    debug!("Audio receive timeout, continuing...");
                }
            }
        }

        // Cleanup
        self.running.store(false, Ordering::SeqCst);
        *self.encoder.lock().await = None;

        let stats = self.stats().await;
        info!(
            "Audio encoding task ended: {} frames encoded, {} dropped, {:.1}s runtime",
            stats.frames_encoded, stats.frames_dropped, stats.running_time_secs
        );
    }
}

impl Drop for SharedAudioPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SharedAudioPipelineConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bitrate, 64000);
    }

    #[test]
    fn test_config_voice() {
        let config = SharedAudioPipelineConfig::voice();
        assert_eq!(config.bitrate, 32000);
        assert_eq!(config.application, OpusApplicationMode::Voip);
    }

    #[test]
    fn test_config_high_quality() {
        let config = SharedAudioPipelineConfig::high_quality();
        assert_eq!(config.bitrate, 128000);
    }

    #[tokio::test]
    async fn test_pipeline_creation() {
        let config = SharedAudioPipelineConfig::default();
        let pipeline = SharedAudioPipeline::new(config);
        assert!(pipeline.is_ok());

        let pipeline = pipeline.unwrap();
        assert!(!pipeline.is_running());
        assert_eq!(pipeline.subscriber_count(), 0);
    }
}
