//! Opus audio encoder for WebRTC

use audiopus::coder::GenericCtl;
use audiopus::{coder::Encoder, Application, Bitrate, Channels, SampleRate};
use bytes::Bytes;
use std::time::Instant;
use tracing::{info, trace};

use super::capture::AudioFrame;
use crate::error::{AppError, Result};

/// Opus encoder configuration
#[derive(Debug, Clone)]
pub struct OpusConfig {
    /// Sample rate (must be 8000, 12000, 16000, 24000, or 48000)
    pub sample_rate: u32,
    /// Channels (1 or 2)
    pub channels: u32,
    /// Target bitrate in bps
    pub bitrate: u32,
    /// Application mode
    pub application: OpusApplication,
    /// Enable forward error correction
    pub fec: bool,
}

impl Default for OpusConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 64000, // 64 kbps
            application: OpusApplication::Audio,
            fec: true,
        }
    }
}

impl OpusConfig {
    /// Create config for voice (lower latency)
    pub fn voice() -> Self {
        Self {
            application: OpusApplication::Voip,
            bitrate: 32000,
            ..Default::default()
        }
    }

    /// Create config for music (higher quality)
    pub fn music() -> Self {
        Self {
            application: OpusApplication::Audio,
            bitrate: 128000,
            ..Default::default()
        }
    }

    fn to_audiopus_sample_rate(&self) -> SampleRate {
        match self.sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            _ => SampleRate::Hz48000,
        }
    }

    fn to_audiopus_channels(&self) -> Channels {
        if self.channels == 1 {
            Channels::Mono
        } else {
            Channels::Stereo
        }
    }

    fn to_audiopus_application(&self) -> Application {
        match self.application {
            OpusApplication::Voip => Application::Voip,
            OpusApplication::Audio => Application::Audio,
            OpusApplication::LowDelay => Application::LowDelay,
        }
    }
}

/// Opus application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusApplication {
    /// Voice over IP
    Voip,
    /// General audio
    Audio,
    /// Low delay mode
    LowDelay,
}

/// Encoded Opus frame
#[derive(Debug, Clone)]
pub struct OpusFrame {
    /// Encoded Opus data
    pub data: Bytes,
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Sequence number
    pub sequence: u64,
    /// Timestamp
    pub timestamp: Instant,
    /// RTP timestamp (samples)
    pub rtp_timestamp: u32,
}

impl OpusFrame {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Opus encoder
pub struct OpusEncoder {
    config: OpusConfig,
    encoder: Encoder,
    /// Output buffer
    output_buffer: Vec<u8>,
    /// Frame counter for RTP timestamp
    frame_count: u64,
    /// Samples per frame
    samples_per_frame: u32,
}

impl OpusEncoder {
    /// Create a new Opus encoder
    pub fn new(config: OpusConfig) -> Result<Self> {
        let sample_rate = config.to_audiopus_sample_rate();
        let channels = config.to_audiopus_channels();
        let application = config.to_audiopus_application();

        let mut encoder = Encoder::new(sample_rate, channels, application).map_err(|e| {
            AppError::AudioError(format!("Failed to create Opus encoder: {:?}", e))
        })?;

        // Configure encoder
        encoder
            .set_bitrate(Bitrate::BitsPerSecond(config.bitrate as i32))
            .map_err(|e| AppError::AudioError(format!("Failed to set bitrate: {:?}", e)))?;

        if config.fec {
            encoder
                .set_inband_fec(true)
                .map_err(|e| AppError::AudioError(format!("Failed to enable FEC: {:?}", e)))?;
        }

        // Calculate samples per frame (20ms at sample_rate)
        let samples_per_frame = config.sample_rate / 50;

        info!(
            "Opus encoder created: {}Hz {}ch {}bps",
            config.sample_rate, config.channels, config.bitrate
        );

        Ok(Self {
            config,
            encoder,
            output_buffer: vec![0u8; 4000], // Max Opus frame size
            frame_count: 0,
            samples_per_frame,
        })
    }

    /// Create with default configuration
    pub fn default_config() -> Result<Self> {
        Self::new(OpusConfig::default())
    }

    /// Encode PCM audio data (S16LE interleaved)
    pub fn encode(&mut self, pcm_data: &[i16]) -> Result<OpusFrame> {
        let encoded_len = self
            .encoder
            .encode(pcm_data, &mut self.output_buffer)
            .map_err(|e| AppError::AudioError(format!("Opus encode failed: {:?}", e)))?;

        let samples = pcm_data.len() as u32 / self.config.channels;
        let duration_ms = (samples * 1000) / self.config.sample_rate;
        let rtp_timestamp = (self.frame_count * self.samples_per_frame as u64) as u32;

        self.frame_count += 1;

        trace!(
            "Encoded {} samples to {} bytes Opus",
            pcm_data.len(),
            encoded_len
        );

        Ok(OpusFrame {
            data: Bytes::copy_from_slice(&self.output_buffer[..encoded_len]),
            duration_ms,
            sequence: self.frame_count - 1,
            timestamp: Instant::now(),
            rtp_timestamp,
        })
    }

    /// Encode from AudioFrame
    ///
    /// Uses zero-copy conversion from bytes to i16 samples via bytemuck.
    pub fn encode_frame(&mut self, frame: &AudioFrame) -> Result<OpusFrame> {
        // Zero-copy: directly cast bytes to i16 slice
        // AudioFrame.data is S16LE format, which matches native little-endian i16
        let samples: &[i16] = bytemuck::cast_slice(&frame.data);
        self.encode(samples)
    }

    /// Get encoder configuration
    pub fn config(&self) -> &OpusConfig {
        &self.config
    }

    /// Reset encoder state
    pub fn reset(&mut self) -> Result<()> {
        self.encoder
            .reset_state()
            .map_err(|e| AppError::AudioError(format!("Failed to reset encoder: {:?}", e)))?;
        self.frame_count = 0;
        Ok(())
    }

    /// Set bitrate dynamically
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.encoder
            .set_bitrate(Bitrate::BitsPerSecond(bitrate as i32))
            .map_err(|e| AppError::AudioError(format!("Failed to set bitrate: {:?}", e)))?;
        Ok(())
    }
}

/// Audio encoder statistics
#[derive(Debug, Clone, Default)]
pub struct EncoderStats {
    pub frames_encoded: u64,
    pub bytes_output: u64,
    pub avg_frame_size: usize,
    pub current_bitrate: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_config_default() {
        let config = OpusConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bitrate, 64000);
    }

    #[test]
    fn test_create_encoder() {
        let config = OpusConfig::default();
        let encoder = OpusEncoder::new(config);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_encode_silence() {
        let config = OpusConfig::default();
        let mut encoder = OpusEncoder::new(config).unwrap();

        // 20ms of stereo silence at 48kHz
        let silence = vec![0i16; 960 * 2];
        let result = encoder.encode(&silence);
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert!(!frame.is_empty());
        assert!(frame.len() < silence.len() * 2); // Should be compressed
    }
}
