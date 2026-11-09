//! Session descriptor frames.
//!
//! A *descriptor frame* is a regular wire frame whose payload (instead of a
//! RaptorQ symbol) carries the authoritative [`ObjectMeta`] needed by a
//! receiver to build its decoder, plus (v2) the file metadata (filename,
//! original size, CRC32). It is flagged with `FLAG_DESCRIPTOR` in the header.
//!
//! The sender emits a descriptor frame every `N` data frames so that a
//! receiver that joins mid-stream learns the object layout within seconds.

use crate::segment::SegmentMeta;
use crate::{Error, Result};
use qr_protocol::{frame::FLAG_DESCRIPTOR, Frame};
use raptorq_core::ObjectMeta;
use std::vec::Vec;

/// File metadata carried alongside the RaptorQ object metadata.
///
/// Kept separate from `ObjectMeta` so that the `meta != self.meta` equality
/// check in the receiver (which gates decoder rebuilds) is unaffected by
/// filename / checksum changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    /// Original filename (UTF-8). May be truncated for very large block counts.
    pub filename: String,
    /// Original file size in bytes (before compression).
    pub original_size: u64,
    /// CRC32 of the original file bytes (for post-recovery verification).
    pub crc32: u32,
    /// Compression algorithm applied to the payload before RaptorQ encoding.
    /// Mirrors [`qr_protocol::compress`] constants: 0=None, 1=Zstd, 2=Xz.
    pub compression: u8,
    /// Size in bytes of the *compressed* payload that was RaptorQ-encoded.
    /// Receivers truncate RaptorQ zero-padding back to this before decompress.
    pub compressed_size: u64,
    /// Whether `compressed_size` carries a real value (vs. "unknown").
    ///
    /// This is a *runtime-only* flag — it is **not** serialized. It exists so
    /// the receiver can distinguish a genuinely empty payload (0 bytes) from
    /// "the descriptor never supplied this field" (e.g. a v1/v2 descriptor or a
    /// `FileMeta::default()`). The previous design used `compressed_size == 0`
    /// as a sentinel for "unknown", which silently broke empty/tiny payloads.
    pub compressed_size_known: bool,
    /// Whether `crc32` carries a real value (vs. "unknown").
    ///
    /// Like `compressed_size_known`, this is a *runtime-only* flag (not on the
    /// wire). CRC32 is a 32-bit hash whose output can legitimately be 0, so the
    /// old `crc32 == 0` sentinel for "unknown" mislabelled ~1 in 2^32 files as
    /// unverified. The descriptor v3 tail always carries a real CRC; v1 and
    /// `FileMeta::default()` set this false so the receiver skips verification
    /// instead of comparing against a meaningless 0.
    pub crc32_known: bool,
}

impl Default for FileMeta {
    fn default() -> Self {
        Self {
            filename: String::new(),
            original_size: 0,
            crc32: 0,
            compression: qr_protocol::compress::COMPRESSION_NONE,
            compressed_size: 0,
            compressed_size_known: false,
            crc32_known: false,
        }
    }
}

/// Parsed descriptor info: codec metadata + optional file metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorInfo {
    pub meta: ObjectMeta,
    pub file_meta: FileMeta,
    /// Present only for descriptor v5 large-transfer child objects.
    pub segment: Option<SegmentMeta>,
}

