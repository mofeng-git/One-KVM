#[cfg(feature = "std")]
use std::{collections::HashSet as Set, iter, vec::Vec};

#[cfg(not(feature = "std"))]
use core::iter;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet as Set, vec::Vec};

use crate::base::EncodingPacket;
use crate::base::ObjectTransmissionInformation;
use crate::base::intermediate_tuple;
use crate::base::partition;
use crate::constraint_matrix::enc_indices;
use crate::constraint_matrix::generate_constraint_matrix;
use crate::constraint_matrix::generate_constraint_matrix_no_hdpc;
use crate::encoder::SPARSE_MATRIX_THRESHOLD;
use crate::matrix::{BinaryMatrix, DenseBinaryMatrix};
use crate::octet_matrix::DenseOctetMatrix;
use crate::octets::add_assign;
use crate::pi_solver::fused_inverse_mul_symbols;
use crate::pi_solver::fused_inverse_mul_symbols_no_hdpc;
use crate::sparse_matrix::SparseBinaryMatrix;
use crate::symbol::Symbol;
use crate::symbol_slab::SymbolSlab;
use crate::systematic_constants::num_hdpc_symbols;
use crate::systematic_constants::num_ldpc_symbols;
use crate::systematic_constants::{
    calculate_p1, extended_source_block_symbols, num_intermediate_symbols, num_lt_symbols,
    num_pi_symbols, systematic_index,
};
use crate::util::int_div_ceil;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Decoder {
    config: ObjectTransmissionInformation,
    block_decoders: Vec<SourceBlockDecoder>,
    blocks: Vec<Option<Vec<u8>>>,
}

impl Decoder {
    pub fn new(config: ObjectTransmissionInformation) -> Decoder {
        let kt = int_div_ceil(config.transfer_length(), config.symbol_size() as u64);

        let (kl, ks, zl, zs) = partition(kt, config.source_blocks());

        let mut decoders = vec![];
        for i in 0..zl {
            decoders.push(SourceBlockDecoder::new(
                i as u8,
                &config,
                u64::from(kl) * u64::from(config.symbol_size()),
            ));
        }

        for i in zl..(zl + zs) {
            decoders.push(SourceBlockDecoder::new(
                i as u8,
                &config,
                u64::from(ks) * u64::from(config.symbol_size()),
            ));
        }

        Decoder {
            config,
            block_decoders: decoders,
            blocks: vec![None; (zl + zs) as usize],
        }
    }

    #[cfg(all(any(test, feature = "benchmarking"), not(feature = "python")))]
    pub fn set_sparse_threshold(&mut self, value: u32) {
        for block_decoder in self.block_decoders.iter_mut() {
            block_decoder.set_sparse_threshold(value);
        }
    }

    pub fn decode(&mut self, packet: EncodingPacket) -> Option<Vec<u8>> {
        let block_number = packet.payload_id.source_block_number() as usize;
        if self.blocks[block_number].is_none() {
            self.blocks[block_number] =
                self.block_decoders[block_number].decode(iter::once(packet));
        }
        for block in self.blocks.iter() {
            if block.is_none() {
                return None;
            }
        }

        let mut result = vec![];
        for block in self.blocks.iter().flatten() {
            result.extend(block);
        }

        result.truncate(self.config.transfer_length() as usize);
        Some(result)
    }

    #[cfg(not(feature = "python"))]
    pub fn add_new_packet(&mut self, packet: EncodingPacket) {
        let block_number = packet.payload_id.source_block_number() as usize;
        if self.blocks[block_number].is_none() {
            self.blocks[block_number] =
                self.block_decoders[block_number].decode(iter::once(packet));
        }
    }

