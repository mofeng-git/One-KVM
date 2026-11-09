//! QR matrix rendering.
//!
//! Encodes frame bytes into the *smallest* Error-Correction-L QR version that
//! holds the frame, and exposes the resulting module matrix as a flat
//! `Vec<bool>` (`true` == dark module) plus the side length. The display layer
//! (Canvas on the browser, native draw on Android) maps this matrix to pixels.
//!
//! ## QR backend
//!
//! Encoding is done by the [`fast_qr`] crate, an optimized QR generator that is
//! ~7-9× faster than the scalar `qrcode` crate on the per-frame Reed-Solomon
//! path — the dominant cost (~98%) of each render tick in the sender's rAF loop.
//! `fast_qr` compiles cleanly to `wasm32-unknown-unknown`, so the speedup
//! reaches the browser sender where it matters most on low-end hardware.
//!
//! ## Fixed-mask fast path (~10× additional speedup)
//!
//! fast_qr's default encoder evaluates all 8 data masks per QR code (cloning
//! the full module matrix 8 times + scoring each). For AirFerry, the frame size
//! is constant per session, so we fix one mask (Checkerboard / mask 0) and skip
//! the search entirely. The vendored fast_qr's `place_on_matrix` short-circuits
//! when a mask is specified (see `vendor/fast_qr/src/placement.rs`). Any valid
//! mask is decodable by ZXing — the receiver reads the mask ID from format info
//! and inverts it — so the optimal-mask search is unnecessary here.
//!
//! ## Why the minimal version (not a fixed Version 40)
//!
//! A Version-40 code is 177×177 modules — the densest QR there is. Forcing it
//! for a frame that only needs ~V23 wastes module budget and makes the code
//! hard for a phone camera to resolve, which manifests as the receiver never
//! accepting data symbols (stuck at "恢复中 0%"). We therefore pick the
//! smallest version whose *byte-mode* L capacity fits the frame, then encode the
//! payload explicitly in QR byte mode. Because the frame size is constant within
//! a session, every frame renders at the same fixed version, keeping the
//! on-screen QR size stable for the scanning camera. A 1088-byte frame drops
//! from V40 (177²) to V23 (109²); the smaller 576-byte default frame lands on
//! V16 (81²), far easier to scan.

use crate::{Error, Result};
use fast_qr::{datamasking::Mask, qr::QRBuilder, Version, ECL};
use std::vec::Vec;

/// Maximum payload (bytes) a Version-40 / L QR can carry in binary mode.
pub const QR_MAX_BYTES: usize = 2953;

/// Byte-mode data capacity (bytes) for QR Versions 1..=40 at EC level L
/// (ISO/IEC 18004 Table 7). Index `i` is the capacity of version `i+1`.
const QR_BYTE_CAPACITY_L: [usize; 40] = [
    17, 32, 53, 78, 106, 134, 154, 192, 230, 271, 321, 367, 425, 458, 520, 586, 644, 718, 792, 858,
    929, 1003, 1091, 1171, 1273, 1367, 1465, 1528, 1628, 1732, 1840, 1952, 2068, 2188, 2303, 2431,
    2563, 2699, 2809, 2953,
];

/// `fast_qr::Version` is a C-like enum (`V01=0`..`V40=39`) whose `from_n(n)`
/// constructor is `pub(crate)`, so we keep our own lookup table to convert a
/// 1-based version number (1..=40) into the enum without `unsafe` transmute.
const VERSIONS: [Version; 40] = [
    Version::V01,
    Version::V02,
    Version::V03,
    Version::V04,
    Version::V05,
    Version::V06,
    Version::V07,
    Version::V08,
    Version::V09,
    Version::V10,
    Version::V11,
    Version::V12,
    Version::V13,
    Version::V14,
    Version::V15,
    Version::V16,
    Version::V17,
    Version::V18,
    Version::V19,
    Version::V20,
    Version::V21,
    Version::V22,
    Version::V23,
    Version::V24,
    Version::V25,
    Version::V26,
    Version::V27,
    Version::V28,
    Version::V29,
    Version::V30,
    Version::V31,
    Version::V32,
    Version::V33,
    Version::V34,
    Version::V35,
    Version::V36,
    Version::V37,
    Version::V38,
    Version::V39,
    Version::V40,
];

/// Fixed data mask used for all QR codes. Mask 0 (Checkerboard: `(row + col) % 2 == 0`)
/// is chosen for its even distribution. The vendored fast_qr skips the 8-mask
/// search loop when a mask is specified via `QRBuilder.mask()` — any valid mask
/// is decodable by ZXing (it reads the mask ID from format info and inverts).
const FIXED_MASK: Mask = Mask::Checkerboard;

/// Smallest QR version (1..=40) whose byte-mode EC-L capacity holds `len`
/// bytes, or `None` if `len` exceeds Version 40's capacity ([`QR_MAX_BYTES`]).
pub fn min_version_for(len: usize) -> Option<u8> {
    QR_BYTE_CAPACITY_L
        .iter()
        .position(|&cap| cap >= len)
        .map(|i| (i + 1) as u8)
}

/// A rendered QR code: flat module grid (row-major) + side length.
#[derive(Debug, Clone)]
pub struct QrMatrix {
    /// `true` = dark module, `false` = light module. Row-major.
    pub modules: Vec<bool>,
    /// Modules per side (e.g. 177 for Version 40).
    pub size: usize,
}

impl QrMatrix {
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.modules[y * self.size + x]
    }
}

