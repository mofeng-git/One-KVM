//! Sender-side session orchestration.

use crate::descriptor::FileMeta;
use crate::segment::SegmentMeta;
use crate::time::now_ms;
use crate::{progress::Stats, Error, Result};
use qr_protocol::{chunker, frame::SessionIdRaw, Frame, SessionId};
use raptorq_core::{Config, Encoder, ObjectMeta, Symbol};
use std::vec::Vec;

/// Configuration for a sender session.
#[derive(Debug, Clone, Copy)]
pub struct SenderConfig {
    /// Codec configuration (symbol size).
    pub codec: Config,
    /// Redundancy ratio in percent (5..=50). **Retained for API/UI
    /// compatibility and frame-count estimation only** — the live stream now
    /// emits the K source symbols once and then a long-lived stream of fresh
    /// repair symbols (see [`build_source_plan`] + [`SenderSession::next_symbol_id`]),
    /// so it no longer bounds how many repair symbols are transmitted.
    pub redundancy_pct: u8,
}

impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            codec: Config::default(),
            // 25% is a sensible default for the UI frame-count estimate under
            // real camera-scan conditions (the receiver commonly reports 30–50%
            // frame loss). NOTE: with the fresh-repair emitter this no
            // longer caps the stream — the sender keeps producing new repair
            // symbols until the receiver completes — it only drives the on-screen
            // time/throughput estimate.
            redundancy_pct: 25,
        }
    }
}

impl SenderConfig {
    pub fn validate(&self) -> Result<()> {
        // The protocol specification allows 5..=50% redundancy for optimal
        // performance. Values outside this range may work but are not recommended:
        // - Too low (<5%): insufficient error correction for lossy channels
        // - Too high (>50%): diminishing returns, wastes bandwidth
        if !(5..=50).contains(&self.redundancy_pct) {
            return Err(Error::InvalidRedundancy(self.redundancy_pct));
        }
        Ok(())
    }
}

/// A sender session.
///
/// Holds the (already Zstd-compressed) payload and a RaptorQ encoder over it,
/// plus a deterministic session id. Call [`SenderSession::next_frame`] in a
/// render loop to drive the QR video stream; it loops forever across passes,
/// so a receiver can rejoin at any point.
pub struct SenderSession {
    config: SenderConfig,
    session_id: SessionIdRaw,
    encoder: Encoder,
    meta: ObjectMeta,
    file_meta: FileMeta,
    /// Descriptor-v5 metadata for a large-transfer child object.
    segment_meta: Option<SegmentMeta>,
    /// Total source symbols K across all blocks.
    total_k: u32,
    /// Source-symbol emission order (round-robin across blocks), emitted once.
    source_plan: Vec<(u32, u32)>,
    /// Cursor into `source_plan` during the initial source pass.
    source_cursor: usize,
    /// Monotonic counter driving the fresh-repair phase. It
    /// round-robins across blocks (`% nblocks`) with a per-block repair offset
    /// that only ever increases (`/ nblocks`), so every post-source data frame
    /// carries a brand-new symbol the receiver has never seen. This eliminates
    /// the duplicate frames the old looping plan produced, so recovery progress
    /// stays ~linear instead of stalling on coupon-collector repeats near the end.
    repair_cursor: u64,
    frame_index: u64,
    descriptor_interval: u32,
    /// Whether `start_ms` has been set. Kept separate from `start_ms` so a
    /// wall-clock of 0 (impossible in practice but defensive) is handled.
    started: bool,
    /// Timestamp of the first emitted frame; 0 until `started` becomes true.
    /// Delayed so that the cumulative-average FPS (frames / elapsed) is not
    /// diluted by the WASM init gap between `new()` and the first render tick.
    start_ms: u64,
    stats: Stats,
}

impl SenderSession {
    /// Create a session from payload bytes + file metadata.
    pub fn new(
        compressed_payload: &[u8],
        session_id: SessionId,
        config: SenderConfig,
        file_meta: FileMeta,
    ) -> Result<Self> {
        Self::new_inner(compressed_payload, session_id, config, file_meta, None)
    }

