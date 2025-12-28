//! Module management for One-KVM
//!
//! This module provides infrastructure for managing feature modules
//! (video streaming, HID control, MSD, ATX) as independent async tasks.

use std::future::Future;
use std::pin::Pin;
use tokio::sync::broadcast;

/// Module status
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

/// Trait for feature modules
pub trait Module: Send + Sync {
    /// Module name
    fn name(&self) -> &'static str;

    /// Current status
    fn status(&self) -> ModuleStatus;

    /// Start the module
    fn start(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;

    /// Stop the module
    fn stop(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}

/// Module manager for coordinating feature modules
pub struct ModuleManager {
    shutdown_rx: broadcast::Receiver<()>,
}

impl ModuleManager {
    pub fn new(shutdown_rx: broadcast::Receiver<()>) -> Self {
        Self { shutdown_rx }
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&mut self) {
        let _ = self.shutdown_rx.recv().await;
    }
}
