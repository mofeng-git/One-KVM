//! Unified compression interface (Zstd + XZ/LZMA2).
//!
//! Only available on native / Android targets. On `wasm32-unknown-unknown` the
//! underlying C libraries do not compile, so the browser extension performs
//! compression on the JavaScript side (using the same standard zstd format)
//! before handing the bytes to the Rust core. The on-wire format is identical,
//! so bytes compressed on one side decompress correctly on the other.
//!
//! ## Algorithm selection
//!
//! The wire protocol tags every transfer with a [`COMPRESSION_*`] byte so the
//! receiver knows which decoder to run. Zstd is the default; XZ (LZMA2) gives
//! a better ratio for text-heavy payloads. Native Rust implements both directions,
//! while the browser supplies interoperable zstd/XZ streams from its worker.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::Error;
use crate::Result;

/// Maximum compression level for small files where compression time is negligible.
/// Using level 22 (maximum) for best compression ratio on typical small files (<10MB).
pub const DEFAULT_LEVEL: i32 = 22;

/// Compression-algorithm tags carried in the descriptor (1 byte, big-endian).
pub const COMPRESSION_NONE: u8 = 0;
pub const COMPRESSION_ZSTD: u8 = 1;
pub const COMPRESSION_XZ: u8 = 2;

/// True for on-wire algorithm tags the stack implements end-to-end.
#[inline]
pub fn is_known_compression_tag(tag: u8) -> bool {
    matches!(tag, COMPRESSION_NONE | COMPRESSION_ZSTD | COMPRESSION_XZ)
}

/// XZ/LZMA2 preset. The low 5 bits are the compression level (0..=9); bit 31
/// is `LZMA_PRESET_EXTREME` (0x8000_0000), which enables a much slower but
/// higher-ratio search at the given level.
///
/// We use level 6 (the default for xz tools) with the EXTREME flag. Level 9
/// peaks at ~700 MB of memory on the *decoder* side, which OOMs the typical
/// Android JVM heap (256 MB); level 6 keeps the decoder footprint around
/// ~95 MB while still compressing text-heavy payloads well.
///
/// NOTE: the browser sender (`compress.ts`) uses level 9. The two presets
/// produce *interoperable* .xz streams (any compliant LZMA2 reader handles
/// either), so the cross-language link is correct — only the ratio/speed
/// trade-off differs per side. See `XZ_COMPRESSION_PLAN.md` for the rationale.
#[cfg(not(target_arch = "wasm32"))]
const LZMA_PRESET_EXTREME: u32 = 0x8000_0000;
#[cfg(not(target_arch = "wasm32"))]
const XZ_PRESET: u32 = 6 | LZMA_PRESET_EXTREME;
/// Decoder dictionary/memory ceiling independent of the output byte cap.
#[cfg(not(target_arch = "wasm32"))]
const XZ_DECODER_MEMORY_LIMIT: u64 = 128 * 1024 * 1024;

/// Maximum zstd back-reference window (`2^log` bytes) enforced on BOTH sides
/// of this stack (audit L1). libzstd defaults to
/// `ZSTD_WINDOWLOG_LIMIT_DEFAULT = 27`, i.e. a hostile CRC-valid frame header
/// can force a ~128 MiB window allocation *before* any output is produced —
/// RAM that is independent of `read_capped`/`decompress_stream_to_file`'s
/// output cap (the bomb defense only bounds output bytes). Clamping the
/// window tightens that input-side hole, mirroring the XZ path's
/// `XZ_DECODER_MEMORY_LIMIT`.
///
/// Choice of 23 (8 MiB window):
/// - **Production sender** = the TS worker's wasm-zstd at **level 1**: zstd's
///   default cParams table caps level 1 at `windowLog = 19` for any source
///   size (`clevels.h`, "> 256 KB" row), so ≤ 256 MiB originals stay at 19 —
///   23 leaves four levels of headroom (table values only reach 23 at
///   level 17).
/// - **Rust-side compression** (`DEFAULT_LEVEL = 22`, test path): the zstd
///   crate's `encode_all` is a *streaming* encoder with no pledged src size,
///   so level 22 used to emit `windowLog = 27` frames regardless of input
///   size (the table value is never srcSize-clamped on that path — verified
///   empirically: even an 8 KiB level-22 stream was rejected by a 23-clamped
///   decoder before this change). [`compress`] now caps the encoder window
///   at the same 23, so every stream this stack produces decodes under the
///   clamp by construction (root `cargo test` green).
#[cfg(not(target_arch = "wasm32"))]
const ZSTD_WINDOW_LOG_MAX: u32 = 23;

