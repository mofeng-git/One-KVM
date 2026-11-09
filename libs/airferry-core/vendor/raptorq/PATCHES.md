Source: crates.io raptorq 2.0.1, Apache-2.0 (see LICENSE).

ARM32 stable Rust fix: gate the five NEON implementation functions in
`src/octets.rs` on aarch64 only. Upstream already disables all four ARM32
NEON dispatch calls, but still compiles the implementations, which refer to
unstable ARM32 intrinsics, AArch64-only table instructions and an unavailable
`uint8x16_t` type. ARMv7 uses the existing scalar fallback. x86 and aarch64
dispatch and the protocol remain unchanged. The registry cache is not modified.
