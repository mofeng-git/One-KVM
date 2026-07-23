use audiopus::coder::Decoder as OpusDecoder;
use audiopus::{Channels, SampleRate};
use axum::extract::ws::{Message, WebSocket};
use tracing::{debug, error, info, warn};

use super::uac_streamer::{UacPcmFrame, UacPlaybackWriter};
use std::sync::Arc;

/// Binary protocol header for UAC audio input.
///
///  0x03 — message type (reverse audio / microphone passthrough)
/// timestamp — u32 LE  (milliseconds, for future sync)
/// duration  — u16 LE  (frame duration in ms, typically 20)
/// sequence  — u32 LE  (frame counter, for loss detection)
/// data_len  — u32 LE  (Opus payload length in bytes)
const UAC_AUDIO_HEADER_SIZE: usize = 15;
const UAC_AUDIO_MSG_TYPE: u8 = 0x03;

/// Accept incoming Opus audio frames over WebSocket and route them
/// to the UAC playback writer.
pub async fn handle_uac_audio_ws(
    mut ws: WebSocket,
    playback: Arc<UacPlaybackWriter>,
) {
    // Create an Opus decoder: 48kHz stereo → PCM S16LE.
    let mut decoder = match OpusDecoder::new(SampleRate::Hz48000, Channels::Stereo) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to create Opus decoder: {}", e);
            let _ = ws.send(Message::Close(None)).await;
            return;
        }
    };

    info!("UAC audio WebSocket connected (mic passthrough)");

    while let Some(msg) = ws.recv().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                warn!("UAC WebSocket error: {}", e);
                break;
            }
        };

        match msg {
            Message::Binary(data) => {
                if data.len() < UAC_AUDIO_HEADER_SIZE {
                    warn!(
                        "UAC audio frame too short: {} bytes (min {})",
                        data.len(),
                        UAC_AUDIO_HEADER_SIZE
                    );
                    continue;
                }

                let msg_type = data[0];
                if msg_type == 0x04 {
                    // Raw PCM passthrough — no Opus decode needed.
                    // Useful for testing and for clients that encode locally.
                    let duration = u16::from_le_bytes([data[5], data[6]]);
                    let data_len = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;
                    if data.len() < UAC_AUDIO_HEADER_SIZE + data_len {
                        warn!("UAC PCM frame truncated");
                        continue;
                    }
                    let pcm_bytes = &data[UAC_AUDIO_HEADER_SIZE..UAC_AUDIO_HEADER_SIZE + data_len];
                    if let Err(e) = playback
                        .write(super::uac_streamer::UacPcmFrame {
                            data: pcm_bytes.to_vec(),
                            duration_ms: duration as u32,
                        })
                        .await
                    {
                        error!("Failed to send UAC PCM frame: {}", e);
                        break;
                    }
                    continue;
                }
                if msg_type != UAC_AUDIO_MSG_TYPE {
                    warn!("UAC unknown msg type: 0x{msg_type:02x}");
                    continue;
                }

                let duration = u16::from_le_bytes([data[5], data[6]]);
                let data_len = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;

                if data.len() < UAC_AUDIO_HEADER_SIZE + data_len {
                    warn!("UAC audio frame truncated");
                    continue;
                }

                let opus_payload = &data[UAC_AUDIO_HEADER_SIZE..UAC_AUDIO_HEADER_SIZE + data_len];

                let frame_samples = (48000u32 * duration as u32 / 1000) as usize * 2; // 2 channels
                let mut pcm_i16 = vec![0i16; frame_samples];

                match decoder.decode(Some(opus_payload), &mut pcm_i16, false) {
                    Ok(decoded) => {
                        // Convert i16 → bytes (S16LE interleaved)
                        let pcm_bytes: Vec<u8> = pcm_i16[..decoded]
                            .iter()
                            .flat_map(|s| s.to_le_bytes())
                            .collect();
                        if let Err(e) = playback
                            .write(UacPcmFrame {
                                data: pcm_bytes,
                                duration_ms: duration as u32,
                            })
                            .await
                        {
                            error!("Failed to send UAC PCM frame: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Opus decode error: {}", e);
                    }
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => {
                debug!("UAC audio WebSocket closing");
                break;
            }
            Message::Text(_) => {
                // Ignore text messages
            }
        }
    }

    info!("UAC audio WebSocket disconnected");
}
