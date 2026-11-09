#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::octet::Octet;
use crate::octets::{add_assign, fused_addassign_mul_scalar, mulassign_scalar};
use crate::symbol::Symbol;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

/// Contiguous slab storage for symbols.
///
/// Instead of `Vec<Symbol>` (one heap allocation per symbol), this stores all
/// symbol data in a single `Vec<u8>`. Symbol `i` occupies bytes
/// `[i * symbol_size .. (i+1) * symbol_size]`.
///
/// This gives much better spatial locality for the row operations in the PI
/// solver, where `AddAssign` / `FMA` ops touch pairs of symbols millions of
/// times.
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SymbolSlab {
    data: Vec<u8>,
    count: usize,
    symbol_size: usize,
    mapping: Option<Vec<usize>>,
}

impl SymbolSlab {
    /// Create a slab with `count` symbols, all zeroed.
    pub fn with_zeros(count: usize, symbol_size: usize) -> Self {
        SymbolSlab {
            data: vec![0u8; count * symbol_size],
            count,
            symbol_size,
            mapping: None,
        }
    }

    /// Convert a `Vec<Symbol>` into a contiguous slab.
    /// All symbols must have the same length.
    #[allow(dead_code)]
    pub fn from_symbols(symbols: Vec<Symbol>, symbol_size: usize) -> Self {
        let count = symbols.len();
        let mut data = Vec::with_capacity(count * symbol_size);
        for symbol in symbols {
            let bytes = symbol.into_bytes();
            assert_eq!(
                bytes.len(),
                symbol_size,
                "symbol length mismatch in SymbolSlab::from_symbols"
            );
            data.extend_from_slice(&bytes);
        }
        SymbolSlab {
            data,
            count,
            symbol_size,
            mapping: None,
        }
    }

