use webrtc::ice::mdns::MulticastDnsMode;

pub fn mdns_mode_from_env() -> Option<MulticastDnsMode> {
    let raw = std::env::var("ONE_KVM_WEBRTC_MDNS_MODE").ok()?;
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }

    match value.as_str() {
        "disabled" | "off" | "false" | "0" => Some(MulticastDnsMode::Disabled),
        "query" | "query_only" | "query-only" => Some(MulticastDnsMode::QueryOnly),
        "gather" | "query_and_gather" | "query-and-gather" | "on" | "true" | "1" => {
            Some(MulticastDnsMode::QueryAndGather)
        }
        _ => None,
    }
}

pub fn mdns_mode() -> MulticastDnsMode {
    // Default to QueryOnly to avoid gathering .local host candidates by default.
    // This is generally more stable for LAN first-connection while preserving mDNS queries.
    mdns_mode_from_env().unwrap_or(MulticastDnsMode::QueryOnly)
}

pub fn mdns_mode_label(mode: MulticastDnsMode) -> &'static str {
    match mode {
        MulticastDnsMode::Disabled => "disabled",
        MulticastDnsMode::QueryOnly => "query_only",
        MulticastDnsMode::QueryAndGather => "query_and_gather",
    }
}

pub fn default_mdns_host_name(session_id: &str) -> String {
    format!("{session_id}.local")
}
