#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::octet::Octet;
use crate::symbol_slab::SymbolSlab;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
#[allow(clippy::upper_case_acronyms)]
pub enum SymbolOps {
    AddAssign {
        dest: usize,
        src: usize,
    },
    MulAssign {
        dest: usize,
        scalar: Octet,
    },
    FMA {
        dest: usize,
        src: usize,
        scalar: Octet,
    },
    Reorder {
        order: Vec<usize>,
    },
}

pub fn perform_op(op: &SymbolOps, symbols: &mut SymbolSlab) {
    match op {
        SymbolOps::AddAssign { dest, src } => {
            symbols.add_assign(*dest, *src);
        }
        SymbolOps::MulAssign { dest, scalar } => {
            symbols.mulassign_scalar(*dest, scalar);
        }
        SymbolOps::FMA { dest, src, scalar } => {
            symbols.fma(*dest, *src, scalar);
        }
        SymbolOps::Reorder { order } => {
            symbols.set_reorder(order.clone());
        }
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::vec::Vec;

    use crate::octet::Octet;
    use crate::operation_vector::{SymbolOps, perform_op};
    use crate::symbol::Symbol;
    use crate::symbol_slab::SymbolSlab;

    #[test]
    fn test_add() {
        let symbol_size = 1316;
        let mut raw0: Vec<u8> = vec![0; symbol_size];
        let mut raw1: Vec<u8> = vec![0; symbol_size];
        for b in raw0.iter_mut() {
            *b = rand::rng().random();
        }
        for b in raw1.iter_mut() {
            *b = rand::rng().random();
        }
        let expected: Vec<u8> = raw0.iter().zip(raw1.iter()).map(|(a, b)| a ^ b).collect();

        let mut slab =
            SymbolSlab::from_symbols(vec![Symbol::new(raw0), Symbol::new(raw1)], symbol_size);
        perform_op(&SymbolOps::AddAssign { dest: 0, src: 1 }, &mut slab);
        assert_eq!(expected, slab.get(0));
    }

    #[test]
    fn test_add_mul() {
        let symbol_size = 1316;
        let mut raw0: Vec<u8> = vec![0; symbol_size];
        let mut raw1: Vec<u8> = vec![0; symbol_size];
        for b in raw0.iter_mut() {
            *b = rand::rng().random();
        }
        for b in raw1.iter_mut() {
            *b = rand::rng().random();
        }
        let value = 173;
        let expected: Vec<u8> = raw0
            .iter()
            .zip(raw1.iter())
            .map(|(d0, d1)| *d0 ^ (Octet::new(*d1) * Octet::new(value)).byte())
            .collect();

        let mut slab =
            SymbolSlab::from_symbols(vec![Symbol::new(raw0), Symbol::new(raw1)], symbol_size);
        perform_op(
            &SymbolOps::FMA {
                dest: 0,
                src: 1,
                scalar: Octet::new(value),
            },
            &mut slab,
        );
        assert_eq!(expected, slab.get(0));
    }

    #[test]
    fn test_mul() {
        let symbol_size = 1316;
        let mut raw0: Vec<u8> = vec![0; symbol_size];
        for b in raw0.iter_mut() {
            *b = rand::rng().random();
        }
        let value = 215;
        let expected: Vec<u8> = raw0
            .iter()
            .map(|d0| (Octet::new(*d0) * Octet::new(value)).byte())
            .collect();

        let mut slab = SymbolSlab::from_symbols(vec![Symbol::new(raw0)], symbol_size);
        perform_op(
            &SymbolOps::MulAssign {
                dest: 0,
                scalar: Octet::new(value),
            },
            &mut slab,
        );
        assert_eq!(expected, slab.get(0));
    }

    #[test]
    fn test_reorder() {
        let rows = 10;
        let symbol_size = 10;
        let symbols: Vec<Symbol> = (0..rows)
            .map(|i| Symbol::new(vec![i as u8; symbol_size]))
            .collect();
        let mut slab = SymbolSlab::from_symbols(symbols, symbol_size);

        assert_eq!(slab.get(0)[0], 0);
        assert_eq!(slab.get(1)[0], 1);
        assert_eq!(slab.get(2)[0], 2);
        assert_eq!(slab.get(9)[0], 9);

        perform_op(
            &SymbolOps::Reorder {
                order: vec![9, 7, 5, 3, 1, 8, 0, 6, 2, 4],
            },
            &mut slab,
        );
        assert_eq!(slab.get(0)[0], 9);
        assert_eq!(slab.get(1)[0], 7);
        assert_eq!(slab.get(2)[0], 5);
        assert_eq!(slab.get(3)[0], 3);
        assert_eq!(slab.get(4)[0], 1);
        assert_eq!(slab.get(5)[0], 8);
        assert_eq!(slab.get(6)[0], 0);
        assert_eq!(slab.get(7)[0], 6);
        assert_eq!(slab.get(8)[0], 2);
        assert_eq!(slab.get(9)[0], 4);
    }
}
