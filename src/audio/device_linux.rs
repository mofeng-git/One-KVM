use alsa::pcm::HwParams;
use alsa::{Direction, PCM};
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub description: String,
    pub card_index: i32,
    pub device_index: i32,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u32>,
    pub is_capture: bool,
    pub is_hdmi: bool,
    pub usb_bus: Option<String>,
}

fn get_usb_bus_info(card_index: i32) -> Option<String> {
    if card_index < 0 {
        return None;
    }

    let device_path = format!("/sys/class/sound/card{}/device", card_index);
    let link_target = std::fs::read_link(&device_path).ok()?;
    let link_str = link_target.to_string_lossy();

    for component in link_str.split('/') {
        if component.contains('-') && !component.contains(':') {
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

pub fn enumerate_audio_devices() -> Result<Vec<AudioDeviceInfo>> {
    enumerate_audio_devices_with_current(None)
}

pub fn enumerate_audio_devices_with_current(
    current_device: Option<&str>,
) -> Result<Vec<AudioDeviceInfo>> {
    let mut devices = Vec::new();

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

        let long_lower = card_longname.to_lowercase();
        let is_hdmi = long_lower.contains("hdmi")
            || long_lower.contains("capture")
            || long_lower.contains("usb");

        let usb_bus = get_usb_bus_info(card_index);

        for device_index in 0..8 {
            let device_name = format!("hw:{},{}", card_index, device_index);
            let is_current_device = current_device == Some(device_name.as_str());

            let mut push_info =
                |sample_rates: Vec<u32>, channels: Vec<u32>, description: String| {
                    devices.push(AudioDeviceInfo {
                        name: device_name.clone(),
                        description,
                        card_index,
                        device_index,
                        sample_rates,
                        channels,
                        is_capture: true,
                        is_hdmi,
                        usb_bus: usb_bus.clone(),
                    });
                };

            match PCM::new(&device_name, Direction::Capture, false) {
                Ok(pcm) => {
                    let (sample_rates, channels) = query_device_caps(&pcm);

                    if !sample_rates.is_empty() && !channels.is_empty() {
                        push_info(
                            sample_rates,
                            channels,
                            format!("{} - Device {}", card_longname, device_index),
                        );
                    }
                }
                Err(_) => {
                    if is_current_device {
                        debug!(
                            "Device {} is busy (in use by us), adding with default caps",
                            device_name
                        );
                        push_info(
                            vec![44100, 48000],
                            vec![2],
                            format!("{} - Device {} (in use)", card_longname, device_index),
                        );
                    }
                }
            }
        }
    }

    info!("Found {} audio capture devices", devices.len());
    Ok(devices)
}

fn query_device_caps(pcm: &PCM) -> (Vec<u32>, Vec<u32>) {
    let hwp = match HwParams::any(pcm) {
        Ok(h) => h,
        Err(_) => return (vec![], vec![]),
    };

    let common_rates = [8000, 16000, 22050, 44100, 48000, 96000];
    let mut supported_rates = Vec::new();

    for rate in &common_rates {
        if hwp.test_rate(*rate).is_ok() {
            supported_rates.push(*rate);
        }
    }

    let mut supported_channels = Vec::new();
    for ch in 1..=8 {
        if hwp.test_channels(ch).is_ok() {
            supported_channels.push(ch);
        }
    }

    (supported_rates, supported_channels)
}

pub fn find_best_audio_device() -> Result<AudioDeviceInfo> {
    let devices = enumerate_audio_devices()?;

    if devices.is_empty() {
        return Err(AppError::AudioError(
            "No audio capture devices found".to_string(),
        ));
    }

    let mut first_48k_stereo: Option<&AudioDeviceInfo> = None;
    for device in &devices {
        if !device.sample_rates.contains(&48000) || !device.channels.contains(&2) {
            continue;
        }
        if device.is_hdmi {
            info!("Selected HDMI audio device: {}", device.description);
            return Ok(device.clone());
        }
        if first_48k_stereo.is_none() {
            first_48k_stereo = Some(device);
        }
    }
    if let Some(device) = first_48k_stereo {
        info!("Selected audio device: {}", device.description);
        return Ok(device.clone());
    }

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
        let result = enumerate_audio_devices();
        println!("Audio devices: {:?}", result);
        assert!(result.is_ok());
    }
}
