use crate::config::{AppConfig, Ch9329DescriptorConfig, HidBackend, HidConfig};
use crate::error::{AppError, Result};
use crate::web::state::UsbApiState;

use super::types::HidConfigUpdate;
use crate::runtime::try_apply_lock;

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

pub(super) async fn update_usb_config<F>(state: &UsbApiState, stage_update: F) -> Result<AppConfig>
where
    F: FnOnce(&mut AppConfig) -> Result<Option<Ch9329DescriptorConfig>>,
{
    update_usb_config_with_reset(state, false, stage_update).await
}

pub(super) async fn update_usb_config_with_reset<F>(
    state: &UsbApiState,
    reset: bool,
    stage_update: F,
) -> Result<AppConfig>
where
    F: FnOnce(&mut AppConfig) -> Result<Option<Ch9329DescriptorConfig>>,
{
    let _guard = try_apply_lock(&state.apply_lock, "otg")?;

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
        staged_config.uac.validate()?;
    }

    staged_config.hid.bluetooth.validate()?;
    if reset && staged_config.hid.backend != HidBackend::Bluetooth {
        return Err(AppError::BadRequest(
            "Pairing reset requires Bluetooth HID".into(),
        ));
    }
    #[cfg(not(target_os = "linux"))]
    if reset {
        return Err(AppError::BadRequest("Bluetooth HID requires Linux".into()));
    }
    #[cfg(target_os = "linux")]
    if reset {
        use one_kvm_bluetooth_hid::{
            adapters,
            bonds::{self, Bond, BondStore},
        };
        let adapters = adapters().await.map_err(AppError::Config)?;
        if !adapters
            .iter()
            .any(|a| a.name == staged_config.hid.bluetooth.adapter)
        {
            return Err(AppError::BadRequest(
                "Selected Bluetooth adapter is missing".into(),
            ));
        }
        let mut explicit = Vec::new();
        let mut runtime = state.hid.bluetooth_status().await.ok();
        if runtime
            .as_ref()
            .is_some_and(|s| s["initialized"].as_bool() == Some(true))
        {
            // Publish the latest bond before shutdown, including pairing completed
            // since the last 500 ms status update. No new peers can enter afterward.
            state.hid.bluetooth_action("close", 120).await?;
            runtime = state.hid.bluetooth_status().await.ok();
        }
        if let Some(status) = runtime {
            if let (Some(adapter), Some(peer)) =
                (status["adapter_address"].as_str(), status["peer"].as_str())
            {
                if !adapter.is_empty() {
                    explicit.push(Bond {
                        adapter: adapter.into(),
                        peer: peer.into(),
                        pending: true,
                    });
                }
            }
        }
        let owned = state
            .config
            .hid_bonds()
            .list()
            .await
            .map_err(AppError::Config)?;
        for config in [&old_config.hid.bluetooth, &staged_config.hid.bluetooth] {
            // A recorded hardware address takes precedence over a potentially
            // renumbered hci index. Runtime targets above also carry real addresses.
            if config.peer.as_ref().is_some_and(|peer| {
                owned
                    .iter()
                    .chain(explicit.iter())
                    .any(|bond| bond.peer.eq_ignore_ascii_case(peer))
            }) {
                continue;
            }
            if let (Some(peer), Some(adapter)) = (
                &config.peer,
                adapters.iter().find(|a| a.name == config.adapter),
            ) {
                explicit.push(Bond {
                    adapter: adapter.address.clone(),
                    peer: peer.clone(),
                    pending: true,
                });
            }
        }
        // Stop observation before writing tombstones or removing bonds.
        if old_config.hid.backend == HidBackend::Bluetooth {
            state.hid.reload(crate::hid::HidBackendType::None).await?;
        }
        if let Err(error) = bonds::reset(&state.config.hid_bonds(), explicit).await {
            return Err(AppError::Config(format!("Bluetooth binding cleanup failed; some old bindings may already be cleared and require pairing again: {error}")));
        }
        staged_config.hid.bluetooth.peer = None;
    }

    let result = async {
        if let Err(error) = state
            .coordinator
            .apply_config(&old_config, &staged_config)
            .await
        {
            return Err(
                rollback_after_failure(state, &staged_config, &old_config, error, false).await,
            );
        }

        #[cfg(target_os = "linux")]
        if reset
            && !matches!(
                state.hid.backend_type().await,
                crate::hid::HidBackendType::Bluetooth { .. }
            )
        {
            // Even identical device selections must rebuild with no pinned peer.
            state
                .hid
                .reload(crate::hid::HidBackendType::Bluetooth {
                    config: staged_config.hid.bluetooth.clone(),
                })
                .await?;
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
                config.uac = staged_config.uac.clone();
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
    .await;
    result.map_err(|error| if reset {
        AppError::Config(format!("Old One-KVM HID bindings were cleared; pair again. Configuration apply failed: {error}"))
    } else { error })
}

async fn rollback_after_failure(
    state: &UsbApiState,
    failed_config: &AppConfig,
    old_config: &AppConfig,
    primary_error: AppError,
    restore_descriptor: bool,
) -> AppError {
    let mut rollback_errors = Vec::new();

    if let Err(error) = state
        .coordinator
        .apply_config(failed_config, old_config)
        .await
    {
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
    state.otg.mark_degraded(message.clone()).await;
    AppError::Config(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HidBackend;
    use crate::web::handlers::config::types::Ch9329DescriptorConfigUpdate;

    fn hid_update() -> HidConfigUpdate {
        HidConfigUpdate {
            bluetooth_reset_pairing: None,
            bluetooth: None,
            backend: None,
            ch9329_port: None,
            ch9329_baudrate: None,
            ch9329_hybrid_mouse: None,
            mouse_macos_drag: None,
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
    fn reset_is_request_only_and_feature_updates_preserve_device_selection() {
        let mut hid = HidConfig::default();
        hid.backend = HidBackend::Ch9329;
        hid.ch9329_port = "COM7".into();
        let update: HidConfigUpdate = serde_json::from_value(serde_json::json!({
            "ch9329_hybrid_mouse": true, "bluetooth_reset_pairing": true
        }))
        .unwrap();
        stage_hid_config_update(&mut hid, &update).unwrap();
        assert_eq!(hid.backend, HidBackend::Ch9329);
        assert_eq!(hid.ch9329_port, "COM7");
        assert!(serde_json::to_value(&hid)
            .unwrap()
            .get("bluetooth_reset_pairing")
            .is_none());
    }

    #[test]
    fn invalid_target_does_not_mutate_staged_config() {
        let mut hid = HidConfig::default();
        let before = serde_json::to_value(&hid).unwrap();
        let update: HidConfigUpdate = serde_json::from_value(serde_json::json!({
            "backend": "bluetooth", "bluetooth_reset_pairing": true,
            "bluetooth": { "adapter": "hci0", "name": "" }
        }))
        .unwrap();
        assert!(stage_hid_config_update(&mut hid, &update).is_err());
        assert_eq!(serde_json::to_value(hid).unwrap(), before);
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