/// Compact on-wire descriptor layout (big-endian):
///   u8  magic        = 0xD5
///   u8  version      = 3 (legacy object) or 5 (large-transfer segment)
///   u16 num_blocks
///   u64 transfer_length
///   u32 symbol_size
///   u8[12] oti_bytes
///   repeated num_blocks × { u32 sbn, u32 num_source_symbols, u64 block_length }
///   --- v2 extension ---
///   u8  filename_len (0..=255)
///   u8[filename_len] filename (UTF-8)
///   u64 original_file_size
///   u32 crc32
///   --- v3 extension ---
///   u8  compression         (0=None, 1=Zstd, 2=Xz)
///   u64 compressed_size     (compressed payload bytes, before RaptorQ padding)
///   --- v5 segmented extension (compress-then-segment model) ---
///   u128 root_session_id
///   u32  segment_index
///   u32  segment_count
///   u64  original_offset      (offset in the compressed stream)
///   u64  root_original_size   (whole compressed stream size)
///   u8[32] root_sha256       (whole decompressed original)
///   u8[32] raw_sha256        (this segment's compressed bytes)
///
/// Total v1 part = 28 + 16*B. v2 extension = 1 + filename_len + 8 + 4.
/// v3 extension = 1 + 8 = 9.
/// Must fit in one symbol payload (default 1024 bytes).
///
/// # Version history
/// - v1/v2/v3: legacy single-object descriptors (still emitted for non-segmented
///   transfers; a segmented child object never uses these).
/// - v4 (LEGACY, REJECTED): the original segmentation model — 8 MiB *raw original*
///   segments, each independently compressed. Used by withdrawn early v1.2.0
///   pre-release builds. Superseded by v5; receivers fail-closed reject v4 because its
///   field semantics (raw_sha256 over uncompressed segment bytes, 8 MiB segment
///   size, per-segment compression) are incompatible with v5 and silently
///   accepting it would mis-parse a segmented transfer as a single object.
/// - v5 (CURRENT): compress-then-segment model — compress the whole payload
///   once, then split the compressed stream into ~31.9 MiB segments. Introduced
///   in the final v1.2.0 release; withdrawn pre-release builds used stale v4
///   semantics and are intentionally unsupported.
const DESC_MAGIC: u8 = 0xD5;
const DESC_VERSION: u8 = 5;
/// Legacy v4 segmentation model (8 MiB raw-segment, per-segment compression).
/// Superseded by v5 (compress-then-segment). Receivers reject v4 fail-closed.
const DESC_LEGACY_V4: u8 = 4;
const DESC_V3_VERSION: u8 = 3;
const DESC_FIXED_OVERHEAD: usize = 28;
/// Size of the v2 extension fields excluding the variable filename bytes:
/// u8 filename_len + u64 original_size + u32 crc32 = 13.
const DESC_V2_TAIL_FIXED: usize = 13;
/// Size of the v3 extension fields: u8 compression + u64 compressed_size = 9.
const DESC_V3_TAIL_FIXED: usize = 9;
/// Size of the segmented-extension tail (introduced by v4, unchanged layout in
/// v5 — only the field *semantics* and the version byte differ). 104 bytes:
/// u128 root_session_id + u32 segment_index + u32 segment_count +
/// u64 original_offset + u64 root_original_size + u8[32] root_sha256 +
/// u8[32] raw_sha256.
const DESC_SEGMENT_TAIL_FIXED: usize = 16 + 4 + 4 + 8 + 8 + 32 + 32;

/// Serialize object metadata + file metadata into a descriptor payload, padded
/// with zeros to `symbol_size` bytes.
pub fn build_payload(meta: &ObjectMeta, file_meta: &FileMeta) -> Result<Vec<u8>> {
    build_payload_inner(meta, file_meta, None)
}

/// Serialize a descriptor-v5 large-transfer child object.
pub fn build_segment_payload(
    meta: &ObjectMeta,
    file_meta: &FileMeta,
    segment: &SegmentMeta,
) -> Result<Vec<u8>> {
    build_payload_inner(meta, file_meta, Some(segment))
}

