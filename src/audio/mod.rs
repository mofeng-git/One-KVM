//! ALSA capture, Opus encode, device enumeration, streaming, controller, health monitor.

pub mod capture;
pub mod controller;
pub mod device;
pub mod encoder;
pub mod monitor;
pub mod streamer;

pub use capture::{AudioCapturer, AudioConfig, AudioFrame};
pub use controller::{AudioController, AudioControllerConfig, AudioQuality, AudioStatus};
pub use device::{enumerate_audio_devices, enumerate_audio_devices_with_current, AudioDeviceInfo};
pub use encoder::{OpusConfig, OpusEncoder, OpusFrame};
pub use monitor::{AudioHealthMonitor, AudioHealthStatus};
pub use streamer::{AudioStreamState, AudioStreamer, AudioStreamerConfig};
