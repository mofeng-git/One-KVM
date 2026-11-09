#[cfg(feature = "std")]
use std::{mem::size_of, vec::Vec};

#[cfg(not(feature = "std"))]
use core::mem::size_of;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::arraymap::{ImmutableListMap, ImmutableListMapBuilder};
use crate::iterators::OctetIter;
use crate::matrix::BinaryMatrix;
use crate::octet::Octet;
use crate::octets::BinaryOctetVec;
use crate::sparse_vec::SparseBinaryVec;
use crate::util::get_both_indices;

// Stores a matrix in sparse representation, with an optional dense block for the right most columns
// The logical storage is as follows:
// |---------------------------------------|
// |                          | (optional) |
// |      sparse rows         | dense      |
// |                          | columns    |
// |---------------------------------------|
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct SparseBinaryMatrix {
    height: usize,
    width: usize,
    sparse_elements: Vec<SparseBinaryVec>,
    // Note these are stored right aligned, so that the right most element is always at
    // dense_elements[x] & (1 << 63)
    dense_elements: Vec<u64>,
    // Columnar storage of values. Only stores rows that have a 1-valued entry in the given column
    sparse_columnar_values: Option<ImmutableListMap>,
    // Mapping of logical row numbers to index in sparse_elements, dense_elements, and sparse_column_index
    logical_row_to_physical: Vec<u32>,
    physical_row_to_logical: Vec<u32>,
    logical_col_to_physical: Vec<u16>,
    physical_col_to_logical: Vec<u16>,
    column_index_disabled: bool,
    // Only include for debug to avoid taking up extra memory in the cache
    #[cfg(debug_assertions)]
    debug_indexed_column_valid: Vec<bool>,
    num_dense_columns: usize,
}

const WORD_WIDTH: usize = 64;

impl SparseBinaryMatrix {
    #[cfg(debug_assertions)]
    fn verify(&self) {
        if self.column_index_disabled {
            return;
        }
        let columns = self.sparse_columnar_values.as_ref().unwrap();
        for row in 0..self.height {
            for (col, value) in self.sparse_elements[row].keys_values() {
                if value != Octet::zero() {
                    debug_assert!(columns.get(col as u16).contains(&(row as u32)));
                }
            }
        }
    }

    // Convert a logical col index to the bit index in the dense columns
    fn logical_col_to_dense_col(&self, col: usize) -> usize {
        assert!(col >= self.width - self.num_dense_columns);
        col - (self.width - self.num_dense_columns)
    }

    // Returns (word in elements vec, and bit in word) for the given col
    fn bit_position(&self, row: usize, col: usize) -> (usize, usize) {
        return (
            row * self.row_word_width() + self.word_offset(col),
            (self.left_padding_bits() + col) % WORD_WIDTH,
        );
    }

    // Number of words required per row
    fn row_word_width(&self) -> usize {
        self.num_dense_columns.div_ceil(WORD_WIDTH)
    }

    // Returns the number of unused bits on the left of each row
    fn left_padding_bits(&self) -> usize {
        (WORD_WIDTH - (self.num_dense_columns % WORD_WIDTH)) % WORD_WIDTH
    }

    // Return the word in which bit lives, offset from the first for a row
    fn word_offset(&self, bit: usize) -> usize {
        (self.left_padding_bits() + bit) / WORD_WIDTH
    }

    // Returns mask to select the given bit in a word
    fn select_mask(bit: usize) -> u64 {
        1u64 << (bit as u64)
    }

    fn clear_bit(word: &mut u64, bit: usize) {
        *word &= !SparseBinaryMatrix::select_mask(bit);
    }

    fn set_bit(word: &mut u64, bit: usize) {
        *word |= SparseBinaryMatrix::select_mask(bit);
    }
}

impl BinaryMatrix for SparseBinaryMatrix {
    fn new(height: usize, width: usize, trailing_dense_column_hint: usize) -> SparseBinaryMatrix {
        debug_assert!(height < 16777216);
        // Matrix width can never exceed maximum L
        debug_assert!(width < 65536);
        let mut col_mapping = vec![0; width];
        let elements = vec![SparseBinaryVec::with_capacity(10); height];
        let mut row_mapping = vec![0; height];
        #[allow(clippy::needless_range_loop)]
        for i in 0..height {
            row_mapping[i] = i as u32;
        }
        #[allow(clippy::needless_range_loop)]
        for i in 0..width {
            col_mapping[i] = i as u16;
        }
        let dense_elements = if trailing_dense_column_hint > 0 {
            vec![0; height * ((trailing_dense_column_hint - 1) / WORD_WIDTH + 1)]
        } else {
            vec![]
        };
        SparseBinaryMatrix {
            height,
            width,
            sparse_elements: elements,
            dense_elements,
            sparse_columnar_values: None,
            logical_row_to_physical: row_mapping.clone(),
            physical_row_to_logical: row_mapping,
            logical_col_to_physical: col_mapping.clone(),
            physical_col_to_logical: col_mapping,
            column_index_disabled: true,
            num_dense_columns: trailing_dense_column_hint,
            #[cfg(debug_assertions)]
            debug_indexed_column_valid: vec![true; width],
        }
    }

