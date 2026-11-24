use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpProxyType {
    Tcp,
    Udp,
    Http,
    Https,
    Stcp,
    Sudp,
    Xtcp,
}

impl Default for FrpProxyType {
    fn default() -> Self {
        Self::Tcp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrpcConfigMode {
    Quick,
    Full,
}

impl Default for FrpcConfigMode {
    fn default() -> Self {
        Self::Quick
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FrpcConfig {
    pub config_mode: FrpcConfigMode,
    pub proxy_name: String,
    pub proxy_type: FrpProxyType,
    pub server_addr: String,
    pub server_port: u16,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub token: String,
    pub local_ip: String,
    pub local_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub secret_key: String,
    pub tls: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub custom_toml: String,
}

impl Default for FrpcConfig {
    fn default() -> Self {
        Self {
            config_mode: FrpcConfigMode::Quick,
            proxy_name: String::new(),
            proxy_type: FrpProxyType::Tcp,
            server_addr: String::new(),
            server_port: 7000,
            token: String::new(),
            local_ip: "127.0.0.1".to_string(),
            local_port: 22,
            remote_port: None,
            custom_domain: None,
            secret_key: String::new(),
            tls: true,
            custom_toml: String::new(),
        }
    }
}

impl FrpcConfig {
    pub fn render(&self) -> Result<String> {
        match self.config_mode {
            FrpcConfigMode::Quick => self.render_quick(),
            FrpcConfigMode::Full => {
                let text = self.custom_toml.trim();
                if text.is_empty() {
                    bail!("FRP full configuration is required");
                }
                text.parse::<DocumentMut>().context("Invalid FRP TOML")?;
                Ok(text.to_owned())
            }
        }
    }

    fn render_quick(&self) -> Result<String> {
        for (name, field) in [
            ("Proxy name", self.proxy_name.trim()),
            ("Server address", self.server_addr.trim()),
            ("Token", self.token.as_str()),
            ("Local IP", self.local_ip.trim()),
        ] {
            if field.is_empty() {
                bail!("{name} is required");
            }
        }
        let proxy_type = match self.proxy_type {
            FrpProxyType::Tcp => "tcp",
            FrpProxyType::Udp => "udp",
            FrpProxyType::Http => "http",
            FrpProxyType::Https => "https",
            FrpProxyType::Stcp => "stcp",
            FrpProxyType::Sudp => "sudp",
            FrpProxyType::Xtcp => "xtcp",
        };

        let mut doc = DocumentMut::new();
        doc["serverAddr"] = value(self.server_addr.trim());
        doc["serverPort"] = value(i64::from(self.server_port));
        doc["auth"]["method"] = value("token");
        doc["auth"]["token"] = value(&self.token);
        doc["transport"]["protocol"] = value("tcp");
        doc["transport"]["tls"]["enable"] = value(self.tls);

        let mut proxy = Table::new();
        proxy["name"] = value(self.proxy_name.trim());
        proxy["type"] = value(proxy_type);
        proxy["localIP"] = value(self.local_ip.trim());
        proxy["localPort"] = value(i64::from(self.local_port));
        match self.proxy_type {
            FrpProxyType::Tcp | FrpProxyType::Udp => {
                let port = self
                    .remote_port
                    .filter(|port| *port != 0)
                    .context("Remote port is required for TCP/UDP")?;
                proxy["remotePort"] = value(i64::from(port));
            }
            FrpProxyType::Http | FrpProxyType::Https => {
                if let Some(domain) = self
                    .custom_domain
                    .as_deref()
                    .map(str::trim)
                    .filter(|domain| !domain.is_empty())
                {
                    let mut domains = Array::new();
                    domains.push(domain);
                    proxy["customDomains"] = value(domains);
                }
            }
            FrpProxyType::Stcp | FrpProxyType::Sudp | FrpProxyType::Xtcp => {
                if !self.secret_key.is_empty() {
                    proxy["secretKey"] = value(&self.secret_key);
                }
            }
        }
        let mut proxies = ArrayOfTables::new();
        proxies.push(proxy);
        doc["proxies"] = toml_edit::Item::ArrayOfTables(proxies);
        Ok(doc.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_config_encodes_fields_and_requires_remote_port() {
        let mut config = FrpcConfig {
            proxy_name: "kvm".into(),
            server_addr: "example.com".into(),
            token: "quoted \" token".into(),
            ..Default::default()
        };
        assert!(config.render().is_err());
        config.remote_port = Some(8001);
        let doc: DocumentMut = config.render().unwrap().parse().unwrap();
        assert_eq!(doc["auth"]["token"].as_str(), Some("quoted \" token"));
        assert_eq!(doc["proxies"][0]["remotePort"].as_integer(), Some(8001));
    }

    #[test]
    fn full_config_is_validated() {
        let mut config = FrpcConfig {
            config_mode: FrpcConfigMode::Full,
            ..Default::default()
        };
        assert!(config.render().is_err());
        config.custom_toml = "serverAddr = \"example.com\"".into();
        assert_eq!(config.render().unwrap(), config.custom_toml);
    }
}
