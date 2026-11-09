#[cfg(feature = "std")]
use std::{mem::size_of, vec::Vec};

#[cfg(not(feature = "std"))]
use core::mem::size_of;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::gf2::add_assign_binary;
use crate::iterators::OctetIter;
use crate::octet::Octet;
use crate::octets::BinaryOctetVec;
use crate::util::get_both_ranges;

// TODO: change this struct to not use the Octet class, since it's binary not GF(256)
pub trait BinaryMatrix: Clone {
    fn new(height: usize, width: usize, trailing_dense_column_hint: usize) -> Self;

    fn set(&mut self, i: usize, j: usize, value: Octet);

    fn height(&self) -> usize;

    fn width(&self) -> usize;

    #[allow(dead_code)]
    fn size_in_bytes(&self) -> usize;

    fn count_ones(&self, row: usize, start_col: usize, end_col: usize) -> usize;

    // Once "impl Trait" is supported in traits, it would be better to return "impl Iterator<...>"
    fn get_row_iter(&self, row: usize, start_col: usize, end_col: usize) -> OctetIter<'_>;

    // An iterator over rows with a 1-valued entry for the given col
    fn get_ones_in_column(&self, col: usize, start_row: usize, end_row: usize) -> Vec<u32>;

    #[inline]
    fn get_ones_in_column_into(
        &self,
        col: usize,
        start_row: usize,
        end_row: usize,
        out: &mut Vec<u32>,
    ) {
        out.clear();
        out.extend(self.get_ones_in_column(col, start_row, end_row));
    }
    // Get a slice of columns from a row as Octets
    fn get_sub_row_as_octets(&self, row: usize, start_col: usize) -> BinaryOctetVec;

    // Returns a list of columns with non-zero values in the given row, starting with start_col
    fn query_non_zero_columns(&self, row: usize, start_col: usize) -> Vec<usize>;

    #[inline]
    fn query_non_zero_columns_into(&self, row: usize, start_col: usize, out: &mut Vec<usize>) {
        out.clear();
        out.extend(self.query_non_zero_columns(row, start_col));
    }
    fn get(&self, i: usize, j: usize) -> Octet;

    fn swap_rows(&mut self, i: usize, j: usize);

    // start_row_hint indicates that all preceding rows don't need to be swapped, because they have
    // identical values
    fn swap_columns(&mut self, i: usize, j: usize, start_row_hint: usize);

    fn enable_column_access_acceleration(&mut self);

    // After calling this method swap_columns() and other column oriented methods, may be much slower
    fn disable_column_access_acceleration(&mut self);

    // Hints that column i will not be swapped again, and is likely to become dense'ish
    fn hint_column_dense_and_frozen(&mut self, i: usize);

    // If start_col is non-zero, values left of start_col in dest row are undefined after this operation
    fn add_assign_rows(&mut self, dest: usize, src: usize, start_col: usize);

    fn resize(&mut self, new_height: usize, new_width: usize);
}

const WORD_WIDTH: usize = 64;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct DenseBinaryMatrix {
    height: usize,
    width: usize,
    // Values are bit-packed into u64
    elements: Vec<u64>,
}

impl DenseBinaryMatrix {
    // Returns (word in elements vec, and bit in word) for the given col
    fn bit_position(&self, row: usize, col: usize) -> (usize, usize) {
        return (
            row * self.row_word_width() + Self::word_offset(col),
            col % WORD_WIDTH,
        );
    }

    fn word_offset(col: usize) -> usize {
        col / WORD_WIDTH
    }

    // Number of words required per row
    fn row_word_width(&self) -> usize {
        self.width.div_ceil(WORD_WIDTH)
    }

    // Returns mask to select the given bit in a word
    pub fn select_mask(bit: usize) -> u64 {
        1u64 << (bit as u64)
    }

    // Select the bit and all bits to the left
    fn select_bit_and_all_left_mask(bit: usize) -> u64 {
        !DenseBinaryMatrix::select_all_right_of_mask(bit)
    }

    // Select all bits right of the given bit
    fn select_all_right_of_mask(bit: usize) -> u64 {
        let mask = DenseBinaryMatrix::select_mask(bit);
        // Subtract one to convert e.g. 0100 -> 0011
        mask - 1
    }

    fn clear_bit(word: &mut u64, bit: usize) {
        *word &= !DenseBinaryMatrix::select_mask(bit);
    }

