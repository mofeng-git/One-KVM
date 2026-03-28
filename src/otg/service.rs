//! OTG Service - unified gadget lifecycle management
//!
//! This module provides centralized management for USB OTG gadget functions.
//! It is the single owner of the USB gadget desired state and reconciles
//! ConfigFS to match that state.

use std::path::PathBuf;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use super::manager::{wait_for_hid_devices, GadgetDescriptor, OtgGadgetManager};
use super::msd::MsdFunction;
use crate::config::{HidBackend, HidConfig, MsdConfig, OtgDescriptorConfig, OtgHidFunctions};
use crate::error::{AppError, Result};

/// HID device paths
#[derive(Debug, Clone, Default)]
pub struct HidDevicePaths {
    pub keyboard: Option<PathBuf>,
    pub mouse_relative: Option<PathBuf>,
    pub mouse_absolute: Option<PathBuf>,
    pub consumer: Option<PathBuf>,
    pub udc: Option<String>,
    pub keyboard_leds_enabled: bool,
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

/// Desired OTG gadget state derived from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtgDesiredState {
    pub udc: Option<String>,
    pub descriptor: GadgetDescriptor,
    pub hid_functions: Option<OtgHidFunctions>,
    pub keyboard_leds: bool,
    pub msd_enabled: bool,
    pub max_endpoints: u8,
}

impl Default for OtgDesiredState {
    fn default() -> Self {
        Self {
            udc: None,
            descriptor: GadgetDescriptor::default(),
            hid_functions: None,
            keyboard_leds: false,
            msd_enabled: false,
            max_endpoints: super::endpoint::DEFAULT_MAX_ENDPOINTS,
        }
    }
}

impl OtgDesiredState {
    pub fn from_config(hid: &HidConfig, msd: &MsdConfig) -> Result<Self> {
        let hid_functions = if hid.backend == HidBackend::Otg {
            let functions = hid.constrained_otg_functions();
            Some(functions)
        } else {
            None
        };

        hid.validate_otg_endpoint_budget(msd.enabled)?;

        Ok(Self {
            udc: hid.resolved_otg_udc(),
            descriptor: GadgetDescriptor::from(&hid.otg_descriptor),
            hid_functions,
            keyboard_leds: hid.effective_otg_keyboard_leds(),
            msd_enabled: msd.enabled,
            max_endpoints: hid
                .resolved_otg_endpoint_limit()
                .unwrap_or(super::endpoint::DEFAULT_MAX_ENDPOINTS),
        })
    }

    #[inline]
    pub fn hid_enabled(&self) -> bool {
        self.hid_functions.is_some()
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
    /// Bound UDC name
    pub configured_udc: Option<String>,
    /// HID device paths (set after gadget setup)
    pub hid_paths: Option<HidDevicePaths>,
    /// HID function selection (set after gadget setup)
    pub hid_functions: Option<OtgHidFunctions>,
    /// Whether keyboard LED/status feedback is enabled.
    pub keyboard_leds_enabled: bool,
    /// Applied endpoint budget.
    pub max_endpoints: u8,
    /// Applied descriptor configuration
    pub descriptor: Option<GadgetDescriptor>,
    /// Error message if setup failed
    pub error: Option<String>,
}

/// OTG Service - unified gadget lifecycle management
pub struct OtgService {
    /// The underlying gadget manager
    manager: Mutex<Option<OtgGadgetManager>>,
    /// Current state
    state: RwLock<OtgServiceState>,
    /// MSD function handle (for runtime LUN configuration)
    msd_function: RwLock<Option<MsdFunction>>,
    /// Desired OTG state
    desired: RwLock<OtgDesiredState>,
}

impl OtgService {
    /// Create a new OTG service
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(None),
            state: RwLock::new(OtgServiceState::default()),
            msd_function: RwLock::new(None),
            desired: RwLock::new(OtgDesiredState::default()),
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

    /// Apply desired OTG state derived from the current application config.
    pub async fn apply_config(&self, hid: &HidConfig, msd: &MsdConfig) -> Result<()> {
        let desired = OtgDesiredState::from_config(hid, msd)?;
        self.apply_desired_state(desired).await
    }

    /// Apply a fully materialized desired OTG state.
    pub async fn apply_desired_state(&self, desired: OtgDesiredState) -> Result<()> {
        {
            let mut current = self.desired.write().await;
            *current = desired;
        }

        self.reconcile_gadget().await
    }

