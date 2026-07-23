use std::path::{Path, PathBuf};

use tracing::{debug, info};

use super::configfs::{create_dir, create_symlink, remove_dir, write_file};
use super::function::GadgetFunction;
use crate::error::{AppError, Result};

/// USB Audio Class 2.0 (UAC1) gadget function.
///
/// Creates a virtual USB microphone that the USB host sees as a standard
/// USB audio input device. Audio written to the PCM playback device on the
/// gadget side appears as microphone input on the host.
#[derive(Debug, Clone)]
pub struct UacFunction {
    name: String,
    sample_rate: u32,
    channels: u8,
}

impl UacFunction {
    /// Create a new UAC1 function instance.
    ///
    /// `instance` is a zero-based index to avoid name collisions
    /// (e.g. `uac2.usb0`).
    pub fn new(instance: u8, sample_rate: u32, channels: u8) -> Result<Self> {
        if sample_rate == 0 || sample_rate > 384_000 {
            return Err(AppError::BadRequest(format!(
                "invalid UAC sample rate: {sample_rate}"
            )));
        }
        if channels == 0 || channels > 8 {
            return Err(AppError::BadRequest(format!(
                "invalid UAC channel count: {channels}"
            )));
        }
        Ok(Self {
            name: format!("uac1.usb{instance}"),
            sample_rate,
            channels,
        })
    }

    fn function_path(&self, gadget_path: &Path) -> PathBuf {
        gadget_path.join("functions").join(&self.name)
    }
}

impl GadgetFunction for UacFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn create(&self, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        create_dir(&func_path)?;

        // Playback direction (gadget → host): the controlled machine sees
        // this as a microphone/line-in input.
        let chmask: u32 = (1u32 << self.channels) - 1;
        write_file(&func_path.join("p_chmask"), &chmask.to_string())?;
        write_file(&func_path.join("p_srate"), &self.sample_rate.to_string())?;
        write_file(&func_path.join("p_ssize"), "2")?; // 16-bit S16LE
        // UAC1 does not need p_hs_bint — Windows has native built-in
        // UAC1 drivers and handles isochronous streaming automatically.

        // Only enable playback direction (gadget → host = mic).
        // Disabling capture saves one isochronous endpoint.
        write_file(&func_path.join("c_chmask"), "0")?;

        // req_number=4: explicitly allocate 4 USB requests for the
        // isochronous endpoint.  Default (0 = auto) may not be enough
        // for composite gadgets on DWC3.
        let _ = write_file(&func_path.join("req_number"), "4");

        debug!(
            "UAC1 function {} created: {}ch {}Hz",
            &self.name, self.channels, self.sample_rate
        );
        Ok(())
    }

    fn link(&self, config_path: &Path, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        let link_path = config_path.join(&self.name);
        create_symlink(&func_path, &link_path)?;
        debug!("UAC1 function {} linked into configuration", &self.name);
        Ok(())
    }

    fn unlink(&self, config_path: &Path) -> Result<()> {
        let link_path = config_path.join(&self.name);
        if link_path.exists() {
            std::fs::remove_file(&link_path).map_err(|e| {
                AppError::Internal(format!(
                    "Failed to unlink UAC1 function {}: {}",
                    &self.name, e
                ))
            })?;
            debug!("UAC1 function {} unlinked", &self.name);
        }
        Ok(())
    }

    fn cleanup(&self, gadget_path: &Path) -> Result<()> {
        let func_path = self.function_path(gadget_path);
        if func_path.exists() {
            remove_dir(&func_path).map_err(|e| {
                AppError::Internal(format!(
                    "Failed to remove UAC1 function {}: {}",
                    &self.name, e
                ))
            })?;
            info!("UAC1 function {} removed", &self.name);
        }
        Ok(())
    }
}

/// Return the ALSA PCM device name that the kernel assigns to a UAC1
/// gadget after binding. The device appears as a playback-only PCM on
/// the gadget side.
pub fn uac_pcm_device() -> String {
    // The kernel assigns the card name based on the gadget name.
    // The PCM name is typically "playback" for UAC1.
    "hw:UAC1Gadget,0".to_string()
}

/// Resolve the actual PCM device name for a UAC1 playback device
/// by scanning /proc/asound/ for the gadget audio card.
pub fn find_uac_pcm_device() -> Option<String> {
    for entry in std::fs::read_dir("/proc/asound").ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_str()?;
        if !name.starts_with("card") {
            continue;
        }
        let card_path = entry.path().join("id");
        if let Ok(id) = std::fs::read_to_string(&card_path) {
            if id.trim().starts_with("UAC1Gadget") || id.trim().starts_with("gadget") {
                let card_num = name.strip_prefix("card")?;
                return Some(format!("hw:{card_num},0"));
            }
        }
    }
    None
}
