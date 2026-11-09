//! Resume / checkpoint state.
//!
//! The receiver can serialize its progress so that, after an app restart, it
//! can recognise the same session id (from incoming frames). RaptorQ state
//! itself is rebuilt only by feeding saved symbol payloads into a fresh decoder;
//! an ESI without its payload is not replayable and must not suppress a future
//! retransmission. See
//! [`crate::ReceiverSession::save_state`] / [`crate::ReceiverSession::restore`].
//!
//! Format: serde JSON (feature-gated). When the `serde` feature is off, the
//! structs still exist but `[de]serialize` are unavailable.

use qr_protocol::frame::SessionIdRaw;
use raptorq_core::{ObjectMeta, MAX_OBJECT_BYTES, MAX_TOTAL_SOURCE_SYMBOLS};
use std::collections::HashSet;
use std::vec::Vec;

/// Persistable receiver state.
#[derive(Debug, Clone)]
pub struct ResumeState {
    pub session_id: SessionIdRaw,
    pub meta: ObjectMeta,
    /// Per-block set of ESIs with replayable payloads. Older checkpoints may
    /// contain additional entries; restore treats those as advisory and ignores
    /// any ESI that has no matching item in `symbols`.
    pub received: Vec<HashSet<u32>>,
    /// Stored symbol bytes, keyed by flat index = sbn*K_max + esi (simple).
    pub symbols: Vec<(u32, u32, Vec<u8>)>,
}

/// Hard pre-deserialization ceiling. `Vec<u8>` is represented as JSON numbers,
/// so a valid checkpoint is larger than its binary symbols; this is still low
/// enough to reject an accidentally or maliciously unbounded input before
/// serde starts allocating nested vectors.
#[cfg(feature = "serde")]
const MAX_RESUME_JSON_BYTES: usize = 128 * 1024 * 1024;

impl ResumeState {
    /// Validate a checkpoint independently of its serialization format.
    /// `restore` calls this too, so programmatically-created states cannot bypass
    /// the same memory/coordinate limits applied to JSON.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.meta.validate()?;
        if self.received.len() > self.meta.blocks.len() {
            return Err("too many received block sets");
        }

        let max_symbol_records = MAX_TOTAL_SOURCE_SYMBOLS
            .checked_mul(2)
            .ok_or("resume symbol-count budget overflow")?;
        let mut received_count = 0u64;
        for set in &self.received {
            received_count = received_count
                .checked_add(set.len() as u64)
                .ok_or("resume received count overflow")?;
            if received_count > max_symbol_records || set.iter().any(|&esi| esi >= (1 << 24)) {
                return Err("received ESI set exceeds resume budget");
            }
        }

        if self.symbols.len() as u64 > max_symbol_records {
            return Err("too many stored resume symbols");
        }
        let padding = u64::from(self.meta.symbol_size)
            .checked_mul(self.meta.blocks.len() as u64)
            .ok_or("resume padding budget overflow")?;
        let max_symbol_bytes = MAX_OBJECT_BYTES
            .checked_mul(2)
            .and_then(|v| v.checked_add(padding))
            .ok_or("resume byte budget overflow")?;
        let mut symbol_bytes = 0u64;
        for (sbn, esi, data) in &self.symbols {
            if (*sbn as usize) >= self.meta.blocks.len() || *esi >= (1 << 24) {
                return Err("stored resume symbol coordinate out of range");
            }
            if data.len() != self.meta.symbol_size as usize {
                return Err("stored resume symbol has wrong size");
            }
            symbol_bytes = symbol_bytes
                .checked_add(data.len() as u64)
                .ok_or("resume symbol byte count overflow")?;
            if symbol_bytes > max_symbol_bytes {
                return Err("stored resume symbols exceed byte budget");
            }
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashSet;
    use std::io::{self, Write};

    struct CappedWriter {
        bytes: Vec<u8>,
    }

    impl Write for CappedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let next_len = self.bytes.len().checked_add(buf.len()).ok_or_else(|| {
                io::Error::new(io::ErrorKind::OutOfMemory, "resume JSON size overflow")
            })?;
            if next_len > MAX_RESUME_JSON_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "resume JSON exceeds local byte budget",
                ));
            }
            self.bytes.try_reserve(buf.len()).map_err(|_| {
                io::Error::new(io::ErrorKind::OutOfMemory, "cannot allocate resume JSON")
            })?;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[derive(Deserialize)]
    struct SerResume {
        session_id: SessionIdRaw,
        meta: ObjectMeta,
        received: Vec<Vec<u32>>,
        symbols: Vec<(u32, u32, Vec<u8>)>,
    }

    #[derive(Serialize)]
    struct SerResumeRef<'a> {
        session_id: SessionIdRaw,
        meta: &'a ObjectMeta,
        received: Vec<Vec<u32>>,
        symbols: &'a [(u32, u32, Vec<u8>)],
    }

    impl ResumeState {
        pub fn to_json(&self) -> serde_json::Result<String> {
            self.validate()
                .map_err(<serde_json::Error as serde::ser::Error>::custom)?;
            let ser = SerResumeRef {
                session_id: self.session_id,
                meta: &self.meta,
                received: self
                    .received
                    .iter()
                    .map(|s| s.iter().copied().collect())
                    .collect(),
                symbols: &self.symbols,
            };
            let mut writer = CappedWriter { bytes: Vec::new() };
            serde_json::to_writer(&mut writer, &ser)?;
            String::from_utf8(writer.bytes).map_err(|error| {
                <serde_json::Error as serde::ser::Error>::custom(error.to_string())
            })
        }

        pub fn from_json(s: &str) -> serde_json::Result<Self> {
            if s.len() > MAX_RESUME_JSON_BYTES {
                return Err(<serde_json::Error as serde::de::Error>::custom(
                    "resume JSON exceeds local byte budget",
                ));
            }
            let ser: SerResume = serde_json::from_str(s)?;
            let received = ser
                .received
                .into_iter()
                .map(|v| v.into_iter().collect::<HashSet<u32>>())
                .collect();
            let state = Self {
                session_id: ser.session_id,
                meta: ser.meta,
                received,
                symbols: ser.symbols,
            };
            state
                .validate()
                .map_err(<serde_json::Error as serde::de::Error>::custom)?;
            Ok(state)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use raptorq_core::{Config, Encoder};

    fn valid_state() -> ResumeState {
        let data = vec![7u8; 1024];
        let meta = Encoder::new(&data, Config::default())
            .unwrap()
            .meta()
            .clone();
        ResumeState {
            session_id: 1,
            received: vec![HashSet::new(); meta.blocks.len()],
            meta,
            symbols: Vec::new(),
        }
    }

    #[test]
    fn resume_validation_rejects_bad_coordinates_and_sizes() {
        let mut state = valid_state();
        state.symbols.push((0, 0, vec![0; 3]));
        assert!(state.validate().is_err());

        let mut state = valid_state();
        state.received[0].insert(1 << 24);
        assert!(state.validate().is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_rejects_invalid_resume_before_restore() {
        let mut state = valid_state();
        state.symbols.push((99, 0, vec![0; 1024]));
        assert!(state.to_json().is_err());

        let mut state = valid_state();
        state.symbols.push((0, 0, vec![0; 1024]));
        let mut value: serde_json::Value = serde_json::from_str(&state.to_json().unwrap()).unwrap();
        value["symbols"][0][0] = serde_json::Value::from(99);
        let json = serde_json::to_string(&value).unwrap();
        assert!(ResumeState::from_json(&json).is_err());
    }
}
