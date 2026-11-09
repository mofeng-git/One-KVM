//! Large-transfer segment metadata and invariants.
//!
//! A logical transfer (single file, multi-file bundle, or text) is compressed
//! **once** into a single compressed stream, then that stream is split into
//! fixed `SEGMENT_BYTES` (32 MiB) segments. Each segment is an independent
//! RaptorQ-encoded child object. The existing 60-byte frame header remains
//! unchanged: each object uses a deterministic child session id, while
//! descriptor v5 carries the stable root transfer id and the segment's
//! canonical range **within the compressed stream**.
//!
//! On the receive side the compressed bytes of every segment are concatenated
//! in `segment_index` order to rebuild the full compressed stream, which is
//! then decompressed exactly once to recover the original payload. Unlike the
//! legacy per-segment raw model, segments are therefore NOT independently
//! decompressible — a single zstd/xz stream cannot be sliced into decodable
//! pieces — so the receiver must complete the whole set before recovering.

use crate::descriptor::FileMeta;
use qr_protocol::{frame::SessionIdRaw, SessionId};
use raptorq_core::{MAX_OBJECT_BYTES, MAX_SYMBOL_SIZE};

/// Fixed **compressed-stream** segment size used by descriptor v5.
///
/// The sender splits the compressed payload into chunks of at most this many
/// bytes. It is sized `MAX_OBJECT_BYTES - MAX_SYMBOL_SIZE` (≈ 32 MiB) so that a
/// full segment, after RaptorQ pads it up to a whole symbol, still fits the
/// 32 MiB wire ceiling ([`MAX_OBJECT_BYTES`]) for **any** symbol size the
/// sender may choose (largest is [`MAX_SYMBOL_SIZE`]). Keeping this protocol
/// constant makes offsets canonical, bounds memory on all receivers, and limits
/// a safe-pause rollback to at most one ~32 MiB segment.
pub const SEGMENT_RAW_BYTES: u64 = MAX_OBJECT_BYTES - (MAX_SYMBOL_SIZE as u64);

/// Resource ceiling for a single root task. Hosts may enforce lower
/// product/storage limits, but must never allocate from an untrusted
/// descriptor count above this bound.
pub const MAX_SEGMENT_COUNT: u32 = 131_072;

/// Descriptor-v5 metadata for one segment of the compressed root stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentMeta {
    /// Stable identity of the complete file/task.
    pub root_session_id: SessionIdRaw,
    /// Zero-based segment index.
    pub segment_index: u32,
    /// Total number of segments in the root transfer.
    pub segment_count: u32,
    /// Canonical byte offset of this segment within the **compressed** root
    /// stream.
    pub original_offset: u64,
    /// Total size of the **compressed** root stream (== sum of every segment's
    /// `file_meta.compressed_size`).
    pub root_original_size: u64,
    /// SHA-256 of the complete **decompressed** original payload. Every
    /// segment of the same task carries the identical value so receivers
    /// cannot accidentally combine individually-valid segments from different
    /// file revisions, and it is verified after the final decompression.
    pub root_sha256: [u8; 32],
    /// SHA-256 of this segment's **compressed** bytes.
    pub raw_sha256: [u8; 32],
}

impl SegmentMeta {
    /// Validate descriptor-controlled segment coordinates before a host uses
    /// them for allocation or positioned writes.
    pub fn validate(
        &self,
        child_session_id: SessionIdRaw,
        file_meta: &FileMeta,
    ) -> Result<(), &'static str> {
        if self.segment_count == 0 || self.segment_count > MAX_SEGMENT_COUNT {
            return Err("segment count out of range");
        }
        if self.segment_index >= self.segment_count {
            return Err("segment index out of range");
        }
        if self.root_original_size == 0 {
            return Err("root compressed size must be non-zero");
        }

