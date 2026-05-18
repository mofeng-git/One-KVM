use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tracing::info;

use super::super::schema::*;
use crate::state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/EventService", get(event_service))
        .route("/v1/EventService/SSE", get(event_service_sse))
        .route(
            "/v1/EventService/Actions/EventService.SubmitTestEvent",
            post(event_submit_test),
        )
        .with_state(state)
}

async fn event_service() -> Json<EventService> {
    Json(EventService {
        odata_type: "#EventService.v1_8_1.EventService".to_string(),
        odata_id: "/redfish/v1/EventService".to_string(),
        odata_context: "/redfish/v1/$metadata#EventService.EventService".to_string(),
        id: "EventService".to_string(),
        name: "Event Service".to_string(),
        description: "Event Service".to_string(),
        service_enabled: true,
        delivery_retry_attempts: 3,
        delivery_retry_interval_seconds: 30,
        event_format_types: vec!["Event".to_string(), "MetricReport".to_string()],
        registry_prefixes: vec!["Base".to_string()],
        subordinate_resources: true,
        sse_filter_properties_supported: SseFilterPropertiesSupported {
            event_format_type: false,
            message_id: false,
            metric_report_definition: false,
            origin_resource: false,
            registry_prefix: false,
            resource_type: false,
        },
        server_sent_event_uri: Some("/redfish/v1/EventService/SSE".to_string()),
        actions: EventServiceActions {
            submit_test_event: ActionTarget {
                target: "/redfish/v1/EventService/Actions/EventService.SubmitTestEvent".to_string(),
            },
        },
    })
}

async fn event_service_sse(State(state): State<Arc<AppState>>) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let mut device_info_rx = state.subscribe_device_info();

    let stream = async_stream::stream! {
        loop {
            match device_info_rx.changed().await {
                Ok(()) => {
                    let payload = json!({
                        "@odata.type": "#Event.v1_7_0.Event",
                        "Id": uuid::Uuid::new_v4().to_string(),
                        "Name": "One-KVM Event",
                        "Context": "One-KVM",
                        "Events": [{
                            "EventType": "ResourceUpdated",
                            "EventId": uuid::Uuid::new_v4().to_string(),
                            "Severity": "OK",
                            "Message": "Device state updated",
                            "MessageId": "ResourceUpdated.1.0.0.ResourceUpdated"
                        }]
                    });

                    let event = Event::default()
                        .data(serde_json::to_string(&payload).unwrap_or_default());
                    yield Ok::<_, Infallible>(event);
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(Box::pin(stream))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text(":\n"),
        )
        .into_response()
}

async fn event_submit_test() -> StatusCode {
    info!("Redfish: SubmitTestEvent received (no-op)");
    StatusCode::NO_CONTENT
}
