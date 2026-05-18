//! Core library for One-KVM (IP‑KVM: capture, HID, OTG, streaming, Web UI glue).

#[cfg(not(any(unix, windows)))]
compile_error!("One-KVM supports Linux and Windows targets only.");

pub mod atx;
pub mod audio;
pub mod auth;
pub mod config;
pub mod db;
pub mod diagnostics;
pub mod error;
pub mod events;
pub mod extensions;
pub mod hid;
#[cfg(unix)]
pub mod msd;
#[cfg(unix)]
pub mod otg;
pub mod platform;
pub mod redfish;
pub mod rtsp;
pub mod rustdesk;
pub mod state;
pub mod stream;
pub mod stream_encoder;
pub mod update;
pub mod utils;
pub mod video;
pub mod web;
pub mod webrtc;

pub mod secrets {
    include!(concat!(env!("OUT_DIR"), "/secrets_generated.rs"));
}

pub use error::{AppError, Result};
