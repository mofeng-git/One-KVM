use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tracing::info;

use super::super::schema::*;
use super::{empty_collection, get_power_state, service_unavailable, validate_id, RESOURCE_ID};
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/Systems", get(systems_collection))
        .route(
            "/v1/Systems/{system_id}",
            get(system_detail).patch(system_patch),
        )
        .route(
            "/v1/Systems/{system_id}/Actions/ComputerSystem.Reset",
            post(system_reset),
        )
        .route(
            "/v1/Systems/{system_id}/Actions/ComputerSystem.SetDefaultBootOrder",
            post(system_set_default_boot_order),
        )
        .with_state(state)
}

fn build_computer_system(system_id: &str, power_state: &str, boot: Boot) -> ComputerSystem {
    ComputerSystem {
        odata_type: "#ComputerSystem.v1_20_0.ComputerSystem".to_string(),
        odata_id: format!("/redfish/v1/Systems/{}", system_id),
        odata_context: "/redfish/v1/$metadata#ComputerSystem.ComputerSystem".to_string(),
        odata_etag: "W/\"168\"".to_string(),
        id: system_id.to_string(),
        name: "Managed System".to_string(),
        description: "The managed computer system connected via One-KVM".to_string(),
        system_type: "Physical".to_string(),
        asset_tag: String::new(),
        manufacturer: "Unknown".to_string(),
        model: "Unknown".to_string(),
        serial_number: String::new(),
        part_number: String::new(),
        power_state: power_state.to_string(),
        bios_version: "Unknown".to_string(),
        status: Status::enabled_ok(),
        boot,
        processor_summary: ProcessorSummary {
            count: None,
            logical_processor_count: None,
            model: "Unknown".to_string(),
            status: Status::enabled_ok(),
        },
        memory_summary: MemorySummary {
            total_system_memory_gi_b: None,
            status: Status::enabled_ok(),
        },
        trusted_modules: vec![],
        actions: ComputerSystemActions {
            reset: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{}/Actions/ComputerSystem.Reset",
                    system_id
                ),
            },
            set_default_boot_order: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{}/Actions/ComputerSystem.SetDefaultBootOrder",
                    system_id
                ),
            },
        },
        links: ComputerSystemLinks {
            chassis: vec![odata_ref(&format!("/redfish/v1/Chassis/{}", RESOURCE_ID))],
            managed_by: vec![odata_ref(&format!("/redfish/v1/Managers/{}", RESOURCE_ID))],
        },
    }
}

async fn systems_collection() -> Json<Collection<ODataLink>> {
    Json(empty_collection(
        "#ComputerSystemCollection.ComputerSystemCollection",
        "/redfish/v1/Systems",
        "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection",
        "Computer System Collection",
        "Collection of Computer Systems",
        vec![odata_ref("/redfish/v1/Systems/1")],
    ))
}

async fn system_detail(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Response {
    if let Some(resp) = validate_id(&system_id) {
        return resp;
    }

    let power_state = get_power_state(&state).await;
    let system = build_computer_system(
        &system_id,
        power_state,
        Boot {
            boot_source_override_enabled: "Disabled".to_string(),
            boot_source_override_mode: None,
            boot_source_override_target: None,
            uefi_target_boot_source_override: None,
        },
    );

    Json(system).into_response()
}

async fn system_patch(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Json(req): Json<ComputerSystemPatchRequest>,
) -> Response {
    if let Some(resp) = validate_id(&system_id) {
        return resp;
    }

    if let Some(boot) = &req.boot {
        if let Some(target) = &boot.boot_source_override_target {
            info!(
                "Redfish: PATCH Systems/{} BootSourceOverrideTarget='{}' (accepted, no-op)",
                system_id, target
            );
        }
    }

    let power_state = get_power_state(&state).await;
    let boot = match req.boot {
        Some(b) => Boot {
            boot_source_override_enabled: b
                .boot_source_override_enabled
                .unwrap_or_else(|| "Disabled".to_string()),
            boot_source_override_mode: b.boot_source_override_mode,
            boot_source_override_target: b.boot_source_override_target,
            uefi_target_boot_source_override: b.uefi_target_boot_source_override,
        },
        None => Boot {
            boot_source_override_enabled: "Disabled".to_string(),
            boot_source_override_mode: None,
            boot_source_override_target: None,
            uefi_target_boot_source_override: None,
        },
    };

    let system = build_computer_system(&system_id, power_state, boot);
    Json(system).into_response()
}

async fn system_reset(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Json(req): Json<ResetRequest>,
) -> Response {
    if let Some(resp) = validate_id(&system_id) {
        return resp;
    }

    let result = {
        let guard = state.atx.read().await;
        let atx = match guard.as_ref() {
            Some(atx) => atx,
            None => return service_unavailable("ATX power control not available"),
        };

        match req.reset_type.as_str() {
            "On" | "ForceOn" | "PushPowerButton" => atx.power_short().await,
            "ForceOff" | "GracefulShutdown" => atx.power_long().await,
            "ForceRestart" | "GracefulRestart" | "PowerCycle" => atx.reset().await,
            "Nmi" => {
                return (
                    StatusCode::NOT_ACCEPTABLE,
                    Json(RedfishError::action_not_supported("Nmi")),
                )
                    .into_response()
            }
            _ => {
                return (
                    StatusCode::NOT_ACCEPTABLE,
                    Json(RedfishError::action_not_supported(&req.reset_type)),
                )
                    .into_response()
            }
        }
    };

    match result {
        Ok(()) => {
            info!("Redfish: System reset '{}' executed", req.reset_type);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::warn!("Redfish: System reset failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RedfishError::general_error(&e.to_string())),
            )
                .into_response()
        }
    }
}

async fn system_set_default_boot_order(Path(system_id): Path<String>) -> Response {
    if let Some(resp) = validate_id(&system_id) {
        return resp;
    }

    info!(
        "Redfish: SetDefaultBootOrder for system {} (accepted, no-op)",
        system_id
    );
    StatusCode::NO_CONTENT.into_response()
}
