use bytes::Bytes;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, info};

use crate::audio::device::{find_wasapi_device, AudioDeviceInfo};
use crate::error::{AppError, Result};
use crate::error_throttled;
use crate::utils::LogThrottler;

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

        debug!(
            "Starting WASAPI audio capture on {} at {}Hz {}ch",
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
                error_throttled!(
                    log_throttler,
                    "capture_error",
                    "WASAPI audio capture error: {}",
                    e
                );
                let _ = state.send(CaptureState::Error);
            } else {
                let _ = state.send(CaptureState::Stopped);
            }
        });

        *self.capture_handle.lock().await = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping WASAPI audio capture");
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
    let device = find_wasapi_device(&config.device_name)?;
    let device_label = device_label(&device);

    let supported = select_input_config(&device, config)?;
    let sample_format = supported.sample_format();
    let input_channels = supported.channels() as u32;
    let input_rate = supported.sample_rate();
    let stream_config = StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: BufferSize::Fixed(config.period_frames.max(128)),
    };

    debug!(
        "WASAPI capture selected: {} @ {}Hz {}ch {:?}",
        device_label, input_rate, input_channels, sample_format
    );

    let (tx, rx) = mpsc::sync_channel::<Vec<i16>>(8);
    let (err_tx, err_rx) = mpsc::sync_channel::<String>(1);
    let callback_stop = Arc::new(AtomicBool::new(false));

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            stream_config,
            input_channels,
            input_rate,
            tx.clone(),
            err_tx.clone(),
            callback_stop.clone(),
        ),
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            stream_config,
            input_channels,
            input_rate,
            tx.clone(),
            err_tx.clone(),
            callback_stop.clone(),
        ),
        SampleFormat::U16 => build_stream::<u16>(
            &device,
            stream_config,
            input_channels,
            input_rate,
            tx.clone(),
            err_tx.clone(),
            callback_stop.clone(),
        ),
        other => {
            return Err(AppError::AudioError(format!(
                "Unsupported WASAPI sample format: {:?}",
                other
            )));
        }
    }?;

    stream
        .play()
        .map_err(|e| AppError::AudioError(format!("Failed to start WASAPI stream: {}", e)))?;

    let _ = state.send(CaptureState::Running);

    while !stop_flag.load(Ordering::Relaxed) {
        if let Ok(err) = err_rx.try_recv() {
            return Err(AppError::AudioError(format!(
                "WASAPI stream error for {}: {}",
                device_label, err
            )));
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(samples) => {
                if samples.is_empty() {
                    continue;
                }
                let seq = sequence.fetch_add(1, Ordering::Relaxed);
                let frame = AudioFrame::new_interleaved(
                    Bytes::copy_from_slice(bytemuck::cast_slice(&samples)),
                    2,
                    48_000,
                    seq,
                );
                if frame_tx.receiver_count() > 0 {
                    if let Err(e) = frame_tx.send(frame) {
                        debug!("No audio receivers: {}", e);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AppError::AudioError(format!(
                    "WASAPI capture callback stopped for {}",
                    device_label
                )));
            }
        }
    }

    callback_stop.store(true, Ordering::SeqCst);
    drop(stream);

    info!("WASAPI audio capture stopped");
    let _ = log_throttler;
    Ok(())
}

