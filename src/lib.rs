//! Core library for One-KVM (IP‑KVM: capture, HID, OTG, streaming, Web UI glue).

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("One-KVM supports Linux and Windows targets only.");
#[cfg(feature = "airferry")]
pub mod airferry;
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
#[cfg(feature = "mcp")]
pub mod mcp;
#[cfg(unix)]
pub mod msd;
#[cfg(unix)]
pub mod otg;
pub mod platform;
#[cfg(feature = "redfish")]
pub mod redfish;
pub mod rtsp;
pub mod runtime;
pub mod rustdesk;
pub mod state;
pub mod stream;
pub mod stream_encoder;
pub mod update;
pub mod utils;
pub mod video;
pub mod vnc;
pub mod watchdog;
pub mod web;
pub mod webrtc;
pub use error::{AppError, Result};
