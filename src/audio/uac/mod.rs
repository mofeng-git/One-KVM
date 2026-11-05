//! Browser-to-USB microphone audio pipeline.

mod decoder;
mod playback;
mod protocol;

pub use decoder::UacOpusDecoder;
pub use playback::{UacPlayback, UacPlaybackConfig, UacPlaybackState, UacSession};
pub use protocol::{parse_audio_packet, UacAudioPacket};
