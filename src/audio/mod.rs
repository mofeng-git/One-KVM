//! Platform audio capture, Opus encode, device enumeration, streaming, controller, health monitor.

#[cfg(any(unix, windows))]
mod capture;
mod controller;
#[cfg(any(unix, windows))]
mod device;
#[cfg(any(unix, windows))]
mod encoder;
mod monitor;
mod recovery;
mod streamer;
mod types;
#[cfg(unix)]
pub mod uac;

pub use capture::{AudioCapturer, AudioConfig, AudioFrame};
pub use controller::AudioController;
pub use device::{enumerate_audio_devices, enumerate_audio_devices_with_current, AudioDeviceInfo};
pub use encoder::{OpusConfig, OpusEncoder, OpusFrame};
pub use monitor::{AudioHealthMonitor, AudioHealthStatus};
pub use streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
pub use types::{AudioControllerConfig, AudioQuality, AudioStatus};
