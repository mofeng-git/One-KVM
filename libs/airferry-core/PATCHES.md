# Local patches

Local changes to the third-party sources in `airferry-core`, grouped by component.
Paths within each section are relative to that component directory.

## ZXing-C++

Directory: [native/vendor/zxing](native/vendor/zxing/).

Source: ZXing-C++, revision 8dd1cf5c4fd6fb6211bb96713db926ac6f2cf825, Apache-2.0.

One-KVM retains only the QR reader subset. Removed the aztec, datamatrix,
maxicode, oned, pdf417 and libzint trees, QR encoders, unused common encoder
implementations, C API and Qt integration. CMake omits their source lists and
rejects configurations that enable removed features. Detector, bit-matrix
parsing, Reed-Solomon correction and compiler optimization options are retained.

Further reader-only cleanup removes CreateBarcode/WriteBarcode API stubs,
MultiFormatWriter.h, the ZXingCpp version wrapper, the deprecated ZXVersion.h
shim and the unused WhiteRectDetector. Remove writer friend/forward declarations
from Barcode.h. Reader algorithms and compiler optimization options are unchanged.
ZXTestSupport.h remains because QRDecoder.cpp includes it.

Remove standalone pkg-config/CMake package templates, public-header installation
and export rules. The parent still installs libZXing.a and libairferry_native.a
for Cargo; Version.h is still generated for feature macros used by the reader.
The development CLI and fixture generator have been removed. LICENSE remains.

Raw-byte-only content:
- Remove text/textMode/characterSet/contentType/bytesECI APIs, encoding guessing,
  HRI formatting and Unicode conversion. Barcode::extra("ALL") retains raw hex
  bytes and detection metadata, omitting text and content-type fields.
- Content stores only original payload bytes, symbology and ECI presence. Byte,
  numeric, alphanumeric, Kanji, Hanzi, FNC1 and structured-append parsing retain
  their original byte output. ECI designators are still consumed and validated,
  including truncated and malformed-designator errors, without transcoding.
- Remove libzueci, its conversion tables, CharacterSet/ECI/TextDecoder/Utf/HRI
  implementations and GTIN helpers. These text APIs are not used by One-KVM.
- File names and file contents remain protocol bytes; Unicode file names are
  handled by the Rust protocol/receiver, independently of QR text conversion.

Validation: native/tests/raw_bytes.cpp covers every byte value, invalid UTF-8,
ECI widths and switches, numeric/alphanumeric/Kanji/Hanzi/FNC1, structured append,
malformed streams, and a fixed binary QR image (normal/inverted/transposed).
The same vectors pass against the pre-change decoder and this raw-byte subset.

## raptorq

Directory: [vendor/raptorq](vendor/raptorq/).

Source: crates.io raptorq 2.0.1, Apache-2.0 (see [LICENSE](vendor/raptorq/LICENSE)).

ARM32 stable Rust fix: gate the five NEON implementation functions in
`src/octets.rs` on aarch64 only. Upstream already disables all four ARM32
NEON dispatch calls, but still compiles the implementations, which refer to
unstable ARM32 intrinsics, AArch64-only table instructions and an unavailable
`uint8x16_t` type. ARMv7 uses the existing scalar fallback. x86 and aarch64
dispatch and the protocol remain unchanged. The registry cache is not modified.

Native-only cleanup: remove python.rs, the python/pyo3 feature and dependency,
benchmark-only exports and memory-accounting helpers, and stale example/bench
manifest entries. Python exclusion guards are removed; test-only decoder
controls remain test-only. Remove Criterion and dependency-local profile
sections (the parent application's profiles control dependency builds).
Keep primal/rand/threadpool because the retained upstream unit tests use them.
Encoding/decoding algorithms and optional serde/no_std support are unchanged.

## transfer-engine

Directory: [vendor/transfer-engine](vendor/transfer-engine/).

Source: UR-SillyB/AirFerry, revision 8a72ab86dee9d8f19b74bfec56c270101c5980ba, MIT.

Native-only packaging: retain the protocol/session/sender/receiver/assembler sources unchanged.
Replace the workspace-inherited manifest with explicit native dependencies and emit an rlib.
Omit the optional WASM/JNI/C-ABI binding modules and their dependencies. Their legacy exact
wasm-bindgen/js-sys version pins conflict with One-KVM's HTTP stack during Cargo resolution,
even for native builds. The wire protocol and recovery algorithms are unchanged.

Receiver/development split: expose SenderSession and SenderConfig only with the
`sender` feature (also available to this crate's own tests). The core regression
suite enables it as a dev-dependency. The development CLI has been removed.
Remove the unused ingest_status packing module after deleting its binding callers,
and remove the unused direct crc32fast dependency. CRC remains in qr-protocol.
Protocol parsing, descriptor validation and recovery algorithms are unchanged.

## qr-protocol

Directory: [vendor/qr-protocol](vendor/qr-protocol/).

Source: UR-SillyB/AirFerry revision 8a72ab86dee9d8f19b74bfec56c270101c5980ba, MIT.

Use an explicit native manifest instead of inherited workspace metadata.
Remove the unused qr_render module and its fast_qr dependency after deleting
the development CLI. Frame layout, CRC, compression and session code are
unchanged. Default builds retain all three supported decompression formats.