    async fn reconcile_gadget(&self) -> Result<()> {
        let desired = self.desired.read().await.clone();

        info!(
            "Reconciling OTG gadget: HID={}, MSD={}, UDC={:?}",
            desired.hid_enabled(),
            desired.msd_enabled,
            desired.udc
        );

        {
            let state = self.state.read().await;
            if state.gadget_active
                && state.hid_enabled == desired.hid_enabled()
                && state.msd_enabled == desired.msd_enabled
                && state.configured_udc == desired.udc
                && state.hid_functions == desired.hid_functions
                && state.keyboard_leds_enabled == desired.keyboard_leds
                && state.max_endpoints == desired.max_endpoints
                && state.descriptor.as_ref() == Some(&desired.descriptor)
            {
                info!("OTG gadget already matches desired state");
                return Ok(());
            }
        }

        {
            let mut manager = self.manager.lock().await;
            if let Some(mut m) = manager.take() {
                info!("Cleaning up existing gadget before OTG reconcile");
                if let Err(e) = m.cleanup() {
                    warn!("Error cleaning up existing gadget: {}", e);
                }
            }
        }

        *self.msd_function.write().await = None;

        {
            let mut state = self.state.write().await;
            state.gadget_active = false;
            state.hid_enabled = false;
            state.msd_enabled = false;
            state.configured_udc = None;
            state.hid_paths = None;
            state.hid_functions = None;
            state.keyboard_leds_enabled = false;
            state.max_endpoints = super::endpoint::DEFAULT_MAX_ENDPOINTS;
            state.descriptor = None;
            state.error = None;
        }

        if !desired.hid_enabled() && !desired.msd_enabled {
            info!("OTG desired state is empty, gadget removed");
            return Ok(());
        }

        if let Err(e) = super::configfs::ensure_libcomposite_loaded() {
            warn!("Failed to ensure libcomposite is available: {}", e);
        }

        if !OtgGadgetManager::is_available() {
            let error = "OTG not available: ConfigFS not mounted".to_string();
            self.state.write().await.error = Some(error.clone());
            return Err(AppError::Internal(error));
        }

        let udc = desired.udc.clone().ok_or_else(|| {
            let error = "OTG not available: no UDC found".to_string();
            AppError::Internal(error)
        })?;

        let mut manager = OtgGadgetManager::with_descriptor(
            super::configfs::DEFAULT_GADGET_NAME,
            desired.max_endpoints,
            desired.descriptor.clone(),
        );

        let mut hid_paths = None;
        if let Some(hid_functions) = desired.hid_functions.clone() {
            let mut paths = HidDevicePaths {
                udc: Some(udc.clone()),
                keyboard_leds_enabled: desired.keyboard_leds,
                ..Default::default()
            };

            if hid_functions.keyboard {
                match manager.add_keyboard(desired.keyboard_leds) {
                    Ok(kb) => paths.keyboard = Some(kb),
                    Err(e) => {
                        let error = format!("Failed to add keyboard HID function: {}", e);
                        self.state.write().await.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.mouse_relative {
                match manager.add_mouse_relative() {
                    Ok(rel) => paths.mouse_relative = Some(rel),
                    Err(e) => {
                        let error = format!("Failed to add relative mouse HID function: {}", e);
                        self.state.write().await.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.mouse_absolute {
                match manager.add_mouse_absolute() {
                    Ok(abs) => paths.mouse_absolute = Some(abs),
                    Err(e) => {
                        let error = format!("Failed to add absolute mouse HID function: {}", e);
                        self.state.write().await.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            if hid_functions.consumer {
                match manager.add_consumer_control() {
                    Ok(consumer) => paths.consumer = Some(consumer),
                    Err(e) => {
                        let error = format!("Failed to add consumer HID function: {}", e);
                        self.state.write().await.error = Some(error.clone());
                        return Err(AppError::Internal(error));
                    }
                }
            }

            hid_paths = Some(paths);
            debug!("HID functions added to gadget");
        }

        let msd_func = if desired.msd_enabled {
            match manager.add_msd() {
                Ok(func) => {
                    debug!("MSD function added to gadget");
                    Some(func)
                }
                Err(e) => {
                    let error = format!("Failed to add MSD function: {}", e);
                    self.state.write().await.error = Some(error.clone());
                    return Err(AppError::Internal(error));
                }
            }
        } else {
            None
        };

        if let Err(e) = manager.setup() {
            let error = format!("Failed to setup gadget: {}", e);
            self.state.write().await.error = Some(error.clone());
            return Err(AppError::Internal(error));
        }

        if let Err(e) = manager.bind(&udc) {
            let error = format!("Failed to bind gadget to UDC {}: {}", udc, e);
            self.state.write().await.error = Some(error.clone());
            let _ = manager.cleanup();
            return Err(AppError::Internal(error));
        }

        if let Some(ref paths) = hid_paths {
            let device_paths = paths.existing_paths();
            if !device_paths.is_empty() && !wait_for_hid_devices(&device_paths, 2000).await {
                warn!("HID devices did not appear after gadget setup");
            }
        }

        *self.manager.lock().await = Some(manager);
        *self.msd_function.write().await = msd_func;

        {
            let mut state = self.state.write().await;
            state.gadget_active = true;
            state.hid_enabled = desired.hid_enabled();
            state.msd_enabled = desired.msd_enabled;
            state.configured_udc = Some(udc);
            state.hid_paths = hid_paths;
            state.hid_functions = desired.hid_functions;
            state.keyboard_leds_enabled = desired.keyboard_leds;
            state.max_endpoints = desired.max_endpoints;
            state.descriptor = Some(desired.descriptor);
            state.error = None;
        }

        info!("OTG gadget reconciled successfully");
        Ok(())
    }

    /// Shutdown the OTG service and cleanup all resources
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down OTG service");

        {
            let mut desired = self.desired.write().await;
            *desired = OtgDesiredState::default();
        }

        let mut manager = self.manager.lock().await;
        if let Some(mut m) = manager.take() {
            if let Err(e) = m.cleanup() {
                warn!("Error cleaning up gadget during shutdown: {}", e);
            }
        }

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
        debug!("OtgService dropping");
    }
}

impl From<&OtgDescriptorConfig> for GadgetDescriptor {
    fn from(config: &OtgDescriptorConfig) -> Self {
        Self {
            vendor_id: config.vendor_id,
            product_id: config.product_id,
            device_version: super::configfs::DEFAULT_USB_BCD_DEVICE,
            manufacturer: config.manufacturer.clone(),
            product: config.product.clone(),
            serial_number: config
                .serial_number
                .clone()
                .unwrap_or_else(|| "0123456789".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let _service = OtgService::new();
        let _ = OtgService::is_available();
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
