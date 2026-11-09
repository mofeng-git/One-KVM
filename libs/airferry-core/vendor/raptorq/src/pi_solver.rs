#[cfg(feature = "std")]
use std::{mem, mem::size_of, vec::Vec};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use core::{mem, mem::size_of, u16};

use crate::arraymap::UndirectedGraph;
use crate::arraymap::{U16ArrayMap, U32VecMap};
use crate::graph::ConnectedComponentGraph;
use crate::matrix::BinaryMatrix;
use crate::octet::Octet;
use crate::octet_matrix::DenseOctetMatrix;
use crate::octets::BinaryOctetVec;
use crate::operation_vector::SymbolOps;
use crate::symbol_slab::SymbolSlab;
use crate::systematic_constants::num_hdpc_symbols;
use crate::systematic_constants::num_intermediate_symbols;
use crate::systematic_constants::num_ldpc_symbols;
use crate::systematic_constants::num_pi_symbols;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
enum RowOp {
    AddAssign { src: usize, dest: usize },
    Swap { row1: usize, row2: usize },
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
struct FirstPhaseRowSelectionStats {
    original_degree: U16ArrayMap,
    ones_per_row: U16ArrayMap,
    ones_histogram: U32VecMap,
    start_col: usize,
    end_col: usize,
    start_row: usize,
    rows_with_single_one: Vec<usize>,
    // Mapping from columns (graph nodes) to their connected component id for the r = 2 substep
    col_graph: ConnectedComponentGraph,
}

impl FirstPhaseRowSelectionStats {
    #[inline(never)]
    #[allow(non_snake_case)]
    pub fn new<T: BinaryMatrix>(
        matrix: &T,
        end_col: usize,
        end_row: usize,
    ) -> FirstPhaseRowSelectionStats {
        let mut result = FirstPhaseRowSelectionStats {
            original_degree: U16ArrayMap::new(0, 0),
            ones_per_row: U16ArrayMap::new(0, matrix.height()),
            ones_histogram: U32VecMap::new(0),
            start_col: 0,
            end_col,
            start_row: 0,
            rows_with_single_one: vec![],
            col_graph: ConnectedComponentGraph::new(end_col),
        };

        for row in 0..matrix.height() {
            let ones = matrix.count_ones(row, 0, end_col);
            result.ones_per_row.insert(row, ones as u16);
            result.ones_histogram.increment(ones);
            if ones == 1 {
                result.rows_with_single_one.push(row);
            }
        }
        // Original degree is the degree of each row before processing begins
        result.original_degree = result.ones_per_row.clone();

        result.rebuild_connected_components(0, end_row, matrix);

        result
    }

    #[allow(dead_code)]
    pub fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();

        bytes += self.original_degree.size_in_bytes();
        bytes += self.ones_per_row.size_in_bytes();
        bytes += self.ones_histogram.size_in_bytes();

        bytes
    }

    pub fn swap_rows(&mut self, i: usize, j: usize) {
        self.ones_per_row.swap(i, j);
        self.original_degree.swap(i, j);
        for row in self.rows_with_single_one.iter_mut() {
            if *row == i {
                *row = j;
            } else if *row == j {
                *row = i;
            }
        }
    }

    pub fn swap_columns(&mut self, i: usize, j: usize) {
        self.col_graph.swap(i, j);
    }

    // Update the connected component graph, by adding an edge (specified by row)
    fn add_graph_edge<T: BinaryMatrix>(
        &mut self,
        row: usize,
        matrix: &T,
        start_col: usize,
        end_col: usize,
    ) {
        let mut ones = [0; 2];
        let mut found = 0;
        for (col, value) in matrix.get_row_iter(row, start_col, end_col) {
            if value == Octet::one() {
                ones[found] = col;
                found += 1;
            }
            if found == 2 {
                break;
            }
        }
        assert_eq!(found, 2);

        self.col_graph.add_edge(ones[0], ones[1]);
    }

    fn remove_graph_edge<T: BinaryMatrix>(&mut self, _row: usize, _matrix: &T) {
        // No-op. Graph edges are only removed when eliminating an entire connected component.
        // The effected nodes (cols) will be swapped to the beginning or end of V.
        // Therefore there is no need to update the connected component graph
    }

    // Recompute all stored statistics for the given row
    pub fn recompute_row<T: BinaryMatrix>(&mut self, row: usize, matrix: &T) {
        let ones = matrix.count_ones(row, self.start_col, self.end_col);
        if let Some(index) = self.rows_with_single_one.iter().position(|x| *x == row) {
            self.rows_with_single_one.swap_remove(index);
        }
        if ones == 1 {
            self.rows_with_single_one.push(row);
        }
        self.ones_histogram
            .decrement(self.ones_per_row.get(row) as usize);
        self.ones_histogram.increment(ones);
        if self.ones_per_row.get(row) == 2 {
            self.remove_graph_edge(row, matrix);
        }
        self.ones_per_row.insert(row, ones as u16);
        if ones == 2 {
            self.add_graph_edge(row, matrix, self.start_col, self.end_col);
        }
    }

