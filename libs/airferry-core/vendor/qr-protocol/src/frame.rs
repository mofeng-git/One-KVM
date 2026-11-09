//! QR frame wire format — see crate docs for the byte layout.
//!
//! The header is a fixed 60-byte big-endian structure. Keeping it fixed-width
//! (rather than variable-length encoding) makes parsing branch-light and lets
//! receivers validate `magic` + length before touching anything else.

use crate::{Error, Result};
use crc32fast::Hasher as Crc32;
use std::vec::Vec;

/// Protocol magic: ASCII 'E' 'T' big-endian → 0x4554. Stored as the first two
/// bytes so a receiver can lock onto a frame boundary by scanning for it.
pub const MAGIC: u16 = 0x4554;
pub const PROTOCOL_VERSION: u8 = 1;

pub const FRAME_HEADER_SIZE: usize = 60;
pub const FRAME_FOOTER_SIZE: usize = 4;

/// Flag bit: this frame's payload is a session descriptor (carries the OTI +
/// per-block symbol counts) rather than a RaptorQ symbol. See
/// `transfer_engine::descriptor`.
pub const FLAG_DESCRIPTOR: u8 = 0x01;

/// 128-bit session identifier. Deterministically derived from file identity
/// (see [`crate::session`]) so a receiver that restarts mid-transfer can
/// recognise the same object and resume.
pub type SessionIdRaw = u128;

/// Fixed-layout frame header (60 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameHeader {
    pub session_id: SessionIdRaw,
    /// Bitfield. Bit 0 (`FLAG_DESCRIPTOR`) marks a descriptor frame.
    pub flags: u8,
    /// RaptorQ Source Block Number.
    pub sbn: u32,
    /// RaptorQ Encoding Symbol ID (source < K, repair >= K).
    pub esi: u32,
    /// Total source blocks in the object.
    pub total_blocks: u32,
    /// Total source symbols K across the whole object.
    pub total_symbols: u32,
    /// Per-symbol byte size T (== payload length).
    pub symbol_size: u32,
    /// Monotonic frame sequence number (for loss/throughput stats).
    pub frame_index: u64,
    /// Sender wall-clock timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// CRC32 of the payload bytes.
    pub payload_crc32: u32,
}

/// A complete QR frame: header + payload + footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
    /// Whole-frame CRC32 (over header bytes + payload).
    pub frame_crc32: u32,
}

impl Frame {
    /// Total serialized length: header + payload + footer.
    #[inline]
    pub fn wire_size(symbol_size: u32) -> usize {
        FRAME_HEADER_SIZE
            .saturating_add(symbol_size as usize)
            .saturating_add(FRAME_FOOTER_SIZE)
    }

    /// Build a frame from a payload + header fields, computing both CRCs.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        session_id: SessionIdRaw,
        flags: u8,
        sbn: u32,
        esi: u32,
        total_blocks: u32,
        total_symbols: u32,
        symbol_size: u32,
        frame_index: u64,
        timestamp_ms: u64,
        payload: &[u8],
    ) -> Self {
        debug_assert_eq!(payload.len(), symbol_size as usize);
        let payload_crc32 = crc32(payload);
        let header = FrameHeader {
            session_id,
            flags,
            sbn,
            esi,
            total_blocks,
            total_symbols,
            symbol_size,
            frame_index,
            timestamp_ms,
            payload_crc32,
        };
        let mut buf = Vec::with_capacity(FRAME_HEADER_SIZE + payload.len());
        header.write_bytes(&mut buf);
        buf.extend_from_slice(payload);
        let frame_crc32 = crc32(&buf);
        Frame {
            header,
            payload: payload.to_vec(),
            frame_crc32,
        }
    }

    /// Serialize to a byte buffer (header + payload + footer).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf =
            Vec::with_capacity(FRAME_HEADER_SIZE + self.payload.len() + FRAME_FOOTER_SIZE);
        self.header.write_bytes(&mut buf);
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(&self.frame_crc32.to_be_bytes());
        buf
    }

    /// Parse + validate a frame from `bytes`.
    ///
    /// Performs: magic check, version check, length check, payload CRC, and
    /// whole-frame CRC. Any failure yields an [`Error`] and the caller should
    /// discard the frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let need = FRAME_HEADER_SIZE + FRAME_FOOTER_SIZE;
        if bytes.len() < need {
            return Err(Error::BufferTooShort {
                need,
                have: bytes.len(),
            });
        }
        let (header, _) = FrameHeader::read_bytes(bytes)?;
        let symbol_size = header.symbol_size as usize;
        // Compare against the already-bounded input length before performing
        // additions. On wasm32, a hostile u32::MAX symbol_size would otherwise
        // overflow usize while computing header + payload + footer.
        let actual_payload_size = bytes.len() - need;
        if symbol_size != actual_payload_size {
            // Distinguish "too short" from "too long" via a clearer error than
            // the old BufferTooShort, which was returned even when the input
            // carried extra trailing bytes.
            return Err(Error::LengthMismatch {
                expected: symbol_size.saturating_add(need),
                actual: bytes.len(),
            });
        }
        let payload_end = FRAME_HEADER_SIZE + symbol_size;
        let payload = &bytes[FRAME_HEADER_SIZE..payload_end];
        let footer = &bytes[payload_end..];
        // Payload CRC.
        if crc32(payload) != header.payload_crc32 {
            return Err(Error::PayloadCrcMismatch);
        }
        // Whole-frame CRC over header+payload.
        let mut crc = Crc32::new();
        crc.update(&bytes[..payload_end]);
        let expected_frame_crc = u32::from_be_bytes([footer[0], footer[1], footer[2], footer[3]]);
        if crc.finalize() != expected_frame_crc {
            return Err(Error::FrameCrcMismatch);
        }
        Ok(Frame {
            header,
            payload: payload.to_vec(),
            frame_crc32: expected_frame_crc,
        })
    }
}

