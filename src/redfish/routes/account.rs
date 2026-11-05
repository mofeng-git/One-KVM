use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};

use std::sync::Arc;

use super::super::schema::*;
use super::{empty_collection, resource_not_found};
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/AccountService", get(account_service))
        .route("/v1/AccountService/Accounts", get(account_list))
        .route(
            "/v1/AccountService/Accounts/{account_id}",
            get(account_detail),
        )
        .route("/v1/AccountService/Roles", get(roles_stub))
        .route("/v1/AccountService/Roles/{role_id}", get(role_detail_stub))
        .with_state(state)
}

async fn account_service() -> Json<AccountService> {
    Json(AccountService {
        odata_type: "#AccountService.v1_13_0.AccountService".to_string(),
        odata_id: "/redfish/v1/AccountService".to_string(),
        odata_context: "/redfish/v1/$metadata#AccountService.AccountService".to_string(),
        id: "AccountService".to_string(),
        name: "Account Service".to_string(),
        description: "Account Service".to_string(),
        service_enabled: true,
        accounts: odata_ref("/redfish/v1/AccountService/Accounts"),
        roles: odata_ref("/redfish/v1/AccountService/Roles"),
    })
}

async fn account_list(State(state): State<Arc<AppState>>) -> Response {
    let user = match state.users.single_user().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Json(empty_collection(
                "#ManagerAccountCollection.ManagerAccountCollection",
                "/redfish/v1/AccountService/Accounts",
                "/redfish/v1/$metadata#ManagerAccountCollection.ManagerAccountCollection",
                "Accounts Collection",
                "Collection of Accounts",
                vec![],
            ))
            .into_response()
        }
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(RedfishError::general_error(&e.to_string())),
            )
                .into_response()
        }
    };

    Json(empty_collection(
        "#ManagerAccountCollection.ManagerAccountCollection",
        "/redfish/v1/AccountService/Accounts",
        "/redfish/v1/$metadata#ManagerAccountCollection.ManagerAccountCollection",
        "Accounts Collection",
        "Collection of Accounts",
        vec![odata_ref(&format!(
            "/redfish/v1/AccountService/Accounts/{}",
            user.id
        ))],
    ))
    .into_response()
}

async fn account_detail(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Response {
    let user = match state.users.single_user().await {
        Ok(Some(u)) => u,
        Ok(None) => return resource_not_found(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RedfishError::general_error(&e.to_string())),
            )
                .into_response()
        }
    };

    if user.id != account_id {
        return resource_not_found();
    }

    Json(ManagerAccount {
        odata_type: "#ManagerAccount.v1_12_0.ManagerAccount".to_string(),
        odata_id: format!("/redfish/v1/AccountService/Accounts/{}", user.id),
        odata_context: "/redfish/v1/$metadata#ManagerAccount.ManagerAccount".to_string(),
        id: user.id,
        name: format!("Account {}", user.username),
        description: "User Account".to_string(),
        enabled: true,
        user_name: user.username,
        role_id: "Administrator".to_string(),
        locked: false,
        links: ManagerAccountLinks {
            role: odata_ref("/redfish/v1/AccountService/Roles/Administrator"),
        },
    })
    .into_response()
}

async fn roles_stub() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "@odata.type": "#RoleCollection.RoleCollection",
        "@odata.id": "/redfish/v1/AccountService/Roles",
        "@odata.context": "/redfish/v1/$metadata#RoleCollection.RoleCollection",
        "Name": "Roles Collection",
        "Description": "Collection of Roles",
        "Members@odata.count": 1,
        "Members": [{ "@odata.id": "/redfish/v1/AccountService/Roles/Administrator" }]
    }))
}

async fn role_detail_stub(Path(role_id): Path<String>) -> Response {
    if role_id != "Administrator" {
        return resource_not_found();
    }

    Json(serde_json::json!({
        "@odata.type": "#Role.v1_3_1.Role",
        "@odata.id": "/redfish/v1/AccountService/Roles/Administrator",
        "@odata.context": "/redfish/v1/$metadata#Role.Role",
        "Id": "Administrator",
        "Name": "Administrator Role",
        "Description": "Administrator role with full access",
        "IsPredefined": true,
        "AssignedPrivileges": [
            "Login", "ConfigureManager", "ConfigureUsers",
            "ConfigureSelf", "ConfigureComponents"
        ],
        "OemPrivileges": []
    }))
    .into_response()
}
