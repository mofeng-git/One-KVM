use super::{
    actions::{self, Action},
    executor::{failure, Command},
    hardware::{valid_image_id, HardwareCommand, PowerAction},
    McpService,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotParams {
    /// Optional JPEG quality, 1 to 100. Native width and height are always preserved.
    pub quality: Option<u8>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionsParams {
    /// One to 32 actions. Pointer x/y values are normalized to [0,1] over the whole screen.
    pub actions: Vec<Action>,
    /// Return a fresh native-resolution JPEG after executing the batch.
    #[serde(default)]
    pub return_screenshot: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PowerParams {
    /// Physical ATX button: short power press, long power press (force off), or reset.
    action: PowerAction,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WakeParams {
    /// Target MAC address, e.g. AA:BB:CC:DD:EE:FF. Uses the configured WOL interface.
    mac_address: String,
}
fn default_true() -> bool {
    true
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MountMediaParams {
    /// Image ID from one_kvm_list_media, never a filesystem path or URL.
    image_id: String,
    /// Present as a CD-ROM. Defaults to true.
    #[serde(default = "default_true")]
    cdrom: bool,
    /// Read-only access. Defaults to true; CD-ROM is always read-only.
    #[serde(default = "default_true")]
    read_only: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnmountMediaParams {
    /// Image ID from one_kvm_list_media. Does not unmount other images or the virtual drive.
    image_id: String,
}
#[derive(Clone)]
pub struct McpTools {
    service: Arc<McpService>,
    tool_router: ToolRouter<Self>,
}
impl McpTools {
    pub fn new(service: Arc<McpService>) -> Self {
        Self {
            service,
            tool_router: Self::tool_router(),
        }
    }
}
#[tool_router]
impl McpTools {
    #[tool(
        name = "one_kvm_get_status",
        description = "Get One-KVM video signal, current resolution, HID capabilities and global queue length."
    )]
    async fn status(&self) -> CallToolResult {
        self.service.call(Command::Status).await
    }

    #[tool(
        name = "one_kvm_screenshot",
        description = "Capture a fresh JPEG at the current source resolution, without resizing or cropping. No browser is needed. Coordinates for pointer actions are normalized to [0,1] over this whole image."
    )]
    async fn screenshot(&self, Parameters(params): Parameters<ScreenshotParams>) -> CallToolResult {
        if params.quality.is_some_and(|q| !(1..=100).contains(&q)) {
            return failure("invalid_arguments", "quality must be between 1 and 100", 0);
        }
        self.service.call(Command::Screenshot(params.quality)).await
    }

    #[tool(
        name = "one_kvm_execute_actions",
        description = "Execute a bounded batch of physical HID actions in global FIFO order. Pointer coordinates are normalized: (0,0) top-left, (1,1) bottom-right. Positive vertical_steps scrolls down. Text uses the target's US keyboard layout (ASCII, newline, tab); no clipboard or arbitrary Unicode. All parameters are checked before input. No client ownership, tasks or cancellation. On connection failure, inspect the screen before retrying: actions may already have executed."
    )]
    async fn actions(&self, Parameters(params): Parameters<ActionsParams>) -> CallToolResult {
        match actions::prepare(params.actions) {
            Ok(actions) => {
                self.service
                    .call(Command::Actions(actions, params.return_screenshot))
                    .await
            }
            Err(message) => failure("invalid_arguments", &message, 0),
        }
    }

    #[tool(
        name = "one_kvm_power",
        description = "Press a configured physical ATX button: press_short, press_long (force off), or reset. Inspect ATX availability with one_kvm_get_status first. Success means the button pulse was sent, not that the target OS completed startup/shutdown. LED state is unknown when not wired. Uses the global FIFO and rate limit; never automatically repeat power operations."
    )]
    async fn power(&self, Parameters(params): Parameters<PowerParams>) -> CallToolResult {
        self.service
            .call(Command::Hardware(HardwareCommand::Power(params.action)))
            .await
    }

    #[tool(
        name = "one_kvm_wake_on_lan",
        description = "Send Wake-on-LAN magic packets to a MAC address, using One-KVM's configured network interface. Reports packet transmission, not successful startup. Does not require ATX wiring. Shares the global FIFO and rate limit."
    )]
    async fn wake(&self, Parameters(params): Parameters<WakeParams>) -> CallToolResult {
        match crate::atx::normalize_wol_mac_address(&params.mac_address) {
            Ok(mac) => {
                self.service
                    .call(Command::Hardware(HardwareCommand::Wake(mac)))
                    .await
            }
            Err(error) => failure("invalid_arguments", &error.to_string(), 0),
        }
    }

    #[tool(
        name = "one_kvm_list_media",
        description = "List uploaded ISO/IMG images, their image IDs and sizes, currently mounted media, and USB media slot capacity. No upload or file paths. Virtual media requires a supported USB gadget backend. Uses the global FIFO and rate limit.",
        annotations(read_only_hint = true)
    )]
    async fn list_media(&self) -> CallToolResult {
        self.service
            .call(Command::Hardware(HardwareCommand::ListMedia))
            .await
    }

    #[tool(
        name = "one_kvm_mount_media",
        description = "Mount an uploaded image by its image_id from one_kvm_list_media into a free USB media slot. Defaults to CD-ROM and read-only; CD-ROM always forces read-only. To mount a disk, explicitly set cdrom=false. Does not upload, download, replace existing media, change USB configuration, or initialize/format a drive. Shares global FIFO/rate limits and existing USB operation locks."
    )]
    async fn mount_media(
        &self,
        Parameters(params): Parameters<MountMediaParams>,
    ) -> CallToolResult {
        if !valid_image_id(&params.image_id) {
            return failure(
                "invalid_arguments",
                "image_id must be an image ID returned by one_kvm_list_media",
                0,
            );
        }
        self.service
            .call(Command::Hardware(HardwareCommand::MountMedia {
                image_id: params.image_id,
                cdrom: params.cdrom,
                read_only: params.read_only,
            }))
            .await
    }

    #[tool(
        name = "one_kvm_unmount_media",
        description = "Unmount only the uploaded image identified by image_id, releasing its USB media slot. An already unmounted image succeeds. Does not unmount the virtual drive or other images, delete files, or force removal when the target prevents ejection. Shares the global FIFO/rate limit and existing USB operation locks."
    )]
    async fn unmount_media(
        &self,
        Parameters(params): Parameters<UnmountMediaParams>,
    ) -> CallToolResult {
        if !valid_image_id(&params.image_id) {
            return failure(
                "invalid_arguments",
                "image_id must be an image ID returned by one_kvm_list_media",
                0,
            );
        }
        self.service
            .call(Command::Hardware(HardwareCommand::UnmountMedia(
                params.image_id,
            )))
            .await
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new("one-kvm", env!("CARGO_PKG_VERSION")))
            .with_instructions("One-KVM controls a physical computer using video and HID. First inspect the screen, then execute short batches and observe again. The service has one global FIFO and rate limit; coordinate multiple operators externally.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screenshot_accepts_native_resolution_only() {
        assert!(serde_json::from_value::<ScreenshotParams>(serde_json::json!({})).is_ok());
        assert!(
            serde_json::from_value::<ScreenshotParams>(serde_json::json!({"max_width": 1920}))
                .is_err()
        );
        assert!(serde_json::from_value::<ScreenshotParams>(
            serde_json::json!({"max_height": 1080})
        )
        .is_err());
        assert!(serde_json::from_value::<ActionsParams>(
            serde_json::json!({"actions": [{"type": "wait", "ms": 1}], "control_id": "unneeded"})
        )
        .is_err());
    }

    #[test]
    fn hardware_params_have_safe_defaults_and_require_ids() {
        let mount: MountMediaParams =
            serde_json::from_value(serde_json::json!({"image_id":"abcdef"})).unwrap();
        assert!(mount.cdrom && mount.read_only);
        assert!(valid_image_id(&mount.image_id));
        for id in [
            "",
            "drive",
            "/etc/passwd",
            "../image.iso",
            "https://host/disk.iso",
            "0123456789abcdef0",
        ] {
            assert!(!valid_image_id(id), "{id}");
        }
        assert!(serde_json::from_value::<PowerParams>(serde_json::json!({"action":"on"})).is_err());
        assert!(serde_json::from_value::<MountMediaParams>(
            serde_json::json!({"image_id":"abc", "path":"/tmp/a"})
        )
        .is_err());
    }
}
