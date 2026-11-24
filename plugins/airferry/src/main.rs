mod receiver;

use std::sync::Arc;

use anyhow::{bail, Result};
use one_kvm_plugin_sdk::{
    runtime::{run, Client, Service},
    Status,
};
use receiver::{eligibility, AirferryManager};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    import_images: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            import_images: true,
        }
    }
}

struct Plugin {
    client: Client,
    receiver: Arc<AirferryManager>,
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config: Config = client.read_config().await?;
        let output = client.data.join("files");
        tokio::fs::create_dir_all(&output).await?;
        let receiver = AirferryManager::new(output, config.import_images);
        Ok(Plugin { client, receiver })
    })
    .await
}

impl Service for Plugin {
    async fn status(&self) -> Result<Status> {
        let (eligible, reason) = eligibility(&self.client).await;
        let status = self.receiver.status(eligible, reason).await?;
        let fields = [
            ("receiving", json!(status.running)),
            ("capture", json!(status.reason)),
            (
                "event",
                json!(status.event.as_ref().map(|event| &event.state)),
            ),
            (
                "progress",
                json!(status
                    .event
                    .as_ref()
                    .and_then(|event| event.progress.as_ref())
                    .map(|progress| progress.decoded_fraction.clamp(0.0, 1.0))),
            ),
            (
                "last_saved",
                json!(status.last_saved.as_ref().map(|file| &file.filename)),
            ),
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
                self.receiver.start(self.client.clone()).await?;
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
