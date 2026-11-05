//! ALSA 48 kHz stereo → Opus 20 ms frames, fan-out per subscriber.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, watch, Mutex as AsyncMutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::capture::{AudioCapturer, AudioConfig, CaptureState};
use super::encoder::{OpusConfig, OpusEncoder, OpusFrame};
use crate::error::{AppError, Result};

/// 48 kHz stereo: 20 ms = 960 × 2 samples (S16LE).
const OPUS_STEREO_SAMPLES: usize = 960 * 2;

struct PcmFrames<'audio> {
    pending: &'audio mut Vec<i16>,
    samples: &'audio [i16],
}

impl<'audio> PcmFrames<'audio> {
    fn new(pending: &'audio mut Vec<i16>, samples: &'audio [i16]) -> Self {
        Self { pending, samples }
    }

    fn next_frame(&mut self) -> Option<&[i16]> {
        if self.pending.len() == OPUS_STEREO_SAMPLES {
            self.pending.clear();
        }
        if self.pending.is_empty() && self.samples.len() >= OPUS_STEREO_SAMPLES {
            let (frame, remaining) = self.samples.split_at(OPUS_STEREO_SAMPLES);
            self.samples = remaining;
            return Some(frame);
        }
        if self.samples.is_empty() {
            return None;
        }
        let count = (OPUS_STEREO_SAMPLES - self.pending.len()).min(self.samples.len());
        if self.pending.capacity() < OPUS_STEREO_SAMPLES {
            self.pending
                .reserve_exact(OPUS_STEREO_SAMPLES - self.pending.len());
        }
        self.pending.extend_from_slice(&self.samples[..count]);
        self.samples = &self.samples[count..];
        (self.pending.len() == OPUS_STEREO_SAMPLES).then(|| self.pending.as_slice())
    }
}

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
}

pub struct AudioStreamer {
    config: RwLock<AudioStreamerConfig>,
    state: watch::Sender<AudioStreamState>,
    state_rx: watch::Receiver<AudioStreamState>,
    capturer: RwLock<Option<Arc<AudioCapturer>>>,
    encoder: Arc<AsyncMutex<Option<OpusEncoder>>>,
    opus_subscribers: Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
    stop_flag: Arc<AtomicBool>,
    shutdown_generation: watch::Sender<u64>,
    lifecycle: AsyncMutex<()>,
    stream_task: AsyncMutex<Option<JoinHandle<()>>>,
}

impl AudioStreamer {
    pub fn new() -> Self {
        Self::with_config(AudioStreamerConfig::default())
    }

    pub fn with_config(config: AudioStreamerConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(AudioStreamState::Stopped);
        let (shutdown_generation, _) = watch::channel(0);

        Self {
            config: RwLock::new(config),
            state: state_tx,
            state_rx,
            capturer: RwLock::new(None),
            encoder: Arc::new(AsyncMutex::new(None)),
            opus_subscribers: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            shutdown_generation,
            lifecycle: AsyncMutex::new(()),
            stream_task: AsyncMutex::new(None),
        }
    }

    pub fn state(&self) -> AudioStreamState {
        *self.state_rx.borrow()
    }

    pub fn state_watch(&self) -> watch::Receiver<AudioStreamState> {
        self.state_rx.clone()
    }