    // Set the valid columns, and recalculate statistics
    #[inline(never)]
    pub fn resize<T: BinaryMatrix>(
        &mut self,
        start_row: usize,
        end_row: usize,
        start_col: usize,
        end_col: usize,
        // Ones from start_row to end_row, i.e. matrix.get_ones_in_column(self.start_col, start_row, end_row)
        ones_in_start_col: &[u32],
        matrix: &T,
    ) {
        // Only shrinking is supported
        assert!(end_col <= self.end_col);
        assert_eq!(self.start_row, start_row - 1);
        assert_eq!(self.start_col, start_col - 1);

        // Remove this separately, since it's not part of ones_in_start_col
        if matrix.get(self.start_row, self.start_col) == Octet::one() {
            let row = self.start_row;
            self.ones_per_row.decrement(row);
            let ones = self.ones_per_row.get(row);
            if ones == 0 {
                if let Some(index) = self.rows_with_single_one.iter().position(|x| *x == row) {
                    self.rows_with_single_one.swap_remove(index);
                }
            } else if ones == 1 {
                self.remove_graph_edge(row, matrix);
            }
            self.ones_histogram.decrement((ones + 1) as usize);
            self.ones_histogram.increment(ones as usize);
        }

        let mut possible_new_graph_edges = vec![];
        for &row in ones_in_start_col {
            let row = row as usize;
            self.ones_per_row.decrement(row);
            let ones = self.ones_per_row.get(row);
            if ones == 0 {
                if let Some(index) = self.rows_with_single_one.iter().position(|x| *x == row) {
                    self.rows_with_single_one.swap_remove(index);
                }
            } else if ones == 1 {
                self.rows_with_single_one.push(row);
                self.remove_graph_edge(row, matrix);
            }
            if ones == 2 {
                possible_new_graph_edges.push(row);
            }
            self.ones_histogram.decrement((ones + 1) as usize);
            self.ones_histogram.increment(ones as usize);
        }

        self.col_graph.remove_node(start_col - 1);

        let mut ones_in_removed_col = Vec::new();
        for col in end_col..self.end_col {
            matrix.get_ones_in_column_into(col, self.start_row, end_row, &mut ones_in_removed_col);
            for row in ones_in_removed_col.iter().copied() {
                let row = row as usize;
                self.ones_per_row.decrement(row);
                let ones = self.ones_per_row.get(row);
                if ones == 0 {
                    if let Some(index) = self.rows_with_single_one.iter().position(|x| *x == row) {
                        self.rows_with_single_one.swap_remove(index);
                    }
                } else if ones == 1 {
                    self.rows_with_single_one.push(row);
                    self.remove_graph_edge(row, matrix);
                }
                if ones == 2 {
                    possible_new_graph_edges.push(row);
                }
                self.ones_histogram.decrement((ones + 1) as usize);
                self.ones_histogram.increment(ones as usize);
            }
            self.col_graph.remove_node(col);
        }

        for row in possible_new_graph_edges {
            if self.ones_per_row.get(row) == 2 {
                self.add_graph_edge(row, matrix, start_col, end_col);
            }
        }

        self.start_col = start_col;
        self.end_col = end_col;
        self.start_row = start_row;
    }

    #[inline(never)]
    fn first_phase_graph_substep_build_adjacency<T: BinaryMatrix>(
        &self,
        start_row: usize,
        end_row: usize,
        matrix: &T,
    ) -> UndirectedGraph {
        let mut graph = UndirectedGraph::with_capacity(
            self.start_col as u16,
            self.end_col as u16,
            end_row - start_row,
        );

        for row in start_row..end_row {
            if self.ones_per_row.get(row) != 2 {
                continue;
            }
            let mut ones = [0; 2];
            let mut found = 0;
            for (col, value) in matrix.get_row_iter(row, self.start_col, self.end_col) {
                // "The following graph defined by the structure of V is used in determining which
                // row of A is chosen. The columns that intersect V are the nodes in the graph,
                // and the rows that have exactly 2 nonzero entries in V and are not HDPC rows
                // are the edges of the graph that connect the two columns (nodes) in the positions
                // of the two ones."
                // This part of the matrix is over GF(2), so "nonzero entries" is equivalent to "ones"
                if value == Octet::one() {
                    ones[found] = col;
                    found += 1;
                }
                if found == 2 {
                    break;
                }
            }
            assert_eq!(found, 2);
            graph.add_edge(ones[0] as u16, ones[1] as u16);
        }
        graph.build();
        return graph;
    }

    #[inline(never)]
    fn rebuild_connected_components<T: BinaryMatrix>(
        &mut self,
        start_row: usize,
        end_row: usize,
        matrix: &T,
    ) {
        // Reset connected component structures
        self.col_graph.reset();

        let graph = self.first_phase_graph_substep_build_adjacency(start_row, end_row, matrix);

        let mut node_queue = Vec::with_capacity(10);
        for key in graph.nodes() {
            let connected_component_id = self.col_graph.create_connected_component();
            // Pick arbitrary node (column) to start
            node_queue.clear();
            node_queue.push(key);
            while let Some(node) = node_queue.pop() {
                if self.col_graph.contains(node as usize) {
                    continue;
                }
                self.col_graph
                    .add_node(node as usize, connected_component_id);
                for next_node in graph.get_adjacent_nodes(node) {
                    node_queue.push(next_node);
                }
            }
        }
    }

    #[inline(never)]
    fn first_phase_graph_substep<T: BinaryMatrix>(
        &self,
        start_row: usize,
        end_row: usize,
        matrix: &T,
        ones_in_column: &mut Vec<u32>,
    ) -> usize {
        // Find a node (col) in the largest connected component
        let node = self
            .col_graph
            .get_node_in_largest_connected_component(self.start_col, self.end_col);

        // Find a row with two ones in the given column
        matrix.get_ones_in_column_into(node, start_row, end_row, ones_in_column);
        for row in ones_in_column.iter().copied() {
            let row = row as usize;
            if self.ones_per_row.get(row) == 2 {
                return row;
            }
        }
        unreachable!();
    }

    #[inline(never)]
    fn first_phase_original_degree_substep(
        &self,
        start_row: usize,
        end_row: usize,
        r: usize,
    ) -> usize {
        // There's no need for special handling of HDPC rows, since Errata 2 guarantees we won't
        // select any, and they're excluded in the first_phase solver
        let mut chosen = None;
        let mut chosen_original_degree = u16::MAX;
        // Fast path for r=1, since this is super common
        if r == 1 {
            assert_ne!(0, self.rows_with_single_one.len());
            for &row in self.rows_with_single_one.iter() {
                let row_original_degree = self.original_degree.get(row);
                if row_original_degree < chosen_original_degree {
                    chosen = Some(row);
                    chosen_original_degree = row_original_degree;
                }
            }
        } else {
            for row in start_row..end_row {
                let ones = self.ones_per_row.get(row);
                let row_original_degree = self.original_degree.get(row);
                if ones as usize == r && row_original_degree < chosen_original_degree {
                    chosen = Some(row);
                    chosen_original_degree = row_original_degree;
                }
            }
        }
        return chosen.unwrap();
    }

