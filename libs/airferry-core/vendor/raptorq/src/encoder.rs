#[cfg(feature = "std")]
use std::collections::{HashMap, VecDeque};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(feature = "std")]
use std::vec::Vec;

use crate::ObjectTransmissionInformation;
use crate::base::EncodingPacket;
use crate::base::PayloadId;
use crate::base::intermediate_tuple;
use crate::base::partition;
use crate::constraint_matrix::generate_constraint_matrix;
use crate::matrix::DenseBinaryMatrix;
use crate::octets::add_assign;
use crate::operation_vector::{SymbolOps, perform_op};
use crate::pi_solver::fused_inverse_mul_symbols;
use crate::sparse_matrix::SparseBinaryMatrix;
use crate::symbol::Symbol;
use crate::symbol_slab::SymbolSlab;
use crate::systematic_constants::extended_source_block_symbols;
use crate::systematic_constants::num_hdpc_symbols;
use crate::systematic_constants::num_intermediate_symbols;
use crate::systematic_constants::num_ldpc_symbols;
use crate::systematic_constants::num_lt_symbols;
use crate::systematic_constants::num_pi_symbols;
use crate::systematic_constants::{calculate_p1, systematic_index};
use crate::util::int_div_ceil;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

pub const SPARSE_MATRIX_THRESHOLD: u32 = 250;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct EncoderBuilder {
    decoder_memory_requirement: u64,
    max_packet_size: u16,
}

impl EncoderBuilder {
    pub fn new() -> EncoderBuilder {
        EncoderBuilder {
            decoder_memory_requirement: 10 * 1024 * 1024,
            max_packet_size: 1024,
        }
    }

    pub fn set_decoder_memory_requirement(&mut self, bytes: u64) {
        self.decoder_memory_requirement = bytes;
    }

    pub fn set_max_packet_size(&mut self, bytes: u16) {
        self.max_packet_size = bytes;
    }

    pub fn build(&self, data: &[u8]) -> Encoder {
        let config = ObjectTransmissionInformation::generate_encoding_parameters(
            data.len() as u64,
            self.max_packet_size,
            self.decoder_memory_requirement,
        );

        Encoder::new(data, config)
    }
}

