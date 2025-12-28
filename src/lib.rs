//! One-KVM - Lightweight IP-KVM solution
//!
//! This crate provides the core functionality for One-KVM,
//! a remote KVM (Keyboard, Video, Mouse) solution written in Rust.

pub mod atx;
pub mod audio;
pub mod auth;
pub mod config;
pub mod error;
pub mod events;
pub mod extensions;
pub mod hid;
pub mod modules;
pub mod msd;
pub mod otg;
pub mod state;
pub mod stream;
pub mod utils;
pub mod video;
pub mod web;
pub mod webrtc;

pub use error::{AppError, Result};
