use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tracing::{info, warn};

use super::super::schema::*;
use super::{empty_collection, resource_not_found, service_unavailable, validate_id};
use crate::error::AppError;
use crate::msd::{ImageInfo, ImageManager, MountedMedia, MountedMediaKind};
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

async fn virtual_media_collection(
    State(state): State<Arc<AppState>>,
    Path(manager_id): Path<String>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }

    let capacity = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        msd.state().await.disk_mode.capacity()
    };
    let members = (1..=capacity)
        .map(|slot| {
            odata_ref(&format!(
                "/redfish/v1/Managers/{}/VirtualMedia/{}",
                manager_id, slot
            ))
        })
        .collect();

    Json(empty_collection(
        "#VirtualMediaCollection.VirtualMediaCollection",
        &format!("/redfish/v1/Managers/{}/VirtualMedia", manager_id),
        "/redfish/v1/$metadata#VirtualMediaCollection.VirtualMediaCollection",
        "Virtual Media Collection",
        "Collection of Virtual Media",
        members,
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

    let (msd_state, lun) = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        let msd_state = msd.state().await;
        let Some(lun) = parse_slot_id(&media_id, msd_state.disk_mode.capacity()) else {
            return resource_not_found();
        };
        (msd_state, lun)
    };
    let media = msd_state
        .mounted_media
        .iter()
        .find(|media| media.lun == lun);

    Json(virtual_media_resource(&manager_id, &media_id, media)).into_response()
}

fn virtual_media_resource(
    manager_id: &str,
    media_id: &str,
    media: Option<&MountedMedia>,
) -> VirtualMedia {
    let inserted = media.is_some();
    let is_image = media.is_some_and(|media| media.kind == MountedMediaKind::Image);
    let media_types = match media {
        Some(media) if media.cdrom => vec!["CD".to_string(), "DVD".to_string()],
        Some(_) => vec!["USBStick".to_string()],
        None => vec!["CD".to_string(), "DVD".to_string(), "USBStick".to_string()],
    };

    VirtualMedia {
        odata_type: "#VirtualMedia.v1_6_2.VirtualMedia".to_string(),
        odata_id: format!(
            "/redfish/v1/Managers/{}/VirtualMedia/{}",
            manager_id, media_id
        ),
        odata_context: "/redfish/v1/$metadata#VirtualMedia.VirtualMedia".to_string(),
        id: media_id.to_string(),
        name: format!("Virtual Media Slot {}", media_id),
        description: "Virtual Media Slot".to_string(),
        media_types,
        connected_via: media.map(|_| if is_image { "URI" } else { "Applet" }.to_string()),
        inserted,
        image: media
            .filter(|_| is_image)
            .map(|media| format!("/api/msd/images/{}", media.id)),
        image_name: media.map(|media| media.name.clone()),
        write_protected: media.is_none_or(|media| media.read_only),
        transfer_method: is_image.then(|| "Upload".to_string()),
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
    }
}

