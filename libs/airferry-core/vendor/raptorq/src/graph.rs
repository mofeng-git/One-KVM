#[cfg(feature = "std")]
use std::cmp::{max, min};

#[cfg(not(feature = "std"))]
use core::cmp::{max, min};

use crate::arraymap::U16ArrayMap;

const NO_CONNECTED_COMPONENT: u16 = 0;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct ConnectedComponentGraph {
    // Mapping from nodes to their connected component id
    node_connected_component: U16ArrayMap,
    // Mapping from original connected component id to the one they've been merged with
    merged_connected_components: U16ArrayMap,
    // Size of each connected component in the graph
    connected_component_size: U16ArrayMap,
    num_connected_components: usize,
}

impl ConnectedComponentGraph {
    pub fn new(max_nodes: usize) -> ConnectedComponentGraph {
        let first_connected_component = NO_CONNECTED_COMPONENT + 1;
        let mut result = ConnectedComponentGraph {
            node_connected_component: U16ArrayMap::new(0, max_nodes),
            merged_connected_components: U16ArrayMap::new(
                first_connected_component as usize,
                first_connected_component as usize + max_nodes,
            ),
            connected_component_size: U16ArrayMap::new(
                first_connected_component as usize,
                first_connected_component as usize + max_nodes,
            ),
            num_connected_components: 0,
        };
        for i in result.merged_connected_components.keys() {
            result.merged_connected_components.insert(i, i as u16);
        }

        result
    }

    pub fn create_connected_component(&mut self) -> u16 {
        self.num_connected_components += 1;

        NO_CONNECTED_COMPONENT + self.num_connected_components as u16
    }

    pub fn add_node(&mut self, node: usize, connected_component: u16) {
        assert!(connected_component <= self.num_connected_components as u16);
        assert_eq!(
            self.node_connected_component.get(node),
            NO_CONNECTED_COMPONENT
        );
        let canonical = self.canonical_component_id(connected_component as usize);
        self.node_connected_component.insert(node, canonical as u16);
        self.connected_component_size.increment(canonical);
    }

    pub fn swap(&mut self, node1: usize, node2: usize) {
        self.node_connected_component.swap(node1, node2);
    }

    pub fn contains(&self, node: usize) -> bool {
        self.node_connected_component.get(node) != NO_CONNECTED_COMPONENT
    }

    pub fn remove_node(&mut self, node: usize) {
        let connected_component =
            self.canonical_component_id(self.node_connected_component.get(node) as usize);
        if connected_component == NO_CONNECTED_COMPONENT as usize {
            return;
        }
        self.connected_component_size.decrement(connected_component);
        self.node_connected_component
            .insert(node, NO_CONNECTED_COMPONENT);
    }

    pub fn get_node_in_largest_connected_component(
        &self,
        start_node: usize,
        end_node: usize,
    ) -> usize {
        let mut max_size = 0;
        let mut largest_connected_component = NO_CONNECTED_COMPONENT as usize;
        for i in 1..=self.num_connected_components {
            let size = self.connected_component_size.get(i);
            if size > max_size {
                max_size = size;
                largest_connected_component = i;
            }
        }
        assert_ne!(largest_connected_component, NO_CONNECTED_COMPONENT as usize);

        // Find a node (column) in that connected component
        (start_node..end_node)
            .find(|node| {
                self.canonical_component_id(self.node_connected_component.get(*node) as usize)
                    == largest_connected_component
            })
            .unwrap()
    }

    // This function will implicitly create any missing nodes
    pub fn add_edge(&mut self, node1: usize, node2: usize) {
        let connected_component1 =
            self.canonical_component_id(self.node_connected_component.get(node1) as usize);
        let connected_component2 =
            self.canonical_component_id(self.node_connected_component.get(node2) as usize);

        if connected_component1 == NO_CONNECTED_COMPONENT as usize
            && connected_component2 == NO_CONNECTED_COMPONENT as usize
        {
            // Create a new connected component
            let connected_component_id = self.create_connected_component();
            self.node_connected_component
                .insert(node1, connected_component_id);
            self.node_connected_component
                .insert(node2, connected_component_id);
            self.connected_component_size
                .insert(connected_component_id as usize, 2);
        } else if connected_component1 == NO_CONNECTED_COMPONENT as usize {
            self.connected_component_size
                .increment(connected_component2);
            self.node_connected_component
                .insert(node1, connected_component2 as u16);
        } else if connected_component2 == NO_CONNECTED_COMPONENT as usize {
            self.connected_component_size
                .increment(connected_component1);
            self.node_connected_component
                .insert(node2, connected_component1 as u16);
        } else if connected_component1 != connected_component2 {
            // Merge into the lowest to keep chains short
            let merge_to = min(connected_component1, connected_component2);
            let merge_from = max(connected_component1, connected_component2);
            let to_size = self.connected_component_size.get(merge_to);
            let from_size = self.connected_component_size.get(merge_from);
            self.connected_component_size.insert(merge_from, 0);
            self.connected_component_size
                .insert(merge_to, to_size + from_size);
            self.merged_connected_components
                .insert(merge_from, merge_to as u16);
        }
    }

    fn canonical_component_id(&self, mut id: usize) -> usize {
        if id == NO_CONNECTED_COMPONENT as usize {
            return id;
        }
        while self.merged_connected_components.get(id) as usize != id {
            id = self.merged_connected_components.get(id) as usize;
        }

        id
    }

    pub fn reset(&mut self) {
        for i in 1..=self.num_connected_components {
            self.connected_component_size.insert(i, 0);
            self.merged_connected_components.insert(i, i as u16);
        }
        self.num_connected_components = 0;
        for i in self.node_connected_component.keys() {
            self.node_connected_component
                .insert(i, NO_CONNECTED_COMPONENT);
        }
    }
}
