//! ALSA 48 kHz stereo → Opus 20 ms frames, fan-out per subscriber.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, watch, Mutex as AsyncMutex, RwLock};
use tracing::{error, info, warn};

use super::capture::{AudioCapturer, AudioConfig, AudioFrame, CaptureState};
use super::encoder::{OpusConfig, OpusEncoder, OpusFrame};
use crate::error::{AppError, Result};
use bytemuck;
use bytes::Bytes;

/// 48 kHz stereo: 20 ms = 960 × 2 samples (S16LE).
const OPUS_STEREO_SAMPLES: usize = 960 * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioStreamState {
    #[default]
    Stopped,
    Starting,
    Running,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct AudioStreamerConfig {
    pub capture: AudioConfig,
    pub opus: OpusConfig,
}

impl AudioStreamerConfig {
    pub fn for_device(device_name: &str) -> Self {
        Self {
            capture: AudioConfig {
                device_name: device_name.to_string(),
                ..Default::default()
            },
            opus: OpusConfig::default(),
        }
    }

    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.opus.bitrate = bitrate;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct AudioStreamStats {
    pub subscriber_count: usize,
}

pub struct AudioStreamer {
    config: RwLock<AudioStreamerConfig>,
    state: watch::Sender<AudioStreamState>,
    state_rx: watch::Receiver<AudioStreamState>,
    capturer: RwLock<Option<Arc<AudioCapturer>>>,
    encoder: Arc<AsyncMutex<Option<OpusEncoder>>>,
    opus_subscribers: Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
    stop_flag: Arc<AtomicBool>,
}

impl AudioStreamer {
    pub fn new() -> Self {
        Self::with_config(AudioStreamerConfig::default())
    }

    pub fn with_config(config: AudioStreamerConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(AudioStreamState::Stopped);

        Self {
            config: RwLock::new(config),
            state: state_tx,
            state_rx,
            capturer: RwLock::new(None),
            encoder: Arc::new(AsyncMutex::new(None)),
            opus_subscribers: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn state(&self) -> AudioStreamState {
        *self.state_rx.borrow()
    }

    pub fn state_watch(&self) -> watch::Receiver<AudioStreamState> {
        self.state_rx.clone()
    }

    pub fn subscribe_opus(&self) -> mpsc::Receiver<Arc<OpusFrame>> {
        let (tx, rx) = mpsc::channel::<Arc<OpusFrame>>(128);
        self.opus_subscribers.lock().unwrap().push(tx);
        rx
    }

    pub fn subscriber_count(&self) -> usize {
        self.opus_subscribers
            .lock()
            .unwrap()
            .iter()
            .filter(|s| !s.is_closed())
            .count()
    }

    pub fn stats(&self) -> AudioStreamStats {
        AudioStreamStats {
            subscriber_count: self.subscriber_count(),
        }
    }

    pub async fn set_config(&self, config: AudioStreamerConfig) -> Result<()> {
        if self.state() != AudioStreamState::Stopped {
            return Err(AppError::AudioError(
                "Cannot change config while streaming".to_string(),
            ));
        }
        *self.config.write().await = config;
        Ok(())
    }

    pub async fn set_bitrate(&self, bitrate: u32) -> Result<()> {
        self.config.write().await.opus.bitrate = bitrate;

        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate)?;
        }

        info!("Audio bitrate changed to {}bps", bitrate);
        Ok(())
    }

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

        let capturer = Arc::new(AudioCapturer::new(config.capture.clone()));
        *self.capturer.write().await = Some(capturer.clone());

        let encoder = OpusEncoder::new(config.opus.clone())?;
        *self.encoder.lock().await = Some(encoder);

        capturer.start().await?;

        let capturer_for_task = capturer.clone();
        let encoder = self.encoder.clone();
        let opus_subscribers = self.opus_subscribers.clone();
        let state = self.state.clone();
        let stop_flag = self.stop_flag.clone();

        tokio::spawn(async move {
            Self::stream_task(
                capturer_for_task,
                encoder,
                opus_subscribers,
                state,
                stop_flag,
            )
            .await;
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if self.state() == AudioStreamState::Stopped {
            return Ok(());
        }

        info!("Stopping audio stream");

        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(ref capturer) = *self.capturer.read().await {
            capturer.stop().await?;
        }

        *self.capturer.write().await = None;
        *self.encoder.lock().await = None;
        self.opus_subscribers.lock().unwrap().clear();

        let _ = self.state.send(AudioStreamState::Stopped);
        info!("Audio stream stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.state() == AudioStreamState::Running
    }

    async fn fanout_opus(
        subscribers: &Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
        frame: Arc<OpusFrame>,
    ) {
        let txs: Vec<_> = {
            let g = subscribers.lock().unwrap();
            if g.is_empty() {
                return;
            }
            g.clone()
        };
        for tx in &txs {
            let _ = tx.send(frame.clone()).await;
        }
        if txs.iter().any(|tx| tx.is_closed()) {
            let mut g = subscribers.lock().unwrap();
            g.retain(|tx| !tx.is_closed());
        }
    }

    async fn stream_task(
        capturer: Arc<AudioCapturer>,
        encoder: Arc<AsyncMutex<Option<OpusEncoder>>>,
        opus_subscribers: Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
        state: watch::Sender<AudioStreamState>,
        stop_flag: Arc<AtomicBool>,
    ) {
        let mut pcm_rx = capturer.subscribe();
        let _ = state.send(AudioStreamState::Running);

        info!("Audio stream task started (48 kHz stereo → Opus, mpsc fan-out)");

        let mut pending: Vec<i16> = Vec::new();

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            if capturer.state() == CaptureState::Error {
                error!("Audio capture error, stopping stream");
                let _ = state.send(AudioStreamState::Error);
                break;
            }

            let recv_result =
                tokio::time::timeout(std::time::Duration::from_secs(2), pcm_rx.recv()).await;

            match recv_result {
                Ok(Ok(audio_frame)) => {
                    if audio_frame.sample_rate != 48_000 || audio_frame.channels != 2 {
                        warn!(
                            "Skip non–48 kHz/stereo PCM ({} Hz, {} ch)",
                            audio_frame.sample_rate, audio_frame.channels
                        );
                        continue;
                    }

                    let samples: &[i16] = match bytemuck::try_cast_slice(&audio_frame.data) {
                        Ok(s) => s,
                        Err(_) => {
                            warn!("Audio frame size not multiple of 2; skipping");
                            continue;
                        }
                    };
                    if !samples.is_empty() {
                        pending.extend_from_slice(samples);
                    }

                    while pending.len() >= OPUS_STEREO_SAMPLES {
                        let pcm_20ms = Bytes::copy_from_slice(bytemuck::cast_slice(
                            &pending[..OPUS_STEREO_SAMPLES],
                        ));
                        pending.drain(..OPUS_STEREO_SAMPLES);

                        let frame_48k = AudioFrame::new_interleaved(pcm_20ms, 2, 48_000, 0);

                        let opus_result = {
                            let mut enc_guard = encoder.lock().await;
                            (*enc_guard)
                                .as_mut()
                                .map(|enc| enc.encode_frame(&frame_48k))
                        };

                        match opus_result {
                            Some(Ok(opus_frame)) => {
                                Self::fanout_opus(&opus_subscribers, Arc::new(opus_frame)).await;
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
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    info!("Audio capture channel closed");
                    break;
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("PCM receiver lagged by {} frames", n);
                }
                Err(_) => {
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
