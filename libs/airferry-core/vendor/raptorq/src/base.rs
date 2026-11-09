#[cfg(feature = "std")]
use std::{cmp::min, vec::Vec};

#[cfg(not(feature = "std"))]
use core::cmp::min;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::rng::rand;
use crate::systematic_constants::{
    MAX_SOURCE_SYMBOLS_PER_BLOCK, SYSTEMATIC_INDICES_AND_PARAMETERS,
};
use crate::util::int_div_ceil;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

// As defined in section 3.2
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct PayloadId {
    source_block_number: u8,
    encoding_symbol_id: u32,
}

impl PayloadId {
    pub fn new(source_block_number: u8, encoding_symbol_id: u32) -> PayloadId {
        // Encoding Symbol ID must be a 24-bit unsigned int
        assert!(encoding_symbol_id < 16777216);
        PayloadId {
            source_block_number,
            encoding_symbol_id,
        }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn deserialize(data: &[u8; 4]) -> PayloadId {
        PayloadId {
            source_block_number: data[0],
            encoding_symbol_id: ((data[1] as u32) << 16) + ((data[2] as u32) << 8) + data[3] as u32,
        }
    }

    pub fn serialize(&self) -> [u8; 4] {
        [
            self.source_block_number,
            (self.encoding_symbol_id >> 16) as u8,
            ((self.encoding_symbol_id >> 8) & 0xFF) as u8,
            (self.encoding_symbol_id & 0xFF) as u8,
        ]
    }

    pub fn source_block_number(&self) -> u8 {
        self.source_block_number
    }

    pub fn encoding_symbol_id(&self) -> u32 {
        self.encoding_symbol_id
    }
}

/// Contains encoding symbols generated from a source block.
///
/// As defined in section [4.4.2](https://tools.ietf.org/html/rfc6330#section-4.4.2).
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct EncodingPacket {
    pub(crate) payload_id: PayloadId,
    pub(crate) data: Vec<u8>,
}

impl EncodingPacket {
    pub fn new(payload_id: PayloadId, data: Vec<u8>) -> EncodingPacket {
        EncodingPacket { payload_id, data }
    }

    pub fn deserialize(data: &[u8]) -> EncodingPacket {
        let payload_data = [data[0], data[1], data[2], data[3]];
        EncodingPacket {
            payload_id: PayloadId::deserialize(&payload_data),
            data: Vec::from(&data[4..]),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut serialized = Vec::with_capacity(4 + self.data.len());
        serialized.extend_from_slice(&self.payload_id.serialize());
        serialized.extend(self.data.iter());
        return serialized;
    }

    /// Retrieves packet payload ID.
    pub fn payload_id(&self) -> &PayloadId {
        &self.payload_id
    }

    /// Retrieves packet payload.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Split a packet into its underlying ID and payload.
    pub fn split(self) -> (PayloadId, Vec<u8>) {
        (self.payload_id, self.data)
    }
}

// As defined in section 3.3.2 and 3.3.3
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct ObjectTransmissionInformation {
    transfer_length: u64, // Limited to u40
    symbol_size: u16,
    num_source_blocks: u8,
    num_sub_blocks: u16,
    symbol_alignment: u8,
}

impl ObjectTransmissionInformation {
    pub fn new(
        transfer_length: u64,
        symbol_size: u16,
        source_blocks: u8,
        sub_blocks: u16,
        alignment: u8,
    ) -> ObjectTransmissionInformation {
        // See errata (https://www.rfc-editor.org/errata/eid5548)
        assert!(transfer_length <= 942574504275);
        assert_eq!(symbol_size % alignment as u16, 0);
        // See section 4.4.1.2. "These parameters MUST be set so that ceil(ceil(F/T)/Z) <= K'_max."

        if (symbol_size != 0) && (source_blocks != 0) {
            let symbols_required = int_div_ceil(
                int_div_ceil(transfer_length, symbol_size as u64) as u64,
                source_blocks as u64,
            );
            assert!((symbols_required) <= MAX_SOURCE_SYMBOLS_PER_BLOCK);
        }

        ObjectTransmissionInformation {
            transfer_length,
            symbol_size,
            num_source_blocks: source_blocks,
            num_sub_blocks: sub_blocks,
            symbol_alignment: alignment,
        }
    }

