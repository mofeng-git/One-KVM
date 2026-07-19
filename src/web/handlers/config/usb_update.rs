use std::sync::Arc;

use crate::config::{AppConfig, Ch9329DescriptorConfig, HidBackend, HidConfig};
use crate::error::{AppError, Result};
use crate::state::AppState;

use super::apply::{apply_usb_config, try_apply_lock};
use super::types::HidConfigUpdate;

pub(super) fn stage_hid_config_update(
    staged_hid: &mut HidConfig,
    update: &HidConfigUpdate,
) -> Result<Option<Ch9329DescriptorConfig>> {
    update.validate()?;

    let old_descriptor = staged_hid.ch9329_descriptor.clone();
    update.apply_to(staged_hid);

    let requested_descriptor = update
        .ch9329_descriptor
        .as_ref()
        .map(|_| staged_hid.ch9329_descriptor.clone());
    if requested_descriptor.is_some() {
        staged_hid.ch9329_descriptor = old_descriptor;
    }

    Ok(requested_descriptor)
}

pub(super) async fn update_usb_config<F>(
    state: &Arc<AppState>,
    stage_update: F,
) -> Result<AppConfig>
where
    F: FnOnce(&mut AppConfig) -> Result<Option<Ch9329DescriptorConfig>>,
{
    let _guard = try_apply_lock(&state.config_apply_locks.otg, "otg")?;

    let old_config = state.config.get();
    let mut staged_config = old_config.as_ref().clone();
    let requested_ch9329_descriptor = stage_update(&mut staged_config)?;

    staged_config.enforce_invariants();
    staged_config.hid.validate_otg_functions()?;

    #[cfg(unix)]
    {
        if staged_config.otg_network.enabled
            && (staged_config.otg_network.device_mac.is_empty()
                || staged_config.otg_network.host_mac.is_empty())
        {
            let (device_mac, host_mac) =
                crate::otg::network::resolved_mac_pair(&staged_config.otg_network);
            staged_config.otg_network.device_mac = device_mac;
            staged_config.otg_network.host_mac = host_mac;
        }
        staged_config.otg_network.validate()?;
    }

    if let Err(error) = apply_usb_config(state, &old_config, &staged_config).await {
        return Err(rollback_after_failure(state, &staged_config, &old_config, error, false).await);
    }

    let descriptor_was_applied = if let Some(ref descriptor) = requested_ch9329_descriptor {
        if staged_config.hid.backend == HidBackend::Ch9329 {
            match state.hid.apply_ch9329_descriptor(descriptor).await {
                Ok(actual) => {
                    staged_config.hid.ch9329_descriptor = actual.descriptor;
                    true
                }
                Err(error) => {
                    return Err(rollback_after_failure(
                        state,
                        &staged_config,
                        &old_config,
                        error,
                        true,
                    )
                    .await);
                }
            }
        } else {
            false
        }
    } else {
        false
    };

    if let Err(error) = state
        .config
        .update(|config| {
            config.hid = staged_config.hid.clone();
            config.msd = staged_config.msd.clone();
            config.otg_network = staged_config.otg_network.clone();
            config.enforce_invariants();
        })
        .await
    {
        return Err(rollback_after_failure(
            state,
            &staged_config,
            &old_config,
            AppError::Config(format!(
                "Failed to persist USB configuration after apply: {error}"
            )),
            descriptor_was_applied,
        )
        .await);
    }

    Ok(staged_config)
}

async fn rollback_after_failure(
    state: &Arc<AppState>,
    failed_config: &AppConfig,
    old_config: &AppConfig,
    primary_error: AppError,
    restore_descriptor: bool,
) -> AppError {
    let mut rollback_errors = Vec::new();

    if let Err(error) = apply_usb_config(state, failed_config, old_config).await {
        rollback_errors.push(format!("runtime rollback failed: {error}"));
    }
    if restore_descriptor && old_config.hid.backend == HidBackend::Ch9329 {
        if let Err(error) = state
            .hid
            .apply_ch9329_descriptor(&old_config.hid.ch9329_descriptor)
            .await
        {
            rollback_errors.push(format!("CH9329 descriptor rollback failed: {error}"));
        }
    }

    if rollback_errors.is_empty() {
        return primary_error;
    }

    let message = format!("{primary_error}; {}", rollback_errors.join("; "));
    #[cfg(unix)]
    state.otg_service.mark_degraded(message.clone()).await;
    AppError::Config(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HidBackend;
    use crate::web::handlers::config::types::Ch9329DescriptorConfigUpdate;

    fn hid_update() -> HidConfigUpdate {
        HidConfigUpdate {
            backend: None,
            ch9329_port: None,
            ch9329_baudrate: None,
            ch9329_hybrid_mouse: None,
            ch9329_descriptor: None,
            otg_udc: None,
            otg_descriptor: None,
            otg_profile: None,
            otg_functions: None,
            otg_keyboard_leds: None,
            mouse_absolute: None,
        }
    }

    #[test]
    fn stages_regular_hid_fields_immediately() {
        let mut hid = HidConfig::default();
        let mut update = hid_update();
        update.backend = Some(HidBackend::Ch9329);
        update.ch9329_port = Some("COM7".to_string());

        let requested_descriptor = stage_hid_config_update(&mut hid, &update).unwrap();

        assert_eq!(hid.backend, HidBackend::Ch9329);
        assert_eq!(hid.ch9329_port, "COM7");
        assert!(requested_descriptor.is_none());
    }

    #[test]
    fn defers_ch9329_descriptor_until_runtime_apply() {
        let mut hid = HidConfig::default();
        let old_descriptor = hid.ch9329_descriptor.clone();
        let mut update = hid_update();
        update.ch9329_descriptor = Some(Ch9329DescriptorConfigUpdate {
            vendor_id: Some(0x1234),
            product_id: Some(0x5678),
            manufacturer: None,
            product: None,
            serial_number: None,
        });

        let requested_descriptor = stage_hid_config_update(&mut hid, &update)
            .unwrap()
            .expect("descriptor update should be deferred");

        assert_eq!(hid.ch9329_descriptor, old_descriptor);
        assert_eq!(requested_descriptor.vendor_id, 0x1234);
        assert_eq!(requested_descriptor.product_id, 0x5678);
    }
}
