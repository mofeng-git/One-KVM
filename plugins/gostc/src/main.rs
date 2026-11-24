use anyhow::{ensure, Result};
use one_kvm_plugin_sdk::{embedded::EmbeddedProcess, runtime::run};
use serde::Deserialize;

const CORE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded-client"));

#[derive(Deserialize)]
struct GostcConfig {
    addr: String,
    key: String,
    tls: bool,
}

impl GostcConfig {
    fn args(&self) -> Result<[String; 6]> {
        let addr = self.addr.trim();
        ensure!(!addr.is_empty(), "GOSTC server address is required");
        ensure!(!self.key.is_empty(), "GOSTC client key is required");

        Ok([
            "-console".into(),
            format!("--tls={}", self.tls),
            "-addr".into(),
            addr.into(),
            "-key".into(),
            self.key.clone(),
        ])
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config: GostcConfig = client.read_config().await?;
        let args = config.args()?;
        EmbeddedProcess::launch(&client.data, CORE, |command, _| {
            command.args(args);
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_address_and_key_and_preserves_launch_arguments() {
        let mut config = GostcConfig {
            addr: " ".into(),
            key: "".into(),
            tls: true,
        };
        assert!(config.args().is_err());
        config.addr = " example.com:443 ".into();
        assert!(config.args().is_err());
        config.key = "key with spaces".into();
        for tls in [true, false] {
            config.tls = tls;
            assert_eq!(
                config.args().unwrap(),
                [
                    "-console".into(),
                    format!("--tls={tls}"),
                    "-addr".into(),
                    "example.com:443".into(),
                    "-key".into(),
                    "key with spaces".into(),
                ]
            );
        }
    }
}