    fn set(&mut self, i: usize, j: usize, value: Octet) {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        if self.width - j <= self.num_dense_columns {
            let (word, bit) = self.bit_position(physical_i, self.logical_col_to_dense_col(j));
            if value == Octet::zero() {
                SparseBinaryMatrix::clear_bit(&mut self.dense_elements[word], bit);
            } else {
                SparseBinaryMatrix::set_bit(&mut self.dense_elements[word], bit);
            }
        } else {
            self.sparse_elements[physical_i].insert(physical_j, value);
            assert!(self.column_index_disabled);
        }
    }

    fn height(&self) -> usize {
        self.height
    }

    fn width(&self) -> usize {
        self.width
    }

    fn count_ones(&self, row: usize, start_col: usize, end_col: usize) -> usize {
        if end_col > self.width - self.num_dense_columns {
            unimplemented!(
                "It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A"
            );
        }
        let mut ones = 0;
        let physical_row = self.logical_row_to_physical[row] as usize;
        for (physical_col, value) in self.sparse_elements[physical_row].keys_values() {
            let col = self.physical_col_to_logical[physical_col] as usize;
            if col >= start_col && col < end_col && value == Octet::one() {
                ones += 1;
            }
        }
        return ones;
    }

    fn get_sub_row_as_octets(&self, row: usize, start_col: usize) -> BinaryOctetVec {
        let first_dense_column = self.width - self.num_dense_columns;
        assert_eq!(start_col, first_dense_column);
        // The following implementation is equivalent to .map(|x| self.get(row, x))
        // but this implementation optimizes for sequential access and avoids all the
        // extra bit index math
        let physical_row = self.logical_row_to_physical[row] as usize;
        let (first_word, _) =
            self.bit_position(physical_row, self.logical_col_to_dense_col(start_col));
        let last_word = first_word + self.row_word_width();

        BinaryOctetVec::new(
            self.dense_elements[first_word..last_word].to_vec(),
            self.num_dense_columns,
        )
    }

    fn query_non_zero_columns(&self, row: usize, start_col: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(self.num_dense_columns);
        self.query_non_zero_columns_into(row, start_col, &mut result);
        result
    }

    fn query_non_zero_columns_into(&self, row: usize, start_col: usize, out: &mut Vec<usize>) {
        // The following implementation is equivalent to .filter(|x| self.get(row, x) != Octet::zero())
        // but this implementation optimizes for sequential access and avoids all the
        // extra bit index math
        assert_eq!(start_col, self.width - self.num_dense_columns);
        out.clear();
        out.reserve(self.num_dense_columns);
        let physical_row = self.logical_row_to_physical[row] as usize;
        let (mut word, bit) =
            self.bit_position(physical_row, self.logical_col_to_dense_col(start_col));
        let mut col = start_col;
        // Process the first word, which may not be entirely filled, due to left zero padding
        // Because of the assert that start_col is always the first dense column, the first one
        // must be the column we're looking for, so they're no need to zero out columns left of it.
        let mut block = self.dense_elements[word];
        while block.trailing_zeros() < WORD_WIDTH as u32 {
            out.push(col + block.trailing_zeros() as usize - bit);
            block &= !(SparseBinaryMatrix::select_mask(block.trailing_zeros() as usize));
        }
        col += WORD_WIDTH - bit;
        word += 1;

        while col < self.width() {
            let mut block = self.dense_elements[word];
            // process the whole word in one shot to improve efficiency
            while block.trailing_zeros() < WORD_WIDTH as u32 {
                out.push(col + block.trailing_zeros() as usize);
                block &= !(SparseBinaryMatrix::select_mask(block.trailing_zeros() as usize));
            }
            col += WORD_WIDTH;
            word += 1;
        }
    }

    fn get(&self, i: usize, j: usize) -> Octet {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        if self.width - j <= self.num_dense_columns {
            let (word, bit) = self.bit_position(physical_i, self.logical_col_to_dense_col(j));
            if self.dense_elements[word] & SparseBinaryMatrix::select_mask(bit) == 0 {
                return Octet::zero();
            } else {
                return Octet::one();
            }
        } else {
            return self.sparse_elements[physical_i]
                .get(physical_j)
                .unwrap_or_else(Octet::zero);
        }
    }

