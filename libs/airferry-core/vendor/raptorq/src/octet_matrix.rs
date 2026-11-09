#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::octet::Octet;
use crate::octets::{BinaryOctetVec, fused_addassign_mul_scalar};
use crate::octets::{add_assign, fused_addassign_mul_scalar_binary, mulassign_scalar};
use crate::util::get_both_indices;
#[cfg(feature = "benchmarking")]
use std::mem::size_of;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct DenseOctetMatrix {
    height: usize,
    width: usize,
    elements: Vec<Vec<u8>>,
}

impl DenseOctetMatrix {
    pub fn new(height: usize, width: usize, _: usize) -> DenseOctetMatrix {
        let mut elements: Vec<Vec<u8>> = Vec::with_capacity(height);
        for _ in 0..height {
            elements.push(vec![0; width]);
        }
        DenseOctetMatrix {
            height,
            width,
            elements,
        }
    }

    pub fn fma_sub_row(
        &mut self,
        row: usize,
        start_col: usize,
        scalar: &Octet,
        other: &BinaryOctetVec,
    ) {
        fused_addassign_mul_scalar_binary(
            &mut self.elements[row][start_col..(start_col + other.len())],
            other,
            scalar,
        );
    }

    pub fn set(&mut self, i: usize, j: usize, value: Octet) {
        self.elements[i][j] = value.byte();
    }

    pub fn height(&self) -> usize {
        self.height
    }

    #[cfg(all(test, feature = "std"))]
    pub fn width(&self) -> usize {
        self.width
    }

    #[cfg(feature = "benchmarking")]
    pub fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();
        bytes += size_of::<Vec<u8>>() * self.elements.len();
        bytes += size_of::<u8>() * self.height * self.width;

        bytes
    }

    pub fn mul_assign_row(&mut self, row: usize, value: &Octet) {
        mulassign_scalar(&mut self.elements[row], value);
    }

    pub fn get(&self, i: usize, j: usize) -> Octet {
        Octet::new(self.elements[i][j])
    }

    pub fn swap_rows(&mut self, i: usize, j: usize) {
        self.elements.swap(i, j);
    }

    pub fn swap_columns(&mut self, i: usize, j: usize, start_row_hint: usize) {
        for row in start_row_hint..self.elements.len() {
            self.elements[row].swap(i, j);
        }
    }

    pub fn fma_rows(&mut self, dest: usize, multiplicand: usize, scalar: &Octet) {
        assert_ne!(dest, multiplicand);
        let (dest_row, temp_row) = get_both_indices(&mut self.elements, dest, multiplicand);

        if *scalar == Octet::one() {
            add_assign(dest_row, temp_row);
        } else {
            fused_addassign_mul_scalar(dest_row, temp_row, scalar);
        }
    }
}
