use bytes::Bytes;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tracing::{debug, info};

use super::{AudioConfig, AudioFrame, CaptureState};
use crate::audio::device::find_wasapi_device;
use crate::error::{AppError, Result};
use crate::utils::LogThrottler;

pub(super) fn run_capture(
    config: &AudioConfig,
    state: &watch::Sender<CaptureState>,
    frame_tx: &broadcast::Sender<AudioFrame>,
    stop_flag: &AtomicBool,
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
                if frame_tx.receiver_count() > 0 {
                    let frame = AudioFrame::new_interleaved(
                        Bytes::copy_from_slice(bytemuck::cast_slice(&samples)),
                        2,
                        48_000,
                    );
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
        .unwrap_or_else(|_| "Unknown WASAPI capture device".to_string())
}