    fn get_row_iter(&self, row: usize, start_col: usize, end_col: usize) -> OctetIter<'_> {
        if end_col > self.width - self.num_dense_columns {
            unimplemented!(
                "It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A"
            );
        }
        let physical_row = self.logical_row_to_physical[row] as usize;
        let sparse_elements = &self.sparse_elements[physical_row];
        OctetIter::new_sparse(
            start_col,
            end_col,
            sparse_elements,
            &self.physical_col_to_logical,
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
        assert!(!self.column_index_disabled);
        #[cfg(debug_assertions)]
        debug_assert!(self.debug_indexed_column_valid[col]);
        let physical_col = self.logical_col_to_physical[col];
        out.clear();
        out.reserve(end_row.saturating_sub(start_row));
        for physical_row in self
            .sparse_columnar_values
            .as_ref()
            .unwrap()
            .get(physical_col)
        {
            let logical_row = self.physical_row_to_logical[*physical_row as usize];
            if start_row <= logical_row as usize && logical_row < end_row as u32 {
                out.push(logical_row);
            }
        }
    }

    fn swap_rows(&mut self, i: usize, j: usize) {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_row_to_physical[j] as usize;
        self.logical_row_to_physical.swap(i, j);
        self.physical_row_to_logical.swap(physical_i, physical_j);
    }

    fn swap_columns(&mut self, i: usize, j: usize, _: usize) {
        if j >= self.width - self.num_dense_columns {
            unimplemented!(
                "It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A"
            );
        }

        #[cfg(debug_assertions)]
        self.debug_indexed_column_valid.swap(i, j);

        let physical_i = self.logical_col_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        self.logical_col_to_physical.swap(i, j);
        self.physical_col_to_logical.swap(physical_i, physical_j);
    }

    fn enable_column_access_acceleration(&mut self) {
        self.column_index_disabled = false;
        let mut builder = ImmutableListMapBuilder::new(self.height);
        for (physical_row, elements) in self.sparse_elements.iter().enumerate() {
            for (physical_col, _) in elements.keys_values() {
                builder.add(physical_col as u16, physical_row as u32);
            }
        }
        self.sparse_columnar_values = Some(builder.build());
    }

    fn disable_column_access_acceleration(&mut self) {
        self.column_index_disabled = true;
        self.sparse_columnar_values = None;
    }

    fn hint_column_dense_and_frozen(&mut self, i: usize) {
        assert_eq!(
            self.width - self.num_dense_columns - 1,
            i,
            "Can only freeze the last sparse column"
        );
        assert!(!self.column_index_disabled);
        self.num_dense_columns += 1;
        let (last_word, _) = self.bit_position(self.height - 1, self.num_dense_columns - 1);
        // If this is in a new word
        if last_word >= self.dense_elements.len() {
            // Append a new set of words
            let mut src = self.dense_elements.len();
            self.dense_elements.extend(vec![0; self.height]);
            let mut dest = self.dense_elements.len();
            // Re-space the elements, so that each row has an empty word
            while src > 0 {
                src -= 1;
                dest -= 1;
                self.dense_elements[dest] = self.dense_elements[src];
                if dest % self.row_word_width() == 1 {
                    dest -= 1;
                    self.dense_elements[dest] = 0;
                }
            }
            assert_eq!(src, 0);
            assert_eq!(dest, 0);
        }
        let physical_i = self.logical_col_to_physical[i] as usize;
        for maybe_present_in_row in self
            .sparse_columnar_values
            .as_ref()
            .unwrap()
            .get(physical_i as u16)
        {
            let physical_row = *maybe_present_in_row as usize;
            if let Some(value) = self.sparse_elements[physical_row].remove(physical_i) {
                let (word, bit) = self.bit_position(physical_row, 0);
                if value == Octet::zero() {
                    SparseBinaryMatrix::clear_bit(&mut self.dense_elements[word], bit);
                } else {
                    SparseBinaryMatrix::set_bit(&mut self.dense_elements[word], bit);
                }
            }
        }
    }

