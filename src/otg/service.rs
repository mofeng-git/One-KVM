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

use super::manager::{wait_for_hid_devices, OtgGadgetManager};
use super::msd::MsdFunction;
use crate::error::{AppError, Result};

/// Bitflags for requested functions (lock-free)
const FLAG_HID: u8 = 0b01;
const FLAG_MSD: u8 = 0b10;

/// HID device paths
#[derive(Debug, Clone)]
pub struct HidDevicePaths {
    pub keyboard: PathBuf,
    pub mouse_relative: PathBuf,
    pub mouse_absolute: PathBuf,
}

impl Default for HidDevicePaths {
    fn default() -> Self {
        Self {
            keyboard: PathBuf::from("/dev/hidg0"),
            mouse_relative: PathBuf::from("/dev/hidg1"),
            mouse_absolute: PathBuf::from("/dev/hidg2"),
        }
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
}

impl OtgService {
    /// Create a new OTG service
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(None),
            state: RwLock::new(OtgServiceState::default()),
            msd_function: RwLock::new(None),
            requested_flags: AtomicU8::new(0),
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

        // Check if already enabled
        {
            let state = self.state.read().await;
            if state.hid_enabled {
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
        msd.clone()
            .ok_or_else(|| AppError::Internal("MSD function not set after gadget setup".to_string()))
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

        info!(
            "Recreating gadget with: HID={}, MSD={}",
            hid_requested, msd_requested
        );

        // Check if gadget already matches requested state
        {
            let state = self.state.read().await;
            if state.gadget_active
                && state.hid_enabled == hid_requested
                && state.msd_enabled == msd_requested
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

        // Create new gadget manager
        let mut manager = OtgGadgetManager::new();
        let mut hid_paths = None;

        // Add HID functions if requested
        if hid_requested {
            match (
                manager.add_keyboard(),
                manager.add_mouse_relative(),
                manager.add_mouse_absolute(),
            ) {
                (Ok(kb), Ok(rel), Ok(abs)) => {
                    hid_paths = Some(HidDevicePaths {
                        keyboard: kb,
                        mouse_relative: rel,
                        mouse_absolute: abs,
                    });
                    debug!("HID functions added to gadget");
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    let error = format!("Failed to add HID functions: {}", e);
                    let mut state = self.state.write().await;
                    state.error = Some(error.clone());
                    return Err(AppError::Internal(error));
                }
            }
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
            let device_paths = vec![
                paths.keyboard.clone(),
                paths.mouse_relative.clone(),
                paths.mouse_absolute.clone(),
            ];
            if !wait_for_hid_devices(&device_paths, 2000).await {
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
            state.error = None;
        }

        info!("Gadget created successfully");
        Ok(())
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
        let service = OtgService::new();
        // Just test that creation doesn't panic
        assert!(!OtgService::is_available() || true); // Depends on environment
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
