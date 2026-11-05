use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};

use std::sync::Arc;

use super::super::schema::*;
use super::{empty_collection, get_power_state, validate_id, RESOURCE_ID};
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/Chassis", get(chassis_collection))
        .route("/v1/Chassis/{chassis_id}", get(chassis_detail))
        .route("/v1/Chassis/{chassis_id}/Power", get(chassis_power))
        .with_state(state)
}

async fn chassis_collection() -> Json<Collection<ODataLink>> {
    Json(empty_collection(
        "#ChassisCollection.ChassisCollection",
        "/redfish/v1/Chassis",
        "/redfish/v1/$metadata#ChassisCollection.ChassisCollection",
        "Chassis Collection",
        "Collection of Chassis",
        vec![odata_ref("/redfish/v1/Chassis/1")],
    ))
}

async fn chassis_detail(
    State(state): State<Arc<AppState>>,
    Path(chassis_id): Path<String>,
) -> Response {
    if let Some(resp) = validate_id(&chassis_id) {
        return resp;
    }

    let power_state = get_power_state(&state).await;

    Json(Chassis {
        odata_type: "#Chassis.v1_25_0.Chassis".to_string(),
        odata_id: format!("/redfish/v1/Chassis/{}", chassis_id),
        odata_context: "/redfish/v1/$metadata#Chassis.Chassis".to_string(),
        id: chassis_id.clone(),
        name: "One-KVM Chassis".to_string(),
        description: "The physical chassis managed by One-KVM".to_string(),
        chassis_type: "RackMount".to_string(),
        asset_tag: String::new(),
        manufacturer: "One-KVM".to_string(),
        model: "Virtual".to_string(),
        serial_number: String::new(),
        part_number: String::new(),
        power_state: power_state.to_string(),
        status: Status::enabled_ok(),
        power: odata_ref(&format!("/redfish/v1/Chassis/{}/Power", chassis_id)),
        links: ChassisLinks {
            computer_systems: vec![odata_ref(&format!("/redfish/v1/Systems/{}", RESOURCE_ID))],
            managed_by: vec![odata_ref(&format!("/redfish/v1/Managers/{}", RESOURCE_ID))],
        },
    })
    .into_response()
}

async fn chassis_power(Path(chassis_id): Path<String>) -> Response {
    if let Some(resp) = validate_id(&chassis_id) {
        return resp;
    }

    Json(Power {
        odata_type: "#Power.v1_7_3.Power".to_string(),
        odata_id: format!("/redfish/v1/Chassis/{}/Power", chassis_id),
        odata_context: "/redfish/v1/$metadata#Power.Power".to_string(),
        id: "Power".to_string(),
        name: "Power".to_string(),
        power_control: vec![PowerControl {
            odata_id: format!("/redfish/v1/Chassis/{}/Power#/PowerControl/0", chassis_id),
            member_id: "0".to_string(),
            name: "System Power Control".to_string(),
            power_consumed_watts: None,
            power_capacity_watts: None,
            power_requested_watts: None,
            power_metrics: None,
            status: Status::enabled_ok(),
        }],
    })
    .into_response()
}
