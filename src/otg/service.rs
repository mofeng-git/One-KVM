//! OTG Service - unified gadget lifecycle management
//!
//! This module provides centralized management for USB OTG gadget functions.
//! It solves the ownership problem where both HID and MSD need access to the
//! same USB gadget but should be independently configurable.
//!
//! Architecture:
//! ```text
//!                    ┌─────────────────────────┐
//!                    │       OtgService        │
//!                    │  ┌───────────────────┐  │
//!                    │  │ OtgGadgetManager  │  │
//!                    │  └───────────────────┘  │
//!                    │     ↓           ↓       │
//!                    │  ┌─────┐     ┌─────┐   │
//!                    │  │ HID │     │ MSD │   │
//!                    │  └─────┘     └─────┘   │
//!                    └─────────────────────────┘
//!                         ↑           ↑
//!                    HidController  MsdController
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use super::manager::{wait_for_hid_devices, GadgetDescriptor, OtgGadgetManager};
use super::msd::MsdFunction;
use crate::config::{OtgDescriptorConfig, OtgHidFunctions};
use crate::error::{AppError, Result};

/// Bitflags for requested functions (lock-free)
const FLAG_HID: u8 = 0b01;
const FLAG_MSD: u8 = 0b10;

/// HID device paths
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct HidDevicePaths {
    pub keyboard: Option<PathBuf>,
    pub mouse_relative: Option<PathBuf>,
    pub mouse_absolute: Option<PathBuf>,
    pub consumer: Option<PathBuf>,
}


impl HidDevicePaths {
    pub fn existing_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(ref p) = self.keyboard {
            paths.push(p.clone());
        }
        if let Some(ref p) = self.mouse_relative {
            paths.push(p.clone());
        }
        if let Some(ref p) = self.mouse_absolute {
            paths.push(p.clone());
        }
        if let Some(ref p) = self.consumer {
            paths.push(p.clone());
        }
        paths
    }
}

/// OTG Service state
#[derive(Debug, Clone, Default)]
pub struct OtgServiceState {
    /// Whether the gadget is created and bound
    pub gadget_active: bool,
    /// Whether HID functions are enabled
    pub hid_enabled: bool,
    /// Whether MSD function is enabled
    pub msd_enabled: bool,
    /// HID device paths (set after gadget setup)
    pub hid_paths: Option<HidDevicePaths>,
    /// HID function selection (set after gadget setup)
    pub hid_functions: Option<OtgHidFunctions>,
    /// Error message if setup failed
    pub error: Option<String>,
}

/// OTG Service - unified gadget lifecycle management
///
/// This service owns the OtgGadgetManager and provides a high-level interface
/// for enabling/disabling HID and MSD functions. It ensures proper coordination
/// between the two subsystems and handles gadget lifecycle management.
pub struct OtgService {
    /// The underlying gadget manager
    manager: Mutex<Option<OtgGadgetManager>>,
    /// Current state
    state: RwLock<OtgServiceState>,
    /// MSD function handle (for runtime LUN configuration)
    msd_function: RwLock<Option<MsdFunction>>,
    /// Requested functions flags (atomic, lock-free read/write)
    requested_flags: AtomicU8,
    /// Requested HID function set
    hid_functions: RwLock<OtgHidFunctions>,
    /// Current descriptor configuration
    current_descriptor: RwLock<GadgetDescriptor>,
}