    // Verify there there are no non-HPDC rows with exactly two non-zero entries, greater than one
    #[inline(never)]
    #[cfg(debug_assertions)]
    fn first_phase_graph_substep_verify(&self, start_row: usize, end_row: usize) {
        for row in start_row..end_row {
            if self.ones_per_row.get(row) == 2 {
                return;
            }
        }
        unreachable!("A row with 2 ones must exist given Errata 8");
    }

    // Helper method for decoder phase 1
    // selects from [start_row, end_row) reading [start_col, end_col)
    // Returns (the chosen row, and "r" number of non-zero values the row has)
    pub fn first_phase_selection<T: BinaryMatrix>(
        &self,
        start_row: usize,
        end_row: usize,
        matrix: &T,
        ones_in_column: &mut Vec<u32>,
    ) -> (Option<usize>, Option<usize>) {
        let mut r = None;
        for i in 1..=(self.end_col - self.start_col) {
            if self.ones_histogram.get(i) > 0 {
                r = Some(i);
                break;
            }
        }

        if r.is_none() {
            return (None, None);
        }

        if r.unwrap() == 2 {
            // Paragraph starting "If r = 2 and there is no row with exactly 2 ones in V" can
            // be ignored due to Errata 8.

            // See paragraph starting "If r = 2 and there is a row with exactly 2 ones in V..."
            #[cfg(debug_assertions)]
            self.first_phase_graph_substep_verify(start_row, end_row);
            let row = self.first_phase_graph_substep(start_row, end_row, matrix, ones_in_column);
            return (Some(row), r);
        } else {
            let row = self.first_phase_original_degree_substep(start_row, end_row, r.unwrap());
            return (Some(row), r);
        }
    }
}

// See section 5.4.2.1
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct IntermediateSymbolDecoder<T: BinaryMatrix> {
    A: T,
    // If present, these are treated as replacing the last rows of A
    // Errata 3 guarantees that these do not need to be included in X
    A_hdpc_rows: Option<DenseOctetMatrix>,
    // Due to Errata 9 & 10 we only store the X matrix for debug builds to verify the results,
    // since it's not actually needed
    #[cfg(debug_assertions)]
    X: T,
    D: SymbolSlab,
    c: Vec<usize>,
    d: Vec<usize>,
    i: usize,
    u: usize,
    L: usize,
    // Operations on D are deferred to the end of the codec to improve cache hits
    deferred_D_ops: Vec<SymbolOps>,
    num_source_symbols: u32,
    debug_symbol_mul_ops: u32,
    debug_symbol_add_ops: u32,
    debug_symbol_mul_ops_by_phase: Vec<u32>,
    debug_symbol_add_ops_by_phase: Vec<u32>,
}

#[allow(non_snake_case)]
impl<T: BinaryMatrix> IntermediateSymbolDecoder<T> {
    pub fn new(
        matrix: T,
        hdpc_rows: DenseOctetMatrix,
        symbols: SymbolSlab,
        num_source_symbols: u32,
    ) -> IntermediateSymbolDecoder<T> {
        assert!(matrix.width() <= symbols.len());
        assert_eq!(matrix.height(), symbols.len());
        let mut c = Vec::with_capacity(matrix.width());
        let mut d = Vec::with_capacity(symbols.len());
        for i in 0..matrix.width() {
            c.push(i);
        }
        for i in 0..symbols.len() {
            d.push(i);
        }

        let intermediate_symbols = num_intermediate_symbols(num_source_symbols) as usize;

        let num_rows = matrix.height();

        let pi_symbols = num_pi_symbols(num_source_symbols) as usize;
        #[cfg(debug_assertions)]
        let mut X = matrix.clone();
        // Drop the PI symbols, since they will never be accessed in X. X will be resized to
        // i-by-i in the second phase.
        #[cfg(debug_assertions)]
        X.resize(X.height(), X.width() - pi_symbols);

        let mut A = matrix;
        A.enable_column_access_acceleration();

        let mut temp = IntermediateSymbolDecoder {
            A,
            A_hdpc_rows: None,
            #[cfg(debug_assertions)]
            X,
            D: symbols,
            c,
            d,
            i: 0,
            u: pi_symbols,
            L: intermediate_symbols,
            deferred_D_ops: Vec::with_capacity(70 * intermediate_symbols),
            num_source_symbols,
            debug_symbol_mul_ops: 0,
            debug_symbol_add_ops: 0,
            debug_symbol_mul_ops_by_phase: vec![0; 5],
            debug_symbol_add_ops_by_phase: vec![0; 5],
        };

        // Swap the HDPC rows, so that they're the last in the matrix
        let S = num_ldpc_symbols(num_source_symbols) as usize;
        let H = num_hdpc_symbols(num_source_symbols) as usize;
        // See section 5.3.3.4.2, Figure 5.
        for i in 0..H {
            temp.swap_rows(S + i, num_rows - H + i);
            #[cfg(debug_assertions)]
            temp.X.swap_rows(S + i, num_rows - H + i);
        }

        temp.A_hdpc_rows = Some(hdpc_rows);

        temp
    }

    /// Creates a solver without HDPC rows.
    /// Used when decoding with sufficient overhead to solve the system in GF(2) only.
    /// The constraint matrix must NOT contain HDPC rows (G_ENC starts at row S).
    pub fn new_no_hdpc(
        matrix: T,
        symbols: SymbolSlab,
        num_source_symbols: u32,
    ) -> IntermediateSymbolDecoder<T> {
        assert!(matrix.width() <= symbols.len());
        assert_eq!(matrix.height(), symbols.len());
        let mut c = Vec::with_capacity(matrix.width());
        let mut d = Vec::with_capacity(symbols.len());
        for i in 0..matrix.width() {
            c.push(i);
        }
        for i in 0..symbols.len() {
            d.push(i);
        }

        let intermediate_symbols = num_intermediate_symbols(num_source_symbols) as usize;
        let pi_symbols = num_pi_symbols(num_source_symbols) as usize;
        #[cfg(debug_assertions)]
        let mut X = matrix.clone();
        #[cfg(debug_assertions)]
        X.resize(X.height(), X.width() - pi_symbols);

        let mut A = matrix;
        A.enable_column_access_acceleration();

        IntermediateSymbolDecoder {
            A,
            A_hdpc_rows: None,
            #[cfg(debug_assertions)]
            X,
            D: symbols,
            c,
            d,
            i: 0,
            u: pi_symbols,
            L: intermediate_symbols,
            deferred_D_ops: Vec::with_capacity(70 * intermediate_symbols),
            num_source_symbols,
            debug_symbol_mul_ops: 0,
            debug_symbol_add_ops: 0,
            debug_symbol_mul_ops_by_phase: vec![0; 5],
            debug_symbol_add_ops_by_phase: vec![0; 5],
        }
        // No HDPC row swapping needed — matrix has no HDPC rows
    }

