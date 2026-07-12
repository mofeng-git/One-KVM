use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tracing::{info, warn};

use std::sync::Arc;

use super::super::schema::*;
use super::{empty_collection, resource_not_found, service_unavailable, validate_id, RESOURCE_ID};
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/v1/Managers/{manager_id}/VirtualMedia",
            get(virtual_media_collection),
        )
        .route(
            "/v1/Managers/{manager_id}/VirtualMedia/{media_id}",
            get(virtual_media_detail),
        )
        .route(
            "/v1/Managers/{manager_id}/VirtualMedia/{media_id}/Actions/VirtualMedia.InsertMedia",
            post(virtual_media_insert),
        )
        .route(
            "/v1/Managers/{manager_id}/VirtualMedia/{media_id}/Actions/VirtualMedia.EjectMedia",
            post(virtual_media_eject),
        )
        .with_state(state)
}

async fn virtual_media_collection(Path(manager_id): Path<String>) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }

    Json(empty_collection(
        "#VirtualMediaCollection.VirtualMediaCollection",
        &format!("/redfish/v1/Managers/{}/VirtualMedia", manager_id),
        "/redfish/v1/$metadata#VirtualMediaCollection.VirtualMediaCollection",
        "Virtual Media Collection",
        "Collection of Virtual Media",
        vec![odata_ref(&format!(
            "/redfish/v1/Managers/{}/VirtualMedia/{}",
            manager_id, RESOURCE_ID
        ))],
    ))
    .into_response()
}

async fn virtual_media_detail(
    State(state): State<Arc<AppState>>,
    Path((manager_id, media_id)): Path<(String, String)>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }
    if media_id != RESOURCE_ID {
        return resource_not_found();
    }

    let (inserted, image_name, connected_via) = {
        let guard = state.msd.read().await;
        match guard.as_ref() {
            Some(msd) => {
                let msd_state = msd.state().await;
                let img_name = msd_state
                    .mounted_media
                    .first()
                    .map(|media| media.name.clone());
                (
                    !msd_state.mounted_media.is_empty(),
                    img_name,
                    if !msd_state.mounted_media.is_empty() {
                        Some("Applet".to_string())
                    } else {
                        None
                    },
                )
            }
            None => (false, None, None),
        }
    };

    Json(VirtualMedia {
        odata_type: "#VirtualMedia.v1_6_2.VirtualMedia".to_string(),
        odata_id: format!(
            "/redfish/v1/Managers/{}/VirtualMedia/{}",
            manager_id, media_id
        ),
        odata_context: "/redfish/v1/$metadata#VirtualMedia.VirtualMedia".to_string(),
        id: media_id.clone(),
        name: "Virtual Media 1".to_string(),
        description: "Virtual Media Device".to_string(),
        media_types: vec!["CD".to_string(), "USBStick".to_string()],
        connected_via: connected_via,
        inserted: inserted,
        image: None,
        image_name: image_name,
        write_protected: true,
        transfer_method: None,
        transfer_protocol_type: None,
        status: if inserted {
            Status::enabled_ok()
        } else {
            Status::disabled_ok()
        },
        actions: VirtualMediaActions {
            insert_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Managers/{}/VirtualMedia/{}/Actions/VirtualMedia.InsertMedia",
                    manager_id, media_id
                ),
            },
            eject_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Managers/{}/VirtualMedia/{}/Actions/VirtualMedia.EjectMedia",
                    manager_id, media_id
                ),
            },
        },
    })
    .into_response()
}

async fn virtual_media_insert(
    State(state): State<Arc<AppState>>,
    Path((manager_id, media_id)): Path<(String, String)>,
    Json(req): Json<InsertMediaRequest>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }
    if media_id != RESOURCE_ID {
        return resource_not_found();
    }

    let result = {
        let guard = state.msd.read().await;
        let msd = match guard.as_ref() {
            Some(msd) => msd,
            None => return service_unavailable("MSD not available"),
        };

        if !msd.state().await.mounted_media.is_empty() {
            return (
                StatusCode::CONFLICT,
                Json(RedfishError::general_error(
                    "Virtual media already inserted",
                )),
            )
                .into_response();
        }

        info!("Redfish: VirtualMedia.InsertMedia image='{}'", req.image);
        msd.mount_drive().await
    };

    match result {
        Ok(()) => {
            info!("Redfish: VirtualMedia.InsertMedia executed");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            warn!("Redfish: VirtualMedia.InsertMedia failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RedfishError::general_error(&e.to_string())),
            )
                .into_response()
        }
    }
}

async fn virtual_media_eject(
    State(state): State<Arc<AppState>>,
    Path((manager_id, media_id)): Path<(String, String)>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }
    if media_id != RESOURCE_ID {
        return resource_not_found();
    }

    let result = {
        let guard = state.msd.read().await;
        let msd = match guard.as_ref() {
            Some(msd) => msd,
            None => return service_unavailable("MSD not available"),
        };

        if msd.state().await.mounted_media.is_empty() {
            return (
                StatusCode::CONFLICT,
                Json(RedfishError::general_error("No virtual media inserted")),
            )
                .into_response();
        }

        msd.disconnect().await
    };

    match result {
        Ok(()) => {
            info!("Redfish: VirtualMedia.EjectMedia executed");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            warn!("Redfish: VirtualMedia.EjectMedia failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RedfishError::general_error(&e.to_string())),
            )
                .into_response()
        }
    }
}
