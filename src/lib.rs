//! Core library for One-KVM (IP‑KVM: capture, HID, OTG, streaming, Web UI glue).

#[cfg(not(any(feature = "android", unix, windows)))]
compile_error!("One-KVM supports Linux and Windows targets only.");

#[cfg(any(feature = "android", feature = "desktop"))]
pub mod runtime;

#[cfg(any(feature = "android", feature = "desktop"))]
pub mod atx;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod audio;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod auth;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod config;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod db;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod diagnostics;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod error;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod events;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod extensions;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod hid;
#[cfg(all(unix, any(feature = "android", feature = "desktop")))]
pub mod msd;
#[cfg(all(unix, any(feature = "android", feature = "desktop")))]
pub mod otg;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod platform;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod redfish;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod rtsp;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod rustdesk;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod state;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod stream;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod stream_encoder;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod update;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod utils;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod video;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod web;
#[cfg(any(feature = "android", feature = "desktop"))]
pub mod webrtc;

#[cfg(any(feature = "android", feature = "desktop"))]
pub mod secrets {
    include!(concat!(env!("OUT_DIR"), "/secrets_generated.rs"));
}

#[cfg(any(feature = "android", feature = "desktop"))]
pub use error::{AppError, Result};