    #[inline(never)]
    fn apply_deferred_symbol_ops(&mut self) {
        for op in self.deferred_D_ops.iter() {
            match op {
                SymbolOps::AddAssign { dest, src } => {
                    self.D.add_assign(*dest, *src);
                }
                SymbolOps::MulAssign { dest, scalar } => {
                    self.D.mulassign_scalar(*dest, scalar);
                }
                SymbolOps::FMA { dest, src, scalar } => {
                    self.D.fma(*dest, *src, scalar);
                }
                SymbolOps::Reorder { order: _order } => {}
            }
        }
    }

    // Returns true iff all elements in A between [start_row, end_row)
    // and [start_column, end_column) are zero
    #[cfg(debug_assertions)]
    fn all_zeroes(
        &self,
        start_row: usize,
        end_row: usize,
        start_column: usize,
        end_column: usize,
    ) -> bool {
        for row in start_row..end_row {
            for column in start_column..end_column {
                if self.get_A_value(row, column) != Octet::zero() {
                    return false;
                }
            }
        }
        return true;
    }

    #[cfg(debug_assertions)]
    fn get_A_value(&self, row: usize, col: usize) -> Octet {
        if let Some(hdpc) = self
            .A_hdpc_rows
            .as_ref()
            .filter(|hdpc| row >= self.A.height() - hdpc.height())
        {
            return hdpc.get(row - (self.A.height() - hdpc.height()), col);
        }
        return self.A.get(row, col);
    }

    // Performs the column swapping substep of first phase, after the row has been chosen
    #[inline(never)]
    fn first_phase_swap_columns_substep(
        &mut self,
        r: usize,
        selection_helper: &mut FirstPhaseRowSelectionStats,
    ) {
        // Fast path when r == 1, since this is very common
        if r == 1 {
            // self.i will never reference an HDPC row, so can ignore self.A_hdpc_rows
            // because of Errata 2.
            let col = self
                .A
                .get_row_iter(self.i, self.i, self.A.width() - self.u)
                .find_map(|(col, value)| {
                    if value != Octet::zero() {
                        Some(col)
                    } else {
                        None
                    }
                })
                .unwrap();
            // No need to swap the first i rows, as they are all zero (see submatrix above V)
            self.swap_columns(self.i, col, self.i);
            selection_helper.swap_columns(self.i, col);
            // Also apply to X
            #[cfg(debug_assertions)]
            self.X.swap_columns(self.i, col, 0);
            return;
        }

        let mut remaining_swaps = r;
        let mut found_first = self.A.get(self.i, self.i) == Octet::one();
        // self.i will never reference an HDPC row, so can ignore self.A_hdpc_rows
        // because of Errata 2.
        for (col, value) in self
            .A
            .get_row_iter(self.i, self.i, self.A.width() - self.u)
            .clone()
        {
            if value == Octet::zero() {
                continue;
            }
            if col >= self.A.width() - self.u - (r - 1) {
                // Skip the column, if it's one of the trailing columns that shouldn't move
                remaining_swaps -= 1;
                continue;
            }
            if col == self.i {
                // Skip the column, if it's already in the first position
                remaining_swaps -= 1;
                found_first = true;
                continue;
            }
            let mut dest;
            if !found_first {
                dest = self.i;
                found_first = true;
            } else {
                dest = self.A.width() - self.u - 1;
                // Some of the right most columns may already contain non-zeros
                while self.A.get(self.i, dest) != Octet::zero() {
                    dest -= 1;
                }
            }
            // No need to swap the first i rows, as they are all zero (see submatrix above V)
            self.swap_columns(dest, col, self.i);
            selection_helper.swap_columns(dest, col);
            // Also apply to X
            #[cfg(debug_assertions)]
            self.X.swap_columns(dest, col, 0);
            remaining_swaps -= 1;
            if remaining_swaps == 0 {
                break;
            }
        }
        assert_eq!(0, remaining_swaps);
    }

