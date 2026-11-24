mod config;
mod vnc;

mod hid {
    pub use one_kvm_plugin_sdk::{keyboard::CanonicalKey, types::*};
}

use std::sync::Arc;

use anyhow::Result;
use one_kvm_plugin_sdk::{
    runtime::{run, Service},
    Status,
};
use serde_json::json;
use vnc::{VncService, VncServiceStatus};

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let mut config: config::VncConfig = client.read_config().await?;
        config.enabled = true;
        if config
            .password
            .as_ref()
            .is_some_and(|password| password.len() > 8)
        {
            anyhow::bail!("VNCAuth password must be at most eight bytes");
        }
        let service = VncService::new(config, Arc::new(client));
        service.start().await?;
        Ok(service)
    })
    .await
}

impl Service for VncService {
    async fn status(&self) -> Result<Status> {
        let status = self.status().await;
        Ok(Status {
            fields: [("clients".into(), json!(self.connection_count()))]
                .into_iter()
                .collect(),
            error: (!matches!(status, VncServiceStatus::Running)).then(|| status.to_string()),
        })
    }

    async fn shutdown(&self) {
        let _ = self.stop().await;
    }
}