/// Build a zstd streaming decoder with the receiver-side window clamp
/// ([`ZSTD_WINDOW_LOG_MAX`]) applied. All untrusted-input decode paths must
/// go through this so no `Decoder` is ever constructed unclamped.
#[cfg(not(target_arch = "wasm32"))]
fn zstd_decoder<R: std::io::BufRead>(reader: R) -> Result<zstd::stream::read::Decoder<'static, R>> {
    let mut dec = zstd::stream::read::Decoder::with_buffer(reader)
        .map_err(|e| Error::Compress(e.to_string()))?;
    dec.window_log_max(ZSTD_WINDOW_LOG_MAX)
        .map_err(|e| Error::Compress(e.to_string()))?;
    Ok(dec)
}

/// Compress `data` with zstd at the given level.
/// For small files, uses maximum compression (level 22) by default.
///
/// The encoder window is capped at [`ZSTD_WINDOW_LOG_MAX`] so every stream
/// this stack produces stays decodable under the matching receiver-side
/// window clamp. (The zstd crate's `encode_all` is a streaming encoder with
/// no pledged src size, so high levels otherwise declare the full table
/// window — level 22 → windowLog 27 — even for tiny inputs, which its own
/// clamped decoder would then reject.)
#[cfg(not(target_arch = "wasm32"))]
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), level)
        .map_err(|e| Error::Compress(e.to_string()))?;
    encoder
        .window_log(ZSTD_WINDOW_LOG_MAX)
        .map_err(|e| Error::Compress(e.to_string()))?;
    encoder
        .write_all(data)
        .map_err(|e| Error::Compress(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| Error::Compress(e.to_string()))
}

/// Decompress zstd-encoded `data`. (Kept for backward compatibility.)
///
/// Uses the same window clamp as [`decompress_with_limit`] so no untrusted
/// input is ever decoded through an unclamped decoder.
#[cfg(not(target_arch = "wasm32"))]
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut dec = zstd_decoder(data)?;
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .map_err(|e| Error::Compress(e.to_string()))?;
    Ok(out)
}

/// Compress `data` with the algorithm identified by a [`COMPRESSION_*`] tag.
///
/// `COMPRESSION_NONE` returns the bytes unchanged. Unknown tags are treated as
/// no compression so a receiver never fails purely on an unrecognized algo.
#[cfg(not(target_arch = "wasm32"))]
pub fn compress_with(data: &[u8], compression: u8) -> Result<Vec<u8>> {
    match compression {
        COMPRESSION_ZSTD => compress(data, DEFAULT_LEVEL),
        COMPRESSION_XZ => xz_compress(data),
        _ => Ok(data.to_vec()),
    }
}

/// Decompress `data` using the algorithm identified by a [`COMPRESSION_*`] tag.
///
/// `COMPRESSION_NONE` (and any unrecognized tag) returns the bytes unchanged,
/// which keeps a descriptor/algorithm mismatch non-fatal.
#[cfg(not(target_arch = "wasm32"))]
pub fn decompress_with(data: &[u8], compression: u8) -> Result<Vec<u8>> {
    match compression {
        COMPRESSION_ZSTD => decompress(data),
        COMPRESSION_XZ => xz_decompress(data),
        _ => Ok(data.to_vec()),
    }
}

