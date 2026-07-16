use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;
use typeshare::typeshare;

use crate::config::{HidBackend, HidConfig, MsdConfig, OtgNetworkConfig};
use crate::error::{AppError, Result};
use crate::otg::OtgNetworkStatus;
use crate::state::AppState;

use super::apply::{apply_otg_config, try_apply_lock};
use super::types::OtgConfigUpdate;

#[typeshare]
#[derive(Debug, Serialize)]
pub struct OtgConfigResponse {
    pub hid: HidConfig,
    pub msd: MsdConfig,
    pub network: OtgNetworkConfig,
    pub status: OtgNetworkStatus,
}

pub async fn update_otg_config(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OtgConfigUpdate>,
) -> Result<Json<OtgConfigResponse>> {
    update_otg_config_inner(&state, request).await.map(Json)
}

pub(super) async fn update_otg_config_inner(
    state: &Arc<AppState>,
    request: OtgConfigUpdate,
) -> Result<OtgConfigResponse> {
    let _guard = try_apply_lock(&state.config_apply_locks.otg, "otg")?;

    if let Some(ref update) = request.hid {
        update.validate()?;
    }
    if let Some(ref update) = request.msd {
        update.validate()?;
    }

    let old_config = state.config.get();
    let mut staged_config = (*old_config).clone();
    let requested_ch9329_descriptor = request.hid.as_ref().and_then(|update| {
        update.ch9329_descriptor.as_ref().map(|_| {
            let mut hid = staged_config.hid.clone();
            update.apply_to(&mut hid);
            hid.ch9329_descriptor
        })
    });

    if let Some(ref update) = request.hid {
        update.apply_to(&mut staged_config.hid);
    }
    if requested_ch9329_descriptor.is_some() {
        staged_config.hid.ch9329_descriptor = old_config.hid.ch9329_descriptor.clone();
    }
    if let Some(ref update) = request.msd {
        update.apply_to(&mut staged_config.msd);
    }
    if let Some(ref update) = request.network {
        update.apply_to(&mut staged_config.otg_network);
    }
    staged_config.enforce_invariants();

    if staged_config.otg_network.enabled
        && (staged_config.otg_network.device_mac.is_empty()
            || staged_config.otg_network.host_mac.is_empty())
    {
        let (device_mac, host_mac) =
            crate::otg::network::resolved_mac_pair(&staged_config.otg_network);
        staged_config.otg_network.device_mac = device_mac;
        staged_config.otg_network.host_mac = host_mac;
    }
    staged_config.hid.validate_otg_functions()?;
    staged_config.otg_network.validate()?;

    if let Err(error) = apply_otg_config(state, &old_config, &staged_config).await {
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
            AppError::Config(format!("Failed to persist OTG config after apply: {error}")),
            descriptor_was_applied,
        )
        .await);
    }

    Ok(OtgConfigResponse {
        hid: staged_config.hid,
        msd: staged_config.msd,
        network: staged_config.otg_network,
        status: state.otg_service.network_status().await,
    })
}

async fn rollback_after_failure(
    state: &Arc<AppState>,
    failed_config: &crate::config::AppConfig,
    old_config: &crate::config::AppConfig,
    primary_error: AppError,
    restore_descriptor: bool,
) -> AppError {
    let mut rollback_errors = Vec::new();

    if let Err(error) = apply_otg_config(state, failed_config, old_config).await {
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
    state.otg_service.mark_degraded(message.clone()).await;
    AppError::Config(message)
}
