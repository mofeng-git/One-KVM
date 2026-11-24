use anyhow::{bail, Result};
use one_kvm_plugin_sdk::{embedded::EmbeddedProcess, runtime::run};
use serde::Deserialize;
use url::Url;

const CORE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded-client"));

#[derive(Deserialize)]
struct Config {
    control_urls: Vec<String>,
}

fn control_arguments(config: &Config) -> Result<Vec<&str>> {
    if config.control_urls.is_empty() {
        bail!("At least one EasyFrp control URL is required");
    }
    let mut arguments = vec!["control"];
    for input in &config.control_urls {
        let value = input.trim();
        if value.chars().any(char::is_whitespace) {
            bail!("EasyFrp control URLs cannot contain whitespace");
        }
        let Ok(url) = Url::parse(value) else {
            bail!("Invalid EasyFrp control URL");
        };
        if !matches!(url.scheme(), "ws" | "wss" | "http" | "https" | "txt")
            || url.host_str().is_none()
            || url.path().trim_matches('/').is_empty()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            bail!("EasyFrp requires a ws/wss/http/https/txt URL with a client key path");
        }
        arguments.push(value);
    }
    Ok(arguments)
}

#[tokio::main]
async fn main() -> Result<()> {
    run(|client| async move {
        let config = client.read_config().await?;
        let args = control_arguments(&config)?;
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
    fn control_urls_are_passed_as_separate_arguments() {
        let config = Config {
            control_urls: vec![" wss://example.com/a ".into(), "txt://example.net/b".into()],
        };
        assert_eq!(
            control_arguments(&config).unwrap(),
            ["control", "wss://example.com/a", "txt://example.net/b"]
        );
    }

    #[test]
    fn rejects_options_and_malformed_urls() {
        for url in [
            "--help",
            "file:///tmp/key",
            "wss://example.com/",
            "wss://example.com/a b",
        ] {
            let config = Config {
                control_urls: vec![url.into()],
            };
            assert!(control_arguments(&config).is_err(), "{url}");
        }
    }
}
