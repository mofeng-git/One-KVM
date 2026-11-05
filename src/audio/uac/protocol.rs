use crate::error::{AppError, Result};

const HEADER_SIZE: usize = 15;
const OPUS_MESSAGE: u8 = 0x03;
const PCM_MESSAGE: u8 = 0x04;
const CHANNELS: usize = 2;
const MAX_PCM_SAMPLES: usize = 5760 * CHANNELS;

#[derive(Debug, PartialEq, Eq)]
pub enum UacAudioPacket<'a> {
    Opus(&'a [u8]),
    Pcm(&'a [u8]),
}

impl UacAudioPacket<'_> {
    pub fn pcm_samples(&self) -> Result<Vec<i16>> {
        let Self::Pcm(bytes) = self else {
            return Err(AppError::BadRequest("packet is not raw PCM".to_string()));
        };
        if bytes.is_empty() || bytes.len() % (CHANNELS * 2) != 0 {
            return Err(AppError::BadRequest(format!(
                "invalid stereo PCM byte length {}",
                bytes.len()
            )));
        }
        if bytes.len() / 2 > MAX_PCM_SAMPLES {
            return Err(AppError::BadRequest("PCM frame exceeds 120 ms".to_string()));
        }

        Ok(bytes
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect())
    }
}

pub fn parse_audio_packet(data: &[u8]) -> Result<UacAudioPacket<'_>> {
    if data.len() < HEADER_SIZE {
        return Err(AppError::BadRequest(
            "UAC frame is shorter than its header".to_string(),
        ));
    }

    let payload_len = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;
    let expected_len = HEADER_SIZE
        .checked_add(payload_len)
        .ok_or_else(|| AppError::BadRequest("UAC payload length overflow".to_string()))?;
    if data.len() != expected_len {
        return Err(AppError::BadRequest(
            "UAC payload length does not match its header".to_string(),
        ));
    }

    let payload = &data[HEADER_SIZE..];
    match data[0] {
        OPUS_MESSAGE => Ok(UacAudioPacket::Opus(payload)),
        PCM_MESSAGE => Ok(UacAudioPacket::Pcm(payload)),
        message_type => Err(AppError::BadRequest(format!(
            "unsupported UAC message type 0x{message_type:02x}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(message_type: u8, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0; HEADER_SIZE + payload.len()];
        data[0] = message_type;
        data[11..15].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        data[HEADER_SIZE..].copy_from_slice(payload);
        data
    }

    #[test]
    fn requires_exact_payload_length() {
        let valid = message(OPUS_MESSAGE, &[1, 2, 3]);
        assert_eq!(
            parse_audio_packet(&valid).unwrap(),
            UacAudioPacket::Opus(&[1, 2, 3])
        );

        let mut trailing = valid.clone();
        trailing.push(4);
        assert!(parse_audio_packet(&trailing).is_err());
        assert!(parse_audio_packet(&valid[..valid.len() - 1]).is_err());
    }

    #[test]
    fn converts_little_endian_stereo_pcm() {
        let data = message(PCM_MESSAGE, &[1, 0, 255, 255]);
        assert_eq!(
            parse_audio_packet(&data).unwrap().pcm_samples().unwrap(),
            vec![1, -1]
        );
    }
}
