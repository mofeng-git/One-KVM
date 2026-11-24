mod config;
mod rtsp;

use std::sync::Arc;

use anyhow::Result;
use one_kvm_plugin_sdk::{
    runtime::{run, Service},
    Status,
};
use rtsp::{RtspService, RtspServiceStatus};
use serde_json::json;

struct Plugin {
    service: RtspService,
    endpoint: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let mut config: config::RtspConfig = client.read_config().await?;
        config.enabled = true;
        config.path = config.path.trim_matches('/').to_owned();
        if config.path.is_empty() {
            anyhow::bail!("RTSP stream path is required");
        }
        let endpoint = format!(
            "rtsp://{}/{}",
            one_kvm_plugin_sdk::net::bind_socket_addr(&config.bind, config.port)?,
            config.path
        );
        let service = RtspService::new(config, Arc::new(client));
        service.start().await?;
        Ok(Plugin { service, endpoint })
    })
    .await
}

impl Service for Plugin {
    async fn status(&self) -> Result<Status> {
        let status = self.service.status().await;
        Ok(Status {
            fields: [("endpoint".into(), json!(self.endpoint))]
                .into_iter()
                .collect(),
            error: (!matches!(status, RtspServiceStatus::Running)).then(|| status.to_string()),
        })
    }

    async fn shutdown(&self) {
        let _ = self.service.stop().await;
    }
}