impl OtgService {
    /// Create a new OTG service
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(None),
            state: RwLock::new(OtgServiceState::default()),
            msd_function: RwLock::new(None),
            requested_flags: AtomicU8::new(0),
            hid_functions: RwLock::new(OtgHidFunctions::default()),
            current_descriptor: RwLock::new(GadgetDescriptor::default()),
        }
    }

    /// Check if HID is requested (lock-free)
    #[inline]
    fn is_hid_requested(&self) -> bool {
        self.requested_flags.load(Ordering::Acquire) & FLAG_HID != 0
    }

    /// Check if MSD is requested (lock-free)
    #[inline]
    fn is_msd_requested(&self) -> bool {
        self.requested_flags.load(Ordering::Acquire) & FLAG_MSD != 0
    }

    /// Set HID requested flag (lock-free)
    #[inline]
    fn set_hid_requested(&self, requested: bool) {
        if requested {
            self.requested_flags.fetch_or(FLAG_HID, Ordering::Release);
        } else {
            self.requested_flags.fetch_and(!FLAG_HID, Ordering::Release);
        }
    }

    /// Set MSD requested flag (lock-free)
    #[inline]
    fn set_msd_requested(&self, requested: bool) {
        if requested {
            self.requested_flags.fetch_or(FLAG_MSD, Ordering::Release);
        } else {
            self.requested_flags.fetch_and(!FLAG_MSD, Ordering::Release);
        }
    }

    /// Check if OTG is available on this system
    pub fn is_available() -> bool {
        OtgGadgetManager::is_available() && OtgGadgetManager::find_udc().is_some()
    }

    /// Get current service state
    pub async fn state(&self) -> OtgServiceState {
        self.state.read().await.clone()
    }

    /// Check if gadget is active
    pub async fn is_gadget_active(&self) -> bool {
        self.state.read().await.gadget_active
    }

    /// Check if HID is enabled
    pub async fn is_hid_enabled(&self) -> bool {
        self.state.read().await.hid_enabled
    }

    /// Check if MSD is enabled
    pub async fn is_msd_enabled(&self) -> bool {
        self.state.read().await.msd_enabled
    }

    /// Get gadget path (for MSD LUN configuration)
    pub async fn gadget_path(&self) -> Option<PathBuf> {
        let manager = self.manager.lock().await;
        manager.as_ref().map(|m| m.gadget_path().clone())
    }

    /// Get HID device paths
    pub async fn hid_device_paths(&self) -> Option<HidDevicePaths> {
        self.state.read().await.hid_paths.clone()
    }

    /// Get current HID function selection
    pub async fn hid_functions(&self) -> OtgHidFunctions {
        self.hid_functions.read().await.clone()
    }

    /// Update HID function selection
    pub async fn update_hid_functions(&self, functions: OtgHidFunctions) -> Result<()> {
        if functions.is_empty() {
            return Err(AppError::BadRequest(
                "OTG HID functions cannot be empty".to_string(),
            ));
        }

        {
            let mut current = self.hid_functions.write().await;
            if *current == functions {
                return Ok(());
            }
            *current = functions;
        }

        // If HID is active, recreate gadget with new function set
        if self.is_hid_requested() {
            self.recreate_gadget().await?;
        }

        Ok(())
    }

    /// Get MSD function handle (for LUN configuration)
    pub async fn msd_function(&self) -> Option<MsdFunction> {
        self.msd_function.read().await.clone()
    }

    /// Enable HID functions
    ///
    /// This will create the gadget if not already created, add HID functions,
    /// and bind the gadget to UDC.
    pub async fn enable_hid(&self) -> Result<HidDevicePaths> {
        info!("Enabling HID functions via OtgService");

        // Mark HID as requested (lock-free)
        self.set_hid_requested(true);

        // Check if already enabled and function set unchanged
        let requested_functions = self.hid_functions.read().await.clone();
        {
            let state = self.state.read().await;
            if state.hid_enabled
                && state.hid_functions.as_ref() == Some(&requested_functions) {
                    if let Some(ref paths) = state.hid_paths {
                        info!("HID already enabled, returning existing paths");
                        return Ok(paths.clone());
                    }
                }
        }

        // Recreate gadget with both HID and MSD if needed
        self.recreate_gadget().await?;

        // Get HID paths from state
        let state = self.state.read().await;
        state
            .hid_paths
            .clone()
            .ok_or_else(|| AppError::Internal("HID paths not set after gadget setup".to_string()))
    }

    /// Disable HID functions
    ///
    /// This will unbind the gadget, remove HID functions, and optionally
    /// recreate the gadget with only MSD if MSD is still enabled.
    pub async fn disable_hid(&self) -> Result<()> {
        info!("Disabling HID functions via OtgService");

        // Mark HID as not requested (lock-free)
        self.set_hid_requested(false);

        // Check if HID is enabled
        {
            let state = self.state.read().await;
            if !state.hid_enabled {
                info!("HID already disabled");
                return Ok(());
            }
        }

        // Recreate gadget without HID (or destroy if MSD also disabled)
        self.recreate_gadget().await
    }

    /// Enable MSD function
    ///
    /// This will create the gadget if not already created, add MSD function,
    /// and bind the gadget to UDC.
    pub async fn enable_msd(&self) -> Result<MsdFunction> {
        info!("Enabling MSD function via OtgService");

        // Mark MSD as requested (lock-free)
        self.set_msd_requested(true);

        // Check if already enabled
        {
            let state = self.state.read().await;
            if state.msd_enabled {
                let msd = self.msd_function.read().await;
                if let Some(ref func) = *msd {
                    info!("MSD already enabled, returning existing function");
                    return Ok(func.clone());
                }
            }
        }

        // Recreate gadget with both HID and MSD if needed
        self.recreate_gadget().await?;

        // Get MSD function
        let msd = self.msd_function.read().await;
        msd.clone().ok_or_else(|| {
            AppError::Internal("MSD function not set after gadget setup".to_string())
        })
    }

    /// Disable MSD function
    ///
    /// This will unbind the gadget, remove MSD function, and optionally
    /// recreate the gadget with only HID if HID is still enabled.
    pub async fn disable_msd(&self) -> Result<()> {
        info!("Disabling MSD function via OtgService");

        // Mark MSD as not requested (lock-free)
        self.set_msd_requested(false);

        // Check if MSD is enabled
        {
            let state = self.state.read().await;
            if !state.msd_enabled {
                info!("MSD already disabled");
                return Ok(());
            }
        }

        // Recreate gadget without MSD (or destroy if HID also disabled)
        self.recreate_gadget().await
    }

    /// Recreate the gadget with currently requested functions
    ///
    /// This is called whenever the set of enabled functions changes.
    /// It will:
    /// 1. Check if recreation is needed (function set changed)
    /// 2. If needed: cleanup existing gadget
    /// 3. Create new gadget with requested functions
    /// 4. Setup and bind
    async fn recreate_gadget(&self) -> Result<()> {
        // Read requested flags atomically (lock-free)
        let hid_requested = self.is_hid_requested();
        let msd_requested = self.is_msd_requested();
        let hid_functions = if hid_requested {
            self.hid_functions.read().await.clone()
        } else {
            OtgHidFunctions::default()
        };

        info!(
            "Recreating gadget with: HID={}, MSD={}",
            hid_requested, msd_requested
        );

        // Check if gadget already matches requested state
        {
            let state = self.state.read().await;
            let functions_match = if hid_requested {
                state.hid_functions.as_ref() == Some(&hid_functions)
            } else {
                state.hid_functions.is_none()
            };
            if state.gadget_active
                && state.hid_enabled == hid_requested
                && state.msd_enabled == msd_requested
                && functions_match
            {
                info!("Gadget already has requested functions, skipping recreate");
                return Ok(());
            }
        }

        // Cleanup existing gadget
        {
            let mut manager = self.manager.lock().await;
            if let Some(mut m) = manager.take() {
                info!("Cleaning up existing gadget before recreate");
                if let Err(e) = m.cleanup() {
                    warn!("Error cleaning up existing gadget: {}", e);
                }
            }
        }

        // Clear MSD function
        *self.msd_function.write().await = None;

        // Update state to inactive
        {
            let mut state = self.state.write().await;
            state.gadget_active = false;
            state.hid_enabled = false;
            state.msd_enabled = false;
            state.hid_paths = None;
            state.hid_functions = None;
            state.error = None;
        }

        // If nothing requested, we're done
        if !hid_requested && !msd_requested {
            info!("No functions requested, gadget destroyed");
            return Ok(());
        }

        // Check if OTG is available
        if !Self::is_available() {
            let error = "OTG not available: ConfigFS not mounted or no UDC found".to_string();
            let mut state = self.state.write().await;
            state.error = Some(error.clone());
            return Err(AppError::Internal(error));
        }

        // Create new gadget manager with current descriptor
        let descriptor = self.current_descriptor.read().await.clone();
        let mut manager = OtgGadgetManager::with_descriptor(
            super::configfs::DEFAULT_GADGET_NAME,
            super::endpoint::DEFAULT_MAX_ENDPOINTS,
            descriptor,
        );
        let mut hid_paths = None;

        // Add HID functions if requested
        if hid_requested {
            if hid_functions.is_empty() {
                let error = "HID functions set is empty".to_string();
                let mut state = self.state.write().await;
                state.error = Some(error.clone());
                return Err(AppError::BadRequest(error));
            }

            let mut paths = HidDevicePaths::default();

            if hid_functions.keyboard {
                match manager.add_keyboard() {
                    Ok(kb) => paths.keyboard = Some(kb),
                    Err(e) => {
                        let error = format!("Failed to add keyboard HID function: {}", e);
                        let mut state = self.state.write().await;
                        state.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.mouse_relative {
                match manager.add_mouse_relative() {
                    Ok(rel) => paths.mouse_relative = Some(rel),
                    Err(e) => {
                        let error = format!("Failed to add relative mouse HID function: {}", e);
                        let mut state = self.state.write().await;
                        state.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.mouse_absolute {
                match manager.add_mouse_absolute() {
                    Ok(abs) => paths.mouse_absolute = Some(abs),
                    Err(e) => {
                        let error = format!("Failed to add absolute mouse HID function: {}", e);
                        let mut state = self.state.write().await;
                        state.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.consumer {
                match manager.add_consumer_control() {
                    Ok(consumer) => paths.consumer = Some(consumer),
                    Err(e) => {
                        let error = format!("Failed to add consumer HID function: {}", e);
                        let mut state = self.state.write().await;
                        state.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            hid_paths = Some(paths);
            debug!("HID functions added to gadget");
        }

        // Add MSD function if requested
        let msd_func = if msd_requested {
            match manager.add_msd() {
                Ok(func) => {
                    debug!("MSD function added to gadget");
                    Some(func)
                }
                Err(e) => {
                    let error = format!("Failed to add MSD function: {}", e);
                    let mut state = self.state.write().await;
                    state.error = Some(error.clone());
                    return Err(AppError::Internal(error));
                }
            }
        } else {
            None
        };

        // Setup gadget
        if let Err(e) = manager.setup() {
            let error = format!("Failed to setup gadget: {}", e);
            let mut state = self.state.write().await;
            state.error = Some(error.clone());
            return Err(AppError::Internal(error));
        }

        // Bind to UDC
        if let Err(e) = manager.bind() {
            let error = format!("Failed to bind gadget to UDC: {}", e);
            let mut state = self.state.write().await;
            state.error = Some(error.clone());
            // Cleanup on failure
            let _ = manager.cleanup();
            return Err(AppError::Internal(error));
        }

        // Wait for HID devices to appear
        if let Some(ref paths) = hid_paths {
            let device_paths = paths.existing_paths();
            if !device_paths.is_empty() && !wait_for_hid_devices(&device_paths, 2000).await {
                warn!("HID devices did not appear after gadget setup");
            }
        }

        // Store manager and update state
        {
            *self.manager.lock().await = Some(manager);
        }

        {
            *self.msd_function.write().await = msd_func;
        }

        {
            let mut state = self.state.write().await;
            state.gadget_active = true;
            state.hid_enabled = hid_requested;
            state.msd_enabled = msd_requested;
            state.hid_paths = hid_paths;
            state.hid_functions = if hid_requested {
                Some(hid_functions)
            } else {
                None
            };
            state.error = None;
        }

        info!("Gadget created successfully");
        Ok(())
    }

    /// Update the descriptor configuration
    ///
    /// This updates the stored descriptor and triggers a gadget recreation
    /// if the gadget is currently active.
    pub async fn update_descriptor(&self, config: &OtgDescriptorConfig) -> Result<()> {
        let new_descriptor = GadgetDescriptor {
            vendor_id: config.vendor_id,
            product_id: config.product_id,
            device_version: super::configfs::DEFAULT_USB_BCD_DEVICE,
            manufacturer: config.manufacturer.clone(),
            product: config.product.clone(),
            serial_number: config
                .serial_number
                .clone()
                .unwrap_or_else(|| "0123456789".to_string()),
        };

        // Update stored descriptor
        *self.current_descriptor.write().await = new_descriptor;

        // If gadget is active, recreate it with new descriptor
        let state = self.state.read().await;
        if state.gadget_active {
            drop(state); // Release read lock before calling recreate
            info!("Descriptor changed, recreating gadget");
            self.force_recreate_gadget().await?;
        }

        Ok(())
    }

    /// Force recreate the gadget (used when descriptor changes)
    async fn force_recreate_gadget(&self) -> Result<()> {
        // Cleanup existing gadget
        {
            let mut manager = self.manager.lock().await;
            if let Some(mut m) = manager.take() {
                info!("Cleaning up existing gadget for descriptor change");
                if let Err(e) = m.cleanup() {
                    warn!("Error cleaning up existing gadget: {}", e);
                }
            }
        }

        // Clear MSD function
        *self.msd_function.write().await = None;

        // Update state to inactive
        {
            let mut state = self.state.write().await;
            state.gadget_active = false;
            state.hid_enabled = false;
            state.msd_enabled = false;
            state.hid_paths = None;
            state.hid_functions = None;
            state.error = None;
        }

        // Recreate with current requested functions
        self.recreate_gadget().await
    }

    /// Shutdown the OTG service and cleanup all resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down OTG service");

        // Mark nothing as requested (lock-free)
        self.requested_flags.store(0, Ordering::Release);

        // Cleanup gadget
        let mut manager = self.manager.lock().await;
        if let Some(mut m) = manager.take() {
            if let Err(e) = m.cleanup() {
                warn!("Error cleaning up gadget during shutdown: {}", e);
            }
        }

        // Clear state
        *self.msd_function.write().await = None;
        {
            let mut state = self.state.write().await;
            *state = OtgServiceState::default();
        }

        info!("OTG service shutdown complete");
        Ok(())
    }
}

impl Default for OtgService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OtgService {
    fn drop(&mut self) {
        // Gadget cleanup is handled by OtgGadgetManager's Drop
        debug!("OtgService dropping");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let _service = OtgService::new();
        // Just test that creation doesn't panic
        let _ = OtgService::is_available(); // Depends on environment
    }

    #[tokio::test]
    async fn test_initial_state() {
        let service = OtgService::new();
        let state = service.state().await;
        assert!(!state.gadget_active);
        assert!(!state.hid_enabled);
        assert!(!state.msd_enabled);
    }
}