    /// Create one independently encoded descriptor-v5 large-transfer segment.
    pub fn new_segment(
        compressed_payload: &[u8],
        child_session_id: SessionId,
        config: SenderConfig,
        file_meta: FileMeta,
        segment_meta: SegmentMeta,
    ) -> Result<Self> {
        if file_meta.compressed_size != compressed_payload.len() as u64
            || !file_meta.compressed_size_known
        {
            return Err(Error::InvalidSegment(
                "segment compressed size does not match payload",
            ));
        }
        segment_meta
            .validate(child_session_id.0, &file_meta)
            .map_err(Error::InvalidSegment)?;
        Self::new_inner(
            compressed_payload,
            child_session_id,
            config,
            file_meta,
            Some(segment_meta),
        )
    }

    fn new_inner(
        compressed_payload: &[u8],
        session_id: SessionId,
        config: SenderConfig,
        file_meta: FileMeta,
        segment_meta: Option<SegmentMeta>,
    ) -> Result<Self> {
        config.validate()?;
        let padded = chunker::pad_to_symbols(compressed_payload, config.codec);
        let encoder = Encoder::new(&padded, config.codec)?;
        let meta = encoder.meta().clone();
        // Session construction must guarantee that its mandatory first frame
        // can actually be emitted. Small symbols or unusually many source
        // blocks may be valid RaptorQ configurations while being too small for
        // the current v3 descriptor.
        if let Some(segment) = &segment_meta {
            crate::descriptor::build_segment_payload(&meta, &file_meta, segment)?;
        } else {
            crate::descriptor::build_payload(&meta, &file_meta)?;
        }
        let total_k: u32 = meta.blocks.iter().map(|b| b.num_source_symbols).sum();

        let source_plan = build_source_plan(&meta);

        Ok(Self {
            config,
            session_id: session_id.into(),
            encoder,
            meta,
            file_meta,
            segment_meta,
            total_k,
            source_plan,
            source_cursor: 0,
            repair_cursor: 0,
            frame_index: 0,
            // Descriptor every ~17 frames (~0.6s at 30fps). 17 is deliberately
            // coprime with the UI's 2/4-code layouts: descriptors rotate through
            // every physical QR slot instead of always landing in slot 0 (the
            // old interval 16 did exactly that). A camera that persistently
            // misses one screen corner can therefore confirm metadata from a
            // different slot instead of caching data forever at "同步中".
            // Overhead remains ~6%, versus ~20% with the old interval of 5.
            descriptor_interval: 17,
            started: false,
            start_ms: 0, // Set on first next_frame() call
            stats: Stats::default(),
        })
    }

    /// How often (in frames) a descriptor frame is emitted. Default 17 so the
    /// descriptor rotates across 2/4-code screen slots.
    pub fn set_descriptor_interval(&mut self, interval: u32) {
        self.descriptor_interval = interval.max(1);
    }

    /// Object metadata (transfer length, OTI, per-block K).
    pub fn meta(&self) -> &ObjectMeta {
        &self.meta
    }

    /// File metadata (filename, original size, CRC32).
    pub fn file_meta(&self) -> &FileMeta {
        &self.file_meta
    }

    pub fn segment_meta(&self) -> Option<&SegmentMeta> {
        self.segment_meta.as_ref()
    }

    pub fn session_id(&self) -> SessionIdRaw {
        self.session_id
    }

    pub fn total_k(&self) -> u32 {
        self.total_k
    }

    pub fn num_blocks(&self) -> usize {
        self.meta.blocks.len()
    }