        let expected_count_u64 = self
            .root_original_size
            .checked_sub(1)
            .and_then(|n| n.checked_div(SEGMENT_RAW_BYTES))
            .and_then(|n| n.checked_add(1))
            .ok_or("segment count overflow")?;
        let expected_count = u32::try_from(expected_count_u64)
            .map_err(|_| "segment count exceeds protocol budget")?;
        if expected_count != self.segment_count {
            return Err("segment count inconsistent with compressed root size");
        }

        let expected_offset = u64::from(self.segment_index)
            .checked_mul(SEGMENT_RAW_BYTES)
            .ok_or("segment offset overflow")?;
        if self.original_offset != expected_offset {
            return Err("segment offset is not canonical");
        }
        let remaining = self
            .root_original_size
            .checked_sub(expected_offset)
            .ok_or("segment offset exceeds compressed root size")?;
        // A segment's compressed bytes must not exceed its canonical slice.
        // The sender leaves up to one symbol of headroom so the RaptorQ-padded
        // object stays within the 32 MiB wire ceiling (`MAX_OBJECT_BYTES`), so
        // we accept `compressed_size <= canonical` rather than exact equality.
        let canonical_len = remaining.min(SEGMENT_RAW_BYTES);
        if !file_meta.compressed_size_known || file_meta.compressed_size == 0 {
            return Err("segment compressed size missing or empty");
        }
        if file_meta.compressed_size > canonical_len {
            return Err("segment compressed length exceeds canonical compressed range");
        }
        if SessionId::derive_segment(self.root_session_id, self.segment_index).0 != child_session_id
        {
            return Err("segment child session id mismatch");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `original_size` is the whole-file decompressed size; `compressed_size`
    /// is this segment's compressed length.
    fn file_meta(original_size: u64, compressed_size: u64) -> FileMeta {
        FileMeta {
            filename: "large.bin".into(),
            original_size,
            crc32: 0,
            compression: qr_protocol::compress::COMPRESSION_NONE,
            compressed_size,
            compressed_size_known: true,
            crc32_known: true,
        }
    }

    #[test]
    fn validates_canonical_final_segment() {
        let root = 7u128;
        let raw = 1234u64;
        // Compression None: compressed stream == original bytes, so the whole
        // decompressed size equals the compressed root size.
        let whole = SEGMENT_RAW_BYTES * 2 + raw;
        let segment = SegmentMeta {
            root_session_id: root,
            segment_index: 2,
            segment_count: 3,
            original_offset: SEGMENT_RAW_BYTES * 2,
            root_original_size: whole,
            root_sha256: [8; 32],
            raw_sha256: [9; 32],
        };
        let child = SessionId::derive_segment(root, 2).0;
        assert_eq!(segment.validate(child, &file_meta(whole, raw)), Ok(()));
    }

    #[test]
    fn validates_nonfinal_full_segment() {
        let root = 7u128;
        let whole = SEGMENT_RAW_BYTES * 3;
        let segment = SegmentMeta {
            root_session_id: root,
            segment_index: 1,
            segment_count: 3,
            original_offset: SEGMENT_RAW_BYTES,
            root_original_size: whole,
            root_sha256: [8; 32],
            raw_sha256: [9; 32],
        };
        let child = SessionId::derive_segment(root, 1).0;
        assert_eq!(
            segment.validate(child, &file_meta(whole, SEGMENT_RAW_BYTES)),
            Ok(())
        );
    }

    #[test]
    fn rejects_holes_and_wrong_child_identity() {
        let root = 9u128;
        let whole = SEGMENT_RAW_BYTES + 5;
        let mut segment = SegmentMeta {
            root_session_id: root,
            segment_index: 1,
            segment_count: 2,
            original_offset: SEGMENT_RAW_BYTES,
            root_original_size: whole,
            root_sha256: [2; 32],
            raw_sha256: [1; 32],
        };
        let fm = file_meta(whole, 5);
        assert!(segment
            .validate(SessionId::derive_segment(root, 0).0, &fm)
            .is_err());
        segment.original_offset += 1;
        assert!(segment
            .validate(SessionId::derive_segment(root, 1).0, &fm)
            .is_err());
    }
}
