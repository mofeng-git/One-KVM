//! # transfer-engine
//!
//! Orchestrates an AirFerry session end-to-end:
//!
//! - [`SenderSession`] holds the (already compressed) payload, owns a
//!   `raptorq-core` encoder, and produces a stream of [`Frame`]s. It
//!   interleaves source symbols with repair symbols to reach a target
//!   redundancy ratio, and emits fresh repair symbols until the RFC 24-bit ESI
//!   space is exhausted so a receiver can rejoin at any practical time.
//! - [`ReceiverSession`] ingests decoded QR payloads as frames, feeds the
//!   RaptorQ decoder, tracks per-block progress, and reassembles the file once
//!   complete. Bounded checkpoint metadata is available via
//!   [`ReceiverSession::save_state`] / [`ReceiverSession::restore`] and
//!   [`ResumeState`] (JSON when `serde` is on); only retained symbol payloads
//!   can be replayed into a fresh decoder.
//!
//! Compression is intentionally **outside** this engine: the caller compresses
//! with Zstd (native/Android via `qr-protocol`, or JS-side on the browser) and
//! hands the compressed bytes in. This keeps the engine target-agnostic.

#![forbid(unsafe_code)]

pub mod assembler;
pub mod descriptor;
pub mod progress;
pub mod receiver;
pub mod resume;
pub mod segment;
#[cfg(any(feature = "sender", test))]
pub mod sender;
pub mod time;

pub use assembler::TransferAssembler;
pub use descriptor::{DescriptorInfo, FileMeta};
pub use progress::{Progress, Stats};
pub use receiver::ReceiverSession;
pub use resume::ResumeState;
pub use segment::{SegmentMeta, MAX_SEGMENT_COUNT, SEGMENT_RAW_BYTES};
#[cfg(any(feature = "sender", test))]
pub use sender::{SenderConfig, SenderSession};

// Re-export the wire-frame helpers so downstream code (tests, JNI host, the
// browser bridge) can parse/validate frames without depending on qr-protocol
// directly.
pub use qr_protocol::{Frame, FrameHeader, FLAG_DESCRIPTOR};
pub use raptorq_core::ObjectMeta;
/// Parse + validate a frame from its wire bytes (magic + double CRC).
pub fn qr_protocol_frame_from_bytes(bytes: &[u8]) -> Result<Frame> {
    Ok(Frame::from_bytes(bytes)?)
}
/// The descriptor-flag bit, re-exported for convenience.
pub const FLAG_DESCRIPTOR_BIT: u8 = FLAG_DESCRIPTOR;

/// Errors raised by the engine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("raptorq error: {0}")]
    Raptorq(#[from] raptorq_core::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] qr_protocol::Error),
    #[error("no payload set")]
    NoPayload,
    #[error("session not yet initialized (no metadata received)")]
    NotInitialized,
    #[error("session id mismatch: expected {expected}, got {got}")]
    SessionMismatch { expected: u128, got: u128 },
    #[error("object already complete")]
    AlreadyComplete,
    #[error("invalid redundancy percentage: {0} (must be between 5 and 50)")]
    InvalidRedundancy(u8),
    #[error("compression error: {0}")]
    Compress(String),
    #[error("RaptorQ's 24-bit encoding-symbol id space is exhausted")]
    SymbolIdSpaceExhausted,
    #[error("invalid resume state: {0}")]
    InvalidResume(&'static str),
    #[error("invalid large-transfer segment: {0}")]
    InvalidSegment(&'static str),
}

pub(crate) type Result<T> = core::result::Result<T, Error>;