    /// Emit the next frame.
    ///
    /// Cadence: a *descriptor frame* every `descriptor_interval` emitted frames
    /// (and always as the very first frame) so late-joining receivers can build
    /// a decoder. Data frames emit the K source symbols once (round-robin across
    /// blocks) and then fresh repair symbols up to the 24-bit ESI limit — every
    /// post-source frame is a new symbol, so the receiver never receives a
    /// sender-induced duplicate and recovery progress stays ~linear.
    pub fn next_frame(&mut self) -> Result<Frame> {
        if self.source_plan.is_empty() {
            return Err(Error::NoPayload);
        }

        // Start the clock on the very first frame so that the cumulative FPS
        // isn't diluted by the initialisation delay between new() and the
        // first render-loop tick.
        if !self.started {
            self.start_ms = now_ms();
            self.started = true;
        }

        let now_ms_val = now_ms();

        // Emit a descriptor every `descriptor_interval` frames (counted in
        // emitted frames of any kind, so the cadence stays regular). The very
        // first emitted frame is always a descriptor so a receiver that joins
        // immediately can build its decoder without having to cache symbols.
        let interval = self.descriptor_interval.max(1) as u64;
        if self.frame_index == 0 || (self.frame_index > 0 && self.frame_index % interval == 0) {
            self.frame_index = self.frame_index.wrapping_add(1);
            let frame = if let Some(segment) = &self.segment_meta {
                crate::descriptor::build_segment_frame(
                    &self.meta,
                    &self.file_meta,
                    segment,
                    self.session_id,
                    self.frame_index,
                    now_ms_val,
                )?
            } else {
                crate::descriptor::build_frame(
                    &self.meta,
                    &self.file_meta,
                    self.session_id,
                    self.frame_index,
                    now_ms_val,
                )?
            };
            self.stats.record_sent(frame.payload.len() as u64);
            return Ok(frame);
        }

        let (sbn, esi) = self.next_symbol_id()?;

        let symbol = self.fetch_symbol(sbn, esi)?;
        self.frame_index = self.frame_index.wrapping_add(1);

        self.stats.record_sent(symbol.data.len() as u64);

        Ok(Frame::build(
            self.session_id,
            0,
            sbn,
            esi,
            self.meta.blocks.len() as u32,
            self.total_k,
            self.config.codec.symbol_size(),
            self.frame_index,
            now_ms_val,
            &symbol.data,
        ))
    }

    fn fetch_symbol(&self, sbn: u32, esi: u32) -> Result<Symbol> {
        let k = self.meta.blocks[sbn as usize].num_source_symbols;
        if esi < k {
            self.encoder.source_symbol(sbn, esi).map_err(Into::into)
        } else {
            // Repair symbol: esi offset within the repair namespace.
            let repair_offset = esi - k;
            let mut syms = self.encoder.repair_symbols(sbn, repair_offset, 1)?;
            Ok(syms.remove(0))
        }
    }

    /// Pick the next data symbol's `(sbn, esi)`.
    ///
    /// Phase 1 — source pass: walk `source_plan` once, emitting each of the K
    /// source symbols a single time (round-robin across blocks).
    ///
    /// Phase 2 — fresh repair: round-robin across blocks via
    /// `repair_cursor % nblocks`, with a per-block repair offset
    /// `repair_cursor / nblocks` that increases forever and never wraps back to
    /// an already-sent ESI. Each returned id is therefore unique for the whole
    /// session, so the receiver never sees a sender-induced duplicate — the fix
    /// for the "progress crawls near the end" coupon-collector problem.
    fn next_symbol_id(&mut self) -> Result<(u32, u32)> {
        if self.source_cursor < self.source_plan.len() {
            let id = self.source_plan[self.source_cursor];
            self.source_cursor += 1;
            return Ok(id);
        }
        // `source_plan` is non-empty (guarded in `next_frame`), so there is at
        // least one block.
        let id = repair_symbol_id(&self.meta.blocks, self.repair_cursor)
            .ok_or(Error::SymbolIdSpaceExhausted)?;
        self.repair_cursor = self
            .repair_cursor
            .checked_add(1)
            .ok_or(Error::SymbolIdSpaceExhausted)?;
        Ok(id)
    }

    /// Live statistics snapshot (sent frames, fps, throughput, elapsed).
    pub fn stats(&self) -> Stats {
        let mut s = self.stats.clone();
        if self.started {
            s.elapsed_ms = now_ms().saturating_sub(self.start_ms);
        }
        s.finalize();
        s
    }
}