    // First phase (section 5.4.2.2)
    //
    // Returns the row operations required to convert the X matrix into the identity
    #[allow(non_snake_case)]
    #[inline(never)]
    fn first_phase(&mut self) -> Option<Vec<RowOp>> {
        // First phase (section 5.4.2.2)

        //    ----------> i                 u <--------
        //  | +-----------+-----------------+---------+
        //  | |           |                 |         |
        //  | |     I     |    All Zeros    |         |
        //  v |           |                 |         |
        //  i +-----------+-----------------+    U    |
        //    |           |                 |         |
        //    |           |                 |         |
        //    | All Zeros |       V         |         |
        //    |           |                 |         |
        //    |           |                 |         |
        //    +-----------+-----------------+---------+
        // Figure 6: Submatrices of A in the First Phase

        let num_hdpc_rows = self.A_hdpc_rows.as_ref().map_or(0, |h| h.height());

        let mut selection_helper = FirstPhaseRowSelectionStats::new(
            &self.A,
            self.A.width() - self.u,
            self.A.height() - num_hdpc_rows,
        );

        // Record of first phase row operations performed on non-HDPC rows
        let mut row_ops = vec![];
        let mut row_selection_ones = Vec::new();
        let mut pivot_column_ones = Vec::new();

        while self.i + self.u < self.L {
            // Calculate r
            // "Let r be the minimum integer such that at least one row of A has
            // exactly r nonzeros in V."
            // Exclude the HDPC rows, since Errata 2 guarantees they won't be chosen.
            let (chosen_row, r) = selection_helper.first_phase_selection(
                self.i,
                self.A.height() - num_hdpc_rows,
                &self.A,
                &mut row_selection_ones,
            );

            r?;
            let r = r.unwrap();
            let chosen_row = chosen_row.unwrap();
            assert!(chosen_row >= self.i);

            // See paragraph beginning: "After the row is chosen in this step..."
            // Reorder rows
            let temp = self.i;
            self.swap_rows(temp, chosen_row);
            #[cfg(debug_assertions)]
            self.X.swap_rows(temp, chosen_row);
            row_ops.push(RowOp::Swap {
                row1: temp,
                row2: chosen_row,
            });
            selection_helper.swap_rows(temp, chosen_row);
            // Reorder columns
            self.first_phase_swap_columns_substep(r, &mut selection_helper);
            // Zero out leading value in following rows
            let temp = self.i;
            // self.i will never reference an HDPC row, so can ignore self.A_hdpc_rows
            // because of Errata 2.
            let temp_value = self.A.get(temp, temp);

            self.A.get_ones_in_column_into(
                temp,
                self.i + 1,
                self.A.height() - num_hdpc_rows,
                &mut pivot_column_ones,
            );
            selection_helper.resize(
                self.i + 1,
                self.A.height() - num_hdpc_rows,
                self.i + 1,
                self.A.width() - self.u - (r - 1),
                &pivot_column_ones,
                &self.A,
            );
            for i in 0..(r - 1) {
                self.A
                    .hint_column_dense_and_frozen(self.A.width() - self.u - 1 - i);
            }

            // Skip the first element since that's the i'th row
            for row in pivot_column_ones.iter().copied() {
                let row = row as usize;
                assert_eq!(&temp_value, &Octet::one());
                // Addition is equivalent to subtraction.
                #[cfg(debug_assertions)]
                self.fma_rows(temp, row, Octet::one(), 0);
                // Only apply to U section of matrix due to Errata 11
                #[cfg(not(debug_assertions))]
                self.fma_rows(temp, row, Octet::one(), self.A.width() - (self.u + (r - 1)));
                row_ops.push(RowOp::AddAssign {
                    src: temp,
                    dest: row,
                });
                if r == 1 {
                    // No need to update the selection helper, since we already resized it to remove
                    // the first column
                } else {
                    selection_helper.recompute_row(row, &self.A);
                }
            }

            // apply to hdpc rows as well, which are stored separately
            if num_hdpc_rows > 0 {
                let pi_octets = self
                    .A
                    .get_sub_row_as_octets(temp, self.A.width() - (self.u + r - 1));
                for row in 0..num_hdpc_rows {
                    let leading_value = self.A_hdpc_rows.as_ref().unwrap().get(row, temp);
                    if leading_value != Octet::zero() {
                        // Addition is equivalent to subtraction
                        let beta = &leading_value / &temp_value;
                        self.fma_rows_with_pi(
                            temp,
                            row + (self.A.height() - num_hdpc_rows),
                            beta,
                            // self.i is the only non-PI column which can have a nonzero,
                            // since all the rest were column swapped into the PI submatrix.
                            Some(temp),
                            Some(&pi_octets),
                            0,
                        );
                        // It's safe to skip updating the selection helper, since it will never
                        // select an HDPC row
                    }
                }
            }

            self.i += 1;
            self.u += r - 1;
            #[cfg(debug_assertions)]
            self.first_phase_verify();
        }

        self.record_symbol_ops(0);

        let mut mapping: Vec<usize> = (0..self.A.height()).collect();
        let mut row_ops: Vec<RowOp> = row_ops
            .iter()
            .rev()
            .filter_map(|x| match x {
                RowOp::AddAssign { src, dest } => {
                    assert!(mapping[*src] < self.i);
                    if mapping[*src] < self.i && mapping[*dest] < self.i {
                        Some(RowOp::AddAssign {
                            src: mapping[*src],
                            dest: mapping[*dest],
                        })
                    } else {
                        None
                    }
                }
                RowOp::Swap { row1, row2 } => {
                    mapping.swap(*row1, *row2);
                    None
                }
            })
            .collect();
        row_ops.reverse();
        return Some(row_ops);
    }

    // See section 5.4.2.2. Verifies the two all-zeros submatrices and the identity submatrix
    #[inline(never)]
    #[cfg(debug_assertions)]
    fn first_phase_verify(&self) {
        for row in 0..self.i {
            for col in 0..self.i {
                if row == col {
                    assert_eq!(Octet::one(), self.A.get(row, col));
                } else {
                    assert_eq!(Octet::zero(), self.A.get(row, col));
                }
            }
        }
        assert!(self.all_zeroes(0, self.i, self.i, self.A.width() - self.u));
        assert!(self.all_zeroes(self.i, self.A.height(), 0, self.i));
    }

    // Second phase (section 5.4.2.3)
    #[allow(non_snake_case)]
    #[inline(never)]
    fn second_phase(&mut self, #[allow(unused_variables)] x_elimination_ops: &[RowOp]) -> bool {
        #[cfg(debug_assertions)]
        self.second_phase_verify(x_elimination_ops);

        #[cfg(debug_assertions)]
        self.X.resize(self.i, self.i);

        // Convert U_lower to row echelon form
        let temp = self.i;
        let size = self.u;
        // HDPC rows can be removed, since they can't have been selected for U_upper
        let hdpc_rows = self
            .A_hdpc_rows
            .take()
            .unwrap_or_else(|| DenseOctetMatrix::new(0, size, 0));
        if let Some(submatrix) = self.record_reduce_to_row_echelon(hdpc_rows, temp, temp, size) {
            // Perform backwards elimination
            self.backwards_elimination(submatrix, temp, temp, size);
        } else {
            return false;
        }

        self.A.resize(self.L, self.L);

        self.record_symbol_ops(1);
        return true;
    }

    // Verifies that X is lower triangular. See section 5.4.2.3
    #[inline(never)]
    #[cfg(debug_assertions)]
    fn second_phase_verify(&self, x_elimination_ops: &[RowOp]) {
        for row in 0..self.i {
            for col in (row + 1)..self.i {
                assert_eq!(Octet::zero(), self.X.get(row, col));
            }
        }

        // Also verify Errata 9
        let mut tempX = self.X.clone();
        for op in x_elimination_ops {
            match op {
                RowOp::AddAssign { src, dest } => {
                    tempX.add_assign_rows(*dest, *src, 0);
                }
                RowOp::Swap { .. } => unreachable!(),
            }
        }
        for row in 0..self.i {
            for col in 0..self.i {
                if row == col {
                    assert_eq!(Octet::one(), tempX.get(row, col));
                } else {
                    assert_eq!(Octet::zero(), tempX.get(row, col));
                }
            }
        }
    }

