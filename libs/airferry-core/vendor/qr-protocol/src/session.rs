//! Deterministic session identifiers.
//!
//! A session id is derived from stable file identity (name + size + mtime +
//! content fingerprint prefix) so that a *re-send of the same file* yields the
//! *same* session id. This is what enables the receiver to resume after a
//! restart: when it sees the session id again it can reattach to the persisted
//! partial state instead of starting over.

use crate::frame::SessionIdRaw;
use std::vec::Vec;

/// A 128-bit session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub SessionIdRaw);

impl SessionId {
    pub fn zero() -> Self {
        Self(0)
    }

    /// Build a deterministic session id from file identity fields and a small
    /// content fingerprint (e.g. first/last 1 KiB hash). Same inputs → same id.
    pub fn derive(name: &str, size: u64, mtime_ms: u64, fingerprint: &[u8]) -> Self {
        // FNV-1a 128-bit over the identity material. Fast, dependency-free,
        // and more than collision-resistant enough for this use case.
        let mut h: u128 = 0x6c62272e07bb01426b82175983ad0b58; // FNV offset basis (128)
        const PRIME: u128 = 0x0000000001000000000000000000013b; // FNV prime (128)

        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u128;
                h = h.wrapping_mul(PRIME);
            }
        };
        feed(name.as_bytes());
        feed(&size.to_le_bytes());
        feed(&mtime_ms.to_le_bytes());
        feed(fingerprint);
        SessionId(h)
    }

    /// Derive the stable child session id for one independently encoded large-
    /// transfer segment.
    ///
    /// The outer QR frame format deliberately stays at protocol version 1. A
    /// segment is therefore demultiplexed by a distinct `session_id`, while its
    /// descriptor carries the root transfer id and segment coordinates. Keep
    /// the domain tag, root byte order, and index byte order frozen: browser,
    /// Android, Windows, and Rust tests all mirror this exact derivation.
    pub fn derive_segment(root: SessionIdRaw, segment_index: u32) -> Self {
        let mut h: u128 = 0x6c62272e07bb01426b82175983ad0b58;
        const PRIME: u128 = 0x0000000001000000000000000000013b;

        for &b in b"AirFerry.segment.v1"
            .iter()
            .chain(root.to_be_bytes().iter())
            .chain(segment_index.to_be_bytes().iter())
        {
            h ^= b as u128;
            h = h.wrapping_mul(PRIME);
        }
        SessionId(h)
    }
}

impl From<SessionId> for SessionIdRaw {
    fn from(s: SessionId) -> Self {
        s.0
    }
}

/// Compute a small content fingerprint: FNV-1a (64-bit) over the concatenation
/// of a head and tail slice of the file. Covers beginning + end so truncation
/// or appending is detected without hashing the whole file.
pub fn content_fingerprint(head: &[u8], tail: &[u8]) -> Vec<u8> {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a 64 offset basis
    const PRIME: u64 = 0x100000001b3;
    for buf in [head, tail] {
        for &b in buf {
            h ^= b as u64;
            h = h.wrapping_mul(PRIME);
        }
    }
    h.to_le_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a = SessionId::derive("file.bin", 1000, 555, &[1, 2, 3]);
        let b = SessionId::derive("file.bin", 1000, 555, &[1, 2, 3]);
        assert_eq!(a, b);
    }

    #[test]
    fn differs_on_size() {
        let a = SessionId::derive("file.bin", 1000, 555, &[1, 2, 3]);
        let b = SessionId::derive("file.bin", 1001, 555, &[1, 2, 3]);
        assert_ne!(a, b);
    }

    #[test]
    fn fingerprint_stable() {
        let a = content_fingerprint(&[1, 2, 3], &[9, 9]);
        let b = content_fingerprint(&[1, 2, 3], &[9, 9]);
        assert_eq!(a, b);
        let c = content_fingerprint(&[1, 2, 4], &[9, 9]);
        assert_ne!(a, c);
    }

    #[test]
    fn segment_ids_are_deterministic_and_domain_separated() {
        let root = 0x00112233445566778899aabbccddeeffu128;
        let a = SessionId::derive_segment(root, 7);
        assert_eq!(a, SessionId::derive_segment(root, 7));
        assert_ne!(a, SessionId::derive_segment(root, 8));
        assert_ne!(a.0, root);
    }
}
