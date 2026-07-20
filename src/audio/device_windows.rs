use cpal::traits::{DeviceTrait, HostTrait};
use cpal::DeviceId;
use serde::Serialize;
use std::str::FromStr;
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

pub fn enumerate_audio_devices() -> Result<Vec<AudioDeviceInfo>> {
    enumerate_audio_devices_with_current(None)
}

pub fn enumerate_audio_devices_with_current(
    current_device: Option<&str>,
) -> Result<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| AppError::AudioError(format!("Failed to enumerate WASAPI devices: {}", e)))?;

    let mut result = Vec::new();

    for (index, device) in devices.enumerate() {
        let labels = device_labels(&device);
        let id = device
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| format!("wasapi-index:{}", index));

        let (sample_rates, channels) = query_device_caps(&device);
        if sample_rates.is_empty() || channels.is_empty() {
            debug!(
                "Skipping WASAPI endpoint without usable input caps: {}",
                labels.search_text
            );
            continue;
        }

        let is_current =
            current_device == Some(id.as_str()) || current_device == Some(labels.display.as_str());
        let description = if is_current {
            format!("{} (in use)", labels.display)
        } else {
            labels.display.clone()
        };

        let lower = labels.search_text.to_lowercase();
        let is_hdmi = lower.contains("hdmi")
            || lower.contains("capture")
            || lower.contains("usb")
            || lower.contains("digital");

        result.push(AudioDeviceInfo {
            name: id,
            description,
            card_index: index as i32,
            device_index: 0,
            sample_rates,
            channels,
            is_capture: true,
            is_hdmi,
            usb_bus: None,
        });
    }

    info!("Found {} WASAPI audio capture devices", result.len());
    Ok(result)
}

fn query_device_caps(device: &cpal::Device) -> (Vec<u32>, Vec<u32>) {
    let mut sample_rates = Vec::new();
    let mut channels = Vec::new();

    if let Ok(configs) = device.supported_input_configs() {
        for cfg in configs {
            for rate in [8000, 16000, 22050, 44100, 48000, 96000] {
                if cfg.min_sample_rate() <= rate
                    && rate <= cfg.max_sample_rate()
                    && !sample_rates.contains(&rate)
                {
                    sample_rates.push(rate);
                }
            }

            let ch = cfg.channels() as u32;
            if !channels.contains(&ch) {
                channels.push(ch);
            }
        }
    }

    if (sample_rates.is_empty() || channels.is_empty()) && device.default_input_config().is_ok() {
        if let Ok(default_cfg) = device.default_input_config() {
            if !sample_rates.contains(&default_cfg.sample_rate()) {
                sample_rates.push(default_cfg.sample_rate());
            }
            let ch = default_cfg.channels() as u32;
            if !channels.contains(&ch) {
                channels.push(ch);
            }
        }
    }

    sample_rates.sort_unstable();
    channels.sort_unstable();
    (sample_rates, channels)
}

struct DeviceLabels {
    display: String,
    search_text: String,
}

fn device_labels(device: &cpal::Device) -> DeviceLabels {
    match device.description() {
        Ok(desc) => {
            let formatted = desc.to_string();
            let display = desc
                .extended()
                .next()
                .map(str::to_owned)
                .unwrap_or_else(|| formatted.clone());
            let mut parts = vec![formatted, desc.name().to_string(), display.clone()];
            parts.extend(desc.extended().map(str::to_owned));

            DeviceLabels {
                display,
                search_text: parts.join(" "),
            }
        }
        Err(_) => {
            let display = "Unknown WASAPI capture device".to_string();
            DeviceLabels {
                display: display.clone(),
                search_text: display,
            }
        }
    }
}

pub(crate) fn find_wasapi_device(requested_device: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    let trimmed = requested_device.trim();

    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return host.default_input_device().ok_or_else(|| {
            AppError::AudioError("No default WASAPI input device found".to_string())
        });
    }

    if let Ok(id) = DeviceId::from_str(trimmed) {
        if let Some(device) = host.device_by_id(&id) {
            return Ok(device);
        }
    }

    let needle = trimmed.to_lowercase();
    let devices = host
        .input_devices()
        .map_err(|e| AppError::AudioError(format!("Failed to enumerate WASAPI devices: {}", e)))?;

    for device in devices {
        let id_match = device
            .id()
            .map(|id| id.to_string() == trimmed)
            .unwrap_or(false);
        let labels = device_labels(&device);
        if id_match || labels.search_text.to_lowercase().contains(&needle) {
            return Ok(device);
        }
    }

    Err(AppError::AudioError(format!(
        "WASAPI audio device not found: {}",
        requested_device
    )))
}

pub fn find_best_audio_device() -> Result<AudioDeviceInfo> {
    let devices = enumerate_audio_devices()?;

    if devices.is_empty() {
        return Err(AppError::AudioError(
            "No WASAPI audio capture devices found".to_string(),
        ));
    }

    let mut first_48k_stereo: Option<&AudioDeviceInfo> = None;
    for device in &devices {
        if !device.sample_rates.contains(&48000) || !device.channels.contains(&2) {
            continue;
        }
        if device.is_hdmi {
            info!("Selected WASAPI capture device: {}", device.description);
            return Ok(device.clone());
        }
        if first_48k_stereo.is_none() {
            first_48k_stereo = Some(device);
        }
    }

    if let Some(device) = first_48k_stereo {
        info!("Selected WASAPI capture device: {}", device.description);
        return Ok(device.clone());
    }

    let device = devices.into_iter().next().unwrap();
    warn!(
        "Using fallback WASAPI audio device: {} (will resample if needed)",
        device.description
    );
    Ok(device)
}