fn build_payload_inner(
    meta: &ObjectMeta,
    file_meta: &FileMeta,
    segment: Option<&SegmentMeta>,
) -> Result<Vec<u8>> {
    let symbol_size = meta.symbol_size as usize;

    // Truncate filename if needed so the whole payload fits in one symbol, but
    // never split a multi-byte UTF-8 scalar: the receiver deliberately uses a
    // strict decoder and would reject an invalid descriptor.
    let blocks_len = meta.blocks.len() * 16;
    let segment_tail = if segment.is_some() {
        DESC_SEGMENT_TAIL_FIXED
    } else {
        0
    };
    let available_for_filename = symbol_size.saturating_sub(
        DESC_FIXED_OVERHEAD + blocks_len + DESC_V2_TAIL_FIXED + DESC_V3_TAIL_FIXED + segment_tail,
    );
    let filename_bytes = file_meta.filename.as_bytes();
    let mut filename_len = filename_bytes.len().min(available_for_filename).min(255);
    while !file_meta.filename.is_char_boundary(filename_len) {
        filename_len -= 1;
    }
    let filename_slice = &filename_bytes[..filename_len];

    // body = fixed overhead + blocks + (filename_len byte + filename) + v2 tail
    // (without its leading filename_len byte, already counted) + v3 tail.
    let body_len = DESC_FIXED_OVERHEAD + blocks_len + 1 + filename_len + DESC_V2_TAIL_FIXED - 1
        + DESC_V3_TAIL_FIXED
        + segment_tail;
    if body_len > symbol_size {
        return Err(Error::Protocol(qr_protocol::Error::BufferTooShort {
            need: body_len,
            have: symbol_size,
        }));
    }

    let mut buf = vec![0u8; symbol_size];
    buf[0] = DESC_MAGIC;
    buf[1] = if segment.is_some() {
        DESC_VERSION
    } else {
        DESC_V3_VERSION
    };
    buf[2..4].copy_from_slice(&(meta.blocks.len() as u16).to_be_bytes());
    buf[4..12].copy_from_slice(&meta.transfer_length.to_be_bytes());
    buf[12..16].copy_from_slice(&meta.symbol_size.to_be_bytes());
    buf[16..28].copy_from_slice(&meta.oti_bytes);

    let mut o = DESC_FIXED_OVERHEAD;
    for b in &meta.blocks {
        buf[o..o + 4].copy_from_slice(&b.sbn.to_be_bytes());
        o += 4;
        buf[o..o + 4].copy_from_slice(&b.num_source_symbols.to_be_bytes());
        o += 4;
        buf[o..o + 8].copy_from_slice(&b.block_length.to_be_bytes());
        o += 8;
    }

    // v2 extension: filename + original_size + crc32.
    buf[o] = filename_len as u8;
    o += 1;
    buf[o..o + filename_len].copy_from_slice(filename_slice);
    o += filename_len;
    buf[o..o + 8].copy_from_slice(&file_meta.original_size.to_be_bytes());
    o += 8;
    buf[o..o + 4].copy_from_slice(&file_meta.crc32.to_be_bytes());
    o += 4;

    // v3 extension: compression + compressed_size.
    buf[o] = file_meta.compression;
    o += 1;
    buf[o..o + 8].copy_from_slice(&file_meta.compressed_size.to_be_bytes());
    o += 8;

    if let Some(segment) = segment {
        buf[o..o + 16].copy_from_slice(&segment.root_session_id.to_be_bytes());
        o += 16;
        buf[o..o + 4].copy_from_slice(&segment.segment_index.to_be_bytes());
        o += 4;
        buf[o..o + 4].copy_from_slice(&segment.segment_count.to_be_bytes());
        o += 4;
        buf[o..o + 8].copy_from_slice(&segment.original_offset.to_be_bytes());
        o += 8;
        buf[o..o + 8].copy_from_slice(&segment.root_original_size.to_be_bytes());
        o += 8;
        buf[o..o + 32].copy_from_slice(&segment.root_sha256);
        o += 32;
        buf[o..o + 32].copy_from_slice(&segment.raw_sha256);
    }

    Ok(buf)
}