    pub fn deserialize(data: &[u8; 12]) -> ObjectTransmissionInformation {
        ObjectTransmissionInformation {
            transfer_length: ((data[0] as u64) << 32)
                + ((data[1] as u64) << 24)
                + ((data[2] as u64) << 16)
                + ((data[3] as u64) << 8)
                + (data[4] as u64),
            symbol_size: ((data[6] as u16) << 8) + data[7] as u16,
            num_source_blocks: data[8],
            num_sub_blocks: ((data[9] as u16) << 8) + data[10] as u16,
            symbol_alignment: data[11],
        }
    }

    pub fn serialize(&self) -> [u8; 12] {
        [
            ((self.transfer_length >> 32) & 0xFF) as u8,
            ((self.transfer_length >> 24) & 0xFF) as u8,
            ((self.transfer_length >> 16) & 0xFF) as u8,
            ((self.transfer_length >> 8) & 0xFF) as u8,
            (self.transfer_length & 0xFF) as u8,
            0, // Reserved
            (self.symbol_size >> 8) as u8,
            (self.symbol_size & 0xFF) as u8,
            self.num_source_blocks,
            (self.num_sub_blocks >> 8) as u8,
            (self.num_sub_blocks & 0xFF) as u8,
            self.symbol_alignment,
        ]
    }

    pub fn transfer_length(&self) -> u64 {
        self.transfer_length
    }

    pub fn symbol_size(&self) -> u16 {
        self.symbol_size
    }

    pub fn source_blocks(&self) -> u8 {
        self.num_source_blocks
    }

    pub fn sub_blocks(&self) -> u16 {
        self.num_sub_blocks
    }

    pub fn symbol_alignment(&self) -> u8 {
        self.symbol_alignment
    }

    pub(crate) fn generate_encoding_parameters(
        transfer_length: u64,
        max_packet_size: u16,
        decoder_memory_requirement: u64,
    ) -> ObjectTransmissionInformation {
        let (alignment, sub_symbol_size) = if max_packet_size >= 8 * 8 {
            (8, 8)
        } else {
            (1, 1)
        };
        assert!(max_packet_size >= alignment);
        let symbol_size = max_packet_size - (max_packet_size % alignment);

        let kt = int_div_ceil(transfer_length, symbol_size as u64);

        let n_max = symbol_size as u32 / (sub_symbol_size * alignment) as u32;

        let kl = |n: u32| -> u32 {
            for &(kprime, _, _, _, _) in SYSTEMATIC_INDICES_AND_PARAMETERS.iter().rev() {
                let x = int_div_ceil(symbol_size as u64, alignment as u64 * n as u64);
                if kprime <= (decoder_memory_requirement / (alignment as u64 * x as u64)) as u32 {
                    return kprime;
                }
            }
            unreachable!();
        };

        let num_source_blocks = int_div_ceil(kt as u64, kl(n_max) as u64);

        let mut n = 1;
        for i in 1..=n_max {
            n = i;
            if int_div_ceil(kt as u64, num_source_blocks as u64) <= kl(n) {
                break;
            }
        }

        ObjectTransmissionInformation {
            transfer_length,
            symbol_size,
            num_source_blocks: num_source_blocks as u8,
            num_sub_blocks: n as u16,
            symbol_alignment: alignment as u8,
        }
    }

