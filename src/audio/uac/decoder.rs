use audiopus::coder::Decoder;
use audiopus::{Channels, SampleRate};

use crate::error::{AppError, Result};

const CHANNELS: usize = 2;
const MAX_PACKET_BYTES: usize = 1275;
const MAX_SAMPLES_PER_CHANNEL: usize = 5760;

pub struct UacOpusDecoder {
    decoder: Decoder,
    buffer: Vec<i16>,
}

impl UacOpusDecoder {
    pub fn new() -> Result<Self> {
        let decoder = Decoder::new(SampleRate::Hz48000, Channels::Stereo)
            .map_err(|error| AppError::AudioError(format!("Opus decoder init failed: {error}")))?;
        Ok(Self {
            decoder,
            buffer: vec![0; MAX_SAMPLES_PER_CHANNEL * CHANNELS],
        })
    }

    pub fn decode(&mut self, packet: &[u8]) -> Result<&[i16]> {
        if packet.is_empty() || packet.len() > MAX_PACKET_BYTES {
            return Err(AppError::BadRequest(format!(
                "invalid Opus packet length {}",
                packet.len()
            )));
        }

        let frames = self
            .decoder
            .decode(Some(packet), &mut self.buffer, false)
            .map_err(|error| AppError::AudioError(format!("Opus decode failed: {error}")))?;
        Ok(&self.buffer[..frames * CHANNELS])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiopus::coder::Encoder;
    use audiopus::Application;

    #[test]
    fn decode_preserves_all_stereo_samples() {
        let encoder =
            Encoder::new(SampleRate::Hz48000, Channels::Stereo, Application::Audio).unwrap();
        let pcm = vec![0i16; 960 * CHANNELS];
        let mut packet = vec![0u8; MAX_PACKET_BYTES];
        let packet_len = encoder.encode(&pcm, &mut packet).unwrap();

        let mut decoder = UacOpusDecoder::new().unwrap();
        let decoded = decoder.decode(&packet[..packet_len]).unwrap();
        assert_eq!(decoded.len(), pcm.len());
    }
}
