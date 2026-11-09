#[cfg(feature = "std")]
use std::{cmp::Ordering, mem::size_of, vec::Vec};

#[cfg(not(feature = "std"))]
use core::{cmp::Ordering, mem::size_of};

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::octet::Octet;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct SparseBinaryVec {
    // Kept sorted by the usize (key). Only ones are stored, zeros are implicit
    elements: Vec<u16>,
}

impl SparseBinaryVec {
    pub fn with_capacity(capacity: usize) -> SparseBinaryVec {
        // Matrix width can never exceed maximum L
        debug_assert!(capacity < 65536);
        SparseBinaryVec {
            elements: Vec::with_capacity(capacity),
        }
    }

    // Returns the internal index into self.elements matching key i, or the index
    // at which it can be inserted (maintaining sorted order)
    fn key_to_internal_index(&self, i: u16) -> Result<usize, usize> {
        self.elements.binary_search(&i)
    }

    #[allow(dead_code)]
    pub fn size_in_bytes(&self) -> usize {
        size_of::<Self>() + size_of::<u16>() * self.elements.len()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn get_by_raw_index(&self, i: usize) -> (usize, Octet) {
        (self.elements[i] as usize, Octet::one())
    }

    // Returns true, if a new column was added
    pub fn add_assign(&mut self, other: &SparseBinaryVec) -> bool {
        // Fast path for a single value that's being eliminated
        if other.elements.len() == 1 {
            let other_index = &other.elements[0];
            match self.key_to_internal_index(*other_index) {
                Ok(index) => {
                    // Adding 1 + 1 = 0 in GF(256), so remove this
                    self.elements.remove(index);
                }
                Err(index) => {
                    self.elements.insert(index, *other_index);
                    return true;
                }
            };
            return false;
        }

        let mut result = Vec::with_capacity(self.elements.len() + other.elements.len());
        let mut self_iter = self.elements.iter();
        let mut other_iter = other.elements.iter();
        let mut self_next = self_iter.next();
        let mut other_next = other_iter.next();

        let mut column_added = false;
        loop {
            if let Some(self_index) = self_next {
                if let Some(other_index) = other_next {
                    match self_index.cmp(other_index) {
                        Ordering::Less => {
                            result.push(*self_index);
                            self_next = self_iter.next();
                        }
                        Ordering::Equal => {
                            // Adding 1 + 1 = 0 in GF(256), so skip this index
                            self_next = self_iter.next();
                            other_next = other_iter.next();
                        }
                        Ordering::Greater => {
                            column_added = true;
                            result.push(*other_index);
                            other_next = other_iter.next();
                        }
                    }
                } else {
                    result.push(*self_index);
                    self_next = self_iter.next();
                }
            } else if let Some(other_index) = other_next {
                column_added = true;
                result.push(*other_index);
                other_next = other_iter.next();
            } else {
                break;
            }
        }
        self.elements = result;

        return column_added;
    }

    pub fn remove(&mut self, i: usize) -> Option<Octet> {
        match self.key_to_internal_index(i as u16) {
            Ok(index) => {
                self.elements.remove(index);
                Some(Octet::one())
            }
            Err(_) => None,
        }
    }

    pub fn retain<P: Fn(&(usize, Octet)) -> bool>(&mut self, predicate: P) {
        self.elements
            .retain(|entry| predicate(&(*entry as usize, Octet::one())));
    }

    pub fn get(&self, i: usize) -> Option<Octet> {
        match self.key_to_internal_index(i as u16) {
            Ok(_) => Some(Octet::one()),
            Err(_) => None,
        }
    }

    pub fn keys_values(&self) -> impl Iterator<Item = (usize, Octet)> + '_ {
        self.elements
            .iter()
            .map(|entry| (*entry as usize, Octet::one()))
    }

    pub fn insert(&mut self, i: usize, value: Octet) {
        debug_assert!(i < 65536);
        if value == Octet::zero() {
            self.remove(i);
        } else {
            match self.key_to_internal_index(i as u16) {
                Ok(_) => {}
                Err(index) => self.elements.insert(index, i as u16),
            }
        }
    }
}