impl FrameHeader {
    /// Write this header big-endian into `out` (must have 60 bytes capacity).
    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.push(PROTOCOL_VERSION);
        out.push(self.flags); // flags bitfield
        out.extend_from_slice(&self.session_id.to_be_bytes());
        out.extend_from_slice(&self.sbn.to_be_bytes());
        out.extend_from_slice(&self.esi.to_be_bytes());
        out.extend_from_slice(&self.total_blocks.to_be_bytes());
        out.extend_from_slice(&self.total_symbols.to_be_bytes());
        out.extend_from_slice(&self.symbol_size.to_be_bytes());
        out.extend_from_slice(&self.frame_index.to_be_bytes());
        out.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        out.extend_from_slice(&self.payload_crc32.to_be_bytes());
        debug_assert_eq!(out.len(), FRAME_HEADER_SIZE);
    }

    /// Read + validate a header from the first 60 bytes of `bytes`.
    /// Returns `(header, payload_crc_checked_marker)`.
    fn read_bytes(bytes: &[u8]) -> Result<(FrameHeader, ())> {
        let h = &bytes[..FRAME_HEADER_SIZE];
        let magic = u16::from_be_bytes([h[0], h[1]]);
        if magic != MAGIC {
            return Err(Error::BadMagic {
                expected: MAGIC,
                got: magic,
            });
        }
        let version = h[2];
        if version != PROTOCOL_VERSION {
            return Err(Error::BadVersion(version));
        }
        let flags = h[3];
        let mut o = 4usize; // skip magic(2)+version(1)+flags(1)
        let take = |o: &mut usize, n: usize| -> &[u8] {
            let s = &h[*o..*o + n];
            *o += n;
            s
        };
        let session_id = u128::from_be_bytes(take(&mut o, 16).try_into().unwrap());
        let sbn = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        let esi = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        let total_blocks = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        let total_symbols = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        let symbol_size = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        let frame_index = u64::from_be_bytes(take(&mut o, 8).try_into().unwrap());
        let timestamp_ms = u64::from_be_bytes(take(&mut o, 8).try_into().unwrap());
        let payload_crc32 = u32::from_be_bytes(take(&mut o, 4).try_into().unwrap());
        Ok((
            FrameHeader {
                session_id,
                flags,
                sbn,
                esi,
                total_blocks,
                total_symbols,
                symbol_size,
                frame_index,
                timestamp_ms,
                payload_crc32,
            },
            (),
        ))
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut h = Crc32::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i & 0xff) as u8).collect()
    }

    #[test]
    fn header_size_is_60() {
        // Build a header via a frame and check the written byte count.
        let payload = sample_payload(1024);
        let frame = Frame::build(1, 0, 0, 5, 3, 100, 1024, 42, 99000, &payload);
        let mut buf = Vec::new();
        frame.header.write_bytes(&mut buf);
        assert_eq!(buf.len(), FRAME_HEADER_SIZE);
        assert_eq!(FRAME_HEADER_SIZE, 60);
    }

    #[test]
    fn round_trip_frame() {
        let payload = sample_payload(1024);
        let frame = Frame::build(
            0x1122334455667788,
            0,
            2,
            7,
            4,
            256,
            1024,
            999,
            123456,
            &payload,
        );
        let bytes = frame.to_bytes();
        assert_eq!(bytes.len(), Frame::wire_size(1024));
        let parsed = Frame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn rejects_bad_magic() {
        let payload = sample_payload(1024);
        let mut bytes = Frame::build(1, 0, 0, 0, 1, 1, 1024, 0, 0, &payload).to_bytes();
        bytes[0] = 0x00;
        assert!(matches!(
            Frame::from_bytes(&bytes).unwrap_err(),
            Error::BadMagic { .. }
        ));
    }

    #[test]
    fn rejects_payload_corruption() {
        let payload = sample_payload(1024);
        let mut bytes = Frame::build(1, 0, 0, 0, 1, 1, 1024, 0, 0, &payload).to_bytes();
        // Flip a payload byte (after 60-byte header).
        bytes[FRAME_HEADER_SIZE] ^= 0xFF;
        assert!(matches!(
            Frame::from_bytes(&bytes).unwrap_err(),
            Error::PayloadCrcMismatch
        ));
    }

    #[test]
    fn rejects_footer_corruption() {
        let payload = sample_payload(1024);
        let mut bytes = Frame::build(1, 0, 0, 0, 1, 1, 1024, 0, 0, &payload).to_bytes();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        assert!(matches!(
            Frame::from_bytes(&bytes).unwrap_err(),
            Error::FrameCrcMismatch
        ));
    }

    #[test]
    fn rejects_too_short() {
        let err = Frame::from_bytes(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, Error::BufferTooShort { .. }));
    }

    #[test]
    fn rejects_huge_declared_symbol_size_without_overflow() {
        let mut bytes = Frame::build(1, 0, 0, 0, 1, 1, 0, 0, 0, &[]).to_bytes();
        bytes[36..40].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            Frame::from_bytes(&bytes).unwrap_err(),
            Error::LengthMismatch { .. }
        ));
    }
}
