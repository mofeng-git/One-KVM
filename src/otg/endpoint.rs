//! USB Endpoint allocation management

use crate::error::{AppError, Result};

/// Default maximum endpoints for typical UDC
pub const DEFAULT_MAX_ENDPOINTS: u8 = 16;

/// Endpoint allocator - manages UDC endpoint resources
#[derive(Debug, Clone)]
pub struct EndpointAllocator {
    max_endpoints: u8,
    used_endpoints: u8,
}

impl EndpointAllocator {
    /// Create a new endpoint allocator
    pub fn new(max_endpoints: u8) -> Self {
        Self {
            max_endpoints,
            used_endpoints: 0,
        }
    }

    /// Allocate endpoints for a function
    pub fn allocate(&mut self, count: u8) -> Result<()> {
        if self.used_endpoints + count > self.max_endpoints {
            return Err(AppError::Internal(format!(
                "Not enough endpoints: need {}, available {}",
                count,
                self.available()
            )));
        }
        self.used_endpoints += count;
        Ok(())
    }

    /// Release endpoints
    pub fn release(&mut self, count: u8) {
        self.used_endpoints = self.used_endpoints.saturating_sub(count);
    }

    /// Get available endpoint count
    pub fn available(&self) -> u8 {
        self.max_endpoints.saturating_sub(self.used_endpoints)
    }

    /// Get used endpoint count
    pub fn used(&self) -> u8 {
        self.used_endpoints
    }

    /// Get maximum endpoint count
    pub fn max(&self) -> u8 {
        self.max_endpoints
    }

    /// Check if can allocate
    pub fn can_allocate(&self, count: u8) -> bool {
        self.available() >= count
    }
}

impl Default for EndpointAllocator {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_ENDPOINTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocator() {
        let mut alloc = EndpointAllocator::new(8);
        assert_eq!(alloc.available(), 8);

        alloc.allocate(2).unwrap();
        assert_eq!(alloc.available(), 6);
        assert_eq!(alloc.used(), 2);

        alloc.allocate(4).unwrap();
        assert_eq!(alloc.available(), 2);

        // Should fail - not enough endpoints
        assert!(alloc.allocate(3).is_err());

        alloc.release(2);
        assert_eq!(alloc.available(), 4);
    }
}
