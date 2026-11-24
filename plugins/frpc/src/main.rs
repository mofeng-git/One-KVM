mod config;

use anyhow::Result;
use one_kvm_plugin_sdk::{embedded::EmbeddedProcess, runtime::run};

const CORE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded-client"));

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config: config::FrpcConfig = client.read_config().await?;
        let text = config.render()?;
        EmbeddedProcess::launch(&client.data, CORE, |command, directory| {
            command.arg("-c").arg(EmbeddedProcess::write_config(
                directory,
                "frpc.toml",
                &text,
            )?);
            Ok(())
        })
    })
    .await
}
