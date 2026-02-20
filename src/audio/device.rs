//! Audio device enumeration using ALSA

use alsa::pcm::HwParams;
use alsa::{Direction, PCM};
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};

/// Audio device information
#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    /// Device name (e.g., "hw:0,0" or "default")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Card index
    pub card_index: i32,
    /// Device index
    pub device_index: i32,
    /// Supported sample rates
    pub sample_rates: Vec<u32>,
    /// Supported channel counts
    pub channels: Vec<u32>,
    /// Is this a capture device
    pub is_capture: bool,
    /// Is this an HDMI audio device (likely from capture card)
    pub is_hdmi: bool,
    /// USB bus info for matching with video devices (e.g., "1-1" from USB path)
    pub usb_bus: Option<String>,
}

impl AudioDeviceInfo {
    /// Get ALSA device name
    pub fn alsa_name(&self) -> String {
        format!("hw:{},{}", self.card_index, self.device_index)
    }
}

/// Get USB bus info for an audio card by reading sysfs
/// Returns the USB port path like "1-1" or "1-2.3"
fn get_usb_bus_info(card_index: i32) -> Option<String> {
    if card_index < 0 {
        return None;
    }

    // Read the device symlink: /sys/class/sound/cardX/device -> ../../usb1/1-1/1-1:1.0
    let device_path = format!("/sys/class/sound/card{}/device", card_index);
    let link_target = std::fs::read_link(&device_path).ok()?;
    let link_str = link_target.to_string_lossy();

    // Extract USB port from path like "../../usb1/1-1/1-1:1.0" or "../../1-1/1-1:1.0"
    // We want the "1-1" part (USB bus-port)
    for component in link_str.split('/') {
        // Match patterns like "1-1", "1-2", "1-1.2", "2-1.3.1"
        if component.contains('-') && !component.contains(':') {
            // Verify it looks like a USB port (starts with digit)
            if component
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                return Some(component.to_string());
            }
        }
    }

    None
}

/// Enumerate available audio capture devices
pub fn enumerate_audio_devices() -> Result<Vec<AudioDeviceInfo>> {
    enumerate_audio_devices_with_current(None)
}

/// Enumerate available audio capture devices, with option to include a currently-in-use device
///
/// # Arguments
/// * `current_device` - Optional device name that is currently in use. This device will be
///   included in the list even if it cannot be opened (because it's already open by us).
pub fn enumerate_audio_devices_with_current(
    current_device: Option<&str>,
) -> Result<Vec<AudioDeviceInfo>> {
    let mut devices = Vec::new();

    // Try to enumerate cards
    let cards = alsa::card::Iter::new();

    for card_result in cards {
        let card = match card_result {
            Ok(c) => c,
            Err(e) => {
                debug!("Error iterating card: {}", e);
                continue;
            }
        };

        let card_index = card.get_index();
        let card_name = card.get_name().unwrap_or_else(|_| "Unknown".to_string());
        let card_longname = card.get_longname().unwrap_or_else(|_| card_name.clone());

        debug!("Found audio card {}: {}", card_index, card_longname);

        // Check if this looks like an HDMI capture device
        let is_hdmi = card_longname.to_lowercase().contains("hdmi")
            || card_longname.to_lowercase().contains("capture")
            || card_longname.to_lowercase().contains("usb");

        // Get USB bus info for this card
        let usb_bus = get_usb_bus_info(card_index);

        // Try to open each device on this card for capture
        for device_index in 0..8 {
            let device_name = format!("hw:{},{}", card_index, device_index);

            // Check if this is the currently-in-use device
            let is_current_device = current_device == Some(device_name.as_str());

            // Try to open for capture
            match PCM::new(&device_name, Direction::Capture, false) {
                Ok(pcm) => {
                    // Query capabilities
                    let (sample_rates, channels) = query_device_caps(&pcm);

                    if !sample_rates.is_empty() && !channels.is_empty() {
                        devices.push(AudioDeviceInfo {
                            name: device_name,
                            description: format!("{} - Device {}", card_longname, device_index),
                            card_index,
                            device_index,
                            sample_rates,
                            channels,
                            is_capture: true,
                            is_hdmi,
                            usb_bus: usb_bus.clone(),
                        });
                    }
                }
                Err(_) => {
                    // Device doesn't exist or can't be opened for capture
                    // But if it's the current device, include it anyway (it's busy because we're using it)
                    if is_current_device {
                        debug!(
                            "Device {} is busy (in use by us), adding with default caps",
                            device_name
                        );
                        devices.push(AudioDeviceInfo {
                            name: device_name,
                            description: format!(
                                "{} - Device {} (in use)",
                                card_longname, device_index
                            ),
                            card_index,
                            device_index,
                            // Use common default capabilities for HDMI capture devices
                            sample_rates: vec![44100, 48000],
                            channels: vec![2],
                            is_capture: true,
                            is_hdmi,
                            usb_bus: usb_bus.clone(),
                        });
                    }
                    continue;
                }
            }
        }
    }

    // Also check for "default" device
    if let Ok(pcm) = PCM::new("default", Direction::Capture, false) {
        let (sample_rates, channels) = query_device_caps(&pcm);
        if !sample_rates.is_empty() {
            devices.insert(
                0,
                AudioDeviceInfo {
                    name: "default".to_string(),
                    description: "Default Audio Device".to_string(),
                    card_index: -1,
                    device_index: -1,
                    sample_rates,
                    channels,
                    is_capture: true,
                    is_hdmi: false,
                    usb_bus: None,
                },
            );
        }
    }

    info!("Found {} audio capture devices", devices.len());
    Ok(devices)
}

/// Query device capabilities
fn query_device_caps(pcm: &PCM) -> (Vec<u32>, Vec<u32>) {
    let hwp = match HwParams::any(pcm) {
        Ok(h) => h,
        Err(_) => return (vec![], vec![]),
    };

    // Common sample rates to check
    let common_rates = [8000, 16000, 22050, 44100, 48000, 96000];
    let mut supported_rates = Vec::new();

    for rate in &common_rates {
        if hwp.test_rate(*rate).is_ok() {
            supported_rates.push(*rate);
        }
    }

    // Check channel counts
    let mut supported_channels = Vec::new();
    for ch in 1..=8 {
        if hwp.test_channels(ch).is_ok() {
            supported_channels.push(ch);
        }
    }

    (supported_rates, supported_channels)
}

/// Find the best audio device for capture
/// Prefers HDMI/capture devices over built-in microphones
pub fn find_best_audio_device() -> Result<AudioDeviceInfo> {
    let devices = enumerate_audio_devices()?;

    if devices.is_empty() {
        return Err(AppError::AudioError(
            "No audio capture devices found".to_string(),
        ));
    }

    // First, look for HDMI/capture card devices that support 48kHz stereo
    for device in &devices {
        if device.is_hdmi && device.sample_rates.contains(&48000) && device.channels.contains(&2) {
            info!("Selected HDMI audio device: {}", device.description);
            return Ok(device.clone());
        }
    }

    // Then look for any device supporting 48kHz stereo
    for device in &devices {
        if device.sample_rates.contains(&48000) && device.channels.contains(&2) {
            info!("Selected audio device: {}", device.description);
            return Ok(device.clone());
        }
    }

    // Fall back to first device
    let device = devices.into_iter().next().unwrap();
    warn!(
        "Using fallback audio device: {} (may not support optimal settings)",
        device.description
    );
    Ok(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_devices() {
        // This test may not find devices in CI environment
        let result = enumerate_audio_devices();
        println!("Audio devices: {:?}", result);
        // Just verify it doesn't panic
        assert!(result.is_ok());
    }
}
