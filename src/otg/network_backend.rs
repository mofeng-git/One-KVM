//! Select OpenWrt netifd or a running NetworkManager, with an explicit override.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkBackend {
    NetworkManager,
    Netifd,
}

pub(crate) fn select_backend(
    requested: Option<&str>,
    openwrt_release_present: bool,
) -> Result<NetworkBackend, String> {
    match requested.unwrap_or("auto") {
        "auto" if openwrt_release_present => Ok(NetworkBackend::Netifd),
        "auto" | "networkmanager" => Ok(NetworkBackend::NetworkManager),
        "netifd" => Ok(NetworkBackend::Netifd),
        value => Err(format!(
            "Unknown ONE_KVM_NETWORK_BACKEND={value}; expected auto, networkmanager or netifd"
        )),
    }
}

pub(crate) fn current_backend() -> Result<NetworkBackend, String> {
    let requested = std::env::var("ONE_KVM_NETWORK_BACKEND").ok();
    let backend = select_backend(
        requested.as_deref(),
        std::path::Path::new("/etc/openwrt_release").is_file(),
    )?;
    if backend == NetworkBackend::NetworkManager {
        let status = std::process::Command::new("nmcli")
            .env("LC_ALL", "C")
            .args(["-t", "-f", "RUNNING", "general"])
            .output()
            .map_err(|_| {
                "OTG bridging requires NetworkManager and nmcli on this system".to_string()
            })?;
        if !status.status.success() || String::from_utf8_lossy(&status.stdout).trim() != "running" {
            return Err("NetworkManager is not running; OTG bridging cannot manage this system's interfaces".to_string());
        }
    }
    Ok(backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_selection_respects_openwrt_marker() {
        assert_eq!(select_backend(None, true), Ok(NetworkBackend::Netifd));
        assert_eq!(
            select_backend(Some("auto"), false),
            Ok(NetworkBackend::NetworkManager)
        );
    }

    #[test]
    fn explicit_selection_is_respected() {
        assert_eq!(
            select_backend(Some("networkmanager"), true),
            Ok(NetworkBackend::NetworkManager)
        );
        assert_eq!(
            select_backend(Some("netifd"), false),
            Ok(NetworkBackend::Netifd)
        );
    }

    #[test]
    fn unknown_selection_is_not_silently_ignored() {
        assert!(select_backend(Some("typo"), true).is_err());
        assert!(select_backend(Some(""), false).is_err());
    }
}
