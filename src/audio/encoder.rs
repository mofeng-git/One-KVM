//! Opus encoder.

use audiopus::coder::GenericCtl;
use audiopus::{coder::Encoder, Application, Bitrate, Channels, SampleRate};
use bytes::Bytes;
use tracing::info;

use super::capture::AudioFrame;
use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct OpusConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: u32,
    pub application: OpusApplication,
    pub fec: bool,
}

impl Default for OpusConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 64000,
            application: OpusApplication::Audio,
            fec: true,
        }
    }
}

impl OpusConfig {
    pub fn voice() -> Self {
        Self {
            application: OpusApplication::Voip,
            bitrate: 32000,
            ..Default::default()
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusApplication {
    Voip,
    Audio,
    LowDelay,
}

#[derive(Debug, Clone)]
pub struct OpusFrame {
    pub data: Bytes,
    pub duration_ms: u32,
    pub sequence: u64,
}

impl OpusFrame {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

pub struct OpusEncoder {
    config: OpusConfig,
    encoder: Encoder,
    output_buffer: Vec<u8>,
    frame_count: u64,
}

impl OpusEncoder {
    pub fn new(config: OpusConfig) -> Result<Self> {
        let sample_rate = config.to_audiopus_sample_rate();
        let channels = config.to_audiopus_channels();
        let application = config.to_audiopus_application();

        let mut encoder = Encoder::new(sample_rate, channels, application)
            .map_err(|e| AppError::AudioError(format!("Failed to create Opus encoder: {:?}", e)))?;

        encoder
            .set_bitrate(Bitrate::BitsPerSecond(config.bitrate as i32))
            .map_err(|e| AppError::AudioError(format!("Failed to set bitrate: {:?}", e)))?;

        if config.fec {
            encoder
                .set_inband_fec(true)
                .map_err(|e| AppError::AudioError(format!("Failed to enable FEC: {:?}", e)))?;
        }

        info!(
            "Opus encoder created: {}Hz {}ch {}bps",
            config.sample_rate, config.channels, config.bitrate
        );

        Ok(Self {
            config,
            encoder,
            output_buffer: vec![0u8; 4000],
            frame_count: 0,
        })
    }

    pub fn encode(&mut self, pcm_data: &[i16]) -> Result<OpusFrame> {
        let encoded_len = self
            .encoder
            .encode(pcm_data, &mut self.output_buffer)
            .map_err(|e| AppError::AudioError(format!("Opus encode failed: {:?}", e)))?;

        let samples = pcm_data.len() as u32 / self.config.channels;
        let duration_ms = (samples * 1000) / self.config.sample_rate;

        self.frame_count += 1;

        Ok(OpusFrame {
            data: Bytes::copy_from_slice(&self.output_buffer[..encoded_len]),
            duration_ms,
            sequence: self.frame_count - 1,
        })
    }

    pub fn encode_frame(&mut self, frame: &AudioFrame) -> Result<OpusFrame> {
        let samples: &[i16] = bytemuck::cast_slice(&frame.data);
        self.encode(samples)
    }

    pub fn config(&self) -> &OpusConfig {
        &self.config
    }

    pub fn reset(&mut self) -> Result<()> {
        self.encoder
            .reset_state()
            .map_err(|e| AppError::AudioError(format!("Failed to reset encoder: {:?}", e)))?;
        self.frame_count = 0;
        Ok(())
    }

    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.encoder
            .set_bitrate(Bitrate::BitsPerSecond(bitrate as i32))
            .map_err(|e| AppError::AudioError(format!("Failed to set bitrate: {:?}", e)))?;
        Ok(())
    }
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

        let silence = vec![0i16; 960 * 2];
        let result = encoder.encode(&silence);
        assert!(result.is_ok());

        let frame = result.unwrap();
        assert!(!frame.is_empty());
        assert!(frame.len() < silence.len() * 2);
    }
}
