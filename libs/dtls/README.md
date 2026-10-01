# DTLS certificate verification backend

This directory contains `dtls` 0.17.2 from the crates.io release of webrtc-rs.
The original MIT and Apache-2.0 licenses are preserved.

One-KVM patches only certificate-chain verification and its configuration to
use vendored OpenSSL instead of rustls/WebPKI. DTLS flights, record protection,
certificate signatures, SRTP negotiation and the application fingerprint check
remain unchanged. `rustls-pki-types` remains a certificate-data representation,
not a TLS implementation.

`certificate_verifier.rs` implements trust-anchor, purpose, certificate-validity
and DNS/IP identity checks. Server names do not fall back to the certificate
subject, and partial-label wildcards are rejected. No trust checks are skipped
by this backend. The upstream application-controlled `insecure_skip_verify`
policy is preserved, including WebRTC's separate certificate fingerprint check.
Authentication security level 2 rejects weak RSA keys and SHA-1 leaf signatures.

Run `cargo test --manifest-path libs/dtls/Cargo.toml --lib` after updating this
fork, and verify the application dependency tree contains no `rustls` engine.
