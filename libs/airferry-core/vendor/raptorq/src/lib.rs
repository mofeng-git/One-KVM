#![allow(
    clippy::needless_return,
    clippy::unreadable_literal,
    clippy::needless_range_loop
)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

#[cfg(not(feature = "std"))]
extern crate core;

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

mod arraymap;
mod base;
mod constraint_matrix;
mod decoder;
mod encoder;
mod gf2;
mod graph;
mod iterators;
mod matrix;
mod octet;
mod octet_matrix;
mod octets;
mod operation_vector;
mod pi_solver;
mod rng;
mod sparse_matrix;
mod sparse_vec;
mod symbol;
mod symbol_slab;
mod systematic_constants;
mod util;

pub use crate::base::EncodingPacket;
pub use crate::base::ObjectTransmissionInformation;
pub use crate::base::PayloadId;
pub use crate::base::partition;
pub use crate::decoder::Decoder;
pub use crate::decoder::SourceBlockDecoder;
pub use crate::encoder::Encoder;
pub use crate::encoder::EncoderBuilder;
pub use crate::encoder::SourceBlockEncoder;
pub use crate::encoder::SourceBlockEncodingPlan;
pub use crate::encoder::calculate_block_offsets;
pub use crate::systematic_constants::extended_source_block_symbols;