    pub fn with_defaults(
        transfer_length: u64,
        max_packet_size: u16,
    ) -> ObjectTransmissionInformation {
        ObjectTransmissionInformation::generate_encoding_parameters(
            transfer_length,
            max_packet_size,
            10 * 1024 * 1024,
        )
    }
}

// Partition[I, J] function, as defined in section 4.4.1.2
pub fn partition<TI, TJ>(i: TI, j: TJ) -> (u32, u32, u32, u32)
where
    TI: Into<u32>,
    TJ: Into<u32>,
{
    let (i, j) = (i.into(), j.into());
    let il = int_div_ceil(i as u64, j as u64);

    let is = i / j;

    let jl = i - is * j;
    let js = j - jl;
    (il, is, jl, js)
}

// Deg[v] as defined in section 5.3.5.2
pub fn deg(v: u32, lt_symbols: u32) -> u32 {
    assert!(v < 1048576);
    let f: [u32; 31] = [
        0, 5243, 529531, 704294, 791675, 844104, 879057, 904023, 922747, 937311, 948962, 958494,
        966438, 973160, 978921, 983914, 988283, 992138, 995565, 998631, 1001391, 1003887, 1006157,
        1008229, 1010129, 1011876, 1013490, 1014983, 1016370, 1017662, 1048576,
    ];

    #[allow(clippy::needless_range_loop)]
    for d in 1..f.len() {
        if v < f[d] {
            return min(d as u32, lt_symbols - 2);
        }
    }
    unreachable!();
}

// Tuple[K', X] as defined in section 5.3.5.4
#[allow(non_snake_case, clippy::many_single_char_names)]
pub fn intermediate_tuple(
    internal_symbol_id: u32,
    lt_symbols: u32,
    systematic_index: u32,
    p1: u32,
) -> (u32, u32, u32, u32, u32, u32) {
    let J = systematic_index;
    let W = lt_symbols;
    let P1 = p1;

    let mut A = 53591 + J * 997;

    if A.is_multiple_of(2) {
        A += 1;
    }

    let B = 10267 * (J + 1);
    let y: u32 = ((B as u64 + internal_symbol_id as u64 * A as u64) % 4294967296) as u32;
    let v = rand(y, 0u32, 1048576);
    let d = deg(v, W);
    let a = 1 + rand(y, 1u32, W - 1);
    let b = rand(y, 2u32, W);

    let d1 = if d < 4 {
        2 + rand(internal_symbol_id, 3u32, 2)
    } else {
        2
    };

    let a1 = 1 + rand(internal_symbol_id, 4u32, P1 - 1);
    let b1 = rand(internal_symbol_id, 5u32, P1);

    (d, a, b, d1, a1, b1)
}

#[cfg(test)]
mod tests {
    use crate::{EncodingPacket, ObjectTransmissionInformation, PayloadId};
    use rand::Rng;

    #[test]
    fn max_transfer_size() {
        ObjectTransmissionInformation::new(942574504275, 65535, 255, 1, 1);
    }

    #[test]
    fn payload_id_serialization() {
        let payload_id = PayloadId::new(
            rand::rng().random(),
            rand::rng().random_range(0..(256 * 256 * 256)),
        );
        let deserialized = PayloadId::deserialize(&payload_id.serialize());
        assert_eq!(deserialized, payload_id);
    }

    #[test]
    fn encoding_packet_serialization() {
        let payload_id = PayloadId::new(
            rand::rng().random(),
            rand::rng().random_range(0..(256 * 256 * 256)),
        );
        let packet = EncodingPacket::new(payload_id, vec![rand::rng().random()]);
        let deserialized = EncodingPacket::deserialize(&packet.serialize());
        assert_eq!(deserialized, packet);
    }

    #[test]
    fn oti_serialization() {
        let oti = ObjectTransmissionInformation::with_defaults(
            rand::rng().random_range(0..(256 * 256 * 256 * 256 * 256)),
            rand::rng().random(),
        );
        let deserialized = ObjectTransmissionInformation::deserialize(&oti.serialize());
        assert_eq!(deserialized, oti);
    }
}