    fn add_assign_rows(&mut self, dest: usize, src: usize, start_col: usize) {
        assert_ne!(dest, src);
        assert!(
            start_col == 0 || start_col == self.width - self.num_dense_columns,
            "start_col must be zero or at the beginning of the U matrix"
        );
        let physical_dest = self.logical_row_to_physical[dest] as usize;
        let physical_src = self.logical_row_to_physical[src] as usize;
        // First handle the dense columns
        if self.num_dense_columns > 0 {
            let (dest_word, _) = self.bit_position(physical_dest, 0);
            let (src_word, _) = self.bit_position(physical_src, 0);
            for word in 0..self.row_word_width() {
                self.dense_elements[dest_word + word] ^= self.dense_elements[src_word + word];
            }
        }

        if start_col == 0 {
            // Then the sparse columns
            let (dest_row, temp_row) =
                get_both_indices(&mut self.sparse_elements, physical_dest, physical_src);
            // This shouldn't be needed, because while column indexing is enabled in first phase,
            // columns are only eliminated one at a time in sparse section of matrix.
            assert!(self.column_index_disabled || temp_row.len() == 1);

            let column_added = dest_row.add_assign(temp_row);
            // This shouldn't be needed, because while column indexing is enabled in first phase,
            // columns are only removed.
            assert!(self.column_index_disabled || !column_added);

            #[cfg(debug_assertions)]
            {
                if !self.column_index_disabled {
                    let col = self.physical_col_to_logical[temp_row.get_by_raw_index(0).0];
                    self.debug_indexed_column_valid[col as usize] = false;
                }
            }
        }

        #[cfg(debug_assertions)]
        self.verify();
    }

    fn resize(&mut self, new_height: usize, new_width: usize) {
        assert!(new_height <= self.height);
        // Only support same width or removing all the dense columns
        let mut columns_to_remove = self.width - new_width;
        assert!(columns_to_remove == 0 || columns_to_remove >= self.num_dense_columns);
        if !self.column_index_disabled {
            unimplemented!(
                "Resize should only be used in phase 2, after column indexing is no longer needed"
            );
        }
        let mut new_sparse = vec![None; new_height];
        for i in (0..self.sparse_elements.len()).rev() {
            let logical_row = self.physical_row_to_logical[i] as usize;
            let sparse = self.sparse_elements.pop();
            if logical_row < new_height {
                new_sparse[logical_row] = sparse;
            }
        }

        if columns_to_remove == 0 && self.num_dense_columns > 0 {
            // TODO: optimize to not allocate this extra vec
            let mut new_dense = vec![0; new_height * self.row_word_width()];
            for logical_row in 0..new_height {
                let physical_row = self.logical_row_to_physical[logical_row] as usize;
                for word in 0..self.row_word_width() {
                    new_dense[logical_row * self.row_word_width() + word] =
                        self.dense_elements[physical_row * self.row_word_width() + word];
                }
            }
            self.dense_elements = new_dense;
        } else {
            columns_to_remove -= self.num_dense_columns;
            self.dense_elements.clear();
            self.num_dense_columns = 0;
        }

        self.logical_row_to_physical.truncate(new_height);
        self.physical_row_to_logical.truncate(new_height);
        for i in 0..new_height {
            self.logical_row_to_physical[i] = i as u32;
            self.physical_row_to_logical[i] = i as u32;
        }
        for row in new_sparse.drain(0..new_height) {
            self.sparse_elements.push(row.unwrap());
        }

        // Next remove sparse columns
        if columns_to_remove > 0 {
            let physical_to_logical = &self.physical_col_to_logical;
            for row in 0..self.sparse_elements.len() {
                self.sparse_elements[row]
                    .retain(|(col, _)| physical_to_logical[*col] < new_width as u16);
            }
        }

        self.height = new_height;
        self.width = new_width;

        #[cfg(debug_assertions)]
        self.verify();
    }

    fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();
        for x in self.sparse_elements.iter() {
            bytes += x.size_in_bytes();
        }
        bytes += size_of::<u64>() * self.dense_elements.len();
        if let Some(ref columns) = self.sparse_columnar_values {
            bytes += columns.size_in_bytes();
        }
        bytes += size_of::<u32>() * self.logical_row_to_physical.len();
        bytes += size_of::<u32>() * self.physical_row_to_logical.len();
        bytes += size_of::<u16>() * self.logical_col_to_physical.len();
        bytes += size_of::<u16>() * self.physical_col_to_logical.len();
        #[cfg(debug_assertions)]
        {
            bytes += size_of::<bool>() * self.debug_indexed_column_valid.len();
        }

        bytes
    }
}

#[cfg(test)]
mod tests {
    use crate::systematic_constants::{MAX_SOURCE_SYMBOLS_PER_BLOCK, num_intermediate_symbols};

    #[test]
    fn check_max_width_optimization() {
        // Check that the optimization of limiting matrix width to 2^16 is safe.
        // Matrix width will never exceed L
        assert!(num_intermediate_symbols(MAX_SOURCE_SYMBOLS_PER_BLOCK) < 65536);
    }
}
