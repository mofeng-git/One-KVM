//! Audio streaming pipeline
//!
//! Coordinates audio capture and Opus encoding, distributing encoded
//! frames to multiple subscribers via broadcast channel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing::{error, info, warn};

use super::capture::{AudioCapturer, AudioConfig, CaptureState};
use super::encoder::{OpusConfig, OpusEncoder, OpusFrame};
use crate::error::{AppError, Result};

/// Audio stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioStreamState {
    /// Stream is stopped
    #[default]
    Stopped,
    /// Stream is starting up
    Starting,
    /// Stream is running
    Running,
    /// Stream encountered an error
    Error,
}

/// Audio streamer configuration
#[derive(Debug, Clone, Default)]
pub struct AudioStreamerConfig {
    /// Audio capture configuration
    pub capture: AudioConfig,
    /// Opus encoder configuration
    pub opus: OpusConfig,
}

impl AudioStreamerConfig {
    /// Create config for a specific device with default quality
    pub fn for_device(device_name: &str) -> Self {
        Self {
            capture: AudioConfig {
                device_name: device_name.to_string(),
                ..Default::default()
            },
            opus: OpusConfig::default(),
        }
    }

    /// Create config with specified bitrate
    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.opus.bitrate = bitrate;
        self
    }
}

/// Audio stream statistics
#[derive(Debug, Clone, Default)]
pub struct AudioStreamStats {
    /// Frames encoded to Opus
    /// Number of active subscribers
    pub subscriber_count: usize,
}

/// Audio streamer
///
/// Manages the audio capture -> encode -> broadcast pipeline.
pub struct AudioStreamer {
    config: RwLock<AudioStreamerConfig>,
    state: watch::Sender<AudioStreamState>,
    state_rx: watch::Receiver<AudioStreamState>,
    capturer: RwLock<Option<Arc<AudioCapturer>>>,
    encoder: Arc<Mutex<Option<OpusEncoder>>>,
    opus_tx: watch::Sender<Option<Arc<OpusFrame>>>,
    stats: Arc<Mutex<AudioStreamStats>>,
    sequence: AtomicU64,
    stream_start_time: RwLock<Option<Instant>>,
    stop_flag: Arc<AtomicBool>,
}

impl AudioStreamer {
    /// Create a new audio streamer with default configuration
    pub fn new() -> Self {
        Self::with_config(AudioStreamerConfig::default())
    }

    /// Create a new audio streamer with specified configuration
    pub fn with_config(config: AudioStreamerConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(AudioStreamState::Stopped);
        let (opus_tx, _opus_rx) = watch::channel(None);

        Self {
            config: RwLock::new(config),
            state: state_tx,
            state_rx,
            capturer: RwLock::new(None),
            encoder: Arc::new(Mutex::new(None)),
            opus_tx,
            stats: Arc::new(Mutex::new(AudioStreamStats::default())),
            sequence: AtomicU64::new(0),
            stream_start_time: RwLock::new(None),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get current state
    pub fn state(&self) -> AudioStreamState {
        *self.state_rx.borrow()
    }

    /// Subscribe to state changes
    pub fn state_watch(&self) -> watch::Receiver<AudioStreamState> {
        self.state_rx.clone()
    }

    /// Subscribe to Opus frames
    pub fn subscribe_opus(&self) -> watch::Receiver<Option<Arc<OpusFrame>>> {
        self.opus_tx.subscribe()
    }

    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.opus_tx.receiver_count()
    }

    /// Get current statistics
    pub async fn stats(&self) -> AudioStreamStats {
        let mut stats = self.stats.lock().await.clone();
        stats.subscriber_count = self.subscriber_count();
        stats
    }

    /// Update configuration (only when stopped)
    pub async fn set_config(&self, config: AudioStreamerConfig) -> Result<()> {
        if self.state() != AudioStreamState::Stopped {
            return Err(AppError::AudioError(
                "Cannot change config while streaming".to_string(),
            ));
        }
        *self.config.write().await = config;
        Ok(())
    }

    /// Update bitrate dynamically (can be done while streaming)
    pub async fn set_bitrate(&self, bitrate: u32) -> Result<()> {
        // Update config
        self.config.write().await.opus.bitrate = bitrate;

        // Update encoder if running
        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate)?;
        }

