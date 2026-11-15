//! RustDesk peer protocol (hbbs / hbbr).

#[cfg(feature = "rustdesk")]
pub mod bytes_codec;
pub mod config;
#[cfg(feature = "rustdesk")]
pub mod connection;
#[cfg(feature = "rustdesk")]
pub mod crypto;
#[cfg(feature = "rustdesk")]
pub mod frame_adapters;
#[cfg(feature = "rustdesk")]
pub mod hid_adapter;
#[cfg(feature = "rustdesk")]
mod keyboard_mapping;
#[cfg(feature = "rustdesk")]
pub mod protocol;
#[cfg(feature = "rustdesk")]
pub mod punch;
#[cfg(feature = "rustdesk")]
pub mod rendezvous;

#[cfg(feature = "rustdesk")]
mod service;
#[cfg(feature = "rustdesk")]
pub use service::*;