/// Parse a descriptor payload. Accepts v1, v2, v3, and segmented v5 descriptors.
///
/// Extension parsing is gated by the explicit version byte. Descriptor symbols
/// are zero-padded, so using payload length to infer a newer version would parse
/// v1/v2 padding as real fields and could truncate a recovered file to zero.
///
/// **Legacy v4 is rejected**: v4 used the 8 MiB raw-segment + per-segment-
/// compression model (incompatible with v5's compress-then-segment). Silently
/// accepting a v4 segmented transfer would mis-parse it as a single object.
///
/// Returns `None` if the payload is not a descriptor, uses an unknown/legacy
/// version, or is truncated below the fixed header + declared block table.
pub fn parse_payload(payload: &[u8]) -> Option<DescriptorInfo> {
    if payload.len() < DESC_FIXED_OVERHEAD || payload[0] != DESC_MAGIC {
        return None;
    }
    let version = payload[1];
    if !(1..=DESC_VERSION).contains(&version) {
        return None;
    }
    // Reject the legacy v4 segmentation model (8 MiB raw-segment, per-segment
    // compression) fail-closed: its field semantics are incompatible with v5,
    // and accepting it would mis-parse a v4 segmented transfer as a single
    // object (the v4 segment tail would be silently dropped).
    if version == DESC_LEGACY_V4 {
        return None;
    }

    let num_blocks = u16::from_be_bytes([payload[2], payload[3]]) as usize;
    let blocks_end = DESC_FIXED_OVERHEAD + num_blocks * 16;
    if payload.len() < blocks_end {
        return None;
    }

    let transfer_length = u64::from_be_bytes(payload[4..12].try_into().unwrap());
    let symbol_size = u32::from_be_bytes(payload[12..16].try_into().unwrap());
    let mut oti_bytes = [0u8; 12];
    oti_bytes.copy_from_slice(&payload[16..28]);

    let mut blocks = Vec::with_capacity(num_blocks);
    let mut o = DESC_FIXED_OVERHEAD;
    for _ in 0..num_blocks {
        let sbn = u32::from_be_bytes(payload[o..o + 4].try_into().unwrap());
        o += 4;
        let num_source_symbols = u32::from_be_bytes(payload[o..o + 4].try_into().unwrap());
        o += 4;
        let block_length = u64::from_be_bytes(payload[o..o + 8].try_into().unwrap());
        o += 8;
        blocks.push(raptorq_core::SourceBlockMeta {
            sbn,
            num_source_symbols,
            block_length,
        });
    }

    let meta = ObjectMeta {
        transfer_length,
        symbol_size,
        oti_bytes,
        blocks,
    };

    // v1 ends at the block table regardless of symbol padding.
    let file_meta = if version == 1 {
        FileMeta::default()
    } else {
        // v2 extension: filename_len + filename + original_size + crc32.
        if payload.len() <= o {
            return None;
        }
        let fn_len = payload[o] as usize;
        o += 1;
        if payload.len() < o + fn_len + 12 {
            return None;
        } else {
            let filename = String::from_utf8(payload[o..o + fn_len].to_vec()).ok()?;
            o += fn_len;
            let original_size = u64::from_be_bytes(payload[o..o + 8].try_into().unwrap());
            o += 8;
            let crc32 = u32::from_be_bytes(payload[o..o + 4].try_into().unwrap());
            o += 4;

            if version >= DESC_V3_VERSION {
                if payload.len() < o + DESC_V3_TAIL_FIXED {
                    return None;
                }
                let compression = payload[o];
                o += 1;
                let compressed_size = u64::from_be_bytes(payload[o..o + 8].try_into().unwrap());
                o += 8;
                FileMeta {
                    filename,
                    original_size,
                    crc32,
                    compression,
                    compressed_size,
                    compressed_size_known: true,
                    crc32_known: true,
                }
            } else {
                FileMeta {
                    filename,
                    original_size,
                    crc32,
                    compression: qr_protocol::compress::COMPRESSION_NONE,
                    // v2/v1 never carries a compressed payload length: assume the
                    // RaptorQ object is exactly the original file (no compression,
                    // no trimming). `compressed_size_known=false` would also be
                    // defensible, but the v3 spec defines the uncompressed case as
                    // "payload == original_size", so mirror that to stay correct
                    // for the (theoretical) v2 single-file sender.
                    compressed_size: original_size,
                    compressed_size_known: true,
                    crc32_known: true,
                }
            }
        }
    };

    let segment = if version == DESC_VERSION {
        if payload.len() < o + DESC_SEGMENT_TAIL_FIXED {
            return None;
        }
        let root_session_id = u128::from_be_bytes(payload[o..o + 16].try_into().ok()?);
        o += 16;
        let segment_index = u32::from_be_bytes(payload[o..o + 4].try_into().ok()?);
        o += 4;
        let segment_count = u32::from_be_bytes(payload[o..o + 4].try_into().ok()?);
        o += 4;
        let original_offset = u64::from_be_bytes(payload[o..o + 8].try_into().ok()?);
        o += 8;
        let root_original_size = u64::from_be_bytes(payload[o..o + 8].try_into().ok()?);
        o += 8;
        let mut root_sha256 = [0u8; 32];
        root_sha256.copy_from_slice(&payload[o..o + 32]);
        o += 32;
        let mut raw_sha256 = [0u8; 32];
        raw_sha256.copy_from_slice(&payload[o..o + 32]);
        Some(SegmentMeta {
            root_session_id,
            segment_index,
            segment_count,
            original_offset,
            root_original_size,
            root_sha256,
            raw_sha256,
        })
    } else {
        None
    };

    Some(DescriptorInfo {
        meta,
        file_meta,
        segment,
    })
}