    // Third phase (section 5.4.2.4)
    #[allow(non_snake_case)]
    #[inline(never)]
    fn third_phase(&mut self, x_elimination_ops: &[RowOp]) {
        #[cfg(debug_assertions)]
        self.third_phase_verify();

        // Perform A[0..i][..] = X * A[0..i][..] by applying Errata 10
        for op in x_elimination_ops.iter().rev() {
            match op {
                RowOp::AddAssign { src, dest } => {
                    #[cfg(debug_assertions)]
                    self.fma_rows(*src, *dest, Octet::one(), 0);
                    #[cfg(not(debug_assertions))]
                    // Skip applying to cols before i due to Errata 11
                    self.fma_rows(*src, *dest, Octet::one(), self.i);
                }
                RowOp::Swap { .. } => unreachable!(),
            }
        }

        self.record_symbol_ops(2);

        #[cfg(debug_assertions)]
        self.third_phase_verify_end();
    }

    #[inline(never)]
    #[cfg(debug_assertions)]
    fn third_phase_verify(&self) {
        for row in 0..self.A.height() {
            for col in 0..self.A.width() {
                if row < self.i && col >= self.A.width() - self.u {
                    // element is in U_upper, which can have arbitrary values at this point
                    continue;
                }
                // The rest of A should be identity matrix
                if row == col {
                    assert_eq!(Octet::one(), self.A.get(row, col));
                } else {
                    assert_eq!(Octet::zero(), self.A.get(row, col));
                }
            }
        }
    }

    #[inline(never)]
    #[cfg(debug_assertions)]
    fn third_phase_verify_end(&self) {
        for row in 0..self.i {
            for col in 0..self.i {
                assert_eq!(self.X.get(row, col), self.A.get(row, col));
            }
        }
    }

    // Fourth phase (section 5.4.2.5)
    #[allow(non_snake_case)]
    #[inline(never)]
    fn fourth_phase(&mut self) {
        let mut non_zero_columns = Vec::new();
        for i in 0..self.i {
            self.A
                .query_non_zero_columns_into(i, self.i, &mut non_zero_columns);
            for j in non_zero_columns.iter().copied() {
                #[cfg(debug_assertions)]
                self.fma_rows(j, i, Octet::one(), 0);
                // Skip applying to cols before i due to Errata 11
                #[cfg(not(debug_assertions))]
                self.fma_rows(j, i, Octet::one(), self.i);
            }
        }

        self.record_symbol_ops(3);

        #[cfg(debug_assertions)]
        self.fourth_phase_verify();
    }

    #[inline(never)]
    #[cfg(debug_assertions)]
    fn fourth_phase_verify(&self) {
        //    ---------> i u <------
        //  | +-----------+--------+
        //  | |\          |        |
        //  | |  \ Zeros  | Zeros  |
        //  v |     \     |        |
        //  i |  X     \  |        |
        //  u +---------- +--------+
        //  ^ |           |        |
        //  | | All Zeros |   I    |
        //  | |           |        |
        //    +-----------+--------+
        // Same assertion about X being equal to the upper left of A
        self.third_phase_verify_end();
        assert!(self.all_zeroes(0, self.i, self.A.width() - self.u, self.A.width()));
        assert!(self.all_zeroes(self.A.height() - self.u, self.A.height(), 0, self.i));
        for row in (self.A.height() - self.u)..self.A.height() {
            for col in (self.A.width() - self.u)..self.A.width() {
                if row == col {
                    assert_eq!(Octet::one(), self.A.get(row, col));
                } else {
                    assert_eq!(Octet::zero(), self.A.get(row, col));
                }
            }
        }
    }

    // Fifth phase (section 5.4.2.6)
    #[allow(non_snake_case)]
    #[inline(never)]
    fn fifth_phase(&mut self, x_elimination_ops: &[RowOp]) {
        // Use the saved operations from first phase: Errata 9
        for op in x_elimination_ops {
            match op {
                RowOp::AddAssign { src, dest } => {
                    #[cfg(debug_assertions)]
                    self.fma_rows(*src, *dest, Octet::one(), 0);
                    // In release builds skip updating the A matrix, since it will never be read
                    #[cfg(not(debug_assertions))]
                    self.record_fma_rows(*src, *dest, Octet::one());
                }
                RowOp::Swap { .. } => unreachable!(),
            }
        }

        self.record_symbol_ops(4);

        #[cfg(debug_assertions)]
        self.fifth_phase_verify();
    }

    #[inline(never)]
    #[cfg(debug_assertions)]
    fn fifth_phase_verify(&self) {
        assert_eq!(self.L, self.A.height());
        for row in 0..self.A.height() {
            assert_eq!(self.L, self.A.width());
            for col in 0..self.A.width() {
                if row == col {
                    assert_eq!(Octet::one(), self.A.get(row, col));
                } else {
                    assert_eq!(Octet::zero(), self.A.get(row, col));
                }
            }
        }
    }

    fn record_symbol_ops(&mut self, phase: usize) {
        self.debug_symbol_add_ops_by_phase[phase] = self.debug_symbol_add_ops;
        self.debug_symbol_mul_ops_by_phase[phase] = self.debug_symbol_mul_ops;
        for i in 0..phase {
            self.debug_symbol_add_ops_by_phase[phase] -= self.debug_symbol_add_ops_by_phase[i];
            self.debug_symbol_mul_ops_by_phase[phase] -= self.debug_symbol_mul_ops_by_phase[i];
        }
    }