async fn virtual_media_insert(
    State(state): State<Arc<AppState>>,
    Path((manager_id, media_id)): Path<(String, String)>,
    Json(req): Json<InsertMediaRequest>,
) -> Response {
    if let Some(resp) = validate_id(&manager_id) {
        return resp;
    }

    let lun = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        let msd_state = msd.state().await;
        let Some(lun) = parse_slot_id(&media_id, msd_state.disk_mode.capacity()) else {
            return resource_not_found();
        };
        if msd_state.mounted_media.iter().any(|media| media.lun == lun) {
            return redfish_error(
                StatusCode::CONFLICT,
                "Virtual media slot is already occupied",
            );
        }
        lun
    };

    if let Err(error) = validate_insert_request(&req) {
        return app_error_response(error);
    }
    let image = match resolve_image(&state, &req).await {
        Ok(image) => image,
        Err(error) => return app_error_response(error),
    };
    let (cdrom, read_only) = match mount_options(&req, &image.name) {
        Ok(options) => options,
        Err(error) => return app_error_response(error),
    };

    let result = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        msd.mount_image_at_lun(&image, cdrom, read_only, lun).await
    };

    match result {
        Ok(()) => {
            info!(slot = %media_id, image = %image.name, "Redfish virtual media inserted");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => {
            warn!(slot = %media_id, %error, "Redfish virtual media insert failed");
            app_error_response(error)
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

    let lun = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        let capacity = msd.state().await.disk_mode.capacity();
        let Some(lun) = parse_slot_id(&media_id, capacity) else {
            return resource_not_found();
        };
        lun
    };

    let result = {
        let guard = state.msd.read().await;
        let Some(msd) = guard.as_ref() else {
            return service_unavailable("MSD not available");
        };
        msd.unmount_lun(lun).await
    };

    match result {
        Ok(true) => {
            info!(slot = %media_id, "Redfish virtual media ejected");
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => redfish_error(
            StatusCode::CONFLICT,
            "No virtual media inserted in this slot",
        ),
        Err(error) => {
            warn!(slot = %media_id, %error, "Redfish virtual media eject failed");
            app_error_response(error)
        }
    }
}

fn parse_slot_id(media_id: &str, capacity: u8) -> Option<u8> {
    media_id
        .parse::<u8>()
        .ok()?
        .checked_sub(1)
        .filter(|lun| *lun < capacity)
}

fn validate_insert_request(req: &InsertMediaRequest) -> Result<(), AppError> {
    if req.inserted == Some(false) {
        return Err(AppError::BadRequest(
            "Inserted=false is not supported".to_string(),
        ));
    }
    if req
        .transfer_method
        .as_deref()
        .is_some_and(|method| !method.eq_ignore_ascii_case("Upload"))
    {
        return Err(AppError::BadRequest(
            "Only TransferMethod=Upload is supported".to_string(),
        ));
    }
    Ok(())
}

fn mount_options(req: &InsertMediaRequest, image_name: &str) -> Result<(bool, bool), AppError> {
    let requested_type = req
        .media_types
        .as_ref()
        .and_then(|types| types.first())
        .map(|value| value.as_str());
    let cdrom = match requested_type {
        Some(value) if value.eq_ignore_ascii_case("CD") || value.eq_ignore_ascii_case("DVD") => {
            true
        }
        Some(value) if value.eq_ignore_ascii_case("USBStick") => false,
        Some(value) => {
            return Err(AppError::BadRequest(format!(
                "Unsupported virtual media type: {value}"
            )))
        }
        None => image_name.to_ascii_lowercase().ends_with(".iso"),
    };

    Ok((cdrom, cdrom || req.write_protected.unwrap_or(true)))
}

async fn resolve_image(
    state: &Arc<AppState>,
    req: &InsertMediaRequest,
) -> Result<ImageInfo, AppError> {
    if req.user_name.is_some() || req.password.is_some() {
        return Err(AppError::BadRequest(
            "Authenticated virtual media URIs are not supported".to_string(),
        ));
    }

    let config = state.config.get();
    let manager = ImageManager::new(config.msd.images_dir());
    if req.image.starts_with("http://") || req.image.starts_with("https://") {
        if let Some(protocol) = req.transfer_protocol_type.as_deref() {
            let expected = if req.image.starts_with("https://") {
                "HTTPS"
            } else {
                "HTTP"
            };
            if !protocol.eq_ignore_ascii_case(expected) {
                return Err(AppError::BadRequest(format!(
                    "TransferProtocolType must be {expected} for this Image URI"
                )));
            }
        }
        return manager.download_from_url(&req.image, None, |_, _| {}).await;
    }
    if req.transfer_protocol_type.is_some() {
        return Err(AppError::BadRequest(
            "TransferProtocolType is only valid for remote Image URIs".to_string(),
        ));
    }

    let image_id = req
        .image
        .strip_prefix("/api/msd/images/")
        .unwrap_or(&req.image)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    if image_id.is_empty() || image_id.contains('/') {
        return Err(AppError::BadRequest(
            "Image must be an HTTP(S) URI, image ID, or /api/msd/images/{id}".to_string(),
        ));
    }
    manager.get(image_id)
}

fn app_error_response(error: AppError) -> Response {
    let status = match &error {
        AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    redfish_error(status, &error.to_string())
}

fn redfish_error(status: StatusCode, message: &str) -> Response {
    (status, Json(RedfishError::general_error(message))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(image: &str) -> InsertMediaRequest {
        InsertMediaRequest {
            image: image.to_string(),
            write_protected: None,
            transfer_method: None,
            transfer_protocol_type: None,
            media_types: None,
            inserted: None,
            user_name: None,
            password: None,
        }
    }

    #[test]
    fn slot_ids_map_to_zero_based_luns() {
        assert_eq!(parse_slot_id("1", 1), Some(0));
        assert_eq!(parse_slot_id("8", 8), Some(7));
        assert_eq!(parse_slot_id("0", 8), None);
        assert_eq!(parse_slot_id("2", 1), None);
        assert_eq!(parse_slot_id("invalid", 8), None);
    }

    #[test]
    fn mount_options_follow_media_type_and_redfish_write_protect_default() {
        assert_eq!(
            mount_options(&request("opaque-id"), "image.iso").unwrap(),
            (true, true)
        );
        assert_eq!(
            mount_options(&request("opaque-id"), "image.img").unwrap(),
            (false, true)
        );

        let mut writable = request("image.img");
        writable.write_protected = Some(false);
        writable.media_types = Some(vec!["USBStick".to_string()]);
        assert_eq!(
            mount_options(&writable, "image.img").unwrap(),
            (false, false)
        );
    }

    #[test]
    fn stream_transfer_and_non_inserted_media_are_rejected() {
        let mut stream = request("image.iso");
        stream.transfer_method = Some("Stream".to_string());
        assert!(validate_insert_request(&stream).is_err());

        let mut not_inserted = request("image.iso");
        not_inserted.inserted = Some(false);
        assert!(validate_insert_request(&not_inserted).is_err());
    }
}
