mod receiver;

use std::sync::Arc;

use anyhow::{bail, Result};
use one_kvm_plugin_sdk::{
    runtime::{run, Client, Service},
    Status,
};
use receiver::{eligibility, AirferryManager};
use serde_json::{json, Value};

struct Plugin {
    client: Client,
    receiver: Arc<AirferryManager>,
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let receiver = AirferryManager::new();
        Ok(Plugin { client, receiver })
    })
    .await
}

impl Service for Plugin {
    async fn status(&self) -> Result<Status> {
        let ((_, reason), output) =
            tokio::join!(eligibility(&self.client), self.client.images_dir());
        let status = self.receiver.status(reason, &output?).await?;
        let fields = [
            ("receiving", json!(status.running)),
            ("capture", json!(status.reason)),
            (
                "event",
                json!(status.event.as_ref().map(|event| json!({
                    "state": event.state, "reason": event.reason, "progress": event.progress,
                }))),
            ),
            ("last_saved", json!(status.last_saved)),
            ("files", json!(status.files)),
        ];
        Ok(Status {
            fields: fields
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
            error: None,
        })
    }

    async fn action(&self, id: &str) -> Result<Value> {
        match id {
            "start_receive" => {
                let (eligible, reason) = eligibility(&self.client).await;
                if !eligible {
                    bail!("{reason}");
                }
                let output = self.client.images_dir().await?;
                self.receiver.start(self.client.clone(), output).await?;
            }
            "stop_receive" => self.receiver.stop().await?,
            _ => bail!("Unknown action"),
        }
        Ok(Value::Null)
    }

    async fn shutdown(&self) {
        let _ = self.receiver.stop().await;
    }
}