    fn set_bit(word: &mut u64, bit: usize) {
        *word |= DenseBinaryMatrix::select_mask(bit);
    }
}

impl BinaryMatrix for DenseBinaryMatrix {
    fn new(height: usize, width: usize, _: usize) -> DenseBinaryMatrix {
        let elements = vec![0; height * (width + WORD_WIDTH - 1) / WORD_WIDTH];
        DenseBinaryMatrix {
            height,
            width,
            elements,
        }
    }

    fn set(&mut self, i: usize, j: usize, value: Octet) {
        let (word, bit) = self.bit_position(i, j);
        if value == Octet::zero() {
            DenseBinaryMatrix::clear_bit(&mut self.elements[word], bit);
        } else {
            DenseBinaryMatrix::set_bit(&mut self.elements[word], bit);
        }
    }

    fn height(&self) -> usize {
        self.height
    }

    fn width(&self) -> usize {
        self.width
    }

    fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();
        bytes += size_of::<Vec<u64>>();
        bytes += size_of::<u64>() * self.elements.len();

        bytes
    }

    fn count_ones(&self, row: usize, start_col: usize, end_col: usize) -> usize {
        let (start_word, start_bit) = self.bit_position(row, start_col);
        let (end_word, end_bit) = self.bit_position(row, end_col);
        // Handle case when there is only one word
        if start_word == end_word {
            let mut mask = DenseBinaryMatrix::select_bit_and_all_left_mask(start_bit);
            mask &= DenseBinaryMatrix::select_all_right_of_mask(end_bit);
            let bits = self.elements[start_word] & mask;
            return bits.count_ones() as usize;
        }

        let first_word_bits =
            self.elements[start_word] & DenseBinaryMatrix::select_bit_and_all_left_mask(start_bit);
        let mut ones = first_word_bits.count_ones();
        for word in (start_word + 1)..end_word {
            ones += self.elements[word].count_ones();
        }
        if end_bit > 0 {
            let bits =
                self.elements[end_word] & DenseBinaryMatrix::select_all_right_of_mask(end_bit);
            ones += bits.count_ones();
        }

        return ones as usize;
    }

    fn get_row_iter(&self, row: usize, start_col: usize, end_col: usize) -> OctetIter<'_> {
        let (first_word, first_bit) = self.bit_position(row, start_col);
        let (last_word, _) = self.bit_position(row, end_col);
        OctetIter::new_dense_binary(
            start_col,
            end_col,
            first_bit,
            &self.elements[first_word..=last_word],
        )
    }

    fn get_ones_in_column(&self, col: usize, start_row: usize, end_row: usize) -> Vec<u32> {
        let mut rows = Vec::with_capacity(end_row.saturating_sub(start_row));
        self.get_ones_in_column_into(col, start_row, end_row, &mut rows);
        rows
    }

    fn get_ones_in_column_into(
        &self,
        col: usize,
        start_row: usize,
        end_row: usize,
        out: &mut Vec<u32>,
    ) {
        out.clear();
        out.reserve(end_row.saturating_sub(start_row));
        for row in start_row..end_row {
            if self.get(row, col) == Octet::one() {
                out.push(row as u32);
            }
        }
    }

    fn get_sub_row_as_octets(&self, row: usize, start_col: usize) -> BinaryOctetVec {
        let mut result = vec![0; (self.width - start_col).div_ceil(BinaryOctetVec::WORD_WIDTH)];
        let mut word = result.len();
        let mut bit = 0;
        for col in (start_col..self.width).rev() {
            if bit == 0 {
                bit = BinaryOctetVec::WORD_WIDTH - 1;
                word -= 1;
            } else {
                bit -= 1;
            }
            if self.get(row, col) == Octet::one() {
                result[word] |= BinaryOctetVec::select_mask(bit);
            }
        }

        BinaryOctetVec::new(result, self.width - start_col)
    }

    fn query_non_zero_columns(&self, row: usize, start_col: usize) -> Vec<usize> {
        let mut cols = Vec::with_capacity(self.width.saturating_sub(start_col));
        self.query_non_zero_columns_into(row, start_col, &mut cols);
        cols
    }

    fn query_non_zero_columns_into(&self, row: usize, start_col: usize, out: &mut Vec<usize>) {
        out.clear();
        out.reserve(self.width.saturating_sub(start_col));
        for col in start_col..self.width {
            if self.get(row, col) != Octet::zero() {
                out.push(col);
            }
        }
    }

    fn get(&self, i: usize, j: usize) -> Octet {
        let (word, bit) = self.bit_position(i, j);
        if self.elements[word] & DenseBinaryMatrix::select_mask(bit) == 0 {
            return Octet::zero();
        } else {
            return Octet::one();
        }
    }

    fn swap_rows(&mut self, i: usize, j: usize) {
        let (row_i, _) = self.bit_position(i, 0);
        let (row_j, _) = self.bit_position(j, 0);
        for k in 0..self.row_word_width() {
            self.elements.swap(row_i + k, row_j + k);
        }
    }

    fn swap_columns(&mut self, i: usize, j: usize, start_row_hint: usize) {
        // Lookup for row zero to get the base word offset
        let (word_i, bit_i) = self.bit_position(0, i);
        let (word_j, bit_j) = self.bit_position(0, j);
        let unset_i = !DenseBinaryMatrix::select_mask(bit_i);
        let unset_j = !DenseBinaryMatrix::select_mask(bit_j);
        let bit_i = DenseBinaryMatrix::select_mask(bit_i);
        let bit_j = DenseBinaryMatrix::select_mask(bit_j);
        let row_width = self.row_word_width();
        for row in start_row_hint..self.height {
            let i_set = self.elements[row * row_width + word_i] & bit_i != 0;
            if self.elements[row * row_width + word_j] & bit_j == 0 {
                self.elements[row * row_width + word_i] &= unset_i;
            } else {
                self.elements[row * row_width + word_i] |= bit_i;
            }
            if i_set {
                self.elements[row * row_width + word_j] |= bit_j;
            } else {
                self.elements[row * row_width + word_j] &= unset_j;
            }
        }
    }

    fn enable_column_access_acceleration(&mut self) {
        // No-op
    }

    fn disable_column_access_acceleration(&mut self) {
        // No-op
    }

    fn hint_column_dense_and_frozen(&mut self, _: usize) {
        // No-op
    }

    fn add_assign_rows(&mut self, dest: usize, src: usize, _start_col: usize) {
        assert_ne!(dest, src);
        let (dest_word, _) = self.bit_position(dest, 0);
        let (src_word, _) = self.bit_position(src, 0);
        let row_width = self.row_word_width();
        let (dest_row, temp_row) =
            get_both_ranges(&mut self.elements, dest_word, src_word, row_width);
        add_assign_binary(dest_row, temp_row);
    }

    fn resize(&mut self, new_height: usize, new_width: usize) {
        assert!(new_height <= self.height);
        assert!(new_width <= self.width);
        let old_row_width = self.row_word_width();
        self.height = new_height;
        self.width = new_width;
        let new_row_width = self.row_word_width();
        let words_to_remove = old_row_width - new_row_width;
        if words_to_remove > 0 {
            let mut src = 0;
            let mut dest = 0;
            while dest < new_height * new_row_width {
                self.elements[dest] = self.elements[src];
                src += 1;
                dest += 1;
                if dest % new_row_width == 0 {
                    // After copying each row, skip over the elements being dropped
                    src += words_to_remove;
                }
            }
            assert_eq!(src, new_height * old_row_width);
        }
        self.elements.truncate(new_height * self.row_word_width());
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::matrix::{BinaryMatrix, DenseBinaryMatrix};
    use crate::octet::Octet;
    use crate::sparse_matrix::SparseBinaryMatrix;

    fn rand_dense_and_sparse(size: usize) -> (DenseBinaryMatrix, SparseBinaryMatrix) {
        let mut dense = DenseBinaryMatrix::new(size, size, 0);
        let mut sparse = SparseBinaryMatrix::new(size, size, 1);
        // Generate 50% filled random matrices
        for _ in 0..(size * size / 2) {
            let i = rand::rng().random_range(0..size);
            let j = rand::rng().random_range(0..size);
            let value = rand::rng().random_range(0..2);
            dense.set(i, j, Octet::new(value));
            sparse.set(i, j, Octet::new(value));
        }

        return (dense, sparse);
    }

    fn assert_matrices_eq<T: BinaryMatrix, U: BinaryMatrix>(matrix1: &T, matrix2: &U) {
        assert_eq!(matrix1.height(), matrix2.height());
        assert_eq!(matrix1.width(), matrix2.width());
        for i in 0..matrix1.height() {
            for j in 0..matrix1.width() {
                assert_eq!(
                    matrix1.get(i, j),
                    matrix2.get(i, j),
                    "Matrices are not equal at row={i} col={j}"
                );
            }
        }
    }

    #[test]
    fn row_iter() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (dense, sparse) = rand_dense_and_sparse(8);
        for row in 0..dense.height() {
            let start_col = rand::rng().random_range(0..(dense.width() - 2));
            let end_col = rand::rng().random_range((start_col + 1)..dense.width());
            let mut dense_iter = dense.get_row_iter(row, start_col, end_col);
            let mut sparse_iter = sparse.get_row_iter(row, start_col, end_col);
            for col in start_col..end_col {
                assert_eq!(dense.get(row, col), sparse.get(row, col));
                assert_eq!((col, dense.get(row, col)), dense_iter.next().unwrap());
                // Sparse iter is not required to return zeros
                if sparse.get(row, col) != Octet::zero() {
                    assert_eq!((col, sparse.get(row, col)), sparse_iter.next().unwrap());
                }
            }
            assert!(dense_iter.next().is_none());
            assert!(sparse_iter.next().is_none());
        }
    }

    #[test]
    fn swap_rows() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (mut dense, mut sparse) = rand_dense_and_sparse(8);
        dense.swap_rows(0, 4);
        dense.swap_rows(1, 6);
        dense.swap_rows(1, 7);
        sparse.swap_rows(0, 4);
        sparse.swap_rows(1, 6);
        sparse.swap_rows(1, 7);
        assert_matrices_eq(&dense, &sparse);
    }

    #[test]
    fn swap_columns() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (mut dense, mut sparse) = rand_dense_and_sparse(8);
        dense.swap_columns(0, 4, 0);
        dense.swap_columns(1, 6, 0);
        dense.swap_columns(1, 1, 0);
        sparse.swap_columns(0, 4, 0);
        sparse.swap_columns(1, 6, 0);
        sparse.swap_columns(1, 1, 0);
        assert_matrices_eq(&dense, &sparse);
    }

    #[test]
    fn count_ones() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (dense, sparse) = rand_dense_and_sparse(8);
        assert_eq!(dense.count_ones(0, 0, 5), sparse.count_ones(0, 0, 5));
        assert_eq!(dense.count_ones(2, 2, 6), sparse.count_ones(2, 2, 6));
        assert_eq!(dense.count_ones(3, 1, 2), sparse.count_ones(3, 1, 2));
    }

    #[test]
    fn fma_rows() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (mut dense, mut sparse) = rand_dense_and_sparse(8);
        dense.add_assign_rows(0, 1, 0);
        dense.add_assign_rows(0, 2, 0);
        dense.add_assign_rows(2, 1, 0);
        sparse.add_assign_rows(0, 1, 0);
        sparse.add_assign_rows(0, 2, 0);
        sparse.add_assign_rows(2, 1, 0);
        assert_matrices_eq(&dense, &sparse);
    }

    #[test]
    fn resize() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (mut dense, mut sparse) = rand_dense_and_sparse(8);
        dense.disable_column_access_acceleration();
        sparse.disable_column_access_acceleration();
        dense.resize(5, 5);
        sparse.resize(5, 5);
        assert_matrices_eq(&dense, &sparse);
    }

    #[test]
    fn hint_column_dense_and_frozen() {
        // rand_dense_and_sparse uses set(), so just check that it works
        let (dense, mut sparse) = rand_dense_and_sparse(8);
        sparse.enable_column_access_acceleration();
        sparse.hint_column_dense_and_frozen(6);
        sparse.hint_column_dense_and_frozen(5);
        assert_matrices_eq(&dense, &sparse);
    }

    #[test]
    fn dense_storage_math() {
        let size = 128;
        let (mut dense, mut sparse) = rand_dense_and_sparse(size);
        sparse.enable_column_access_acceleration();
        for i in (0..(size - 1)).rev() {
            sparse.hint_column_dense_and_frozen(i);
            assert_matrices_eq(&dense, &sparse);
        }
        assert_matrices_eq(&dense, &sparse);
        sparse.disable_column_access_acceleration();
        for _ in 0..1000 {
            let i = rand::rng().random_range(0..size);
            let mut j = rand::rng().random_range(0..size);
            while j == i {
                j = rand::rng().random_range(0..size);
            }
            dense.add_assign_rows(i, j, 0);
            sparse.add_assign_rows(i, j, 0);
        }
        assert_matrices_eq(&dense, &sparse);
    }
}
