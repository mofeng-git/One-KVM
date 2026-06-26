use axum::{
    extract::{ws::WebSocketUpgrade, Query, State},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::computer_use::{
    ComputerUseConfigResponse, ComputerUseConfigUpdate, ComputerUseSessionSummary,
    ComputerUseStartRequest,
};
use crate::error::Result;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ComputerUseWsQuery {
    client_id: Option<String>,
}

pub async fn computer_use_config(
    State(state): State<Arc<AppState>>,
) -> Json<ComputerUseConfigResponse> {
    Json(state.computer_use.config_response())
}

pub async fn computer_use_update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ComputerUseConfigUpdate>,
) -> Result<Json<ComputerUseConfigResponse>> {
    Ok(Json(state.computer_use.update_config(req).await?))
}

pub async fn computer_use_session(
    State(state): State<Arc<AppState>>,
) -> Json<ComputerUseSessionSummary> {
    Json(state.computer_use.summary().await)
}

pub async fn computer_use_start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ComputerUseStartRequest>,
) -> Result<Json<ComputerUseSessionSummary>> {
    Ok(Json(state.computer_use.start(req).await?))
}

pub async fn computer_use_stop(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ComputerUseSessionSummary>> {
    Ok(Json(state.computer_use.stop().await?))
}

pub async fn computer_use_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ComputerUseWsQuery>,
) -> Response {
    ws.on_upgrade(move |socket| {
        state
            .computer_use
            .clone()
            .handle_socket(socket, query.client_id)
    })
}