/// Like [`decompress_with`] but bounds the **output** size to `max_output` bytes.
///
/// The receiver decompresses data recovered from an untrusted optical stream. A
/// tiny zstd/xz payload can legitimately expand 1000×+ (a "decompression bomb"),
/// so without an output cap a crafted transfer would OOM the Android receiver at
/// assemble time. The caller passes the descriptor's expected original size as
/// the cap; if the stream produces more than that, it's rejected. Unknown
/// algorithm tags with non-empty payload return an error (see
/// [`is_known_compression_tag`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn decompress_with_limit(data: &[u8], compression: u8, max_output: usize) -> Result<Vec<u8>> {
    match compression {
        COMPRESSION_ZSTD => {
            let dec = zstd_decoder(data)?;
            read_capped(dec, max_output)
        }
        COMPRESSION_XZ => {
            let stream = xz2::stream::Stream::new_stream_decoder(XZ_DECODER_MEMORY_LIMIT, 0)
                .map_err(|e| Error::Compress(e.to_string()))?;
            read_capped(xz2::read::XzDecoder::new_stream(data, stream), max_output)
        }
        _ => {
            if !is_known_compression_tag(compression) && !data.is_empty() {
                return Err(Error::Compress(format!(
                    "unknown compression algorithm tag {compression}"
                )));
            }
            if data.len() > max_output {
                return Err(Error::Compress("payload exceeds size limit".into()));
            }
            Ok(data.to_vec())
        }
    }
}

/// Read a decoder fully but refuse to produce more than `max_output` bytes.
#[cfg(not(target_arch = "wasm32"))]
fn read_capped<R: std::io::Read>(r: R, max_output: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    // Read one byte past the cap so an over-limit stream can be detected.
    let read_limit = u64::try_from(max_output)
        .unwrap_or(u64::MAX - 1)
        .saturating_add(1);
    r.take(read_limit)
        .read_to_end(&mut out)
        .map_err(|e| Error::Compress(e.to_string()))?;
    if out.len() > max_output {
        return Err(Error::Compress(
            "decompressed output exceeds expected size".into(),
        ));
    }
    Ok(out)
}

/// Streaming result of [`decompress_stream_to_file`].
#[cfg(not(target_arch = "wasm32"))]
pub struct DecompressStreamOutcome {
    /// Number of decompressed bytes written to the output file.
    pub output_size: u64,
    /// Incremental CRC32 over the decompressed bytes.
    pub crc32: u32,
    /// Incremental SHA-256 over the decompressed bytes.
    pub sha256: [u8; 32],
}