fn select_input_config(
    device: &cpal::Device,
    config: &AudioConfig,
) -> Result<cpal::SupportedStreamConfig> {
    let requested_rate = config.sample_rate;
    let mut fallback = None;

    let configs = device.supported_input_configs().map_err(|e| {
        AppError::AudioError(format!("Failed to query WASAPI input configs: {}", e))
    })?;

    for range in configs {
        let sample_format = range.sample_format();
        if !matches!(
            sample_format,
            SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
        ) {
            continue;
        }

        if fallback
            .as_ref()
            .is_none_or(|best: &cpal::SupportedStreamConfigRange| {
                range.cmp_default_heuristics(best).is_gt()
            })
        {
            fallback = Some(range);
        }

        if range.channels() >= 2
            && range.min_sample_rate() <= requested_rate
            && requested_rate <= range.max_sample_rate()
        {
            return Ok(range.with_sample_rate(requested_rate));
        }
    }

    if let Some(range) = fallback {
        let rate = if range.min_sample_rate() <= requested_rate
            && requested_rate <= range.max_sample_rate()
        {
            requested_rate
        } else {
            range.with_max_sample_rate().sample_rate()
        };
        return Ok(range.with_sample_rate(rate));
    }

    device.default_input_config().map_err(|e| {
        AppError::AudioError(format!(
            "No supported WASAPI input format found, and default config failed: {}",
            e
        ))
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    input_channels: u32,
    input_rate: u32,
    tx: mpsc::SyncSender<Vec<i16>>,
    err_tx: mpsc::SyncSender<String>,
    stop_flag: Arc<AtomicBool>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + SampleToI16,
{
    let mut converter = PcmConverter::new(input_channels, input_rate, 2, 48_000);
    let data_tx = tx.clone();
    let stream = device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                let pcm = converter.convert(data);
                if !pcm.is_empty() {
                    let _ = data_tx.try_send(pcm);
                }
            },
            move |err| {
                let _ = err_tx.try_send(err.to_string());
            },
            Some(Duration::from_secs(2)),
        )
        .map_err(|e| AppError::AudioError(format!("Failed to build WASAPI input stream: {}", e)))?;
    Ok(stream)
}

trait SampleToI16: Copy + Send + 'static {
    fn to_i16_sample(self) -> i16;
}

impl SampleToI16 for i16 {
    fn to_i16_sample(self) -> i16 {
        self
    }
}

impl SampleToI16 for u16 {
    fn to_i16_sample(self) -> i16 {
        (self as i32 - 32768).clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }
}

impl SampleToI16 for f32 {
    fn to_i16_sample(self) -> i16 {
        (self.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
    }
}

struct PcmConverter {
    input_channels: usize,
    input_rate: u32,
    output_channels: usize,
    output_rate: u32,
    input_position: u64,
    next_output_position: u64,
}

impl PcmConverter {
    fn new(input_channels: u32, input_rate: u32, output_channels: u32, output_rate: u32) -> Self {
        Self {
            input_channels: input_channels.max(1) as usize,
            input_rate: input_rate.max(1),
            output_channels: output_channels.max(1) as usize,
            output_rate: output_rate.max(1),
            input_position: 0,
            next_output_position: 0,
        }
    }

    fn convert<T: SampleToI16>(&mut self, input: &[T]) -> Vec<i16> {
        let frames = input.len() / self.input_channels;
        if frames == 0 {
            return Vec::new();
        }

        if self.input_rate == self.output_rate {
            self.input_position = self.input_position.saturating_add(frames as u64);
            return self.convert_channels(input, frames);
        }

        let start = self.input_position;
        let end = start.saturating_add(frames as u64);
        let mut out = Vec::with_capacity(
            ((frames as u64 * self.output_rate as u64 / self.input_rate as u64 + 2) as usize)
                * self.output_channels,
        );

        while self.source_position_for_output(self.next_output_position) < end {
            let src = self.source_position_for_output(self.next_output_position);
            if src >= start {
                let local = (src - start) as usize;
                self.push_frame(input, local.min(frames - 1), &mut out);
            }
            self.next_output_position = self.next_output_position.saturating_add(1);
        }

        self.input_position = end;
        out
    }

    fn source_position_for_output(&self, output_position: u64) -> u64 {
        output_position.saturating_mul(self.input_rate as u64) / self.output_rate as u64
    }

    fn convert_channels<T: SampleToI16>(&self, input: &[T], frames: usize) -> Vec<i16> {
        let mut out = Vec::with_capacity(frames * self.output_channels);
        for frame in 0..frames {
            self.push_frame(input, frame, &mut out);
        }
        out
    }

    fn push_frame<T: SampleToI16>(&self, input: &[T], frame: usize, out: &mut Vec<i16>) {
        let base = frame * self.input_channels;
        let left = input
            .get(base)
            .copied()
            .map(SampleToI16::to_i16_sample)
            .unwrap_or(0);
        let right = if self.input_channels > 1 {
            input
                .get(base + 1)
                .copied()
                .map(SampleToI16::to_i16_sample)
                .unwrap_or(left)
        } else {
            left
        };

        out.push(left);
        if self.output_channels > 1 {
            out.push(right);
        }
    }
}

fn device_label(device: &cpal::Device) -> String {
    device
        .description()
        .map(|desc| desc.to_string())
        .or_else(|_| {
            #[allow(deprecated)]
            device.name()
        })
        .unwrap_or_else(|_| "Unknown WASAPI capture device".to_string())
}
