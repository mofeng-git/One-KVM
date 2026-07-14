//! Core library for One-KVM (IP‑KVM: capture, HID, OTG, streaming, Web UI glue).

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("One-KVM supports Linux and Windows targets only.");

#[cfg(feature = "desktop")]
pub mod atx;
#[cfg(feature = "desktop")]
pub mod audio;
#[cfg(feature = "desktop")]
pub mod auth;
#[cfg(feature = "desktop")]
pub mod computer_use;
#[cfg(feature = "desktop")]
pub mod config;
#[cfg(feature = "desktop")]
pub mod db;
#[cfg(feature = "desktop")]
pub mod diagnostics;
#[cfg(feature = "desktop")]
pub mod error;
#[cfg(feature = "desktop")]
pub mod events;
#[cfg(feature = "desktop")]
pub mod extensions;
#[cfg(feature = "desktop")]
pub mod hid;
#[cfg(all(unix, feature = "desktop"))]
pub mod msd;
#[cfg(all(unix, feature = "desktop"))]
pub mod otg;
#[cfg(feature = "desktop")]
pub mod platform;
#[cfg(feature = "desktop")]
pub mod redfish;
#[cfg(feature = "desktop")]
pub mod rtsp;
#[cfg(feature = "desktop")]
pub mod rustdesk;
#[cfg(feature = "desktop")]
pub mod state;
#[cfg(feature = "desktop")]
pub mod stream;
#[cfg(feature = "desktop")]
pub mod stream_encoder;
#[cfg(feature = "desktop")]
pub mod update;
#[cfg(feature = "desktop")]
pub mod utils;
#[cfg(feature = "desktop")]
pub mod video;
#[cfg(feature = "desktop")]
pub mod vnc;
#[cfg(feature = "desktop")]
pub mod web;
#[cfg(feature = "desktop")]
pub mod webrtc;

#[cfg(feature = "desktop")]
pub mod secrets {
    include!(concat!(env!("OUT_DIR"), "/secrets_generated.rs"));
}

#[cfg(feature = "desktop")]
pub use error::{AppError, Result};
