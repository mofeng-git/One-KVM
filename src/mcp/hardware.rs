//! Device operations share the MCP worker and existing controller locks.
use super::executor::output;
use crate::{
    atx::{AtxController, AtxState},
    config::ConfigStore,
    db::WolHistoryStore,
    error::{AppError, Result},
};
use rmcp::{
    model::CallToolResult,
    schemars::{self, JsonSchema},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum PowerAction {
    PressShort,
    PressLong,
    Reset,
}

pub(super) enum HardwareCommand {
    Power(PowerAction),
    Wake(String),
    ListMedia,
    MountMedia {
        image_id: String,
        cdrom: bool,
        read_only: bool,
    },
    UnmountMedia(String),
}

pub(crate) struct McpHardware {
    pub config: ConfigStore,
    pub atx: Arc<RwLock<Option<AtxController>>>,
    pub wol_history: WolHistoryStore,
    #[cfg(unix)]
    pub msd: Arc<RwLock<Option<crate::msd::MsdController>>>,
    #[cfg(unix)]
    pub otg_lock: Arc<tokio::sync::Mutex<()>>,
}

fn atx_json(state: AtxState) -> Value {
    json!({
        "available": state.available, "backend": state.driver,
        "power_configured": state.power_configured, "reset_configured": state.reset_configured,
        "power_status": state.power_status, "led_supported": state.led_supported,
        "hdd_status": state.hdd_status, "hdd_supported": state.hdd_supported,
    })
}

pub(super) fn valid_image_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

impl McpHardware {
    pub(super) async fn status(&self) -> Value {
        let atx = self.atx.read().await;
        let atx = match atx.as_ref() {
            Some(controller) => controller.state().await,
            None => AtxState::default(),
        };
        json!({"atx": atx_json(atx), "media": self.media_status().await})
    }

    #[cfg(unix)]
    async fn media_status(&self) -> Value {
        let guard = self.msd.read().await;
        let state = match guard.as_ref() {
            Some(controller) => controller.state().await,
            None => Default::default(),
        };
        json!({"supported": true, "available": state.available,
            "disk_mode": state.disk_mode, "slot_capacity": state.disk_mode.capacity(),
            "mounted_count": state.mounted_media.len(), "usb_reenumerating": state.usb_reenumerating})
    }

    #[cfg(not(unix))]
    async fn media_status(&self) -> Value {
        json!({"supported": false, "available": false})
    }

    pub(super) async fn execute(&self, command: HardwareCommand) -> CallToolResult {
        match self.run(command).await {
            Ok(value) => output(value, false),
            Err(error) => {
                let code = match &error {
                    AppError::Msd(msd) => msd.code().as_str(),
                    AppError::ServiceUnavailable(_) => "unavailable",
                    AppError::BadRequest(_) => "invalid_arguments",
                    AppError::Conflict(_) => "busy",
                    _ => "hardware_error",
                };
                output(
                    json!({"success": false, "error": code, "message": error.to_string()}),
                    true,
                )
            }
        }
    }

    async fn run(&self, command: HardwareCommand) -> Result<Value> {
        match command {
            HardwareCommand::Power(action) => {
                let guard = self.atx.read().await;
                let controller = guard.as_ref().ok_or_else(|| {
                    AppError::ServiceUnavailable("ATX controller is not configured".into())
                })?;
                let state = controller.state().await;
                let configured = match action {
                    PowerAction::Reset => state.reset_configured,
                    _ => state.power_configured,
                };
                if !state.available || !configured {
                    return Err(AppError::ServiceUnavailable(
                        "Requested ATX button is not configured".into(),
                    ));
                }
                match action {
                    PowerAction::PressShort => controller.power_short().await?,
                    PowerAction::PressLong => controller.power_long().await?,
                    PowerAction::Reset => controller.reset().await?,
                }
                Ok(
                    json!({"success": true, "action_sent": true, "action": action,
                    "atx": atx_json(controller.state().await)}),
                )
            }
            HardwareCommand::Wake(mac_address) => {
                let interface = self.config.get().atx.wol_interface.clone();
                let target = mac_address.clone();
                tokio::task::spawn_blocking(move || {
                    crate::atx::send_wol(
                        &target,
                        (!interface.is_empty()).then_some(interface.as_str()),
                    )
                })
                .await
                .map_err(|e| AppError::Internal(format!("WOL worker failed: {e}")))??;
                let mut result =
                    json!({"success": true, "packet_sent": true, "mac_address": mac_address});
                // Sending succeeded even if recording history fails. Never invite a write retry.
                if let Err(error) = self.wol_history.record(&mac_address).await {
                    tracing::warn!(%error, "Failed to persist MCP WOL history");
                    result["history_saved"] = json!(false);
                } else {
                    result["history_saved"] = json!(true);
                }
                Ok(result)
            }
            HardwareCommand::ListMedia => self.list_media().await,
            command => self.change_media(command).await,
        }
    }

    #[cfg(unix)]
    async fn list_media(&self) -> Result<Value> {
        let guard = self.msd.read().await;
        let state = match guard.as_ref() {
            Some(controller) => controller.state().await,
            None => Default::default(),
        };
        let path = self.config.get().msd.images_dir();
        let images =
            tokio::task::spawn_blocking(move || crate::msd::ImageManager::new(path).list())
                .await
                .map_err(|e| AppError::Internal(format!("Media listing failed: {e}")))??;
        Ok(json!({"success": true, "available": state.available,
            "disk_mode": state.disk_mode, "slot_capacity": state.disk_mode.capacity(),
            "mounted_count": state.mounted_media.len(), "mounted_media": state.mounted_media,
            "usb_reenumerating": state.usb_reenumerating, "images": images}))
    }

    #[cfg(unix)]
    async fn change_media(&self, command: HardwareCommand) -> Result<Value> {
        use crate::error::MsdErrorCode;
        // Same lock as USB settings and disk-mode changes, followed by the controller lock.
        let _otg = self
            .otg_lock
            .try_lock()
            .map_err(|_| AppError::from(MsdErrorCode::MsdOperationInProgress))?;
        let guard = self.msd.write().await;
        let controller = guard
            .as_ref()
            .ok_or_else(|| AppError::from(MsdErrorCode::MsdUnavailable))?;
        match command {
            HardwareCommand::MountMedia {
                image_id,
                cdrom,
                read_only,
            } => {
                let path = self.config.get().msd.images_dir();
                let id = image_id.clone();
                let image = tokio::task::spawn_blocking(move || {
                    crate::msd::ImageManager::new(path).get(&id)
                })
                .await
                .map_err(|e| AppError::Internal(format!("Media lookup failed: {e}")))??;
                controller.mount_image(&image, cdrom, read_only).await?;
                Ok(
                    json!({"success": true, "image_id": image_id, "mounted": true,
                    "cdrom": cdrom, "read_only": cdrom || read_only}),
                )
            }
            HardwareCommand::UnmountMedia(image_id) => {
                // Idempotent: an absent image is already unmounted. Never eject other media.
                controller.unmount_image(&image_id).await?;
                Ok(json!({"success": true, "image_id": image_id, "mounted": false}))
            }
            _ => unreachable!("only media mutations reach change_media"),
        }
    }

    #[cfg(not(unix))]
    async fn list_media(&self) -> Result<Value> {
        Err(AppError::ServiceUnavailable(
            "Virtual media is unsupported on this platform".into(),
        ))
    }

    #[cfg(not(unix))]
    async fn change_media(&self, _: HardwareCommand) -> Result<Value> {
        self.list_media().await
    }

    #[cfg(test)]
    pub(super) fn for_test(config: ConfigStore, db: &crate::db::DatabasePool) -> Self {
        Self {
            config,
            atx: Arc::new(RwLock::new(None)),
            wol_history: db.wol_history(),
            #[cfg(unix)]
            msd: Arc::new(RwLock::new(None)),
            #[cfg(unix)]
            otg_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_power_is_unknown_and_media_is_serialized_with_usb_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::DatabasePool::new(&dir.path().join("hardware.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let config = ConfigStore::new(db.clone_pool());
        config.load().await.unwrap();
        config
            .update(|c| c.msd.msd_dir = dir.path().join("msd").to_string_lossy().into())
            .await
            .unwrap();
        let hardware = McpHardware::for_test(config, &db);
        let status = hardware.status().await;
        assert_eq!(status["atx"]["available"], false);
        assert_eq!(status["atx"]["power_status"], "unknown");
        let power = hardware
            .execute(HardwareCommand::Power(PowerAction::PressShort))
            .await;
        assert_eq!(power.is_error, Some(true));
        assert_eq!(power.structured_content.unwrap()["error"], "unavailable");
        #[cfg(unix)]
        {
            let lock = hardware.otg_lock.lock().await;
            let mount = hardware
                .execute(HardwareCommand::MountMedia {
                    image_id: "abc".into(),
                    cdrom: true,
                    read_only: true,
                })
                .await;
            assert_eq!(
                mount.structured_content.unwrap()["error"],
                "MSD_OPERATION_IN_PROGRESS"
            );
            drop(lock);
            let missing = hardware
                .execute(HardwareCommand::UnmountMedia("abc".into()))
                .await;
            assert_eq!(
                missing.structured_content.unwrap()["error"],
                "MSD_UNAVAILABLE"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn media_listing_returns_ids_and_capacity_without_host_paths() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::DatabasePool::new(&dir.path().join("hardware.db"))
            .await
            .unwrap();
        db.init_schema().await.unwrap();
        let config = ConfigStore::new(db.clone_pool());
        config.load().await.unwrap();
        config
            .update(|c| c.msd.msd_dir = dir.path().join("msd").to_string_lossy().into())
            .await
            .unwrap();
        let images = config.get().msd.images_dir();
        std::fs::create_dir_all(&images).unwrap();
        std::fs::write(images.join("mcp-test.iso"), [0u8; 2048]).unwrap();
        let hardware = McpHardware::for_test(config, &db);
        let result = hardware.execute(HardwareCommand::ListMedia).await;
        assert_eq!(result.is_error, Some(false));
        let value = result.structured_content.unwrap();
        assert_eq!(value["slot_capacity"], 1);
        assert_eq!(value["mounted_count"], 0);
        let image = &value["images"][0];
        assert_eq!(image["name"], "mcp-test.iso");
        assert_eq!(image["size"], 2048);
        assert!(valid_image_id(image["id"].as_str().unwrap()));
        assert!(image.get("path").is_none());
        assert!(!value
            .to_string()
            .contains(&dir.path().to_string_lossy().to_string()));
    }
}
