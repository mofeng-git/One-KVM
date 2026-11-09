#[cfg(feature = "std")]
use std::{mem::size_of, ops::Range, vec::Vec};

#[cfg(not(feature = "std"))]
use core::{mem::size_of, ops::Range, u32};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
// Map<u16, Vec<u32>>
pub struct ImmutableListMap {
    // offset of std::u32::MAX indicates that the key is not present
    offsets: Vec<u32>,
    values: Vec<u32>,
}

impl ImmutableListMap {
    pub fn get(&self, i: u16) -> &[u32] {
        let i = i as usize;
        let start = self.offsets[i] as usize;
        let end = if i == self.offsets.len() - 1 {
            self.values.len()
        } else {
            self.offsets[i + 1] as usize
        };
        &self.values[start..end]
    }

    #[allow(dead_code)]
    pub fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();
        bytes += size_of::<u32>() * self.offsets.len();
        bytes += size_of::<u32>() * self.values.len();

        bytes
    }
}

pub struct ImmutableListMapBuilder {
    entries: Vec<(u16, u32)>,
    num_keys: usize,
}

impl ImmutableListMapBuilder {
    pub fn new(num_keys: usize) -> ImmutableListMapBuilder {
        ImmutableListMapBuilder {
            entries: vec![],
            num_keys,
        }
    }

    pub fn add(&mut self, key: u16, value: u32) {
        self.entries.push((key, value));
    }

    pub fn build(self) -> ImmutableListMap {
        let mut entries = self.entries;
        entries.sort_unstable_by_key(|x| x.0);
        assert!(entries.len() < u32::MAX as usize);
        assert!(!entries.is_empty());
        let mut offsets = vec![u32::MAX; self.num_keys];
        let mut last_key = entries[0].0;
        offsets[last_key as usize] = 0;
        let mut values = vec![];
        for (index, (key, value)) in entries.iter().enumerate() {
            if last_key != *key {
                last_key = *key;
                offsets[*key as usize] = index as u32;
            }
            values.push(*value);
        }
        for i in (0..offsets.len()).rev() {
            if offsets[i] == u32::MAX {
                if i == offsets.len() - 1 {
                    offsets[i] = entries.len() as u32;
                } else {
                    offsets[i] = offsets[i + 1];
                }
            }
        }

        ImmutableListMap { offsets, values }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct UndirectedGraph {
    edges: Vec<(u16, u16)>,
    // Mapping from node id to starting index in edges array
    node_edge_starting_index: U32VecMap,
}

impl UndirectedGraph {
    pub fn with_capacity(start_node: u16, end_node: u16, edges: usize) -> UndirectedGraph {
        UndirectedGraph {
            edges: Vec::with_capacity(edges * 2),
            node_edge_starting_index: U32VecMap::with_capacity(
                start_node as usize,
                end_node as usize,
            ),
        }
    }

    pub fn add_edge(&mut self, node1: u16, node2: u16) {
        self.edges.push((node1, node2));
        self.edges.push((node2, node1));
    }

    pub fn build(&mut self) {
        // Ordering of adjacencies doesn't matter, so just sort by the first node
        self.edges.sort_unstable_by_key(|x| x.0);
        if self.edges.is_empty() {
            return;
        }
        let mut last_node = self.edges[0].0;
        self.node_edge_starting_index.insert(last_node as usize, 0);
        for (index, (node, _)) in self.edges.iter().enumerate() {
            if last_node != *node {
                last_node = *node;
                self.node_edge_starting_index
                    .insert(last_node as usize, index as u32);
            }
        }
    }

    pub fn get_adjacent_nodes(&self, node: u16) -> impl Iterator<Item = u16> + '_ {
        let first_candidate = self.node_edge_starting_index.get(node as usize);
        AdjacentIterator::new(self.edges.iter().skip(first_candidate as usize), node)
    }

    pub fn nodes(&self) -> Vec<u16> {
        let mut result = vec![];
        for &(node, _) in self.edges.iter() {
            if result.is_empty() || result[result.len() - 1] != node {
                result.push(node);
            }
        }

        result
    }
}

struct AdjacentIterator<T> {
    edges: T,
    node: u16,
}

impl<'a, T: Iterator<Item = &'a (u16, u16)>> AdjacentIterator<T> {
    fn new(edges: T, node: u16) -> AdjacentIterator<T> {
        AdjacentIterator { edges, node }
    }
}

impl<'a, T: Iterator<Item = &'a (u16, u16)>> Iterator for AdjacentIterator<T> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((_node, adjacent)) = self.edges.next().filter(|(node, _)| *node == self.node) {
            return Some(*adjacent);
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct U16ArrayMap {
    offset: usize,
    elements: Vec<u16>,
}

impl U16ArrayMap {
    pub fn new(start_key: usize, end_key: usize) -> U16ArrayMap {
        U16ArrayMap {
            offset: start_key,
            elements: vec![0; end_key - start_key],
        }
    }

    pub fn size_in_bytes(&self) -> usize {
        size_of::<Self>() + size_of::<u16>() * self.elements.len()
    }

    pub fn swap(&mut self, key: usize, other_key: usize) {
        self.elements.swap(key, other_key);
    }

    pub fn keys(&self) -> Range<usize> {
        self.offset..(self.offset + self.elements.len())
    }

    pub fn insert(&mut self, key: usize, value: u16) {
        self.elements[key - self.offset] = value;
    }

    pub fn get(&self, key: usize) -> u16 {
        self.elements[key - self.offset]
    }

    pub fn decrement(&mut self, key: usize) {
        self.elements[key - self.offset] -= 1;
    }

    pub fn increment(&mut self, key: usize) {
        self.elements[key - self.offset] += 1;
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct U32VecMap {
    offset: usize,
    elements: Vec<u32>,
}

impl U32VecMap {
    pub fn new(start_key: usize) -> U32VecMap {
        U32VecMap {
            offset: start_key,
            elements: vec![0; 1],
        }
    }

    pub fn with_capacity(start_key: usize, end_key: usize) -> U32VecMap {
        U32VecMap {
            offset: start_key,
            elements: vec![0; end_key - start_key],
        }
    }

    fn grow_if_necessary(&mut self, index: usize) {
        if index >= self.elements.len() {
            self.elements
                .extend(vec![0; index - self.elements.len() + 1]);
        }
    }

    pub fn size_in_bytes(&self) -> usize {
        size_of::<Self>() + size_of::<u32>() * self.elements.len()
    }

    #[allow(dead_code)]
    pub fn insert(&mut self, key: usize, value: u32) {
        self.grow_if_necessary(key - self.offset);
        self.elements[key - self.offset] = value;
    }

    pub fn get(&self, key: usize) -> u32 {
        if key - self.offset >= self.elements.len() {
            return 0;
        }
        self.elements[key - self.offset]
    }

    pub fn decrement(&mut self, key: usize) {
        self.grow_if_necessary(key - self.offset);
        self.elements[key - self.offset] -= 1;
    }

    pub fn increment(&mut self, key: usize) {
        self.grow_if_necessary(key - self.offset);
        self.elements[key - self.offset] += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::arraymap::ImmutableListMapBuilder;

    #[test]
    fn list_map() {
        let mut builder = ImmutableListMapBuilder::new(10);
        builder.add(0, 1);
        builder.add(3, 1);
        builder.add(3, 2);

        let map = builder.build();
        assert!(map.get(0).contains(&1));
        assert!(!map.get(0).contains(&2));

        assert!(map.get(3).contains(&1));
        assert!(map.get(3).contains(&2));
        assert!(!map.get(3).contains(&3));

        assert!(!map.get(2).contains(&1));
    }
}
