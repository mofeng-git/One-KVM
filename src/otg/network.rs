use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use tracing::debug;

use super::configfs::{
    create_dir, create_symlink, remove_dir, remove_file, write_bytes, write_file,
};
use super::function::GadgetFunction;
use crate::config::{OtgNetworkConfig, OtgNetworkDriverMode};
use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct NetworkFunction {
    name: String,
    mode: OtgNetworkDriverMode,
    host_mac: String,
    device_mac: String,
}

impl NetworkFunction {
    pub fn new(instance: u8, config: &OtgNetworkConfig) -> Result<Self> {
        config.validate()?;
        let (device_mac, host_mac) = resolved_mac_pair(config);
        Ok(Self {
            name: format!("{}.usb{}", config.driver_mode.function_name(), instance),
            mode: config.driver_mode,
            host_mac,
            device_mac,
        })
    }

    fn function_path(&self, gadget_path: &Path) -> PathBuf {
        gadget_path.join("functions").join(&self.name)
    }

    pub fn interface_name(&self, gadget_path: &Path) -> Result<String> {
        let path = self.function_path(gadget_path).join("ifname");
        let value = fs::read_to_string(&path).map_err(|e| {
            AppError::Internal(format!(
                "Failed to read OTG network interface from {}: {}",
                path.display(),
                e
            ))
        })?;
        let value = value.trim();
        if value.is_empty() || value.contains('%') {
            return Err(AppError::Internal(format!(
                "Kernel did not allocate an OTG network interface for {}",
                self.name
            )));
        }
        Ok(value.to_string())
    }

    pub fn mode(&self) -> OtgNetworkDriverMode {
        self.mode
    }
}

impl GadgetFunction for NetworkFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn create(&self, gadget_path: &Path) -> Result<()> {
        let function_path = self.function_path(gadget_path);
        create_dir(&function_path)?;
        write_file(&function_path.join("dev_addr"), &self.device_mac)?;
        write_file(&function_path.join("host_addr"), &self.host_mac)?;

        // New kernels accept an unbound interface-name pattern; old kernels expose it read-only.
        let _ = write_file(
            &function_path.join("ifname"),
            &format!("okvm-{}%d", self.mode.function_name()),
        );

        if self.mode == OtgNetworkDriverMode::Rndis {
            write_file(&gadget_path.join("bDeviceClass"), "0xEF")?;
            write_file(&gadget_path.join("bDeviceSubClass"), "0x02")?;
            write_file(&gadget_path.join("bDeviceProtocol"), "0x01")?;
            write_file(&gadget_path.join("os_desc/use"), "1")?;
            write_file(&gadget_path.join("os_desc/b_vendor_code"), "0xcd")?;
            write_bytes(&gadget_path.join("os_desc/qw_sign"), b"MSFT100")?;
            write_file(
                &function_path.join("os_desc/interface.rndis/compatible_id"),
                "RNDIS",
            )?;
            write_file(
                &function_path.join("os_desc/interface.rndis/sub_compatible_id"),
                "5162001",
            )?;
        }

        debug!(
            "Created {} OTG network function {} (device {}, host {})",
            self.mode.function_name(),
            self.name,
            self.device_mac,
            self.host_mac
        );
        Ok(())
    }

    fn link(&self, config_path: &Path, gadget_path: &Path) -> Result<()> {
        let function_path = self.function_path(gadget_path);
        let config_link = config_path.join(&self.name);
        if !config_link.exists() {
            create_symlink(&function_path, &config_link)?;
        }
        if self.mode == OtgNetworkDriverMode::Rndis {
            let os_desc_link = gadget_path.join("os_desc/c.1");
            if !os_desc_link.exists() {
                create_symlink(config_path, &os_desc_link)?;
            }
        }
        Ok(())
    }

    fn unlink(&self, config_path: &Path) -> Result<()> {
        let mut errors = Vec::new();
        if self.mode == OtgNetworkDriverMode::Rndis {
            if let Some(gadget_path) = config_path.parent().and_then(Path::parent) {
                if let Err(error) = remove_file(&gadget_path.join("os_desc/c.1")) {
                    errors.push(error.to_string());
                }
            }
        }
        if let Err(error) = remove_file(&config_path.join(&self.name)) {
            errors.push(error.to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Config(format!(
                "Failed to unlink OTG network function {}: {}",
                self.name,
                errors.join("; ")
            )))
        }
    }

    fn cleanup(&self, gadget_path: &Path) -> Result<()> {
        remove_dir(&self.function_path(gadget_path))
    }
}

pub(crate) fn resolved_mac_pair(config: &OtgNetworkConfig) -> (String, String) {
    if !config.device_mac.is_empty() && !config.host_mac.is_empty() {
        return (config.device_mac.clone(), config.host_mac.clone());
    }

    let identity = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/etc/hostname"))
        .unwrap_or_else(|_| "one-kvm".to_string());
    let mut hasher = DefaultHasher::new();
    identity.trim().hash(&mut hasher);
    let value = hasher.finish().to_be_bytes();
    let device = format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        value[1], value[2], value[3], value[4], value[5]
    );
    let host = format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        value[1],
        value[2],
        value[3],
        value[4],
        value[5] ^ 0x01
    );
    (
        if config.device_mac.is_empty() {
            device
        } else {
            config.device_mac.clone()
        },
        if config.host_mac.is_empty() {
            host
        } else {
            config.host_mac.clone()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_function_uses_selected_driver_name() {
        let function = NetworkFunction::new(0, &OtgNetworkConfig::default()).unwrap();
        assert_eq!(function.name(), "ncm.usb0");
    }
}