/// Build the source-symbol emission order: every source symbol once,
/// **round-robin across blocks** (esi 0 of every block, then esi 1 of every
/// block, …).
///
/// Round-robin (esi-major) interleaving is more robust to bursty loss than
/// block-major order: a burst of dropped frames is spread thinly across all
/// blocks instead of stranding a single block, so every block advances roughly
/// in lockstep toward its decode threshold. Repair symbols are no longer part of
/// a precomputed plan — they are generated on demand up to the ESI limit by
/// [`SenderSession::next_symbol_id`].
fn build_source_plan(meta: &ObjectMeta) -> Vec<(u32, u32)> {
    let total_k: usize = meta
        .blocks
        .iter()
        .map(|b| b.num_source_symbols as usize)
        .sum();
    let mut plan: Vec<(u32, u32)> = Vec::with_capacity(total_k);
    let max_k = meta
        .blocks
        .iter()
        .map(|b| b.num_source_symbols)
        .max()
        .unwrap_or(0);
    for esi in 0..max_k {
        for b in &meta.blocks {
            if esi < b.num_source_symbols {
                plan.push((b.sbn, esi));
            }
        }
    }
    plan
}

/// Compute the `(sbn, esi)` of the `cursor`-th repair symbol in the bounded
/// fresh-repair phase. Round-robins across blocks (`cursor % nblocks`) with a
/// per-block offset that only ever increases (`cursor / nblocks`), so the
/// returned id is unique for every distinct `cursor` and the ESI is always ≥ the
/// block's K (i.e. a repair, never a source, symbol). Callers guarantee `blocks`
/// is non-empty.
fn repair_symbol_id(blocks: &[raptorq_core::SourceBlockMeta], cursor: u64) -> Option<(u32, u32)> {
    let nblocks = blocks.len() as u64;
    let bi = (cursor % nblocks) as usize;
    let offset = u32::try_from(cursor / nblocks).ok()?;
    let b = &blocks[bi];
    let esi = b.num_source_symbols.checked_add(offset)?;
    (esi < (1 << 24)).then_some((b.sbn, esi))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i & 0xff) as u8).collect()
    }

    fn test_file_meta() -> FileMeta {
        FileMeta {
            filename: "test.bin".to_string(),
            original_size: 0,
            crc32: 0,
            compression: qr_protocol::compress::COMPRESSION_NONE,
            compressed_size: 0,
            compressed_size_known: true,
            crc32_known: false,
        }
    }

    #[test]
    fn produces_frames_in_sequence() {
        let data = payload(50_000);
        let mut s = SenderSession::new(
            &data,
            SessionId::derive("f", 50_000, 0, &[]),
            SenderConfig::default(),
            test_file_meta(),
        )
        .unwrap();
        let f0 = s.next_frame().unwrap();
        assert_eq!(f0.header.session_id, s.session_id());
        assert_eq!(f0.payload.len(), 1024);
        assert_eq!((f0.header.sbn, f0.header.esi), (0, 0));
        let f1 = s.next_frame().unwrap();
        assert_eq!(f1.header.frame_index, 2);
    }

    #[test]
    fn rejects_session_when_v3_descriptor_does_not_fit_symbol() {
        let data = payload(64);
        let result = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig {
                codec: Config::new(64).unwrap(),
                redundancy_pct: 25,
            },
            test_file_meta(),
        );
        assert!(matches!(
            result,
            Err(Error::Protocol(qr_protocol::Error::BufferTooShort { .. }))
        ));
    }

    #[test]
    fn emits_frames_indefinitely() {
        let data = payload(2048);
        let mut s = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            test_file_meta(),
        )
        .unwrap();
        // Drive well past the one-shot source pass into the fresh-repair
        // phase; next_frame must keep producing frames without panicking.
        let n = s.source_plan.len() * 3 + 500;
        for _ in 0..n {
            let _ = s.next_frame().unwrap();
        }
    }

    /// Core property of the fix: after the one-shot source pass, every data
    /// frame carries a brand-new `(sbn, esi)`. Across a long run there must be
    /// zero duplicate data-symbol ids (descriptor frames excluded). This is what
    /// removes the coupon-collector tail that made progress crawl near the end.
    #[test]
    fn no_duplicate_data_frames() {
        let data = payload(20_000);
        let mut s = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            test_file_meta(),
        )
        .unwrap();
        let total_k = s.total_k();
        let mut seen = std::collections::HashSet::new();
        let mut repair_seen = false;
        // Run well past the source pass into the repair phase.
        let n = (total_k as usize) * 5 + 500;
        for _ in 0..n {
            let f = s.next_frame().unwrap();
            if f.header.flags & qr_protocol::frame::FLAG_DESCRIPTOR != 0 {
                continue;
            }
            let k = s.meta.blocks[f.header.sbn as usize].num_source_symbols;
            if f.header.esi >= k {
                repair_seen = true;
            }
            let id = (f.header.sbn, f.header.esi);
            assert!(seen.insert(id), "duplicate data frame id emitted: {id:?}");
        }
        assert!(
            repair_seen,
            "repair symbols (esi >= K) must be emitted after the source pass"
        );
    }

    /// Source symbols must be interleaved round-robin across blocks (esi-major
    /// order), not emitted block-major. This is the burst-loss optimisation:
    /// with the old block-major order a single burst of dropped frames could
    /// starve one block while others were already complete. Round-robin spreads
    /// each burst thinly so every block advances together.
    #[test]
    fn source_symbols_interleaved_across_blocks() {
        use raptorq_core::{ObjectMeta, SourceBlockMeta};
        // Hand-build a 3-block object so the test does not depend on the
        // raptorq library's (length-dependent) block-partitioning heuristic,
        // which only produces multiple blocks for very large payloads.
        let meta = ObjectMeta {
            transfer_length: 3 * 10 * 1024,
            symbol_size: 1024,
            oti_bytes: [0u8; 12],
            blocks: vec![
                SourceBlockMeta {
                    sbn: 0,
                    num_source_symbols: 10,
                    block_length: 10 * 1024,
                },
                SourceBlockMeta {
                    sbn: 1,
                    num_source_symbols: 10,
                    block_length: 10 * 1024,
                },
                SourceBlockMeta {
                    sbn: 2,
                    num_source_symbols: 10,
                    block_length: 10 * 1024,
                },
            ],
        };
        let plan = build_source_plan(&meta);

        // Every entry is a source symbol (esi < K_block) by construction.
        let source_plan: Vec<(u32, u32)> = plan;

        // Round-robin: first 3 symbols must be esi=0 of each of the 3 blocks.
        let first_round = &source_plan[..3];
        let sbns: std::collections::HashSet<u32> =
            first_round.iter().map(|(sbn, _)| *sbn).collect();
        assert_eq!(
            sbns.len(),
            3,
            "first round should cover all 3 blocks once, got {:?}",
            first_round
        );
        assert!(
            first_round.iter().all(|(_, esi)| *esi == 0),
            "first round should all be esi=0"
        );

        // The old block-major order would emit sbn=0 ten times in a row.
        // Round-robin must NOT — consecutive entries must differ in sbn.
        let consecutive_same = first_round.windows(2).all(|w| w[0].0 == w[1].0);
        assert!(
            !consecutive_same,
            "plan is block-major, not interleaved: {:?}",
            first_round
        );
    }

    /// The fresh-repair phase, exercised directly on a multi-block
    /// object with **unequal** per-block K. Real payloads stay single-block until
    /// ~56k symbols (RaptorQ partitioning), so the cross-block round-robin in
    /// `repair_symbol_id` is otherwise never exercised by the end-to-end tests.
    /// Every repair id must be unique, must be a repair symbol (esi ≥ K), and the
    /// three blocks must be served at an equal rate (round-robin).
    #[test]
    fn repair_round_robin_multiblock_unique_and_fair() {
        use raptorq_core::SourceBlockMeta;
        let blocks = vec![
            SourceBlockMeta {
                sbn: 0,
                num_source_symbols: 5,
                block_length: 5 * 1024,
            },
            SourceBlockMeta {
                sbn: 1,
                num_source_symbols: 8,
                block_length: 8 * 1024,
            },
            SourceBlockMeta {
                sbn: 2,
                num_source_symbols: 3,
                block_length: 3 * 1024,
            },
        ];
        let mut seen = std::collections::HashSet::new();
        let mut per_block = [0u32; 3];
        for cursor in 0..3000u64 {
            let (sbn, esi) = repair_symbol_id(&blocks, cursor).unwrap();
            let k = blocks[sbn as usize].num_source_symbols;
            assert!(
                esi >= k,
                "esi {esi} below K {k} for block {sbn} (must be a repair symbol)"
            );
            assert!(seen.insert((sbn, esi)), "duplicate repair id ({sbn},{esi})");
            per_block[sbn as usize] += 1;
        }
        // 3000 / 3 = 1000 each; round-robin must serve every block equally.
        assert_eq!(
            per_block,
            [1000, 1000, 1000],
            "repair not round-robin: {per_block:?}"
        );
    }

    #[test]
    fn repair_id_stops_at_rfc_24_bit_boundary() {
        use raptorq_core::SourceBlockMeta;
        let blocks = vec![SourceBlockMeta {
            sbn: 0,
            num_source_symbols: 5,
            block_length: 5 * 1024,
        }];
        assert!(repair_symbol_id(&blocks, (1 << 24) - 5 - 1).is_some());
        assert!(repair_symbol_id(&blocks, (1 << 24) - 5).is_none());
    }

    /// The default descriptor interval should be high enough that descriptors
    /// are a small fraction of the wire (each descriptor is pure overhead).
    #[test]
    fn default_descriptor_interval_is_efficient() {
        let data = payload(50_000);
        let s = SenderSession::new(
            &data,
            SessionId::zero(),
            SenderConfig::default(),
            test_file_meta(),
        )
        .unwrap();
        // <=20% of the wire for descriptors means interval >= 5; we default to
        // 17 (~6%), so this guards against accidentally dropping it back to the
        // old wasteful 5.
        assert!(
            s.descriptor_interval >= 16,
            "descriptor interval too low: {}",
            s.descriptor_interval
        );
    }

    /// Consecutive frames are laid out into fixed physical slots in multi-QR
    /// mode. The default descriptor cadence must visit every 2/4-code slot;
    /// otherwise a camera that consistently misses one corner never sees OTI.
    #[test]
    fn default_descriptor_rotates_across_multi_qr_slots() {
        let data = payload(50_000);
        for slots in [2usize, 4usize] {
            let mut sender = SenderSession::new(
                &data,
                SessionId::zero(),
                SenderConfig::default(),
                test_file_meta(),
            )
            .unwrap();
            let mut seen = vec![false; slots];
            for frame_index in 0..(sender.descriptor_interval as usize * slots) {
                let frame = sender.next_frame().unwrap();
                if frame.header.flags & qr_protocol::frame::FLAG_DESCRIPTOR != 0 {
                    seen[frame_index % slots] = true;
                }
            }
            assert!(
                seen.iter().all(|&slot| slot),
                "descriptor did not visit every {slots}-QR slot: {seen:?}"
            );
        }
    }

    /// A receiver that never sees a single source symbol — only fresh repair
    /// symbols — must still recover the object (RaptorQ fountain property). This
    /// guards the new "source once, then fresh repair" emitter: a receiver
    /// that joins after the source pass relies entirely on repair symbols.
    #[test]
    fn repair_only_recovery() {
        use crate::receiver::ReceiverSession;
        let data = payload(40_000);
        let sid = SessionId::derive("repair", data.len() as u64, 0, &[]);
        // Probe once to learn the padded transfer length, then rebuild with a
        // FileMeta whose compressed_size matches so assemble() trims correctly.
        let probe =
            SenderSession::new(&data, sid, SenderConfig::default(), test_file_meta()).unwrap();
        let meta = probe.meta().clone();
        let mut fm = test_file_meta();
        fm.compressed_size = meta.transfer_length;
        fm.compressed_size_known = true;
        let mut sender = SenderSession::new(&data, sid, SenderConfig::default(), fm).unwrap();

        let mut rx = ReceiverSession::new_confirmed(sid.into(), meta.clone()).unwrap();
        let mut guard = 0;
        while !rx.is_complete() {
            let frame = sender.next_frame().unwrap();
            guard += 1;
            assert!(guard < 200_000, "did not recover from a repair-only stream");
            // Feed ONLY repair symbols: drop descriptors and every source symbol.
            if frame.header.flags & qr_protocol::frame::FLAG_DESCRIPTOR != 0 {
                continue;
            }
            let k = meta.blocks[frame.header.sbn as usize].num_source_symbols;
            if frame.header.esi < k {
                continue; // skip source symbols entirely
            }
            let parsed = Frame::from_bytes(&frame.to_bytes()).unwrap();
            let _ = rx.ingest(parsed);
        }
        let out = rx.assemble().unwrap();
        assert_eq!(
            &out[..data.len()],
            &data[..],
            "repair-only recovery payload mismatch"
        );
    }
}
