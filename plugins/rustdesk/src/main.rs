mod hid;
mod rustdesk;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{bail, Result};
use base64::Engine;
use one_kvm_plugin_sdk::{
    runtime::{run, Service},
    Codec, Status, INSPECT_ARG,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use rustdesk::{
    config::{RustDeskCodec, RustDeskConfig},
    RustDeskService,
};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Identity {
    device_id: String,
    device_password: String,
    public_key: Option<String>,
    private_key: Option<String>,
    signing_public_key: Option<String>,
    signing_private_key: Option<String>,
    uuid: Option<String>,
}

impl From<RustDeskConfig> for Identity {
    fn from(config: RustDeskConfig) -> Self {
        Self {
            device_id: config.device_id,
            device_password: config.device_password,
            public_key: config.public_key,
            private_key: config.private_key,
            signing_public_key: config.signing_public_key,
            signing_private_key: config.signing_private_key,
            uuid: config.uuid,
        }
    }
}

impl Identity {
    fn apply(self, config: &mut RustDeskConfig) {
        config.device_id = self.device_id;
        config.device_password = self.device_password;
        config.public_key = self.public_key;
        config.private_key = self.private_key;
        config.signing_public_key = self.signing_public_key;
        config.signing_private_key = self.signing_private_key;
        config.uuid = self.uuid;
    }
}

struct Plugin {
    service: Arc<RustDeskService>,
    config: Mutex<RustDeskConfig>,
    identity: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new(INSPECT_ARG)) {
        let data = std::env::current_dir()?;
        let identity = load_identity(&data).await?;
        let status = Status {
            fields: identity_fields(&identity.device_id, &identity.device_password)
                .into_iter()
                .collect(),
            ..Status::default()
        };
        serde_json::to_writer(std::io::stdout().lock(), &status)?;
        return Ok(());
    }
    run(|client| async move {
        let mut config: RustDeskConfig = client.read_config().await?;
        config.enabled = true;
        config.rendezvous_server = config.rendezvous_server.trim().to_owned();
        config.relay_server = config
            .relay_server
            .take()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        config.relay_key = config
            .relay_key
            .take()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if let Some(key) = &config.relay_key {
            if base64::engine::general_purpose::STANDARD
                .decode(key.trim())?
                .len()
                != 32
            {
                bail!("RustDesk relay key must encode exactly 32 bytes");
            }
        }
        let identity = client.data.join("identity.json");
        load_identity(&client.data).await?.apply(&mut config);
        if !config.is_valid() {
            bail!("Provide a rendezvous server or select Direct IP mode");
        }
        client
            .claim_video(match config.codec {
                RustDeskCodec::H264 => Codec::H264,
                RustDeskCodec::H265 => Codec::H265,
            })
            .await?;
        let client = Arc::new(client);
        let service = Arc::new(RustDeskService::new(
            config.clone(),
            client.clone(),
            Arc::new(hid::HidController::new(client.as_ref().clone())),
            client,
        ));
        service.start().await?;
        if let Some(saved) = service.save_credentials() {
            config = saved;
        }
        save_identity(&identity, &Identity::from(config.clone()))?;
        Ok(Plugin {
            service,
            config: Mutex::new(config),
            identity,
        })
    })
    .await
}

async fn load_identity(data: &Path) -> Result<Identity> {
    let path = data.join("identity.json");
    let mut identity: Identity = match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let identity: Identity = serde_json::from_slice(&bytes)?;
            if !identity.device_id.is_empty() && !identity.device_password.is_empty() {
                return Ok(identity);
            }
            identity
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Identity::default(),
        Err(error) => return Err(error.into()),
    };
    if identity.device_id.is_empty() {
        identity.device_id = RustDeskConfig::generate_device_id();
    }
    if identity.device_password.is_empty() {
        identity.device_password = RustDeskConfig::generate_password();
    }
    save_identity(&path, &identity)?;
    Ok(identity)
}

fn identity_fields(device_id: &str, password: &str) -> [(String, Value); 2] {
    [
        ("device_id".into(), json!(device_id)),
        ("device_password".into(), json!(password)),
    ]
}

fn save_identity(path: &Path, identity: &Identity) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    serde_json::to_writer(file.as_file_mut(), identity)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

impl Service for Plugin {
    async fn status(&self) -> Result<Status> {
        let config = self.config.lock().await;
        let values = [
            ("service", json!(self.service.status().to_string())),
            (
                "rendezvous",
                json!(self
                    .service
                    .rendezvous_status()
                    .map(|status| status.to_string())),
            ),
            ("clients", json!(self.service.connection_count())),
            (
                "port",
                json!(self
                    .service
                    .is_listening()
                    .then(|| self.service.listen_port())),
            ),
        ];
        Ok(Status {
            fields: values
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .chain(identity_fields(&config.device_id, &config.device_password))
                .collect(),
            error: (!self.service.is_running()).then(|| self.service.status().to_string()),
        })
    }

    async fn action(&self, id: &str) -> Result<Value> {
        let mut current = self.config.lock().await;
        let mut next = current.clone();
        match id {
            "regenerate_id" => next.device_id = RustDeskConfig::generate_device_id(),
            "regenerate_password" => next.device_password = RustDeskConfig::generate_password(),
            _ => bail!("Unknown action"),
        }
        save_identity(&self.identity, &Identity::from(next.clone()))?;
        self.service.restart(next.clone()).await?;
        if let Some(saved) = self.service.save_credentials() {
            next = saved;
        }
        save_identity(&self.identity, &Identity::from(next.clone()))?;
        *current = next;
        Ok(Value::Null)
    }

    async fn shutdown(&self) {
        let _ = self.service.stop().await;
    }
}
