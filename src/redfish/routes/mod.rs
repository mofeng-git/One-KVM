mod account;
mod chassis;
mod event;
mod managers;
mod session;
mod systems;
#[cfg(all(unix, not(feature = "android")))]
mod virtual_media;

use axum::{
    http::{HeaderName, HeaderValue},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::set_header::SetResponseHeaderLayer;

use super::auth::redfish_auth_middleware;
use super::schema::*;
use crate::state::AppState;

pub(crate) const REDFISH_VERSION: &str = "1.18.1";
pub(crate) const RESOURCE_ID: &str = "1";

pub(crate) fn resource_not_found() -> Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(RedfishError::resource_not_found()),
    )
        .into_response()
}

pub(crate) fn validate_id(id: &str) -> Option<Response> {
    if id != RESOURCE_ID {
        return Some(resource_not_found());
    }
    None
}

pub(crate) fn service_unavailable(msg: &str) -> Response {
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(RedfishError::service_unavailable(msg)),
    )
        .into_response()
}

pub(crate) fn empty_collection(
    odata_type: &str,
    odata_id: &str,
    odata_context: &str,
    name: &str,
    description: &str,
    members: Vec<ODataLink>,
) -> Collection<ODataLink> {
    Collection {
        odata_type: odata_type.to_string(),
        odata_id: odata_id.to_string(),
        odata_context: odata_context.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        members_count: members.len() as u64,
        members,
    }
}

pub(crate) async fn get_power_state(state: &Arc<AppState>) -> &'static str {
    let guard = state.atx.read().await;
    match guard.as_ref() {
        Some(atx) => match atx.power_status().await {
            crate::atx::PowerStatus::On => "On",
            crate::atx::PowerStatus::Off => "Off",
            crate::atx::PowerStatus::Unknown => "Unknown",
        },
        None => "Unknown",
    }
}

async fn service_root_redirect() -> Response {
    axum::response::Redirect::permanent("/redfish/v1/").into_response()
}

async fn service_root() -> Json<ServiceRoot> {
    let uuid = "00000000-0000-0000-0000-000000000001".to_string();

    Json(ServiceRoot {
        odata_type: "#ServiceRoot.v1_17_0.ServiceRoot".to_string(),
        odata_id: "/redfish/v1".to_string(),
        odata_context: "/redfish/v1/$metadata#ServiceRoot.ServiceRoot".to_string(),
        id: "RootService".to_string(),
        name: "One-KVM Redfish Service".to_string(),
        redfish_version: REDFISH_VERSION.to_string(),
        uuid,
        protocol_features_supported: ProtocolFeaturesSupported {
            excerpt_query: false,
            expand_query: ExpandQuery {
                expand_all: false,
                levels: false,
                max_levels: 0,
                no_links: false,
                top: false,
            },
            filter_query: false,
            only_member_query: true,
            select_query: false,
        },
        systems: odata_ref("/redfish/v1/Systems"),
        chassis: odata_ref("/redfish/v1/Chassis"),
        managers: odata_ref("/redfish/v1/Managers"),
        session_service: odata_ref("/redfish/v1/SessionService"),
        account_service: odata_ref("/redfish/v1/AccountService"),
        event_service: odata_ref("/redfish/v1/EventService"),
        links: ServiceRootLinks {
            sessions: odata_ref("/redfish/v1/SessionService/Sessions"),
        },
    })
}

async fn odata_document() -> Json<serde_json::Value> {
    Json(json!({
        "@odata.context": "/redfish/v1/$metadata",
        "value": [
            { "name": "ServiceRoot", "kind": "Singleton", "url": "/redfish/v1" },
            { "name": "Systems", "kind": "Collection", "url": "/redfish/v1/Systems" },
            { "name": "Chassis", "kind": "Collection", "url": "/redfish/v1/Chassis" },
            { "name": "Managers", "kind": "Collection", "url": "/redfish/v1/Managers" }
        ]
    }))
}

async fn metadata() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        r#"<?xml version="1.0" encoding="UTF-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ServiceRoot_v1.xml">
    <edmx:Include Namespace="ServiceRoot"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ComputerSystem_v1.xml">
    <edmx:Include Namespace="ComputerSystem"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Manager_v1.xml">
    <edmx:Include Namespace="Manager"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Chassis_v1.xml">
    <edmx:Include Namespace="Chassis"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Power_v1.xml">
    <edmx:Include Namespace="Power"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/VirtualMedia_v1.xml">
    <edmx:Include Namespace="VirtualMedia"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/SessionService_v1.xml">
    <edmx:Include Namespace="SessionService"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/AccountService_v1.xml">
    <edmx:Include Namespace="AccountService"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/EventService_v1.xml">
    <edmx:Include Namespace="EventService"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ManagerNetworkProtocol_v1.xml">
    <edmx:Include Namespace="ManagerNetworkProtocol"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Role_v1.xml">
    <edmx:Include Namespace="Role"/>
  </edmx:Reference>
  <edmx:Reference Uri="http://docs.oasis-open.org/odata/ns/edm">
    <edmx:Include Namespace="Edm" />
  </edmx:Reference>
  <edmx:DataServices>
    <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="OneKVM">
      <EntityContainer Name="Service" Extends="ServiceRoot.v1_17_0.ServiceContainer"/>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>"#,
    )
        .into_response()
}

pub fn create_redfish_router(state: Arc<AppState>) -> Router {
    let redfish_routes = Router::new()
        .route("/", get(service_root))
        .route("/v1", get(service_root_redirect))
        .route("/v1/", get(service_root))
        .route("/v1/odata", get(odata_document))
        .route("/v1/$metadata", get(metadata))
        .merge(systems::router(state.clone()))
        .merge(chassis::router(state.clone()))
        .merge(managers::router(state.clone()))
        .merge(session::router(state.clone()))
        .merge(account::router(state.clone()))
        .merge(event::router(state.clone()))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            redfish_auth_middleware,
        ));

    #[cfg(all(unix, not(feature = "android")))]
    let redfish_routes = redfish_routes.merge(virtual_media::router(state.clone()));

    Router::new()
        .route("/redfish", get(service_root_redirect))
        .nest("/redfish/", redfish_routes)
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("odata-version"),
            HeaderValue::from_static("4.0"),
        ))
        .with_state(state)
}
