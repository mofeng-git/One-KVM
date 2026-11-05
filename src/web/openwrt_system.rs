//! Read-only shortcut to the independent OpenWrt web server. No proxy or shared auth.
use axum::{
    http::{header, uri::Authority, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use std::{net::IpAddr, time::Duration};
use tokio::process::Command;

/// Explicit opt-in supplied by the OpenWrt image's service launcher.
pub fn enabled() -> bool {
    cfg!(target_os = "linux") && std::env::var("ONE_KVM_OPENWRT").as_deref() == Ok("1")
}

fn listener_port(list: &str) -> Option<u16> {
    list.split_whitespace().find_map(|listener| {
        let (address, port) = listener.rsplit_once(':').unwrap_or(("0.0.0.0", listener));
        let ip: IpAddr = address
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .ok()?;
        if ip.is_loopback() {
            return None;
        }
        port.parse::<u16>().ok().filter(|port| *port != 0)
    })
}

fn destination(host: &str, http: &str, https: &str, redirect_https: bool) -> Option<String> {
    let authority: Authority = host.parse().ok()?;
    if authority.as_str().contains('@') {
        return None;
    }
    let http = listener_port(http);
    let https = listener_port(https);
    let (scheme, port) = match (http, https, redirect_https) {
        (_, Some(port), true) | (None, Some(port), _) => ("https", port),
        (Some(port), _, _) => ("http", port),
        _ => return None,
    };
    Some(format!(
        "{scheme}://{}:{port}/cgi-bin/luci/",
        authority.host()
    ))
}

async fn setting(name: &str) -> Result<String, ()> {
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        Command::new("/sbin/uci")
            .args(["-q", "get", name])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    // Missing listen_http/listen_https is valid when that protocol is disabled.
    if !output.status.success() {
        return Ok(String::new());
    }
    String::from_utf8(output.stdout).map_err(|_| ())
}

pub async fn redirect_handler(headers: HeaderMap) -> Response {
    if !enabled() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return (StatusCode::BAD_REQUEST, "Missing or invalid host").into_response();
    };
    let Ok((http, https, redirect)) = tokio::try_join!(
        setting("uhttpd.main.listen_http"),
        setting("uhttpd.main.listen_https"),
        setting("uhttpd.main.redirect_https")
    ) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "OpenWrt web configuration is unavailable",
        )
            .into_response();
    };
    match destination(
        host,
        &http,
        &https,
        matches!(redirect.trim(), "1" | "on" | "true" | "yes"),
    ) {
        Some(url) => (
            [(header::CACHE_CONTROL, "no-store")],
            Redirect::temporary(&url),
        )
            .into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "No public OpenWrt web listener is configured",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_http_port_uses_current_hostname() {
        assert_eq!(
            destination(
                "device.test:8080",
                "0.0.0.0:8081 [::]:8081",
                "0.0.0.0:8444",
                false
            )
            .as_deref(),
            Some("http://device.test:8081/cgi-bin/luci/")
        );
        assert_eq!(
            destination("192.168.1.11:9080", "0.0.0.0:9091", "", false).as_deref(),
            Some("http://192.168.1.11:9091/cgi-bin/luci/")
        );
    }
    #[test]
    fn https_redirect_and_https_only_follow_openwrt() {
        for http in ["0.0.0.0:8081", ""] {
            assert_eq!(
                destination("n1.test:8443", http, "0.0.0.0:9444", true).as_deref(),
                Some("https://n1.test:9444/cgi-bin/luci/")
            );
        }
        assert!(destination("n1.test", "", "[::]:8444", false)
            .unwrap()
            .starts_with("https://"));
        assert!(destination("n1.test", "0.0.0.0:8081", "", true)
            .unwrap()
            .starts_with("http://"));
    }
    #[test]
    fn ipv6_and_default_browser_ports() {
        assert_eq!(
            destination("[2001:db8::1]:8080", "[::]:8081", "", false).as_deref(),
            Some("http://[2001:db8::1]:8081/cgi-bin/luci/")
        );
        assert!(destination("n1.test", "0.0.0.0:8081", "", false)
            .unwrap()
            .contains("n1.test:8081"));
    }
    #[test]
    fn invalid_or_loopback_only_listeners_do_not_invent_a_port() {
        for list in [
            "",
            "127.0.0.1:8081",
            "[::1]:8081",
            "0.0.0.0:0",
            "0.0.0.0:99999",
            "bad",
        ] {
            assert_eq!(destination("n1.test:8080", list, "", false), None);
        }
        assert_eq!(
            destination("user@n1.test:8080", "0.0.0.0:8081", "", false),
            None
        );
        assert_eq!(
            destination("https://n1.test/path", "0.0.0.0:8081", "", false),
            None
        );
        assert_eq!(listener_port("127.0.0.1:8081 0.0.0.0:9091"), Some(9091));
    }
}
