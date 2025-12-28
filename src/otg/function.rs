//! USB Gadget Function trait definition

use std::path::Path;

use crate::error::Result;

/// Function metadata
#[derive(Debug, Clone)]
pub struct FunctionMeta {
    /// Function name (e.g., "hid.usb0")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Number of endpoints used
    pub endpoints: u8,
    /// Whether the function is enabled
    pub enabled: bool,
}

/// USB Gadget Function trait
pub trait GadgetFunction: Send + Sync {
    /// Get function name (e.g., "hid.usb0", "mass_storage.usb0")
    fn name(&self) -> &str;

    /// Get number of endpoints required
    fn endpoints_required(&self) -> u8;

    /// Get function metadata
    fn meta(&self) -> FunctionMeta;

    /// Create function directory and configuration in ConfigFS
    fn create(&self, gadget_path: &Path) -> Result<()>;

    /// Link function to configuration
    fn link(&self, config_path: &Path, gadget_path: &Path) -> Result<()>;

    /// Unlink function from configuration
    fn unlink(&self, config_path: &Path) -> Result<()>;

    /// Cleanup function directory
    fn cleanup(&self, gadget_path: &Path) -> Result<()>;
}