        info!("Audio bitrate changed to {}bps", bitrate);
        Ok(())
    }

    /// Start the audio stream
    pub async fn start(&self) -> Result<()> {
        if self.state() == AudioStreamState::Running {
            return Ok(());
        }

        let _ = self.state.send(AudioStreamState::Starting);
        self.stop_flag.store(false, Ordering::SeqCst);

        let config = self.config.read().await.clone();

        info!(
            "Starting audio stream: {} @ {}Hz {}ch, {}bps Opus",
            config.capture.device_name,
            config.capture.sample_rate,
            config.capture.channels,
            config.opus.bitrate
        );

        // Create capturer
        let capturer = Arc::new(AudioCapturer::new(config.capture.clone()));
        *self.capturer.write().await = Some(capturer.clone());

        // Create encoder
        let encoder = OpusEncoder::new(config.opus.clone())?;
        *self.encoder.lock().await = Some(encoder);

        // Start capture
        capturer.start().await?;

        // Reset stats
        {
            let mut stats = self.stats.lock().await;
            *stats = AudioStreamStats::default();
        }

        // Record start time
        *self.stream_start_time.write().await = Some(Instant::now());
        self.sequence.store(0, Ordering::SeqCst);

        // Start encoding task
        let capturer_for_task = capturer.clone();
        let encoder = self.encoder.clone();
        let opus_tx = self.opus_tx.clone();
        let state = self.state.clone();
        let stop_flag = self.stop_flag.clone();

        tokio::spawn(async move {
            Self::stream_task(capturer_for_task, encoder, opus_tx, state, stop_flag).await;
        });

        Ok(())
    }

    /// Stop the audio stream
    pub async fn stop(&self) -> Result<()> {
        if self.state() == AudioStreamState::Stopped {
            return Ok(());
        }

        info!("Stopping audio stream");

        // Signal stop
        self.stop_flag.store(true, Ordering::SeqCst);

        // Stop capturer
        if let Some(ref capturer) = *self.capturer.read().await {
            capturer.stop().await?;
        }

        // Clear resources
        *self.capturer.write().await = None;
        *self.encoder.lock().await = None;
        *self.stream_start_time.write().await = None;

        let _ = self.state.send(AudioStreamState::Stopped);
        info!("Audio stream stopped");
        Ok(())
    }

    /// Check if streaming
    pub fn is_running(&self) -> bool {
        self.state() == AudioStreamState::Running
    }

    /// Internal streaming task
    async fn stream_task(
        capturer: Arc<AudioCapturer>,
        encoder: Arc<Mutex<Option<OpusEncoder>>>,
        opus_tx: watch::Sender<Option<Arc<OpusFrame>>>,
        state: watch::Sender<AudioStreamState>,
        stop_flag: Arc<AtomicBool>,
    ) {
        let mut pcm_rx = capturer.subscribe();
        let _ = state.send(AudioStreamState::Running);

        info!("Audio stream task started");

        loop {
            // Check stop flag (atomic, no async lock needed)
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Check capturer state
            if capturer.state() == CaptureState::Error {
                error!("Audio capture error, stopping stream");
                let _ = state.send(AudioStreamState::Error);
                break;
            }

            // Receive PCM frame with timeout
            let recv_result =
                tokio::time::timeout(std::time::Duration::from_secs(2), pcm_rx.recv()).await;

            match recv_result {
                Ok(Ok(audio_frame)) => {
                    // Encode to Opus
                    let opus_result = {
                        let mut enc_guard = encoder.lock().await;
                        (*enc_guard)
                            .as_mut()
                            .map(|enc| enc.encode_frame(&audio_frame))
                    };

                    match opus_result {
                        Some(Ok(opus_frame)) => {
                            // Publish latest frame to subscribers
                            if opus_tx.receiver_count() > 0 {
                                let _ = opus_tx.send(Some(Arc::new(opus_frame)));
                            }
                        }
                        Some(Err(e)) => {
                            error!("Opus encode error: {}", e);
                        }
                        None => {
                            warn!("Encoder not available");
                            break;
                        }
                    }
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    info!("Audio capture channel closed");
                    break;
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Audio receiver lagged by {} frames", n);
                }
                Err(_) => {
                    // Timeout - check if still capturing
                    if capturer.state() != CaptureState::Running {
                        info!("Audio capture stopped, ending stream task");
                        break;
                    }
                }
            }
        }

        let _ = state.send(AudioStreamState::Stopped);
        info!("Audio stream task ended");
    }
}

impl Default for AudioStreamer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streamer_config_default() {
        let config = AudioStreamerConfig::default();
        assert_eq!(config.capture.sample_rate, 48000);
        assert_eq!(config.opus.bitrate, 64000);
    }

    #[test]
    fn test_streamer_config_for_device() {
        let config = AudioStreamerConfig::for_device("hw:0,0");
        assert_eq!(config.capture.device_name, "hw:0,0");
    }

    #[tokio::test]
    async fn test_streamer_state() {
        let streamer = AudioStreamer::new();
        assert_eq!(streamer.state(), AudioStreamState::Stopped);
    }
}
