//! # qr-protocol
//!
//! Wire format and helpers that sit between the RaptorQ codec (`raptorq-core`)
//! and the QR display / scan layer.
//!
//! ## Frame layout
//!
//! Every QR frame is a fixed-layout byte buffer:
//!
//! ```text
//! [ Header (60 B) ][ Payload (symbol_size B) ][ Footer (4 B) ]
//! ```
//!
//! - **Header** carries session id, RaptorQ symbol coordinates (SBN/ESI),
//!   block/symbol totals, a frame index for statistics, a timestamp, and a
//!   CRC32 of the payload.
//! - **Payload** is exactly one RaptorQ symbol.
//! - **Footer** is a CRC32 over (Header + Payload), giving whole-frame
//!   integrity. A receiver that fails either CRC simply drops the frame and
//!   relies on RaptorQ fountain redundancy to recover the missing data.
//!
//! All multi-byte integers are big-endian (network order).
//!
//! ## Modules
//!
//! - [`frame`] — `Frame` struct, pack/unpack, double-CRC validation.
//! - [`compress`] — Zstd compression (native/Android only; WASM compresses
//!   in JS to the same standard zstd format).
//! - [`chunker`] — split compressed bytes into RaptorQ source blocks.
//! - [`session`] — deterministic session-id derivation (enables resume).

#![forbid(unsafe_code)]

pub mod chunker;
pub mod compress;
pub mod frame;
pub mod session;

pub use frame::{Frame, FrameHeader, FLAG_DESCRIPTOR, FRAME_FOOTER_SIZE, FRAME_HEADER_SIZE, MAGIC};
pub use session::SessionId;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("buffer too short: need {need} bytes, have {have}")]
    BufferTooShort { need: usize, have: usize },
    #[error("bad magic: expected 0x{expected:04X}, got 0x{got:04X}")]
    BadMagic { expected: u16, got: u16 },
    #[error("unsupported protocol version {0}")]
    BadVersion(u8),
    #[error("frame CRC mismatch")]
    FrameCrcMismatch,
    #[error("payload CRC mismatch")]
    PayloadCrcMismatch,
    #[error("unsupported symbol size {0}")]
    BadSymbolSize(u32),
    #[error("frame length mismatch: expected {expected} bytes, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },
    #[error("compression error: {0}")]
    Compress(String),
}

pub(crate) type Result<T> = core::result::Result<T, Error>;
