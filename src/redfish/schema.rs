use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn odata_ref(id: &str) -> ODataLink {
    ODataLink {
        odata_id: id.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ODataLink {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Status {
    pub state: String,
    pub health: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_rollup: Option<String>,
}

impl Status {
    pub fn enabled_ok() -> Self {
        Self {
            state: "Enabled".to_string(),
            health: "OK".to_string(),
            health_rollup: None,
        }
    }

    pub fn enabled_health(health: &str) -> Self {
        Self {
            state: "Enabled".to_string(),
            health: health.to_string(),
            health_rollup: None,
        }
    }

    pub fn disabled_ok() -> Self {
        Self {
            state: "Disabled".to_string(),
            health: "OK".to_string(),
            health_rollup: None,
        }
    }

    pub fn offline_ok() -> Self {
        Self {
            state: "Offline".to_string(),
            health: "OK".to_string(),
            health_rollup: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceRoot {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub redfish_version: String,
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub protocol_features_supported: ProtocolFeaturesSupported,
    pub systems: ODataLink,
    pub chassis: ODataLink,
    pub managers: ODataLink,
    pub session_service: ODataLink,
    pub account_service: ODataLink,
    pub event_service: ODataLink,
    pub links: ServiceRootLinks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProtocolFeaturesSupported {
    pub excerpt_query: bool,
    pub expand_query: ExpandQuery,
    pub filter_query: bool,
    pub only_member_query: bool,
    pub select_query: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExpandQuery {
    pub expand_all: bool,
    pub levels: bool,
    pub max_levels: u32,
    pub no_links: bool,
    pub top: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceRootLinks {
    pub sessions: ODataLink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Collection<T: Serialize> {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "Members@odata.count")]
    pub members_count: u64,
    pub members: Vec<T>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ComputerSystem {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    #[serde(rename = "@odata.etag")]
    pub odata_etag: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_type: String,
    pub asset_tag: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub part_number: String,
    pub power_state: String,
    pub bios_version: String,
    pub status: Status,
    pub boot: Boot,
    pub processor_summary: ProcessorSummary,
    pub memory_summary: MemorySummary,
    pub trusted_modules: Vec<Value>,
    pub actions: ComputerSystemActions,
    pub links: ComputerSystemLinks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Boot {
    pub boot_source_override_enabled: String,
    pub boot_source_override_mode: Option<String>,
    pub boot_source_override_target: Option<String>,
    pub uefi_target_boot_source_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProcessorSummary {
    pub count: Option<u32>,
    pub logical_processor_count: Option<u32>,
    pub model: String,
    pub status: Status,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MemorySummary {
    pub total_system_memory_gi_b: Option<f64>,
    pub status: Status,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputerSystemActions {
    #[serde(rename = "#ComputerSystem.Reset")]
    pub reset: ActionTarget,
    #[serde(rename = "#ComputerSystem.SetDefaultBootOrder")]
    pub set_default_boot_order: ActionTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionTarget {
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ComputerSystemLinks {
    pub chassis: Vec<ODataLink>,
    pub managed_by: Vec<ODataLink>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResetRequest {
    #[serde(default = "default_reset_type")]
    pub reset_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ComputerSystemPatchRequest {
    #[serde(default)]
    pub boot: Option<BootPatch>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BootPatch {
    #[serde(default)]
    pub boot_source_override_enabled: Option<String>,
    #[serde(default)]
    pub boot_source_override_target: Option<String>,
    #[serde(default)]
    pub boot_source_override_mode: Option<String>,
    #[serde(default)]
    pub uefi_target_boot_source_override: Option<String>,
}

fn default_reset_type() -> String {
    "ForceRestart".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Manager {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub manager_type: String,
    pub status: Status,
    pub firmware_version: String,
    pub manufacturer: String,
    pub model: String,
    pub date_time: String,
    pub date_time_local_offset: String,
    pub service_entry_point_uuid: String,
    pub command_shell: CommandShell,
    pub graphical_console: GraphicalConsole,
    pub virtual_media: ODataLink,
    pub links: ManagerLinks,
    pub network_protocol: ODataLink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct CommandShell {
    pub service_enabled: bool,
    pub max_concurrent_sessions: u32,
    pub connect_types_supported: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GraphicalConsole {
    pub service_enabled: bool,
    pub max_concurrent_sessions: u32,
    pub connect_types_supported: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ManagerLinks {
    pub manager_for_servers: Vec<ODataLink>,
    pub manager_for_chassis: Vec<ODataLink>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct VirtualMedia {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub media_types: Vec<String>,
    pub connected_via: Option<String>,
    pub inserted: bool,
    pub image: Option<String>,
    pub image_name: Option<String>,
    pub write_protected: bool,
    pub transfer_method: Option<String>,
    pub transfer_protocol_type: Option<String>,
    pub status: Status,
    pub actions: VirtualMediaActions,
}

#[derive(Debug, Clone, Serialize)]
pub struct VirtualMediaActions {
    #[serde(rename = "#VirtualMedia.InsertMedia")]
    pub insert_media: ActionTarget,
    #[serde(rename = "#VirtualMedia.EjectMedia")]
    pub eject_media: ActionTarget,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InsertMediaRequest {
    pub image: String,
    #[serde(default)]
    pub write_protected: Option<bool>,
    #[serde(default)]
    pub transfer_method: Option<String>,
    #[serde(default)]
    pub transfer_protocol_type: Option<String>,
    pub media_types: Option<Vec<String>>,
    pub inserted: Option<bool>,
    pub user_name: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Chassis {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub chassis_type: String,
    pub asset_tag: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub part_number: String,
    pub power_state: String,
    pub status: Status,
    pub power: ODataLink,
    pub links: ChassisLinks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChassisLinks {
    pub computer_systems: Vec<ODataLink>,
    pub managed_by: Vec<ODataLink>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Power {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub power_control: Vec<PowerControl>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowerControl {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    pub member_id: String,
    pub name: String,
    pub power_consumed_watts: Option<f64>,
    pub power_capacity_watts: Option<f64>,
    pub power_requested_watts: Option<f64>,
    pub power_metrics: Option<PowerMetric>,
    pub status: Status,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PowerMetric {
    pub interval_in_min: u32,
    pub min_consumed_watts: Option<f64>,
    pub max_consumed_watts: Option<f64>,
    pub average_consumed_watts: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionService {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub service_enabled: bool,
    pub session_timeout: String,
    pub sessions: ODataLink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Session {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub user_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SessionCreateRequest {
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AccountService {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub service_enabled: bool,
    pub accounts: ODataLink,
    pub roles: ODataLink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ManagerAccount {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub user_name: String,
    pub role_id: String,
    pub locked: bool,
    pub links: ManagerAccountLinks,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ManagerAccountLinks {
    pub role: ODataLink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EventService {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub service_enabled: bool,
    pub delivery_retry_attempts: u32,
    pub delivery_retry_interval_seconds: u32,
    pub event_format_types: Vec<String>,
    pub registry_prefixes: Vec<String>,
    pub subordinate_resources: bool,
    #[serde(rename = "SSEFilterPropertiesSupported")]
    pub sse_filter_properties_supported: SseFilterPropertiesSupported,
    pub server_sent_event_uri: Option<String>,
    pub actions: EventServiceActions,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SseFilterPropertiesSupported {
    pub event_format_type: bool,
    pub message_id: bool,
    pub metric_report_definition: bool,
    pub origin_resource: bool,
    pub registry_prefix: bool,
    pub resource_type: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventServiceActions {
    #[serde(rename = "#EventService.SubmitTestEvent")]
    pub submit_test_event: ActionTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedfishError {
    pub error: RedfishErrorBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedfishErrorBody {
    pub code: String,
    pub message: String,
    #[serde(
        rename = "@Message.ExtendedInfo",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub extended_info: Vec<RedfishExtendedInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RedfishExtendedInfo {
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    pub message_id: String,
    pub message: String,
    pub severity: String,
    pub resolution: String,
}

impl RedfishError {
    pub fn general_error(message: &str) -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.GeneralError".to_string(),
                message: message.to_string(),
                extended_info: vec![],
            },
        }
    }

    pub fn authentication_required() -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.AuthenticationRequired".to_string(),
                message: "Authentication is required to access this resource".to_string(),
                extended_info: vec![RedfishExtendedInfo {
                    odata_type: "#Message.v1_2_1.Message".to_string(),
                    message_id: "Base.1.18.AuthenticationRequired".to_string(),
                    message: "Authentication is required to access this resource".to_string(),
                    severity: "Critical".to_string(),
                    resolution: "Authenticate using HTTP Basic auth or create a session via POST /redfish/v1/SessionService/Sessions".to_string(),
                }],
            },
        }
    }

    pub fn invalid_credentials() -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.AuthenticationRequired".to_string(),
                message: "Invalid username or password".to_string(),
                extended_info: vec![RedfishExtendedInfo {
                    odata_type: "#Message.v1_2_1.Message".to_string(),
                    message_id: "Base.1.18.InvalidCredentials".to_string(),
                    message: "Invalid username or password".to_string(),
                    severity: "Critical".to_string(),
                    resolution: "Correct the credentials and retry".to_string(),
                }],
            },
        }
    }

    pub fn resource_not_found() -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.ResourceNotFound".to_string(),
                message: "The requested resource was not found".to_string(),
                extended_info: vec![],
            },
        }
    }

    pub fn action_not_supported(action: &str) -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.ActionNotSupported".to_string(),
                message: format!("Action '{}' is not supported", action),
                extended_info: vec![],
            },
        }
    }

    pub fn property_missing(property: &str) -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.PropertyMissing".to_string(),
                message: format!("Property '{}' is required", property),
                extended_info: vec![],
            },
        }
    }

    pub fn service_unavailable(msg: &str) -> Self {
        Self {
            error: RedfishErrorBody {
                code: "Base.1.18.ServiceUnavailable".to_string(),
                message: msg.to_string(),
                extended_info: vec![],
            },
        }
    }
}