    pub fn subscribe_opus(&self) -> mpsc::Receiver<Arc<OpusFrame>> {
        // Keep latency bounded for real-time consumers. Slow receivers lose
        // new frames instead of accumulating seconds of stale audio.
        let (tx, rx) = mpsc::channel::<Arc<OpusFrame>>(4);
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

    pub async fn set_bitrate(&self, bitrate: u32) -> Result<()> {
        self.config.write().await.opus.bitrate = bitrate;

        if let Some(ref mut encoder) = *self.encoder.lock().await {
            encoder.set_bitrate(bitrate)?;
        }

        info!("Audio bitrate changed to {}bps", bitrate);
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        if matches!(
            self.state(),
            AudioStreamState::Starting | AudioStreamState::Running
        ) {
            return Ok(());
        }

        // Error and stopped states may still own completed task handles. Reap
        // them before installing a new capture pipeline so restart is a clean
        // lifecycle transition rather than an overwrite of old resources.
        if let Some(capturer) = self.capturer.write().await.take() {
            let _ = capturer.stop().await;
        }
        if let Some(task) = self.stream_task.lock().await.take() {
            let _ = task.await;
        }
        *self.encoder.lock().await = None;

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

        let encoder = match OpusEncoder::new(config.opus.clone()) {
            Ok(encoder) => encoder,
            Err(error) => {
                let _ = self.state.send(AudioStreamState::Error);
                return Err(error);
            }
        };
        *self.encoder.lock().await = Some(encoder);

        let capturer = Arc::new(AudioCapturer::new(config.capture.clone()));
        *self.capturer.write().await = Some(capturer.clone());
        if let Err(error) = capturer.start().await {
            self.cleanup_failed_start(&capturer).await;
            return Err(error);
        }

        let mut capture_state = capturer.state_watch();
        let startup_result = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let current_state = *capture_state.borrow();
                match current_state {
                    CaptureState::Running => return Ok(()),
                    CaptureState::Error => {
                        return Err(AppError::AudioError(
                            "Audio capture failed to start".to_string(),
                        ))
                    }
                    CaptureState::Stopped | CaptureState::Starting => {
                        if capture_state.changed().await.is_err() {
                            return Err(AppError::AudioError(
                                "Audio capture stopped during startup".to_string(),
                            ));
                        }
                    }
                }
            }
        })
        .await;

        match startup_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                self.cleanup_failed_start(&capturer).await;
                return Err(e);
            }
            Err(_) => {
                self.cleanup_failed_start(&capturer).await;
                return Err(AppError::AudioError(
                    "Timed out waiting for audio capture to start".to_string(),
                ));
            }
        }

        let capturer_for_task = capturer.clone();
        let encoder = self.encoder.clone();
        let opus_subscribers = self.opus_subscribers.clone();
        let state = self.state.clone();
        let stop_flag = self.stop_flag.clone();
        let shutdown_rx = self.shutdown_generation.subscribe();
        let _ = self.state.send(AudioStreamState::Running);

        let task = tokio::spawn(async move {
            Self::stream_task(
                capturer_for_task,
                encoder,
                opus_subscribers,
                state,
                stop_flag,
                shutdown_rx,
            )
            .await;
        });
        *self.stream_task.lock().await = Some(task);

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        if self.state() == AudioStreamState::Stopped {
            return Ok(());
        }

        info!("Stopping audio stream");

        self.stop_flag.store(true, Ordering::SeqCst);
        self.shutdown_generation.send_modify(|generation| {
            *generation = generation.wrapping_add(1);
        });

        if let Some(ref capturer) = *self.capturer.read().await {
            capturer.stop().await?;
        }
        if let Some(task) = self.stream_task.lock().await.take() {
            let _ = task.await;
        }

        *self.capturer.write().await = None;
        *self.encoder.lock().await = None;
        self.opus_subscribers.lock().unwrap().clear();

        let _ = self.state.send(AudioStreamState::Stopped);
        info!("Audio stream stopped");
        Ok(())
    }

    async fn cleanup_failed_start(&self, capturer: &AudioCapturer) {
        let _ = capturer.stop().await;
        *self.capturer.write().await = None;
        *self.encoder.lock().await = None;
        let _ = self.state.send(AudioStreamState::Error);
    }

    pub fn is_running(&self) -> bool {
        self.state() == AudioStreamState::Running
    }

    fn fanout_opus(
        subscribers: &Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
        frame: Arc<OpusFrame>,
    ) {
        let mut subscribers = subscribers.lock().unwrap();
        subscribers.retain(|subscriber| match subscriber.try_send(frame.clone()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        });
    }

    async fn stream_task(
        capturer: Arc<AudioCapturer>,
        encoder: Arc<AsyncMutex<Option<OpusEncoder>>>,
        opus_subscribers: Arc<Mutex<Vec<mpsc::Sender<Arc<OpusFrame>>>>>,
        state: watch::Sender<AudioStreamState>,
        stop_flag: Arc<AtomicBool>,
        mut shutdown_rx: watch::Receiver<u64>,
    ) {
        let mut pcm_rx = capturer.subscribe();

        debug!("Audio stream task started (48 kHz stereo → Opus, mpsc fan-out)");

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

            let recv_result = tokio::select! {
                biased;
                changed = shutdown_rx.changed() => {
                    if changed.is_ok() || stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                }
                result = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    pcm_rx.recv(),
                ) => result,
            };

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
                    let mut frames = PcmFrames::new(&mut pending, samples);
                    while let Some(samples) = frames.next_frame() {
                        let opus_result = {
                            let mut enc_guard = encoder.lock().await;
                            (*enc_guard).as_mut().map(|enc| enc.encode(samples))
                        };

                        match opus_result {
                            Some(Ok(opus_frame)) => {
                                Self::fanout_opus(&opus_subscribers, Arc::new(opus_frame));
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
                        let _ = state.send(AudioStreamState::Error);
                        break;
                    }
                }
            }
        }

        if stop_flag.load(Ordering::Relaxed) {
            let _ = state.send(AudioStreamState::Stopped);
        } else {
            opus_subscribers.lock().unwrap().clear();
            let _ = capturer.stop().await;
        }
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
    use bytes::Bytes;

    #[test]
    fn aligned_pcm_frames_borrow_input_without_allocating() {
        let samples = vec![123; OPUS_STEREO_SAMPLES * 2];
        let mut pending = Vec::new();
        let mut frames = PcmFrames::new(&mut pending, &samples);
        assert_eq!(frames.next_frame().unwrap().as_ptr(), samples.as_ptr());
        assert_eq!(
            frames.next_frame().unwrap().as_ptr(),
            samples[OPUS_STEREO_SAMPLES..].as_ptr()
        );
        assert!(frames.next_frame().is_none());
        assert!(pending.is_empty());
        assert_eq!(pending.capacity(), 0);
    }

    #[test]
    fn fragmented_pcm_preserves_samples_and_opus_packets() {
        let samples: Vec<i16> = (0..OPUS_STEREO_SAMPLES * 8 + 31)
            .map(|index| ((index * 73 % 65536) as i32 - 32768) as i16)
            .collect();
        let mut pending = Vec::new();
        let mut actual = Vec::new();
        let mut encoder = OpusEncoder::new(OpusConfig::default()).unwrap();
        let mut reference_encoder = OpusEncoder::new(OpusConfig::default()).unwrap();
        let mut offset = 0;
        for size in [1, 317, 4601, 2000, 11, 8470, samples.len()] {
            let end = (offset + size).min(samples.len());
            let mut frames = PcmFrames::new(&mut pending, &samples[offset..end]);
            while let Some(frame) = frames.next_frame() {
                actual.extend_from_slice(frame);
                let encoded = encoder.encode(frame).unwrap();
                let start = actual.len() - OPUS_STEREO_SAMPLES;
                let reference = reference_encoder
                    .encode(&samples[start..actual.len()])
                    .unwrap();
                assert_eq!(encoded.data, reference.data);
                assert_eq!(encoded.sequence, reference.sequence);
                assert_eq!(encoded.duration_ms, reference.duration_ms);
            }
            assert!(pending.len() < OPUS_STEREO_SAMPLES);
            assert!(pending.capacity() <= OPUS_STEREO_SAMPLES);
            offset = end;
        }
        assert_eq!(actual, samples[..OPUS_STEREO_SAMPLES * 8]);
        assert_eq!(pending, samples[OPUS_STEREO_SAMPLES * 8..]);
    }

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

    #[test]
    fn slow_subscriber_does_not_block_or_grow_unbounded() {
        let streamer = AudioStreamer::new();
        let mut receiver = streamer.subscribe_opus();
        for sequence in 0..20 {
            AudioStreamer::fanout_opus(
                &streamer.opus_subscribers,
                Arc::new(OpusFrame {
                    data: Bytes::from_static(&[1]),
                    duration_ms: 20,
                    sequence,
                }),
            );
        }

        let mut received = 0;
        while receiver.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 4);
    }

    #[test]
    fn closed_subscriber_is_pruned() {
        let streamer = AudioStreamer::new();
        let receiver = streamer.subscribe_opus();
        drop(receiver);
        AudioStreamer::fanout_opus(
            &streamer.opus_subscribers,
            Arc::new(OpusFrame {
                data: Bytes::from_static(&[1]),
                duration_ms: 20,
                sequence: 0,
            }),
        );
        assert_eq!(streamer.subscriber_count(), 0);
    }
}
