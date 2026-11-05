use async_trait::async_trait;
use std::time::Duration;

use super::traits::AtxKeyBackend;
use crate::error::{AppError, Result};

pub struct DisabledAtxKeyBackend {
    reason: &'static str,
}

impl DisabledAtxKeyBackend {
    pub fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

#[async_trait]
impl AtxKeyBackend for DisabledAtxKeyBackend {
    async fn init(&mut self) -> Result<()> {
        Err(AppError::Internal(self.reason.to_string()))
    }

    async fn pulse(&self, _duration: Duration) -> Result<()> {
        Err(AppError::Internal(self.reason.to_string()))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        false
    }
}