/// Stream a compressed stream from `input_path` to `output_path`, decompressing
/// as it goes, while computing CRC32 + SHA-256 incrementally. Neither the
/// compressed input nor the decompressed output is ever held wholly in memory,
/// so a very large file can be recovered within bounded RAM.
///
/// `max_output` is a hard cap on the decompressed size (defends against a
/// decompression bomb): the stream is rejected as soon as it would exceed it.
/// On any failure (I/O, cap breach, decoder error) the partial output file is
/// removed so a later retry never reads a truncated file as success.
#[cfg(not(target_arch = "wasm32"))]
pub fn decompress_stream_to_file(
    input_path: &str,
    output_path: &str,
    compression: u8,
    max_output: u64,
) -> Result<DecompressStreamOutcome> {
    use sha2::Digest;
    use std::io::{BufWriter, Read, Write};

    let mut in_file =
        std::fs::File::open(input_path).map_err(|e| Error::Compress(format!("open input: {e}")))?;
    let out_file = std::fs::File::create(output_path)
        .map_err(|e| Error::Compress(format!("create output: {e}")))?;
    let mut writer = BufWriter::with_capacity(1 << 20, out_file);

    // Channel a capped decode into a closure that hashes + writes chunks.
    let mut crc = crc32fast::Hasher::new();
    let mut sha = sha2::Sha256::new();
    let mut written: u64 = 0;
    let mut over = false;

    let mut decode = |reader: &mut dyn Read| -> Result<()> {
        let mut reader = reader.take(max_output.saturating_add(1));
        let mut buf = [0u8; 256 * 1024];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| Error::Compress(format!("read: {e}")))?;
            if n == 0 {
                break;
            }
            written = written.saturating_add(n as u64);
            if written > max_output {
                over = true;
                break;
            }
            crc.update(&buf[..n]);
            sha.update(&buf[..n]);
            writer
                .write_all(&buf[..n])
                .map_err(|e| Error::Compress(format!("write: {e}")))?;
        }
        Ok(())
    };

    // Build the decoder and run the decode loop, threading ALL errors through
    // `result` (never `?` out of this match) so the `if let Err(e) = result`
    // cleanup below removes the partial output file on ANY failure — including
    // decoder construction (a corrupt/truncated compressed stream can make
    // `Decoder::new` / `Stream::new_stream_decoder` fail, leaving a freshly-
    // created empty output file that must not linger).
    let result: Result<()> = match compression {
        // Window-clamped streaming decoder (audit L1): same
        // ZSTD_WINDOW_LOG_MAX bound as the in-memory path. The ~128 KiB
        // BufReader capacity mirrors libzstd's ZSTD_DStreamInSize (what
        // Decoder::new would have used).
        COMPRESSION_ZSTD => zstd_decoder(std::io::BufReader::with_capacity(
            128 * 1024,
            in_file,
        ))
        .and_then(|mut dec| decode(&mut dec)),
        COMPRESSION_XZ => xz2::stream::Stream::new_stream_decoder(XZ_DECODER_MEMORY_LIMIT, 0)
            .map_err(|e| Error::Compress(e.to_string()))
            .and_then(|stream| {
                let mut dec = xz2::read::XzDecoder::new_stream(in_file, stream);
                decode(&mut dec)
            }),
        _ => {
            // COMPRESSION_NONE (or unknown tag with empty input): the "stream"
            // is already the original bytes — copy as-is.
            if is_known_compression_tag(compression) || {
                // An unknown tag with non-empty input is an error, mirroring
                // `decompress_with_limit`.
                match std::fs::metadata(input_path) {
                    Ok(m) => m.len() == 0,
                    Err(_) => false,
                }
            } {
                decode(&mut in_file)
            } else {
                Err(Error::Compress(format!(
                    "unknown compression algorithm tag {compression}"
                )))
            }
        }
    };

    if let Err(e) = result {
        drop(writer);
        let _ = std::fs::remove_file(output_path);
        return Err(e);
    }
    if over {
        drop(writer);
        let _ = std::fs::remove_file(output_path);
        return Err(Error::Compress(
            "decompressed output exceeds expected size".into(),
        ));
    }
    // Flush is the last fallible step. The documented contract is "any failure
    // removes the partial output" — flush must not be a `?` that bypasses the
    // remove (a failed flush can leave a partial/truncated file on disk). Handle
    // it inline and remove on failure, mirroring the decode/over-limit branches.
    if let Err(e) = writer.flush() {
        drop(writer);
        let _ = std::fs::remove_file(output_path);
        return Err(Error::Compress(format!("flush: {e}")));
    }
    let digest = sha.finalize();
    Ok(DecompressStreamOutcome {
        output_size: written,
        crc32: crc.finalize(),
        sha256: digest.into(),
    })
}

/// wasm32 decompress stub.
///
/// The native zstd/xz C libraries do not compile under `wasm32-unknown-unknown`,
/// so the browser cannot decompress inside the Rust core. Historically this was
/// an identity stub (returning the input unchanged) because "the receiver never
/// runs in the browser". That is no longer true: the web receiver now recovers
/// files in the browser. Returning compressed bytes as-is would silently hand
/// the JS layer a zstd/xz stream while claiming it is the original file.
///
/// Instead this is now **fail-closed**:
/// - `COMPRESSION_NONE` returns the bytes unchanged (correct — nothing to do).
/// - `COMPRESSION_ZSTD` / `COMPRESSION_XZ` return `Err`. The web receiver uses
///   [`ReceiverSession::assemble_raw`] (no decompression) and decompresses with
///   its own JS-side zstd/xz WASM, so it never relies on this path — but any
///   caller that *does* hit `assemble_result` on a compressed payload gets a
///   clear error instead of corrupted output.
#[cfg(target_arch = "wasm32")]
pub fn decompress_with_limit(data: &[u8], compression: u8, _max_output: usize) -> Result<Vec<u8>> {
    if compression == COMPRESSION_NONE {
        Ok(data.to_vec())
    } else {
        Err(Error::Compress(format!(
            "decompression is not available on wasm32 for compression tag {compression}; \
             use assemble_raw + JS-side decompression"
        )))
    }
}

