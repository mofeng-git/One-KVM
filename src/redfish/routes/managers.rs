use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};

use std::sync::Arc;

use super::super::schema::*;
use super::{empty_collection, validate_id, RESOURCE_ID};
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/Managers", get(managers_collection))
        .route("/v1/Managers/{manager_id}", get(manager_detail))
        .route(
            "/v1/Managers/{manager_id}/NetworkProtocol",
            get(network_protocol),
        )
        .with_state(state)
}

async fn managers_collection() -> Json<Collection<ODataLink>> {
    Json(empty_collection(
        "#ManagerCollection.ManagerCollection",
        "/redfish/v1/Managers",
        "/redfish/v1/$metadata#ManagerCollection.ManagerCollection",
        "Manager Collection",
        "Collection of Managers",
        vec![odata_ref("/redfish/v1/Managers/1")],
    ))
}

async fn manager_detail(
    State(state): State<Arc<AppState>>,
    Path(manager_id): Path<String>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }

    let now = time::OffsetDateTime::now_utc();
    let datetime = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let offset = now.offset();
    let offset_str = format!(
        "{:+03}{:02}",
        offset.whole_hours(),
        offset.minutes_past_hour().abs()
    );

    let mgr_uuid = "00000000-0000-0000-0000-000000000001".to_string();

    Json(Manager {
        odata_type: "#Manager.v1_15_0.Manager".to_string(),
        odata_id: format!("/redfish/v1/Managers/{}", manager_id),
        odata_context: "/redfish/v1/$metadata#Manager.Manager".to_string(),
        id: manager_id.clone(),
        name: "One-KVM Manager".to_string(),
        description: "One-KVM Management Controller".to_string(),
        manager_type: "BMC".to_string(),
        status: Status::enabled_ok(),
        firmware_version: env!("CARGO_PKG_VERSION").to_string(),
        manufacturer: "One-KVM".to_string(),
        model: "One-KVM".to_string(),
        date_time: datetime,
        date_time_local_offset: offset_str,
        service_entry_point_uuid: mgr_uuid,
        command_shell: CommandShell {
            service_enabled: state
                .extensions
                .check_available(crate::extensions::ExtensionId::Ttyd),
            max_concurrent_sessions: 1,
            connect_types_supported: vec!["WebUI".to_string()],
        },
        graphical_console: GraphicalConsole {
            service_enabled: true,
            max_concurrent_sessions: 4,
            connect_types_supported: vec!["KVMIP".to_string()],
        },
        virtual_media: odata_ref(&format!("/redfish/v1/Managers/{}/VirtualMedia", manager_id)),
        links: ManagerLinks {
            manager_for_servers: vec![odata_ref(&format!("/redfish/v1/Systems/{}", RESOURCE_ID))],
            manager_for_chassis: vec![odata_ref(&format!("/redfish/v1/Chassis/{}", RESOURCE_ID))],
        },
        network_protocol: odata_ref(&format!(
            "/redfish/v1/Managers/{}/NetworkProtocol",
            manager_id
        )),
    })
    .into_response()
}

async fn network_protocol(
    State(state): State<Arc<AppState>>,
    Path(manager_id): Path<String>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }

    let config = state.config.get();
    let http_port = config.web.http_port;
    let https_enabled = config.web.https_enabled;
    let https_port = config.web.https_port;

    Json(serde_json::json!({
        "@odata.type": "#ManagerNetworkProtocol.v1_10_0.ManagerNetworkProtocol",
        "@odata.id": format!("/redfish/v1/Managers/{}/NetworkProtocol", manager_id),
        "@odata.context": "/redfish/v1/$metadata#ManagerNetworkProtocol.ManagerNetworkProtocol",
        "Id": "NetworkProtocol",
        "Name": "Manager Network Protocol",
        "Description": "Network protocol settings",
        "Status": { "State": "Enabled", "Health": "OK" },
        "HTTP": { "ProtocolEnabled": !https_enabled, "Port": http_port },
        "HTTPS": { "ProtocolEnabled": https_enabled, "Port": https_port },
        "SSDP": { "ProtocolEnabled": false }
    }))
    .into_response()
}