// Calculate the splits [start, end) of an object for encoding as blocks.
// If a block extends past the end of the object, it must be zero padded
pub fn calculate_block_offsets(
    data: &[u8],
    config: &ObjectTransmissionInformation,
) -> Vec<(usize, usize)> {
    let kt = int_div_ceil(config.transfer_length(), config.symbol_size() as u64);

    let (kl, ks, zl, zs) = partition(kt, config.source_blocks());

    let mut data_index = 0;
    let mut blocks = vec![];
    if zl > 0 {
        for _ in 0..zl {
            let offset = kl as usize * config.symbol_size() as usize;
            blocks.push((data_index, (data_index + offset)));
            data_index += offset;
        }
    }

    if zs > 0 {
        for _ in zl..(zl + zs) {
            let offset = ks as usize * config.symbol_size() as usize;
            if data_index + offset > data.len() {
                // Should only be possible when Kt * T > F. See third to last paragraph in section 4.4.1.2
                assert!(kt as usize * config.symbol_size() as usize > data.len());
            }
            blocks.push((data_index, (data_index + offset)));
            data_index += offset;
        }
    }

    blocks
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Encoder {
    config: ObjectTransmissionInformation,
    blocks: Vec<SourceBlockEncoder>,
}

impl Encoder {
    pub fn new(data: &[u8], config: ObjectTransmissionInformation) -> Encoder {
        let mut block_encoders = vec![];
        let mut cached_plan: Option<SourceBlockEncodingPlan> = None;
        for (i, (start, end)) in calculate_block_offsets(data, &config).drain(..).enumerate() {
            // Zero pad if necessary
            let mut padded;
            let block: &[u8] = if end > data.len() {
                padded = Vec::from(&data[start..]);
                padded.extend(vec![0; end - data.len()]);
                &padded
            } else {
                &data[start..end]
            };

            let symbol_count = block.len() / config.symbol_size() as usize;
            if cached_plan.is_none()
                || cached_plan.as_ref().unwrap().source_symbol_count != symbol_count as u16
            {
                let plan = SourceBlockEncodingPlan::generate(symbol_count as u16);
                cached_plan = Some(plan);
            }
            block_encoders.push(SourceBlockEncoder::with_encoding_plan(
                i as u8,
                &config,
                block,
                cached_plan.as_ref().unwrap(),
            ));
        }

        Encoder {
            config,
            blocks: block_encoders,
        }
    }

    pub fn with_defaults(data: &[u8], maximum_transmission_unit: u16) -> Encoder {
        let config = ObjectTransmissionInformation::with_defaults(
            data.len() as u64,
            maximum_transmission_unit,
        );

        Encoder::new(data, config)
    }

    pub fn get_config(&self) -> ObjectTransmissionInformation {
        self.config
    }

    pub fn get_encoded_packets(&self, repair_packets_per_block: u32) -> Vec<EncodingPacket> {
        let mut packets = vec![];
        for encoder in self.blocks.iter() {
            packets.extend(encoder.source_packets());
            packets.extend(encoder.repair_packets(0, repair_packets_per_block));
        }
        packets
    }

    pub fn get_block_encoders(&self) -> &Vec<SourceBlockEncoder> {
        &self.blocks
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceBlockEncodingPlan {
    operations: Vec<SymbolOps>,
    source_symbol_count: u16,
}

impl SourceBlockEncodingPlan {
    // Generates an encoding plan that is valid for any combination of data length and symbol size
    // where ceil(data_length / symbol_size) = symbol_count
    pub fn generate(symbol_count: u16) -> SourceBlockEncodingPlan {
        // TODO: refactor pi_solver, so that we don't need this dummy data to generate a plan
        let symbols = vec![Symbol::new(vec![0]); symbol_count as usize];
        let (_, ops) = gen_intermediate_symbols(&symbols, 1, SPARSE_MATRIX_THRESHOLD);
        SourceBlockEncodingPlan {
            operations: ops.unwrap(),
            source_symbol_count: symbol_count,
        }
    }
}

#[cfg(feature = "std")]
const SOURCE_BLOCK_ENCODING_PLAN_CACHE_CAPACITY: usize = 64;

#[cfg(feature = "std")]
#[derive(Default)]
struct SourceBlockEncodingPlanCache {
    plans: HashMap<u16, Arc<SourceBlockEncodingPlan>>,
    insertion_order: VecDeque<u16>,
}

#[cfg(feature = "std")]
fn source_block_encoding_plan_cache() -> &'static Mutex<SourceBlockEncodingPlanCache> {
    static CACHE: OnceLock<Mutex<SourceBlockEncodingPlanCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(SourceBlockEncodingPlanCache::default()))
}

#[cfg(feature = "std")]
fn get_or_generate_source_block_encoding_plan(symbol_count: u16) -> Arc<SourceBlockEncodingPlan> {
    {
        let cache = source_block_encoding_plan_cache();
        let guard = cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(plan) = guard.plans.get(&symbol_count) {
            return Arc::clone(plan);
        }
    }

    let generated = Arc::new(SourceBlockEncodingPlan::generate(symbol_count));
    let cache = source_block_encoding_plan_cache();
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Some(plan) = guard.plans.get(&symbol_count) {
        return Arc::clone(plan);
    }

    if guard.plans.len() >= SOURCE_BLOCK_ENCODING_PLAN_CACHE_CAPACITY
        && let Some(evicted_symbol_count) = guard.insertion_order.pop_front()
    {
        guard.plans.remove(&evicted_symbol_count);
    }

    guard.insertion_order.push_back(symbol_count);
    guard.plans.insert(symbol_count, Arc::clone(&generated));
    generated
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceBlockEncoder {
    source_block_id: u8,
    source_symbols: Vec<Symbol>,
    intermediate_symbols: SymbolSlab,
}

impl SourceBlockEncoder {
    fn create_symbols(config: &ObjectTransmissionInformation, data: &[u8]) -> Vec<Symbol> {
        assert_eq!(data.len() % config.symbol_size() as usize, 0);
        if config.sub_blocks() > 1 {
            let mut symbols = vec![vec![]; data.len() / config.symbol_size() as usize];
            let (tl, ts, nl, ns) = partition(
                (config.symbol_size() / config.symbol_alignment() as u16) as u32,
                config.sub_blocks(),
            );
            // Divide the block into sub-blocks and then concatenate the sub-symbols into symbols
            // See second to last paragraph in section 4.4.1.2.
            let mut offset = 0;
            for sub_block in 0..(nl + ns) {
                let bytes = if sub_block < nl {
                    tl as usize * config.symbol_alignment() as usize
                } else {
                    ts as usize * config.symbol_alignment() as usize
                };
                for symbol in &mut symbols {
                    symbol.extend_from_slice(&data[offset..offset + bytes]);
                    offset += bytes;
                }
            }
            assert_eq!(offset, data.len());
            symbols.drain(..).map(Symbol::new).collect()
        } else {
            data.chunks(config.symbol_size() as usize)
                .map(|x| Symbol::new(Vec::from(x)))
                .collect()
        }
    }

    pub fn new(
        source_block_id: u8,
        config: &ObjectTransmissionInformation,
        data: &[u8],
    ) -> SourceBlockEncoder {
        let source_symbols = SourceBlockEncoder::create_symbols(config, data);

        #[cfg(feature = "std")]
        {
            assert!(
                source_symbols.len() <= u16::MAX as usize,
                "source_symbols.len() exceeds u16::MAX"
            );
            let plan = get_or_generate_source_block_encoding_plan(source_symbols.len() as u16);
            let intermediate_symbols = gen_intermediate_symbols_with_plan(
                &source_symbols,
                config.symbol_size() as usize,
                &plan.operations,
            );
            return SourceBlockEncoder {
                source_block_id,
                source_symbols,
                intermediate_symbols,
            };
        }

        #[cfg(not(feature = "std"))]
        {
            let (intermediate_symbols, _operations) = gen_intermediate_symbols(
                &source_symbols,
                config.symbol_size() as usize,
                SPARSE_MATRIX_THRESHOLD,
            );

            return SourceBlockEncoder {
                source_block_id,
                source_symbols,
                intermediate_symbols: intermediate_symbols.unwrap(),
            };
        }
    }

    pub fn with_encoding_plan(
        source_block_id: u8,
        config: &ObjectTransmissionInformation,
        data: &[u8],
        plan: &SourceBlockEncodingPlan,
    ) -> SourceBlockEncoder {
        let source_symbols = SourceBlockEncoder::create_symbols(config, data);
        // TODO: this could be more lenient and support anything with the same extended symbol count
        assert_eq!(source_symbols.len(), plan.source_symbol_count as usize);

        let intermediate_symbols = gen_intermediate_symbols_with_plan(
            &source_symbols,
            config.symbol_size() as usize,
            &plan.operations,
        );

        SourceBlockEncoder {
            source_block_id,
            source_symbols,
            intermediate_symbols,
        }
    }

    pub fn source_packets(&self) -> Vec<EncodingPacket> {
        (0..self.source_symbols.len())
            .map(|i| {
                EncodingPacket::new(
                    PayloadId::new(self.source_block_id, i as u32),
                    self.source_symbols[i].as_bytes().to_vec(),
                )
            })
            .collect()
    }

    // See section 5.3.4
    pub fn repair_packets(&self, start_repair_symbol_id: u32, packets: u32) -> Vec<EncodingPacket> {
        let start_encoding_symbol_id = start_repair_symbol_id
            + extended_source_block_symbols(self.source_symbols.len() as u32);
        let mut result = vec![];
        let lt_symbols = num_lt_symbols(self.source_symbols.len() as u32);
        let sys_index = systematic_index(self.source_symbols.len() as u32);
        let p1 = calculate_p1(self.source_symbols.len() as u32);
        let symbol_size = self.intermediate_symbols.symbol_size();
        for i in 0..packets {
            let tuple = intermediate_tuple(start_encoding_symbol_id + i, lt_symbols, sys_index, p1);
            let mut data = vec![0u8; symbol_size];
            enc_into(
                &mut data,
                self.source_symbols.len() as u32,
                &self.intermediate_symbols,
                tuple,
            );
            result.push(EncodingPacket::new(
                PayloadId::new(
                    self.source_block_id,
                    self.source_symbols.len() as u32 + start_repair_symbol_id + i,
                ),
                data,
            ));
        }
        result
    }
}

#[allow(non_snake_case)]
fn create_d(
    source_block: &[Symbol],
    symbol_size: usize,
    _extended_source_symbols: usize,
) -> SymbolSlab {
    let L = num_intermediate_symbols(source_block.len() as u32);
    let S = num_ldpc_symbols(source_block.len() as u32);
    let H = num_hdpc_symbols(source_block.len() as u32);

    let mut D = SymbolSlab::with_zeros(L as usize, symbol_size);
    // First S+H entries are zero (already set).
    // Copy source symbols into positions S+H..
    for (i, symbol) in source_block.iter().enumerate() {
        D.get_mut((S + H) as usize + i)
            .copy_from_slice(symbol.as_bytes());
    }
    // Extended padding symbols stay zero.
    D
}

// See section 5.3.3.4
#[allow(non_snake_case)]
fn gen_intermediate_symbols(
    source_block: &[Symbol],
    symbol_size: usize,
    sparse_threshold: u32,
) -> (Option<SymbolSlab>, Option<Vec<SymbolOps>>) {
    let extended_source_symbols = extended_source_block_symbols(source_block.len() as u32);
    let D = create_d(source_block, symbol_size, extended_source_symbols as usize);

    let indices: Vec<u32> = (0..extended_source_symbols).collect();
    let (intermediate_symbols, operations) = if extended_source_symbols >= sparse_threshold {
        let (A, hdpc) =
            generate_constraint_matrix::<SparseBinaryMatrix>(extended_source_symbols, &indices);
        fused_inverse_mul_symbols(A, hdpc, D, extended_source_symbols)
    } else {
        let (A, hdpc) =
            generate_constraint_matrix::<DenseBinaryMatrix>(extended_source_symbols, &indices);
        fused_inverse_mul_symbols(A, hdpc, D, extended_source_symbols)
    };

    (intermediate_symbols, operations)
}
#[allow(non_snake_case)]
fn gen_intermediate_symbols_with_plan(
    source_block: &[Symbol],
    symbol_size: usize,
    operation_vector: &[SymbolOps],
) -> SymbolSlab {
    let extended_source_symbols = extended_source_block_symbols(source_block.len() as u32);
    let mut D = create_d(source_block, symbol_size, extended_source_symbols as usize);

    for op in operation_vector {
        perform_op(op, &mut D);
    }
    D
}

// Allocation-free Enc[] function, as defined in section 5.3.5.3.
// Writes the encoded symbol directly into `dest`.
#[allow(clippy::many_single_char_names)]
fn enc_into(
    dest: &mut [u8],
    source_block_symbols: u32,
    intermediate_symbols: &SymbolSlab,
    source_tuple: (u32, u32, u32, u32, u32, u32),
) {
    let w = num_lt_symbols(source_block_symbols);
    let p = num_pi_symbols(source_block_symbols);
    let p1 = calculate_p1(source_block_symbols);
    let (d, a, mut b, d1, a1, mut b1) = source_tuple;

    assert!(1 <= a && a < w);
    assert!(b < w);
    assert!(d1 == 2 || d1 == 3);
    assert!(1 <= a1 && a1 < p1);
    assert!(b1 < p1);

    dest.copy_from_slice(intermediate_symbols.get(b as usize));
    for _ in 1..d {
        b = (b + a) % w;
        add_assign(dest, intermediate_symbols.get(b as usize));
    }

    while b1 >= p {
        b1 = (b1 + a1) % p1;
    }

    add_assign(dest, intermediate_symbols.get((w + b1) as usize));

    for _ in 1..d1 {
        b1 = (b1 + a1) % p1;
        while b1 >= p {
            b1 = (b1 + a1) % p1;
        }
        add_assign(dest, intermediate_symbols.get((w + b1) as usize));
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::vec::Vec;

    use super::*;

    use crate::PayloadId;
    use crate::base::intermediate_tuple;
    use crate::symbol::Symbol;
    use crate::systematic_constants::num_lt_symbols;
    use crate::systematic_constants::num_pi_symbols;
    use crate::systematic_constants::{
        MAX_SOURCE_SYMBOLS_PER_BLOCK, calculate_p1, num_ldpc_symbols, systematic_index,
    };
    #[cfg(not(feature = "python"))]
    use crate::{Encoder, EncoderBuilder, EncodingPacket, ObjectTransmissionInformation};
    #[cfg(not(feature = "python"))]
    use std::collections::HashSet;

    const SYMBOL_SIZE: usize = 4;
    const NUM_SYMBOLS: u32 = 100;

    fn gen_test_data(size: usize) -> Vec<u8> {
        let mut data: Vec<u8> = vec![0; size];
        for byte in data.iter_mut() {
            *byte = rand::rng().random();
        }
        data
    }

    fn gen_test_symbols() -> Vec<Symbol> {
        let mut source_block: Vec<Symbol> = vec![];
        for _ in 0..NUM_SYMBOLS {
            let data = gen_test_data(SYMBOL_SIZE);
            source_block.push(Symbol::new(data));
        }
        source_block
    }

    #[test]
    fn enc_constraint_dense() {
        enc_constraint(MAX_SOURCE_SYMBOLS_PER_BLOCK + 1);
    }

    #[test]
    fn enc_constraint_sparse() {
        enc_constraint(0);
    }

    fn enc_constraint(sparse_threshold: u32) {
        let source_symbols = gen_test_symbols();

        let (intermediate_symbols, _) =
            gen_intermediate_symbols(&source_symbols, SYMBOL_SIZE, sparse_threshold);
        let intermediate_symbols = intermediate_symbols.unwrap();

        let lt_symbols = num_lt_symbols(NUM_SYMBOLS);
        let sys_index = systematic_index(NUM_SYMBOLS);
        let p1 = calculate_p1(NUM_SYMBOLS);
        // See section 5.3.3.4.1, item 1.
        for (i, source_symbol) in source_symbols.iter().enumerate() {
            let tuple = intermediate_tuple(i as u32, lt_symbols, sys_index, p1);
            let mut encoded = vec![0u8; SYMBOL_SIZE];
            enc_into(&mut encoded, NUM_SYMBOLS, &intermediate_symbols, tuple);
            assert_eq!(source_symbol.as_bytes(), &encoded[..]);
        }
    }

    #[test]
    fn ldpc_constraint_dense() {
        ldpc_constraint(MAX_SOURCE_SYMBOLS_PER_BLOCK + 1);
    }

    #[test]
    fn ldpc_constraint_sparse() {
        ldpc_constraint(0);
    }

    #[allow(non_snake_case)]
    fn ldpc_constraint(sparse_threshold: u32) {
        let (intermediate_symbols, _) =
            gen_intermediate_symbols(&gen_test_symbols(), SYMBOL_SIZE, sparse_threshold);
        let C = intermediate_symbols.unwrap();
        let S = num_ldpc_symbols(NUM_SYMBOLS) as usize;
        let P = num_pi_symbols(NUM_SYMBOLS) as usize;
        let W = num_lt_symbols(NUM_SYMBOLS) as usize;
        let B = W - S;

        // See section 5.3.3.3
        // Work with raw byte slices from the slab
        let ss = C.symbol_size();
        let mut D: Vec<Vec<u8>> = (0..S).map(|i| C.get(B + i).to_vec()).collect();

        for i in 0..B {
            let a = 1 + i / S;
            let b = i % S;
            crate::octets::add_assign(&mut D[b], C.get(i));

            let b = (b + a) % S;
            crate::octets::add_assign(&mut D[b], C.get(i));

            let b = (b + a) % S;
            crate::octets::add_assign(&mut D[b], C.get(i));
        }

        for i in 0..S {
            let a = i % P;
            let b = (i + 1) % P;
            crate::octets::add_assign(&mut D[i], C.get(W + a));
            crate::octets::add_assign(&mut D[i], C.get(W + b));
        }

        for i in 0..S {
            assert_eq!(vec![0u8; ss], D[i]);
        }
    }

    #[test]
    fn encoding_creates_expected_packets() {
        let symbol_size = 2;
        let data: [u8; 6] = [0, 1, 2, 3, 4, 5];
        let encoder = SourceBlockEncoder::new(
            0,
            &ObjectTransmissionInformation::new(0, symbol_size, 1, 1, 1),
            &data,
        );
        assert_eq!(
            encoder.source_packets(),
            [[0, 1], [2, 3], [4, 5]]
                .into_iter()
                .enumerate()
                .map(|(i, d)| EncodingPacket::new(PayloadId::new(0, i as u32), d.into()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            encoder
                .repair_packets(2, 4)
                .into_iter()
                .map(|p| p.payload_id.encoding_symbol_id())
                .collect::<Vec<_>>(),
            &[5, 6, 7, 8]
        );
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn test_builder() {
        let data = vec![0, 1, 2, 3];
        let encoder = Encoder::with_defaults(&data, 1024);
        let mut builder = EncoderBuilder::new();
        builder.set_max_packet_size(1024);
        assert_eq!(builder.build(&data), encoder);
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn padding_constraint_exact() {
        let packet_size: u16 = 1024;
        let padding_size: usize = 0;
        let data_size: usize = packet_size as usize * 2 - padding_size;
        padding_constraint(packet_size, padding_size, data_size);
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn padding_constraint_42_bytes() {
        let packet_size: u16 = 1024;
        let padding_size: usize = 42;
        let data_size: usize = packet_size as usize * 2 - padding_size;
        padding_constraint(packet_size, padding_size, data_size);
    }

    #[cfg(not(feature = "python"))]
    fn padding_constraint(packet_size: u16, padding_size: usize, data_size: usize) {
        let data = gen_test_data(data_size);
        let encoder = Encoder::with_defaults(&data, packet_size);

        fn accumulate_data(acc: Vec<u8>, packet: EncodingPacket) -> Vec<u8> {
            let mut updated_acc = acc;
            updated_acc.extend_from_slice(packet.data());
            updated_acc
        }

        let padded_data = encoder
            .get_block_encoders()
            .iter()
            .flat_map(|block| block.source_packets())
            .fold(vec![], accumulate_data);

        assert_eq!(data_size + padding_size, padded_data.len());
        assert_eq!(data[..], padded_data[..data_size]);
    }

    #[cfg(not(feature = "python"))]
    #[test]
    fn unique_blocks() {
        let data = gen_test_data(120);
        let config = ObjectTransmissionInformation::new(120, 10, 10, 0, 2);
        let encoder = Encoder::new(&data, config);
        assert!(encoder.get_block_encoders().len() > 1);
        let mut ids = HashSet::new();
        for block in encoder.get_block_encoders().iter() {
            ids.insert(block.source_block_id);
        }
        assert_eq!(ids.len(), encoder.get_block_encoders().len());
    }
}
