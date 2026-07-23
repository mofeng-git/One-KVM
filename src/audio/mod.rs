//! Platform audio capture, Opus encode, device enumeration, streaming, controller, health monitor.

#[cfg(any(unix, windows))]
pub mod capture;
pub mod controller;
#[cfg(any(unix, windows))]
pub mod device;
#[cfg(any(unix, windows))]
pub mod encoder;
pub mod monitor;
pub mod recovery;
pub mod streamer;
pub mod types;
pub mod uac_streamer;
pub mod uac_websocket;

pub use capture::{AudioCapturer, AudioConfig, AudioFrame};
pub use controller::AudioController;
pub use device::{enumerate_audio_devices, enumerate_audio_devices_with_current, AudioDeviceInfo};
pub use encoder::{OpusConfig, OpusEncoder, OpusFrame};
pub use monitor::{AudioHealthMonitor, AudioHealthStatus};
pub use streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
pub use types::{AudioControllerConfig, AudioQuality, AudioStatus};
