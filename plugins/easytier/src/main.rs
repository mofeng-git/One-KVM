mod config;

use anyhow::{bail, Result};
use config::{EasytierConfig, EasytierConfigMode};
use one_kvm_plugin_sdk::{embedded::EmbeddedProcess, runtime::run};

const CORE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded-client"));

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config: EasytierConfig = client.read_config().await?;
        EmbeddedProcess::launch(&client.data, CORE, |command, directory| {
            match config.config_mode {
                EasytierConfigMode::Quick => {
                    if config.network_name.trim().is_empty() {
                        bail!("Network name is required");
                    }
                    command.args([
                        "--network-name",
                        &config.network_name,
                        "--network-secret",
                        &config.network_secret,
                    ]);
                    for peer in config
                        .peer_urls
                        .iter()
                        .map(|peer| peer.trim())
                        .filter(|peer| !peer.is_empty())
                    {
                        command.args(["--peers", peer]);
                    }
                    if let Some(ip) = config
                        .virtual_ip
                        .as_deref()
                        .map(str::trim)
                        .filter(|ip| !ip.is_empty())
                    {
                        command.args(["-i", ip]);
                    } else {
                        command.arg("-d");
                    }
                }
                EasytierConfigMode::Full => {
                    if config.custom_toml.trim().is_empty() {
                        bail!("Full configuration is required");
                    }
                    config.custom_toml.parse::<toml_edit::DocumentMut>()?;
                    command.arg("-c").arg(EmbeddedProcess::write_config(
                        directory,
                        "easytier.toml",
                        &config.custom_toml,
                    )?);
                }
            }
            Ok(())
        })
    })
    .await
}