/// Encode `data` into the smallest EC-L QR version that holds it.
///
/// Starts at the version suggested by the byte-mode capacity table
/// ([`min_version_for`]) and, if `fast_qr` rejects that exact version (a rare
/// capacity-edge case where the fixed-width frame header consumes the last few
/// bytes of headroom), walks upward version by version up to Version 40. Walking
/// up guarantees every in-range frame renders, so the high-speed tier never
/// stutters.
///
/// `data` must be ≤ [`QR_MAX_BYTES`]; the caller (frame layer) guarantees this
/// since frames are fixed-size and well under a Version-40 payload.
pub fn encode(data: &[u8]) -> Result<QrMatrix> {
    let start = min_version_for(data.len()).ok_or(Error::BufferTooShort {
        need: QR_MAX_BYTES,
        have: data.len(),
    })?;
    // Walk from the table-suggested version up to 40 inclusive, returning the
    // first version that `fast_qr` accepts.
    for v in start..=40 {
        let version = VERSIONS[(v - 1) as usize];
        // Fixed mask: skips the 8-mask evaluation loop in the vendored fast_qr
        // (placement.rs short-circuits when .mask() is set). ~10× speedup.
        let code = match QRBuilder::new(data)
            .ecl(ECL::L)
            .version(version)
            .mask(FIXED_MASK)
            .build()
        {
            Ok(code) => code,
            // Capacity edge: this version can't hold the payload; try the next.
            Err(_) => continue,
        };
        let size = code.size;
        // `fast_qr::QRCode.data` is a fixed `[Module; 177*177]` array; only the
        // leading `size*size` entries are meaningful. `Module::value()` is
        // `true` for a dark module — exactly the convention `QrMatrix` uses.
        let modules = code.data[..size * size].iter().map(|m| m.value()).collect();
        return Ok(QrMatrix { modules, size });
    }
    Err(Error::BufferTooShort {
        need: QR_MAX_BYTES,
        have: data.len(),
    })
}

// ── Future work ──────────────────────────────────────────────────────────
//
// TODO: 多色码（Color QR）方案
//
// 当前每个 QR 都是黑/白二值，多码只能通过增加码的数量（2/4 个）来提升
// 吞吐。理论上有更高效的方案：
//
//   1. 用不同前景色区分同一帧的多个数据段（比如红+蓝 = 2 个独立码），
//      单帧内叠加更多数据而不缩小单个码的尺寸。ZXing-C++ 在
//      `TryInvert=true` 时会尝试反转颜色，但对彩色前景的处理尚不确定。
//
//   2. 用彩色静默区/背景作为低频信道携带额外的 metadata（如 session id、
//      帧序号），减少描述符帧的占比。
//
// 需要研究的问题：
//   - ZXing / zbar 对 RGB 前景色 QR 的解码能力
//   - 不同手机摄像头的色彩还原准确度（白平衡、伽马）
//   - 摩尔纹对彩色 QR 的影响是否大于二值 QR
//   - 编码端（WASM Canvas）的颜色渲染精度
//   - 接收端是否需要增加 `setTryRotate(false)` 以避免颜色通道混淆
//
// ── End future work ──────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_frame_sized_payload() {
        // A 1088-byte frame fits in Version 23 (109×109) at EC-L — far sparser
        // than the old forced Version 40 (177×177).
        let data = vec![0u8; 1088];
        let m = encode(&data).unwrap();
        assert_eq!(m.size, 109);
        assert_eq!(m.modules.len(), 109 * 109);
    }

    #[test]
    fn encodes_all_sender_symbol_sizes() {
        // Every symbol size the sender exposes as a speed preset must render at
        // the expected minimal QR version/side. This guards the capacity table
        // ↔ `fast_qr` version hand-off for the full preset range.
        use crate::Frame;
        // (symbol_size, expected_side)  — side = 21 + 4*(v-1)
        let cases: [(usize, usize); 5] = [
            (512, 81),   // V16
            (896, 105),  // V22
            (1008, 109), // V23
            (1024, 109), // V23
            (1400, 125), // V27
        ];
        for &(sym_size, expected_side) in &cases {
            let frame = Frame::build(
                0u128,
                0,  // flags
                1,  // sbn
                1,  // esi
                1,  // total_blocks
                10, // total_symbols
                sym_size as u32,
                1,       // frame_index
                1234567, // timestamp_ms
                &vec![0u8; sym_size],
            );
            let bytes = frame.to_bytes();
            let m = encode(&bytes).unwrap();
            assert_eq!(
                m.size,
                expected_side,
                "symbol_size={} frame ({} wire bytes) expected side {}, got {}",
                sym_size,
                bytes.len(),
                expected_side,
                m.size
            );
            assert_eq!(m.modules.len(), expected_side * expected_side);
        }
    }

    #[test]
    fn picks_minimal_version() {
        assert_eq!(min_version_for(1088), Some(23));
        assert_eq!(min_version_for(576), Some(16));
        assert_eq!(min_version_for(17), Some(1));
        assert_eq!(min_version_for(QR_MAX_BYTES), Some(40));
        assert_eq!(min_version_for(QR_MAX_BYTES + 1), None);
    }

    #[test]
    fn rejects_oversized_payload() {
        let data = vec![0u8; QR_MAX_BYTES + 1];
        assert!(encode(&data).is_err());
    }

    #[test]
    fn deterministic_encoding() {
        let data = vec![42u8; 500];
        let a = encode(&data).unwrap();
        let b = encode(&data).unwrap();
        assert_eq!(a.modules, b.modules);
    }
}