    // Reduces the size x size submatrix, starting at row_offset and col_offset as the upper left
    // corner, to row echelon form.
    // Returns the reduced submatrix, which should be written back into this submatrix of A.
    // The state of this submatrix in A is undefined, after calling this function.
    #[inline(never)]
    fn record_reduce_to_row_echelon(
        &mut self,
        hdpc_rows: DenseOctetMatrix,
        row_offset: usize,
        col_offset: usize,
        size: usize,
    ) -> Option<DenseOctetMatrix> {
        // Copy U_lower into a new matrix and merge it with the HDPC rows
        let mut submatrix = DenseOctetMatrix::new(self.A.height() - row_offset, size, 0);
        let first_hdpc_row = self.A.height() - hdpc_rows.height();
        for row in row_offset..self.A.height() {
            for col in col_offset..(col_offset + size) {
                let value = if row < first_hdpc_row {
                    self.A.get(row, col)
                } else {
                    hdpc_rows.get(row - first_hdpc_row, col)
                };
                submatrix.set(row - row_offset, col - col_offset, value);
            }
        }

        for i in 0..size {
            // Swap a row with leading coefficient i into place
            for j in i..submatrix.height() {
                if submatrix.get(j, i) != Octet::zero() {
                    submatrix.swap_rows(i, j);
                    // Record the swap, in addition to swapping in the working submatrix
                    // TODO: optimize to not perform op on A
                    self.swap_rows(row_offset + i, j + row_offset);
                    break;
                }
            }

            if submatrix.get(i, i) == Octet::zero() {
                // If all following rows are zero in this column, then matrix is singular
                return None;
            }

            // Scale leading coefficient to 1
            if submatrix.get(i, i) != Octet::one() {
                let element_inverse = Octet::one() / submatrix.get(i, i);
                submatrix.mul_assign_row(i, &element_inverse);
                // Record the multiplication, in addition to multiplying the working submatrix
                self.record_mul_row(row_offset + i, element_inverse);
            }

            // Zero out all following elements in i'th column
            for j in (i + 1)..submatrix.height() {
                if submatrix.get(j, i) != Octet::zero() {
                    let scalar = submatrix.get(j, i);
                    submatrix.fma_rows(j, i, &scalar);
                    // Record the FMA, in addition to applying it to the working submatrix
                    self.record_fma_rows(row_offset + i, row_offset + j, scalar);
                }
            }
        }

        return Some(submatrix);
    }