/// wasm32 stub mirroring [`decompress_with_limit`] (no output cap). See that
/// function for why compressed payloads fail-closed.
#[cfg(target_arch = "wasm32")]
pub fn decompress_with(data: &[u8], compression: u8) -> Result<Vec<u8>> {
    if compression == COMPRESSION_NONE {
        Ok(data.to_vec())
    } else {
        Err(Error::Compress(format!(
            "decompression is not available on wasm32 for compression tag {compression}; \
             use assemble_raw + JS-side decompression"
        )))
    }
}

/// Compress `data` with XZ/LZMA2 at a high-ratio preset (level 6 + EXTREME).
///
/// Slower than zstd but yields a better ratio for text-heavy payloads. Memory
/// usage stays modest at this level (~95 MB decoder footprint), which keeps
/// the Android JVM heap (typically 256 MB) safe even on low-end devices.
#[cfg(not(target_arch = "wasm32"))]
fn xz_compress(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), XZ_PRESET);
    encoder
        .write_all(data)
        .map_err(|e| Error::Compress(e.to_string()))?;
    encoder.finish().map_err(|e| Error::Compress(e.to_string()))
}

/// Decompress XZ/LZMA2-encoded `data`.
#[cfg(not(target_arch = "wasm32"))]
fn xz_decompress(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = xz2::read::XzDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| Error::Compress(e.to_string()))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_round_trip() {
        let data: Vec<u8> = (0..40_000).map(|i| (i & 0xff) as u8).collect();
        let c = compress(&data, DEFAULT_LEVEL).unwrap();
        let d = decompress(&c).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn zstd_compressed_data_shrinks_for_repetitive_input() {
        let data = vec![0xABu8; 10_000];
        let c = compress(&data, DEFAULT_LEVEL).unwrap();
        assert!(c.len() < data.len());
    }

    #[test]
    fn xz_round_trip() {
        let data: Vec<u8> = (0..10_000).map(|i| (i & 0xff) as u8).collect();
        let compressed = xz_compress(&data).unwrap();
        let decompressed = xz_decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn xz_compresses_repetitive_input_aggressively() {
        let data = vec![0xABu8; 10_000];
        let compressed = xz_compress(&data).unwrap();
        // Highly repetitive input should compress well over 90% (the .xz stream
        // container itself costs ~60 bytes of header/footer/index).
        assert!(compressed.len() < data.len() / 10);
    }

    #[test]
    fn compress_with_and_decompress_with_dispatch() {
        let data: Vec<u8> = (0..8_000).map(|i| (i & 0xff) as u8).collect();

        // Zstd path.
        let z = compress_with(&data, COMPRESSION_ZSTD).unwrap();
        assert_eq!(decompress_with(&z, COMPRESSION_ZSTD).unwrap(), data);

        // XZ path.
        let x = compress_with(&data, COMPRESSION_XZ).unwrap();
        assert_eq!(decompress_with(&x, COMPRESSION_XZ).unwrap(), data);

        // None path is identity.
        assert_eq!(compress_with(&data, COMPRESSION_NONE).unwrap(), data);
        assert_eq!(decompress_with(&data, COMPRESSION_NONE).unwrap(), data);
    }

    #[test]
    fn unknown_compression_tag_is_identity() {
        let data = vec![1u8, 2, 3, 4];
        assert_eq!(compress_with(&data, 99).unwrap(), data);
        assert_eq!(decompress_with(&data, 99).unwrap(), data);
    }

    #[test]
    fn unknown_compression_tag_rejected_on_limited_decompress() {
        let data = vec![1u8, 2, 3, 4];
        assert!(decompress_with_limit(&data, 99, 1024).is_err());
        assert_eq!(
            decompress_with_limit(&[], 99, 1024).unwrap(),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn decompress_with_limit_rejects_bomb() {
        // Highly compressible input expands far beyond a tiny cap.
        let data = vec![0u8; 1_000_000];
        let z = compress(&data, DEFAULT_LEVEL).unwrap();
        assert!(z.len() < 10_000, "should compress tiny");
        // Cap below the true output → rejected (bomb defense).
        assert!(decompress_with_limit(&z, COMPRESSION_ZSTD, 1000).is_err());
        // Cap at the true output → ok.
        assert_eq!(
            decompress_with_limit(&z, COMPRESSION_ZSTD, data.len()).unwrap(),
            data
        );

        // XZ path behaves the same.
        let x = xz_compress(&data).unwrap();
        assert!(decompress_with_limit(&x, COMPRESSION_XZ, 1000).is_err());
        assert_eq!(
            decompress_with_limit(&x, COMPRESSION_XZ, data.len()).unwrap(),
            data
        );
    }

    /// Hand-assembled, otherwise-valid zstd frame that declares a
    /// windowLog of 27 (a 128 MiB back-reference window — the libzstd default
    /// `ZSTD_WINDOWLOG_LIMIT_DEFAULT`, so an *unclamped* streaming decoder
    /// accepts it and allocates the full window before producing any output).
    /// Body is a single raw (uncompressed) block, so the stream itself is
    /// perfectly decodable; only the declared window is oversized.
    fn oversized_window_frame() -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]); // zstd magic (LE)
        f.push(0x00); // Frame_Header_Descriptor: no FCS, no checksum, no dict
        f.push(0x88); // Window_Descriptor: exponent 17, mantissa 0 → windowLog 27
        // Block header (3B LE): last=1, type=0 (Raw), size=4.
        f.extend_from_slice(&[0x21, 0x00, 0x00]);
        f.extend_from_slice(b"ABCD");
        f
    }

    /// Audit L1: the receiver-side zstd decoder must clamp
    /// `ZSTD_d_windowLogMax` so a hostile CRC-valid frame header cannot force
    /// a 128 MiB window allocation. The frame built above is wire-valid (an
    /// unclamped decoder returns its payload), yet the clamped
    /// `decompress_with_limit` must reject it.
    #[test]
    fn decompress_with_limit_rejects_oversized_zstd_window() {
        let frame = oversized_window_frame();
        // Sanity: the frame is a legitimate zstd stream that a default
        // (unclamped) decoder happily decodes.
        assert_eq!(
            zstd::decode_all(&frame[..]).unwrap(),
            b"ABCD".to_vec(),
            "test frame must be a valid zstd stream"
        );
        // The clamped receiver path refuses it (frameParameter_unsupported).
        assert!(
            decompress_with_limit(&frame, COMPRESSION_ZSTD, 1024).is_err(),
            "oversized zstd window must be rejected"
        );
    }

    /// Same clamp on the streaming-to-disk path used by Android/Windows for
    /// descriptor-v5 segmented transfers: an oversized-window frame must fail
    /// (and the partial output file must be cleaned up).
    #[test]
    fn decompress_stream_to_file_rejects_oversized_zstd_window() {
        let frame = oversized_window_frame();
        let dir = std::env::temp_dir();
        let input = dir.join(format!("airferry_win_clamp_in_{}.zst", std::process::id()));
        let output = dir.join(format!("airferry_win_clamp_out_{}.bin", std::process::id()));
        std::fs::write(&input, &frame).unwrap();
        let result = decompress_stream_to_file(
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            COMPRESSION_ZSTD,
            1024,
        );
        let _ = std::fs::remove_file(&input);
        assert!(result.is_err(), "oversized zstd window must be rejected");
        assert!(
            !output.exists(),
            "failed decode must not leave a partial output file"
        );
    }

    /// The clamp must not break legitimate streams: every level/size this
    /// stack produces must still round-trip through the *clamped* decoder.
    /// Level 1 is what the production TS sender uses (≤ 256 MiB inputs,
    /// table windowLog 19); level 22 is the Rust-side default — [`compress`]
    /// caps its encoder window at 23, so its output declares ≤ 23 and is
    /// accepted by construction.
    #[test]
    fn clamped_zstd_decoder_accepts_legitimate_streams() {
        for (level, len) in [(1, 1 << 20), (3, 300_000), (DEFAULT_LEVEL, 40_000)] {
            let data: Vec<u8> = (0..len).map(|i| (i * 31 & 0xff) as u8).collect();
            let z = compress(&data, level).unwrap();
            assert_eq!(
                decompress_with_limit(&z, COMPRESSION_ZSTD, data.len()).unwrap(),
                data,
                "level {level} / {len} bytes must survive the window clamp"
            );
            // And the legacy `decompress` path (same clamp).
            assert_eq!(decompress(&z).unwrap(), data);
        }
    }
}