/// Build a descriptor frame ready for transmission.
pub fn build_frame(
    meta: &ObjectMeta,
    file_meta: &FileMeta,
    session_id: u128,
    frame_index: u64,
    timestamp_ms: u64,
) -> Result<Frame> {
    let payload = build_payload(meta, file_meta)?;
    Ok(Frame::build(
        session_id,
        FLAG_DESCRIPTOR,
        0,
        0,
        meta.blocks.len() as u32,
        meta.blocks.iter().map(|b| b.num_source_symbols).sum(),
        meta.symbol_size,
        frame_index,
        timestamp_ms,
        &payload,
    ))
}

/// Build a descriptor-v5 frame for one large-transfer child object.
pub fn build_segment_frame(
    meta: &ObjectMeta,
    file_meta: &FileMeta,
    segment: &SegmentMeta,
    child_session_id: u128,
    frame_index: u64,
    timestamp_ms: u64,
) -> Result<Frame> {
    segment
        .validate(child_session_id, file_meta)
        .map_err(Error::InvalidSegment)?;
    let payload = build_segment_payload(meta, file_meta, segment)?;
    Ok(Frame::build(
        child_session_id,
        FLAG_DESCRIPTOR,
        0,
        0,
        meta.blocks.len() as u32,
        meta.blocks.iter().map(|b| b.num_source_symbols).sum(),
        meta.symbol_size,
        frame_index,
        timestamp_ms,
        &payload,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sender::{SenderConfig, SenderSession};
    use qr_protocol::SessionId;

    /// Build a v3 descriptor's payload, then simulate exactly what a *real* v2
    /// sender would have produced: drop the v3 tail and zero-fill the rest of the
    /// symbol. This is the layout an actual legacy v2 endpoint writes (it stops
    /// after crc32 and pads with zeros), distinct from the old test which kept the
    /// v3 bytes around and only changed the version byte.
    fn build_real_v2_payload(meta: &ObjectMeta, file_meta: &FileMeta) -> Vec<u8> {
        let mut full = build_payload(meta, file_meta).unwrap();
        // Recompute where the v2 body ends (fixed + blocks + fn_len + filename +
        // original_size + crc32) and clear everything from there to the end of the
        // symbol, exactly as a real v2 sender's zero-pad would.
        let blocks_len = meta.blocks.len() * 16;
        let fn_len = file_meta.filename.len();
        let v2_body_end = DESC_FIXED_OVERHEAD + blocks_len + 1 + fn_len + 8 + 4;
        full[1] = 2; // downgrade version byte
        for b in &mut full[v2_body_end..] {
            *b = 0;
        }
        full
    }

    #[test]
    fn descriptor_roundtrip_v3() {
        let data: Vec<u8> = (0..50_000).map(|i| (i & 0xff) as u8).collect();
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                filename: "test文档.pdf".to_string(),
                original_size: 50_000,
                crc32: 0xDEADBEEF,
                compression: qr_protocol::compress::COMPRESSION_NONE,
                compressed_size: 50_000,
                compressed_size_known: true,
                crc32_known: true,
            },
        )
        .unwrap();
        let meta = sender.meta().clone();
        let file_meta = sender.file_meta().clone();
        let payload = build_payload(&meta, &file_meta).unwrap();
        assert_eq!(payload.len(), 1024);
        let info = parse_payload(&payload).unwrap();
        assert_eq!(info.meta, meta);
        assert_eq!(info.file_meta.filename, "test文档.pdf");
        assert_eq!(info.file_meta.original_size, 50_000);
        assert_eq!(info.file_meta.crc32, 0xDEADBEEF);
        assert_eq!(
            info.file_meta.compression,
            qr_protocol::compress::COMPRESSION_NONE
        );
        assert_eq!(info.file_meta.compressed_size, 50_000);
        assert!(info.file_meta.crc32_known);
    }

    #[test]
    fn descriptor_roundtrip_v3_compressed() {
        // Simulate a zstd-compressed payload: original 50KB, compressed 18KB.
        let data: Vec<u8> = (0..50_000).map(|i| (i & 0xff) as u8).collect();
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                filename: "doc.json".to_string(),
                original_size: 50_000,
                crc32: 0xCAFEBABE,
                compression: qr_protocol::compress::COMPRESSION_ZSTD,
                compressed_size: 18_000,
                compressed_size_known: true,
                crc32_known: true,
            },
        )
        .unwrap();
        let meta = sender.meta().clone();
        let file_meta = sender.file_meta().clone();
        let payload = build_payload(&meta, &file_meta).unwrap();
        // A non-segmented object uses descriptor version 3.
        assert_eq!(payload[1], DESC_V3_VERSION);
        let info = parse_payload(&payload).unwrap();
        assert_eq!(info.segment, None);
        assert_eq!(
            info.file_meta.compression,
            qr_protocol::compress::COMPRESSION_ZSTD
        );
        assert_eq!(info.file_meta.compressed_size, 18_000);
        assert_eq!(info.file_meta.original_size, 50_000);
        assert_eq!(info.file_meta.crc32, 0xCAFEBABE);
        assert!(info.file_meta.crc32_known);
    }

    #[test]
    fn descriptor_roundtrip_v5_binds_root_sha256() {
        let data = vec![0x5au8; 4_096];
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                filename: "large.bin".to_string(),
                original_size: data.len() as u64,
                crc32: 0x12345678,
                compression: qr_protocol::compress::COMPRESSION_NONE,
                compressed_size: data.len() as u64,
                compressed_size_known: true,
                crc32_known: true,
            },
        )
        .unwrap();
        let segment = SegmentMeta {
            root_session_id: 0x1234,
            segment_index: 0,
            segment_count: 1,
            original_offset: 0,
            root_original_size: data.len() as u64,
            root_sha256: [0xa5; 32],
            raw_sha256: [0x5a; 32],
        };
        let payload = build_segment_payload(sender.meta(), sender.file_meta(), &segment).unwrap();
        assert_eq!(payload[1], DESC_VERSION);
        let parsed = parse_payload(&payload).unwrap();
        assert_eq!(parsed.segment, Some(segment));
    }

    #[test]
    fn descriptor_filename_truncation_preserves_utf8() {
        let data = vec![7u8; 1_024];
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                // 255 (the wire limit) falls in the middle of a four-byte
                // scalar, so safe truncation must back up to byte 252.
                filename: "😀".repeat(100),
                original_size: data.len() as u64,
                ..FileMeta::default()
            },
        )
        .unwrap();
        let payload = build_payload(sender.meta(), sender.file_meta()).unwrap();
        let parsed = parse_payload(&payload).expect("truncated name remains valid UTF-8");
        assert_eq!(parsed.file_meta.filename.len(), 252);
    }

    #[test]
    fn rejects_unknown_descriptor_version() {
        let data = vec![9u8; 1_024];
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta::default(),
        )
        .unwrap();
        let mut payload = build_payload(sender.meta(), sender.file_meta()).unwrap();
        payload[1] = DESC_VERSION + 1;
        assert!(parse_payload(&payload).is_none());
    }

    /// The legacy v4 segmentation model (8 MiB raw-segment + per-segment
    /// compression, used by withdrawn early v1.2.0 pre-release builds) is incompatible with v5's
    /// compress-then-segment semantics. Receivers must reject v4 fail-closed so
    /// a v4 segmented transfer is never silently mis-parsed as a single object
    /// (which would drop the segment tail and corrupt recovery). Two builds
    /// both labelled "v1.2.0" carrying different v4 semantics was the original
    /// bug; this rejection makes the incompatibility explicit.
    #[test]
    fn rejects_legacy_v4_descriptor() {
        let data = vec![0x5au8; 4_096];
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                filename: "legacy.bin".to_string(),
                original_size: data.len() as u64,
                ..FileMeta::default()
            },
        )
        .unwrap();
        // Build a current (v5) segment payload, then forge the version byte to
        // legacy v4 — simulating a v1.2.0-tagged sender's frame reaching a v5
        // receiver. parse_payload must refuse it.
        let segment = SegmentMeta {
            root_session_id: 0x1234,
            segment_index: 0,
            segment_count: 1,
            original_offset: 0,
            root_original_size: data.len() as u64,
            root_sha256: [0xa5; 32],
            raw_sha256: [0x5a; 32],
        };
        let mut payload =
            build_segment_payload(sender.meta(), sender.file_meta(), &segment).unwrap();
        assert_eq!(payload[1], DESC_VERSION); // currently 5
        payload[1] = DESC_LEGACY_V4; // forge legacy v4
        assert!(
            parse_payload(&payload).is_none(),
            "legacy v4 descriptor must be rejected, not silently mis-parsed"
        );
        // A non-segmented v3 payload with version forged to 4 is also rejected
        // (no valid descriptor uses v4 anymore).
        let mut plain = build_payload(sender.meta(), sender.file_meta()).unwrap();
        assert_eq!(plain[1], DESC_V3_VERSION);
        plain[1] = DESC_LEGACY_V4;
        assert!(parse_payload(&plain).is_none());
    }

    /// A genuine v2 descriptor — exactly what a real legacy v2 sender transmits:
    /// the v2 body (through crc32) followed by an all-zero padded tail. This must
    /// NOT be misread as a v3 tail with `compressed_size = 0` (which would make
    /// the receiver truncate the recovered payload to an empty file). The fix
    /// treats an all-zero trailing region as v2 padding, not a v3 extension.
    #[test]
    fn real_v2_descriptor_is_not_misread_as_empty_v3() {
        let data: Vec<u8> = (0..50_000).map(|i| (i & 0xff) as u8).collect();
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta::default(),
        )
        .unwrap();
        let meta = sender.meta().clone();
        let v2 = build_real_v2_payload(
            &meta,
            &FileMeta {
                filename: "legacy.bin".to_string(),
                original_size: 1_234,
                crc32: 0x11223344,
                compression: qr_protocol::compress::COMPRESSION_NONE,
                compressed_size: 1_234,
                compressed_size_known: true,
                crc32_known: true,
            },
        );

        let info = parse_payload(&v2).unwrap();
        // The v2 fields are intact.
        assert_eq!(info.file_meta.filename, "legacy.bin");
        assert_eq!(info.file_meta.original_size, 1_234);
        assert_eq!(info.file_meta.crc32, 0x11223344);
        // The all-zero tail must NOT be read as a v3 extension claiming a 0-byte
        // compressed payload — that would truncate the recovered file to empty.
        assert_eq!(
            info.file_meta.compression,
            qr_protocol::compress::COMPRESSION_NONE
        );
        assert_eq!(
            info.file_meta.compressed_size, 1_234,
            "v2 descriptor must fall back to compressed_size == original_size, not 0"
        );
    }

    #[test]
    fn padded_v1_descriptor_does_not_parse_padding_as_v2() {
        let data = vec![7u8; 8_000];
        let sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            FileMeta {
                filename: "must-not-appear.txt".into(),
                original_size: data.len() as u64,
                crc32: 123,
                compression: qr_protocol::compress::COMPRESSION_NONE,
                compressed_size: data.len() as u64,
                compressed_size_known: true,
                crc32_known: true,
            },
        )
        .unwrap();
        let mut payload = build_payload(sender.meta(), sender.file_meta()).unwrap();
        let v1_end = DESC_FIXED_OVERHEAD + sender.meta().blocks.len() * 16;
        payload[1] = 1;
        payload[v1_end..].fill(0);

        let parsed = parse_payload(&payload).unwrap();
        assert_eq!(parsed.file_meta, FileMeta::default());
    }

    /// Regression for the full bug: a receiver fed a real v2 descriptor must
    /// recover the original bytes, not an empty file. Before the fix, the all-zero
    /// padding was parsed as a v3 tail with compressed_size=0, so assemble()
    /// truncated the payload to 0 bytes.
    #[test]
    fn receiver_recovers_from_real_v2_descriptor_not_empty() {
        use crate::receiver::ReceiverSession;
        use raptorq_core::Config;

        let data: Vec<u8> = (0..40_000).map(|i| (i & 0xff) as u8).collect();
        // Build the sender with FileMeta whose compressed_size equals the padded
        // transfer length (the uncompressed path), then craft a real v2 descriptor
        // that drops the v3 tail and zero-pads.
        let probe = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig {
                codec: Config::default(),
                redundancy_pct: 20,
            },
            FileMeta::default(),
        )
        .unwrap();
        let meta = probe.meta().clone();
        let padded_len = meta.transfer_length;
        let fm = FileMeta {
            filename: "legacy.bin".to_string(),
            original_size: data.len() as u64,
            crc32: 0,
            compression: qr_protocol::compress::COMPRESSION_NONE,
            compressed_size: padded_len,
            compressed_size_known: true,
            crc32_known: false,
        };
        let mut sender = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig {
                codec: Config::default(),
                redundancy_pct: 20,
            },
            fm,
        )
        .unwrap();
        sender.set_descriptor_interval(8);

        // Drive a receiver purely from frames, mimicking the wire path.
        let first = sender.next_frame().unwrap();
        let mut rx =
            ReceiverSession::from_first_frame(&Frame::from_bytes(&first.to_bytes()).unwrap());
        let mut guard = 0;
        while !rx.is_complete() {
            let f = sender.next_frame().unwrap();
            let parsed = Frame::from_bytes(&f.to_bytes()).unwrap();
            let _ = rx.ingest(parsed);
            guard += 1;
            assert!(guard < 400_000, "v2 receiver did not recover");
        }
        let out = rx.assemble().expect("must assemble");
        // The killer assertion: recovered bytes are the full file, NOT empty.
        assert_eq!(
            &out[..data.len()],
            &data[..],
            "v2 descriptor must not truncate the recovered payload to empty"
        );
    }

    #[test]
    fn rejects_non_descriptor() {
        assert!(parse_payload(&[0u8; 1024]).is_none());
    }
}