    /// Convert back to individual `Symbol`s (allocates one `Vec<u8>` per symbol).
    #[allow(dead_code)]
    pub fn into_symbols(self) -> Vec<Symbol> {
        if let Some(ref mapping) = self.mapping {
            mapping
                .iter()
                .map(|&phys| {
                    let start = phys * self.symbol_size;
                    Symbol::new(self.data[start..start + self.symbol_size].to_vec())
                })
                .collect()
        } else {
            self.data
                .chunks_exact(self.symbol_size)
                .map(|chunk| Symbol::new(chunk.to_vec()))
                .collect()
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn symbol_size(&self) -> usize {
        self.symbol_size
    }

    #[inline(always)]
    fn physical_index(&self, i: usize) -> usize {
        self.mapping.as_ref().map_or(i, |m| m[i])
    }

    /// Borrow symbol `i` as a byte slice.
    #[inline]
    pub fn get(&self, i: usize) -> &[u8] {
        let i = self.physical_index(i);
        let start = i * self.symbol_size;
        &self.data[start..start + self.symbol_size]
    }

    /// Mutably borrow symbol `i` as a byte slice.
    #[inline]
    pub fn get_mut(&mut self, i: usize) -> &mut [u8] {
        let i = self.physical_index(i);
        let start = i * self.symbol_size;
        &mut self.data[start..start + self.symbol_size]
    }

    /// Get mutable access to `dest` and shared access to `src` simultaneously.
    /// Panics if `dest == src`.
    #[inline]
    pub fn get_pair_mut(&mut self, dest: usize, src: usize) -> (&mut [u8], &[u8]) {
        let dest = self.physical_index(dest);
        let src = self.physical_index(src);
        assert_ne!(dest, src, "dest and src must differ");
        assert!(dest < self.count, "dest out of range");
        assert!(src < self.count, "src out of range");

        let ss = self.symbol_size;
        let dest_start = dest * ss;
        let src_start = src * ss;

        // SAFETY:
        // - dest/src are in-bounds (asserts above), so both ranges are within self.data.
        // - dest != src, and every symbol range has length `ss`, so ranges do not overlap.
        // - we only create one mutable slice (dest) and one shared slice (src).
        unsafe {
            let ptr = self.data.as_mut_ptr();
            let dest_slice = core::slice::from_raw_parts_mut(ptr.add(dest_start), ss);
            let src_slice = core::slice::from_raw_parts(ptr.add(src_start), ss);
            (dest_slice, src_slice)
        }
    }

    /// `dest[i] += src[i]` (GF(2) XOR)
    #[inline]
    pub fn add_assign(&mut self, dest: usize, src: usize) {
        let (d, s) = self.get_pair_mut(dest, src);
        add_assign(d, s);
    }

    /// `dest[i] *= scalar` (GF(256) multiply)
    #[inline]
    pub fn mulassign_scalar(&mut self, dest: usize, scalar: &Octet) {
        let d = self.get_mut(dest);
        mulassign_scalar(d, scalar);
    }

    /// `dest[i] += src[i] * scalar` (GF(256) fused multiply-add)
    #[inline]
    pub fn fma(&mut self, dest: usize, src: usize, scalar: &Octet) {
        let (d, s) = self.get_pair_mut(dest, src);
        fused_addassign_mul_scalar(d, s, scalar);
    }

    /// Set a virtual reorder mapping: logical index `i` maps to physical index `order[i]`.
    pub fn set_reorder(&mut self, order: Vec<usize>) {
        self.mapping = Some(order);
    }

    /// Bulk copy from a contiguous source block into the slab at the given offset.
    /// `source` must be `count * symbol_size` bytes.
    #[allow(dead_code)]
    pub fn copy_block_from(&mut self, dest_symbol_start: usize, source: &[u8]) {
        debug_assert!(
            self.mapping.is_none(),
            "copy_block_from called with active mapping"
        );
        debug_assert_eq!(source.len() % self.symbol_size, 0);
        let start = dest_symbol_start * self.symbol_size;
        self.data[start..start + source.len()].copy_from_slice(source);
    }

    /// Create a new slab by gathering symbols at the given indices from self.
    /// The new slab has `indices.len()` symbols.
    #[allow(dead_code)]
    pub fn gather(&self, indices: &[usize]) -> Self {
        debug_assert!(self.mapping.is_none(), "gather called with active mapping");
        let ss = self.symbol_size;
        let new_count = indices.len();
        let mut data = vec![0u8; new_count * ss];
        for (new_pos, &old_pos) in indices.iter().enumerate() {
            data[new_pos * ss..(new_pos + 1) * ss]
                .copy_from_slice(&self.data[old_pos * ss..(old_pos + 1) * ss]);
        }
        SymbolSlab {
            data,
            count: new_count,
            symbol_size: ss,
            mapping: None,
        }
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::octet::Octet;

    #[test]
    fn roundtrip_from_into_symbols() {
        let symbols = vec![
            Symbol::new(vec![1, 2, 3, 4]),
            Symbol::new(vec![5, 6, 7, 8]),
            Symbol::new(vec![9, 10, 11, 12]),
        ];
        let slab = SymbolSlab::from_symbols(symbols.clone(), 4);
        assert_eq!(slab.len(), 3);
        assert_eq!(slab.symbol_size(), 4);
        assert_eq!(slab.get(0), &[1, 2, 3, 4]);
        assert_eq!(slab.get(1), &[5, 6, 7, 8]);
        assert_eq!(slab.get(2), &[9, 10, 11, 12]);
        let back = slab.into_symbols();
        assert_eq!(back, symbols);
    }

    #[test]
    fn add_assign_xor() {
        let mut slab = SymbolSlab::from_symbols(
            vec![Symbol::new(vec![0xFF, 0x00]), Symbol::new(vec![0x0F, 0xF0])],
            2,
        );
        slab.add_assign(0, 1);
        assert_eq!(slab.get(0), &[0xF0, 0xF0]);
        assert_eq!(slab.get(1), &[0x0F, 0xF0]); // src unchanged
    }

    #[test]
    fn reorder_symbols() {
        let mut slab = SymbolSlab::from_symbols(
            vec![
                Symbol::new(vec![0]),
                Symbol::new(vec![1]),
                Symbol::new(vec![2]),
            ],
            1,
        );
        slab.set_reorder(vec![2, 0, 1]);
        assert_eq!(slab.get(0), &[2]);
        assert_eq!(slab.get(1), &[0]);
        assert_eq!(slab.get(2), &[1]);
    }

    #[test]
    fn fma_operation() {
        let mut slab =
            SymbolSlab::from_symbols(vec![Symbol::new(vec![0x01]), Symbol::new(vec![0x02])], 1);
        let scalar = Octet::new(3);
        slab.fma(0, 1, &scalar);
        // GF(256): 0x01 ^ (0x02 * 3) = 0x01 ^ 0x06 = 0x07
        assert_eq!(slab.get(0), &[0x07]);
    }
}