    #[cfg(not(feature = "python"))]
    pub fn get_result(&self) -> Option<Vec<u8>> {
        for block in self.blocks.iter() {
            if block.is_none() {
                return None;
            }
        }

        let mut result = vec![];
        for block in self.blocks.iter().flatten() {
            result.extend(block);
        }
        result.truncate(self.config.transfer_length() as usize);
        Some(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceBlockDecoder {
    source_block_id: u8,
    symbol_size: u16,
    num_sub_blocks: u16,
    symbol_alignment: u8,
    source_block_symbols: u32,
    source_symbols: Vec<Option<Symbol>>,
    repair_packets: Vec<EncodingPacket>,
    received_source_symbols: u32,
    received_esi: Set<u32>,
    decoded: bool,
    sparse_threshold: u32,
}

#[derive(Copy, Clone)]
struct EncodingParameters {
    lt_symbols: u32,
    pi_symbols: u32,
    sys_index: u32,
    p1: u32,
}
impl SourceBlockDecoder {
    pub fn new(
        source_block_id: u8,
        config: &ObjectTransmissionInformation,
        block_length: u64,
    ) -> SourceBlockDecoder {
        let source_symbols = int_div_ceil(block_length, config.symbol_size() as u64);

        SourceBlockDecoder {
            source_block_id,
            symbol_size: config.symbol_size(),
            num_sub_blocks: config.sub_blocks(),
            symbol_alignment: config.symbol_alignment(),
            source_block_symbols: source_symbols,
            source_symbols: vec![None; source_symbols as usize],
            repair_packets: vec![],
            received_source_symbols: 0,
            received_esi: Set::new(),
            decoded: false,
            sparse_threshold: SPARSE_MATRIX_THRESHOLD,
        }
    }

    #[cfg(any(test, feature = "benchmarking"))]
    pub fn set_sparse_threshold(&mut self, value: u32) {
        self.sparse_threshold = value;
    }

    fn unpack_sub_blocks(&self, result: &mut [u8], symbol: &[u8], symbol_index: usize) {
        let (tl, ts, nl, ns) = partition(
            (self.symbol_size / self.symbol_alignment as u16) as u32,
            self.num_sub_blocks,
        );

        let mut symbol_offset = 0;
        let mut sub_block_offset = 0;
        for sub_block in 0..(nl + ns) {
            let bytes = if sub_block < nl {
                tl as usize * self.symbol_alignment as usize
            } else {
                ts as usize * self.symbol_alignment as usize
            };
            let start = sub_block_offset + bytes * symbol_index;
            result[start..start + bytes]
                .copy_from_slice(&symbol[symbol_offset..symbol_offset + bytes]);
            symbol_offset += bytes;
            sub_block_offset += bytes * self.source_block_symbols as usize;
        }
    }

    fn try_pi_decode(
        &mut self,
        constraint_matrix: impl BinaryMatrix,
        hdpc_rows: DenseOctetMatrix,
        symbols: SymbolSlab,
    ) -> Option<Vec<u8>> {
        let intermediate_symbols = match fused_inverse_mul_symbols(
            constraint_matrix,
            hdpc_rows,
            symbols,
            self.source_block_symbols,
        ) {
            (None, _) => return None,
            (Some(s), _) => s,
        };

        let mut result = vec![0; self.symbol_size as usize * self.source_block_symbols as usize];
        let params = EncodingParameters {
            lt_symbols: num_lt_symbols(self.source_block_symbols),
            pi_symbols: num_pi_symbols(self.source_block_symbols),
            sys_index: systematic_index(self.source_block_symbols),
            p1: calculate_p1(self.source_block_symbols),
        };
        let ss = self.symbol_size as usize;
        let mut rebuilt_buf = vec![0u8; ss];
        for i in 0..self.source_block_symbols as usize {
            if let Some(ref symbol) = self.source_symbols[i] {
                self.unpack_sub_blocks(&mut result, symbol.as_bytes(), i);
            } else {
                self.rebuild_source_symbol_into(
                    &mut rebuilt_buf,
                    &intermediate_symbols,
                    i as u32,
                    params,
                );
                self.unpack_sub_blocks(&mut result, &rebuilt_buf, i);
            }
        }

        self.decoded = true;
        return Some(result);
    }

    /// Attempt to decode without HDPC rows (pure GF(2) solve).
    /// Returns None if the GF(2)-only system is rank-deficient.
    fn try_pi_decode_no_hdpc(
        &mut self,
        constraint_matrix: impl BinaryMatrix,
        symbols: SymbolSlab,
    ) -> Option<Vec<u8>> {
        let intermediate_symbols = match fused_inverse_mul_symbols_no_hdpc(
            constraint_matrix,
            symbols,
            self.source_block_symbols,
        ) {
            (None, _) => return None,
            (Some(s), _) => s,
        };

        let mut result = vec![0; self.symbol_size as usize * self.source_block_symbols as usize];
        let params = EncodingParameters {
            lt_symbols: num_lt_symbols(self.source_block_symbols),
            pi_symbols: num_pi_symbols(self.source_block_symbols),
            sys_index: systematic_index(self.source_block_symbols),
            p1: calculate_p1(self.source_block_symbols),
        };
        let mut rebuilt_buf = vec![0u8; self.symbol_size as usize];
        for i in 0..self.source_block_symbols as usize {
            if let Some(ref symbol) = self.source_symbols[i] {
                self.unpack_sub_blocks(&mut result, symbol.as_bytes(), i);
            } else {
                self.rebuild_source_symbol_into(
                    &mut rebuilt_buf,
                    &intermediate_symbols,
                    i as u32,
                    params,
                );
                self.unpack_sub_blocks(&mut result, &rebuilt_buf, i);
            }
        }

        self.decoded = true;
        Some(result)
    }

    pub fn decode<T: IntoIterator<Item = EncodingPacket>>(
        &mut self,
        packets: T,
    ) -> Option<Vec<u8>> {
        for packet in packets {
            assert_eq!(
                self.source_block_id,
                packet.payload_id.source_block_number()
            );

            let (payload_id, payload) = packet.split();
            if self.received_esi.insert(payload_id.encoding_symbol_id()) {
                if payload_id.encoding_symbol_id() >= self.source_block_symbols {
                    // Repair symbol
                    self.repair_packets
                        .push(EncodingPacket::new(payload_id, payload));
                } else {
                    // Source symbol
                    self.source_symbols[payload_id.encoding_symbol_id() as usize] =
                        Some(Symbol::new(payload));
                    self.received_source_symbols += 1;
                }
            }
        }

        let num_extended_symbols = extended_source_block_symbols(self.source_block_symbols);
        let num_padding_symbols = num_extended_symbols - self.source_block_symbols;

        // Case 1: the number of received packets is insufficient for decoding
        if self.received_esi.len() < self.source_block_symbols as usize {
            return None;
        }

        // Case 2: we have all source symbols and can return them without decoding
        if self.received_source_symbols == self.source_block_symbols {
            let mut result =
                vec![0; self.symbol_size as usize * self.source_block_symbols as usize];
            for (i, symbol) in self.source_symbols.iter().enumerate() {
                self.unpack_sub_blocks(&mut result, symbol.as_ref().unwrap().as_bytes(), i);
            }

            self.decoded = true;
            return Some(result);
        }

        // Case 3: we may have sufficient symbols to do a standard decoding
        let s = num_ldpc_symbols(self.source_block_symbols) as usize;
        let h = num_hdpc_symbols(self.source_block_symbols) as usize;
        let l = num_intermediate_symbols(self.source_block_symbols) as usize;

        let mut encoded_isis = vec![];
        for (i, source) in self.source_symbols.iter().enumerate() {
            if source.is_some() {
                encoded_isis.push(i as u32);
            }
        }
        for i in self.source_block_symbols..num_extended_symbols {
            encoded_isis.push(i);
        }
        for repair_packet in self.repair_packets.iter() {
            encoded_isis.push(repair_packet.payload_id.encoding_symbol_id() + num_padding_symbols);
        }

        // Case 3a: try to solve without HDPC rows (pure GF(2)) when we have enough overhead.
        // This avoids expensive GF(256) operations in the solver.
        // We need at least L total rows: S LDPC + encoded >= L, i.e. encoded >= K' + H.
        let num_padding = (num_extended_symbols - self.source_block_symbols) as usize;
        let num_repair = self.repair_packets.len();
        let ss = self.symbol_size as usize;
        if s + encoded_isis.len() >= l {
            let total_no_hdpc =
                s + self.received_source_symbols as usize + num_padding + num_repair;
            let mut d_no_hdpc = SymbolSlab::with_zeros(total_no_hdpc, ss);
            let mut row = s;
            for symbol in self.source_symbols.iter().flatten() {
                d_no_hdpc.get_mut(row).copy_from_slice(symbol.as_bytes());
                row += 1;
            }
            for _i in self.source_block_symbols..num_extended_symbols {
                // Padding row already zero
                row += 1;
            }
            for repair_packet in self.repair_packets.iter() {
                d_no_hdpc.get_mut(row).copy_from_slice(&repair_packet.data);
                row += 1;
            }
            assert_eq!(row, total_no_hdpc);

            let result = if num_extended_symbols >= self.sparse_threshold {
                let matrix = generate_constraint_matrix_no_hdpc::<SparseBinaryMatrix>(
                    self.source_block_symbols,
                    &encoded_isis,
                );
                self.try_pi_decode_no_hdpc(matrix, d_no_hdpc)
            } else {
                let matrix = generate_constraint_matrix_no_hdpc::<DenseBinaryMatrix>(
                    self.source_block_symbols,
                    &encoded_isis,
                );
                self.try_pi_decode_no_hdpc(matrix, d_no_hdpc)
            };
            if result.is_some() {
                return result;
            }
            // Reset decoded flag since the no-HDPC attempt may have set it on a false path
            self.decoded = false;
        }

        // Case 3b: standard decode with HDPC rows (slab-backed)
        // See section 5.3.3.4.2. There are S + H zero symbols to start the D vector
        let total = s + h + self.received_source_symbols as usize + num_padding + num_repair;
        let mut d = SymbolSlab::with_zeros(total, ss);
        let mut row = s + h;
        for symbol in self.source_symbols.iter().flatten() {
            d.get_mut(row).copy_from_slice(symbol.as_bytes());
            row += 1;
        }
        for _i in self.source_block_symbols..num_extended_symbols {
            // Padding row already zero
            row += 1;
        }
        for repair_packet in self.repair_packets.iter() {
            d.get_mut(row).copy_from_slice(&repair_packet.data);
            row += 1;
        }
        assert_eq!(row, total);

        if num_extended_symbols >= self.sparse_threshold {
            let (constraint_matrix, hdpc) = generate_constraint_matrix::<SparseBinaryMatrix>(
                self.source_block_symbols,
                &encoded_isis,
            );
            self.try_pi_decode(constraint_matrix, hdpc, d)
        } else {
            let (constraint_matrix, hdpc) = generate_constraint_matrix::<DenseBinaryMatrix>(
                self.source_block_symbols,
                &encoded_isis,
            );
            self.try_pi_decode(constraint_matrix, hdpc, d)
        }
    }

    fn rebuild_source_symbol_into(
        &self,
        dest: &mut [u8],
        intermediate_symbols: &SymbolSlab,
        source_symbol_id: u32,
        params: EncodingParameters,
    ) {
        let tuple = intermediate_tuple(
            source_symbol_id,
            params.lt_symbols,
            params.sys_index,
            params.p1,
        );
        let mut first = true;
        enc_indices(
            tuple,
            params.lt_symbols,
            params.pi_symbols,
            params.p1,
            |i| {
                if first {
                    dest.copy_from_slice(intermediate_symbols.get(i));
                    first = false;
                } else {
                    add_assign(dest, intermediate_symbols.get(i));
                }
            },
        );
    }
}
#[cfg(feature = "std")]
#[cfg(test)]
mod codec_tests {
    use std::{
        iter,
        sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        },
        vec::Vec,
    };

    use rand::Rng;
    #[cfg(not(feature = "python"))]
    use rand::seq::SliceRandom;

    #[cfg(not(feature = "python"))]
    use crate::Decoder;
    use crate::systematic_constants::{num_intermediate_symbols, num_ldpc_symbols};
    #[cfg(not(feature = "python"))]
    use crate::{Encoder, EncoderBuilder};
    use crate::{
        ObjectTransmissionInformation, SourceBlockDecoder, SourceBlockEncoder,
        SourceBlockEncodingPlan,
    };

    #[cfg(not(feature = "python"))]
    #[test]
    fn random_erasure_dense() {
        random_erasure(99_999);
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn random_erasure_sparse() {
        random_erasure(0);
    }

    #[cfg(not(feature = "python"))]
    fn random_erasure(sparse_threshold: u32) {
        let elements: usize = rand::rng().random_range(1..1_000_000);
        let mut data: Vec<u8> = vec![0; elements];
        for element in &mut data {
            *element = rand::rng().random();
        }

        // MTU is set to not be too small, otherwise this test may take a very long time
        let mtu = rand::rng().random_range(((elements / 100) as u16)..10_000);

        let encoder = Encoder::with_defaults(&data, mtu);

        let mut packets = encoder.get_encoded_packets(15);
        packets.shuffle(&mut rand::rng());
        // Erase 10 packets at random
        let length = packets.len();
        packets.truncate(length - 10);

        let mut decoder = Decoder::new(encoder.get_config());
        decoder.set_sparse_threshold(sparse_threshold);

        let mut result = None;
        while !packets.is_empty() {
            result = decoder.decode(packets.pop().unwrap());
            if result.is_some() {
                break;
            }
        }

        assert_eq!(result.unwrap(), data);
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn sub_block_erasure() {
        let elements: usize = 10_000;
        let mut data: Vec<u8> = vec![0; elements];
        for element in &mut data {
            *element = rand::rng().random();
        }

        let mut builder = EncoderBuilder::new();
        builder.set_decoder_memory_requirement(5000);
        builder.set_max_packet_size(500);
        let encoder = builder.build(&data);
        assert!(encoder.get_config().sub_blocks() > 2);

        // Test round trip
        let mut decoder = Decoder::new(encoder.get_config());
        let mut result = None;
        for packet in encoder.get_encoded_packets(0) {
            assert_eq!(result, None);
            result = decoder.decode(packet);
        }
        assert_eq!(result.unwrap(), data);

        // Test repair
        let mut packets = encoder.get_encoded_packets(15);
        packets.shuffle(&mut rand::rng());
        // Erase 10 packets at random
        let length = packets.len();
        packets.truncate(length - 10);

        let mut decoder = Decoder::new(encoder.get_config());

        let mut result = None;
        while !packets.is_empty() {
            result = decoder.decode(packets.pop().unwrap());
            if result.is_some() {
                break;
            }
        }

        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn round_trip_dense() {
        round_trip(99_999, 100, false);
    }

    #[test]
    fn round_trip_sparse() {
        round_trip(0, 100, false);
    }

    #[test]
    #[ignore]
    fn round_trip_dense_extended() {
        round_trip(99_999, 5000, true);
    }

    #[test]
    #[ignore]
    fn round_trip_sparse_extended() {
        round_trip(0, 56403, true);
    }

    fn round_trip(sparse_threshold: u32, max_symbols: usize, progress: bool) {
        let symbol_size = 8;
        for symbol_count in 1..=max_symbols {
            let elements = symbol_size * symbol_count;
            let mut data: Vec<u8> = vec![0; elements];
            for element in &mut data {
                *element = rand::rng().random();
            }

            if progress && symbol_count % 100 == 0 {
                println!("Completed {symbol_count} symbols")
            }

            let config = ObjectTransmissionInformation::new(0, symbol_size as u16, 0, 1, 1);
            let encoder = SourceBlockEncoder::new(1, &config, &data);

            let mut decoder = SourceBlockDecoder::new(1, &config, elements as u64);
            decoder.set_sparse_threshold(sparse_threshold);

            let mut result = None;
            for packet in encoder.source_packets() {
                assert_eq!(result, None);
                result = decoder.decode(iter::once(packet));
            }

            assert_eq!(result.unwrap(), data);
        }
    }

    #[test]
    #[ignore]
    fn repair_dense_extended() {
        repair(99_999, 5000, true, false);
    }

    #[test]
    #[ignore]
    fn repair_sparse_extended() {
        repair(0, 56403, true, false);
    }

    #[test]
    fn repair_dense() {
        repair(99_999, 50, false, false);
    }

    #[test]
    fn repair_sparse() {
        repair(0, 50, false, false);
    }

    #[test]
    fn repair_dense_pre_planned() {
        repair(99_999, 50, false, true);
    }

    #[test]
    fn repair_sparse_pre_planned() {
        repair(0, 50, false, true);
    }

    #[test]
    fn issue_120() {
        let symbol_size = 1280;
        let overhead = 0.5;
        let symbol_count = 10;
        let elements = symbol_count * symbol_size as usize;
        let mut data: Vec<u8> = vec![0; elements];
        for byte in data.iter_mut() {
            *byte = rand::rng().random();
        }

        let total_bytes: usize = 1024 * 1024;
        let iterations = total_bytes / elements;
        let config = ObjectTransmissionInformation::new(0, symbol_size, 0, 1, 1);
        let encoder = SourceBlockEncoder::new(1, &config, &data);
        let elements_and_overhead = (symbol_count as f64 * (1.0 + overhead)) as u32;
        let mut packets = encoder.repair_packets(0, iterations as u32 * elements_and_overhead);
        for _ in 0..iterations {
            let mut decoder = SourceBlockDecoder::new(1, &config, elements as u64);
            let start = packets.len() - elements_and_overhead as usize;
            decoder.decode(packets.drain(start..));
        }
    }

    fn repair(sparse_threshold: u32, max_symbols: usize, progress: bool, pre_plan: bool) {
        let pool = threadpool::Builder::new().build();
        let failed = Arc::new(AtomicU32::new(0));
        for symbol_count in 1..=max_symbols {
            let failed = failed.clone();
            pool.execute(move || {
                if failed.load(Ordering::SeqCst) != 0 {
                    return;
                }
                let success = do_repair(symbol_count, sparse_threshold, pre_plan);
                if !success {
                    failed.store(symbol_count as u32, Ordering::SeqCst);
                }

                if progress && symbol_count % 100 == 0 {
                    println!("[repair] Completed {symbol_count} symbols")
                }
            })
        }

        pool.join();
        assert_eq!(0, failed.load(Ordering::SeqCst));
    }

    fn do_repair(symbol_count: usize, sparse_threshold: u32, pre_plan: bool) -> bool {
        let symbol_size = 8;
        let elements = symbol_size * symbol_count;
        let mut data: Vec<u8> = vec![0; elements];
        for element in &mut data {
            *element = rand::rng().random();
        }

        let config = ObjectTransmissionInformation::new(0, 8, 0, 1, 1);
        let encoder = if pre_plan {
            let plan = SourceBlockEncodingPlan::generate(symbol_count as u16);
            SourceBlockEncoder::with_encoding_plan(1, &config, &data, &plan)
        } else {
            SourceBlockEncoder::new(1, &config, &data)
        };

        let mut decoder = SourceBlockDecoder::new(1, &config, elements as u64);
        decoder.set_sparse_threshold(sparse_threshold);

        let mut result = None;
        // This test can theoretically fail with ~1/256^5 probability
        for (parsed_packets, packet) in encoder
            .repair_packets(0, (elements / symbol_size + 4) as u32)
            .into_iter()
            .enumerate()
        {
            if parsed_packets < elements / symbol_size && result.is_some() {
                return false;
            }
            result = decoder.decode(iter::once(packet));
        }

        return result.unwrap() == data;
    }

    /// Test that the no-HDPC decode path produces identical results to the standard path
    /// across a range of symbol counts and overhead levels.
    #[test]
    fn decode_no_hdpc_matches_standard() {
        let symbol_size = 8;
        // Symbol counts spanning small (no-HDPC won't trigger) to large (will trigger)
        for symbol_count in [10, 50, 100, 250, 500] {
            let elements = symbol_size * symbol_count;
            let mut data: Vec<u8> = vec![0; elements];
            for element in &mut data {
                *element = rand::rng().random();
            }

            let config = ObjectTransmissionInformation::new(0, symbol_size as u16, 0, 1, 1);
            let encoder = SourceBlockEncoder::new(1, &config, &data);

            // Drop one source packet so we cannot return via the all-source fast path.
            let mut source_packets = encoder.source_packets();
            source_packets.remove(0);

            // Ensure we cross the no-HDPC trigger: S + encoded >= L.
            let k = symbol_count as u32;
            let required_repair = num_intermediate_symbols(k) - num_ldpc_symbols(k);
            let repair_count = required_repair + 4;
            let repair_packets = encoder.repair_packets(0, repair_count);

            let received_encoded = source_packets.len() as u32 + repair_count;
            assert!(num_ldpc_symbols(k) + received_encoded >= num_intermediate_symbols(k));

            let mut decoder = SourceBlockDecoder::new(1, &config, elements as u64);
            let all_packets = source_packets.into_iter().chain(repair_packets);
            let result = decoder.decode(all_packets);

            assert!(
                result.is_some(),
                "Failed to decode with 10% overhead at symbol_count={symbol_count}"
            );
            assert_eq!(
                result.unwrap(),
                data,
                "Decoded data mismatch at symbol_count={symbol_count}"
            );
        }
    }

    /// Test that the no-HDPC decode path works when only repair packets are used
    /// (all source packets lost, heavy overhead).
    #[test]
    fn decode_no_hdpc_repair_only() {
        let symbol_size = 8;
        for symbol_count in [100, 250, 500] {
            let elements = symbol_size * symbol_count;
            let mut data: Vec<u8> = vec![0; elements];
            for element in &mut data {
                *element = rand::rng().random();
            }

            let config = ObjectTransmissionInformation::new(0, symbol_size as u16, 0, 1, 1);
            let encoder = SourceBlockEncoder::new(1, &config, &data);

            // Ensure we cross the no-HDPC trigger: S + encoded >= L.
            // In repair-only mode, encoded == repair_count.
            let k = symbol_count as u32;
            let required_repair = num_intermediate_symbols(k) - num_ldpc_symbols(k);
            let repair_count = required_repair + 4;
            let repair_packets = encoder.repair_packets(0, repair_count);

            let mut decoder = SourceBlockDecoder::new(1, &config, elements as u64);
            let mut result = None;
            for packet in repair_packets {
                result = decoder.decode(iter::once(packet));
                if result.is_some() {
                    break;
                }
            }

            assert!(
                result.is_some(),
                "Failed to decode repair-only at symbol_count={symbol_count}"
            );
            assert_eq!(
                result.unwrap(),
                data,
                "Decoded data mismatch (repair-only) at symbol_count={symbol_count}"
            );
        }
    }
}