    // Performs backwards elimination in a size x size submatrix, starting at
    // row_offset and col_offset as the upper left corner of the submatrix
    //
    // Applies the submatrix to the size-by-size lower right of A, and performs backwards
    // elimination on it. "submatrix" must be in row echelon form.
    #[inline(never)]
    fn backwards_elimination(
        &mut self,
        submatrix: DenseOctetMatrix,
        row_offset: usize,
        col_offset: usize,
        size: usize,
    ) {
        // Perform backwards elimination
        for i in (0..size).rev() {
            // Zero out all preceding elements in i'th column
            for j in 0..i {
                if submatrix.get(j, i) != Octet::zero() {
                    let scalar = submatrix.get(j, i);
                    // Record the FMA. No need to actually apply it to the submatrix,
                    // since it will be discarded, and we never read these values
                    self.record_fma_rows(row_offset + i, row_offset + j, scalar);
                }
            }
        }

        // Write the identity matrix into A, since that's the resulting value of this function
        for row in row_offset..(row_offset + size) {
            for col in col_offset..(col_offset + size) {
                if row == col {
                    self.A.set(row, col, Octet::one());
                } else {
                    self.A.set(row, col, Octet::zero());
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_symbol_mul_ops(&self) -> u32 {
        self.debug_symbol_mul_ops
    }

    #[allow(dead_code)]
    pub fn get_symbol_add_ops(&self) -> u32 {
        self.debug_symbol_add_ops
    }

    #[allow(dead_code)]
    pub fn get_symbol_mul_ops_by_phase(&self) -> Vec<u32> {
        self.debug_symbol_mul_ops_by_phase.clone()
    }

    #[allow(dead_code)]
    pub fn get_symbol_add_ops_by_phase(&self) -> Vec<u32> {
        self.debug_symbol_add_ops_by_phase.clone()
    }

    #[cfg(feature = "benchmarking")]
    pub fn get_non_symbol_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();

        bytes += self.A.size_in_bytes();
        if let Some(ref hdpc) = self.A_hdpc_rows {
            bytes += hdpc.size_in_bytes();
        }
        #[cfg(debug_assertions)]
        {
            bytes += self.X.size_in_bytes();
        }
        // Skip self.D, since we're calculating non-Symbol bytes
        bytes += size_of::<usize>() * self.c.len();
        bytes += size_of::<usize>() * self.d.len();

        bytes
    }

    // Record operation to apply operations to D.
    fn record_mul_row(&mut self, i: usize, beta: Octet) {
        self.debug_symbol_mul_ops += 1;
        self.deferred_D_ops.push(SymbolOps::MulAssign {
            dest: self.d[i],
            scalar: beta,
        });
        assert!(self.A_hdpc_rows.is_none());
    }

    fn fma_rows(&mut self, i: usize, iprime: usize, beta: Octet, start_col: usize) {
        self.fma_rows_with_pi(i, iprime, beta, None, None, start_col);
    }

    fn record_fma_rows(&mut self, i: usize, iprime: usize, beta: Octet) {
        self.debug_symbol_add_ops += 1;
        if beta == Octet::one() {
            self.deferred_D_ops.push(SymbolOps::AddAssign {
                dest: self.d[iprime],
                src: self.d[i],
            });
        } else {
            self.debug_symbol_mul_ops += 1;
            self.deferred_D_ops.push(SymbolOps::FMA {
                dest: self.d[iprime],
                src: self.d[i],
                scalar: beta,
            });
        }
    }

    fn fma_rows_with_pi(
        &mut self,
        i: usize,
        iprime: usize,
        beta: Octet,
        #[allow(unused_variables)] only_non_pi_nonzero_column: Option<usize>,
        pi_octets: Option<&BinaryOctetVec>,
        start_col: usize,
    ) {
        self.record_fma_rows(i, iprime, beta.clone());

        if let Some(ref mut hdpc) = self.A_hdpc_rows {
            let first_hdpc_row = self.A.height() - hdpc.height();
            // Adding HDPC rows to other rows isn't supported, since it should never happen
            assert!(i < first_hdpc_row);
            if iprime >= first_hdpc_row {
                // Only update the V section of HDPC rows, for debugging. (they will never be read)
                #[cfg(debug_assertions)]
                {
                    let col = only_non_pi_nonzero_column.unwrap();
                    let multiplicand = self.A.get(i, col);
                    let mut value = hdpc.get(iprime - first_hdpc_row, col);
                    value.fma(&multiplicand, &beta);
                    hdpc.set(iprime - first_hdpc_row, col, value);
                }

                // Handle this part separately, since it's in the dense U part of the matrix
                let octets = pi_octets.unwrap();
                hdpc.fma_sub_row(
                    iprime - first_hdpc_row,
                    self.A.width() - octets.len(),
                    &beta,
                    octets,
                );
            } else {
                assert_eq!(&beta, &Octet::one());
                self.A.add_assign_rows(iprime, i, start_col);
            }
        } else {
            assert_eq!(&beta, &Octet::one());
            self.A.add_assign_rows(iprime, i, start_col);
        }
    }

    fn swap_rows(&mut self, i: usize, iprime: usize) {
        if let Some(ref hdpc_rows) = self.A_hdpc_rows {
            // Can't swap HDPC rows
            assert!(i < self.A.height() - hdpc_rows.height());
            assert!(iprime < self.A.height() - hdpc_rows.height());
        }
        self.A.swap_rows(i, iprime);
        self.d.swap(i, iprime);
    }

    fn swap_columns(&mut self, j: usize, jprime: usize, start_row: usize) {
        self.A.swap_columns(j, jprime, start_row);
        if let Some(ref mut hdpc) = self.A_hdpc_rows {
            hdpc.swap_columns(j, jprime, 0);
        }
        self.c.swap(j, jprime);
    }

    #[inline(never)]
    pub fn execute(&mut self) -> (Option<SymbolSlab>, Option<Vec<SymbolOps>>) {
        #[cfg(debug_assertions)]
        self.X.disable_column_access_acceleration();

        if let Some(x_elimination_ops) = self.first_phase() {
            self.A.disable_column_access_acceleration();

            if !self.second_phase(&x_elimination_ops) {
                return (None, None);
            }

            self.third_phase(&x_elimination_ops);
            self.fourth_phase();
            self.fifth_phase(&x_elimination_ops);
        } else {
            return (None, None);
        }

        self.apply_deferred_symbol_ops();

        // See end of section 5.4.2.1
        let mut index_mapping = vec![0; self.L];
        for i in 0..self.L {
            index_mapping[self.c[i]] = self.d[i];
        }

        // Keep D in-place and return logical reorder mapping.
        // D.len() may be > L when decoder has overhead symbols.
        let reorder: Vec<usize> = index_mapping[..self.L].to_vec();

        let mut operation_vector = mem::take(&mut self.deferred_D_ops);
        operation_vector.push(SymbolOps::Reorder {
            order: reorder.clone(),
        });
        self.D.set_reorder(reorder);
        let symbol_size = self.D.symbol_size();
        let result = mem::replace(&mut self.D, SymbolSlab::with_zeros(0, symbol_size));
        return (Some(result), Some(operation_vector));
    }
}

// Fused implementation for self.inverse().mul_symbols(symbols)
// See section 5.4.2.1
pub fn fused_inverse_mul_symbols<T: BinaryMatrix>(
    matrix: T,
    hdpc_rows: DenseOctetMatrix,
    symbols: SymbolSlab,
    num_source_symbols: u32,
) -> (Option<SymbolSlab>, Option<Vec<SymbolOps>>) {
    IntermediateSymbolDecoder::new(matrix, hdpc_rows, symbols, num_source_symbols).execute()
}

// Fused implementation without HDPC rows.
// Used during decoding with sufficient overhead to solve in GF(2) only.
pub fn fused_inverse_mul_symbols_no_hdpc<T: BinaryMatrix>(
    matrix: T,
    symbols: SymbolSlab,
    num_source_symbols: u32,
) -> (Option<SymbolSlab>, Option<Vec<SymbolOps>>) {
    IntermediateSymbolDecoder::new_no_hdpc(matrix, symbols, num_source_symbols).execute()
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::IntermediateSymbolDecoder;
    use crate::constraint_matrix::generate_constraint_matrix;
    use crate::matrix::BinaryMatrix;
    use crate::matrix::DenseBinaryMatrix;
    use crate::symbol_slab::SymbolSlab;
    use crate::systematic_constants::{
        MAX_SOURCE_SYMBOLS_PER_BLOCK, extended_source_block_symbols, num_ldpc_symbols,
        num_lt_symbols,
    };
    use std::vec::Vec;

    #[test]
    fn operations_per_symbol() {
        for &(elements, expected_mul_ops, expected_add_ops) in
            [(10, 35.0, 50.0), (100, 16.0, 35.0)].iter()
        {
            let num_symbols = extended_source_block_symbols(elements);
            let indices: Vec<u32> = (0..num_symbols).collect();
            let (a, hdpc) = generate_constraint_matrix::<DenseBinaryMatrix>(num_symbols, &indices);
            let symbols = SymbolSlab::with_zeros(a.width(), 1);
            let mut decoder = IntermediateSymbolDecoder::new(a, hdpc, symbols, num_symbols);
            decoder.execute();
            assert!(
                (decoder.get_symbol_mul_ops() as f64 / num_symbols as f64) < expected_mul_ops,
                "mul ops per symbol = {}",
                (decoder.get_symbol_mul_ops() as f64 / num_symbols as f64)
            );
            assert!(
                (decoder.get_symbol_add_ops() as f64 / num_symbols as f64) < expected_add_ops,
                "add ops per symbol = {}",
                (decoder.get_symbol_add_ops() as f64 / num_symbols as f64)
            );
        }
    }

    #[test]
    fn check_errata_3() {
        // Check that the optimization of excluding HDPC rows from the X matrix during decoding is
        // safe. This is described in RFC6330_ERRATA.md
        for i in 0..=MAX_SOURCE_SYMBOLS_PER_BLOCK {
            assert!(extended_source_block_symbols(i) + num_ldpc_symbols(i) >= num_lt_symbols(i));
        }
    }
}
