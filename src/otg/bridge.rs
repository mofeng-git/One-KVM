use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use uuid::Uuid;

use crate::error::{AppError, Result};

const BRIDGE_IF: &str = "okvm-br0";
const PROFILE_PREFIX: &str = "one-kvm-otg";
const JOURNAL_PATH: &str = "/run/one-kvm/otg-network-bridge.json";
const JOURNAL_VERSION: u8 = 2;
const NETWORK_MANAGER_DEVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DHCP_IDENTITY_PROPERTIES: &[&str] = &[
    "ipv4.dhcp-client-id",
    "ipv4.dhcp-iaid",
    "ipv4.dhcp-hostname",
    "ipv4.dhcp-fqdn",
    "ipv4.dhcp-send-hostname",
    "ipv4.dhcp-hostname-flags",
];
const STATIC_IPV4_PROPERTIES: &[&str] = &[
    "ipv4.dns",
    "ipv4.dns-search",
    "ipv4.dns-options",
    "ipv4.dns-priority",
    "ipv4.routes",
    "ipv4.route-table",
    "ipv4.routing-rules",
    "ipv4.never-default",
    "ipv4.may-fail",
    "ipv4.ignore-auto-routes",
    "ipv4.ignore-auto-dns",
];

#[typeshare]
#[derive(Debug, Clone, Serialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub interface_type: String,
    pub state: String,
    pub connection: String,
    pub addresses: Vec<String>,
    pub has_default_route: bool,
    pub bridge_supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkManagerDevice {
    name: String,
    interface_type: String,
    state: String,
    connection: String,
    addresses: Vec<String>,
    has_default_route: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BridgeJournal {
    version: u8,
    uplink: String,
    existing_bridge: bool,
    original_connection_uuid: Option<String>,
    bridge_profile_uuid: Option<String>,
    uplink_profile_uuid: Option<String>,
    usb_profile_uuid: String,
}

#[derive(Debug)]
struct TransactionProfiles {
    bridge_name: String,
    bridge_uuid: String,
    uplink_name: String,
    uplink_uuid: String,
    usb_name: String,
    usb_uuid: String,
}

impl TransactionProfiles {
    fn new() -> Self {
        let transaction = Uuid::new_v4().simple().to_string();
        let suffix = &transaction[..12];
        Self {
            bridge_name: format!("{PROFILE_PREFIX}-bridge-{suffix}"),
            bridge_uuid: Uuid::new_v4().to_string(),
            uplink_name: format!("{PROFILE_PREFIX}-uplink-{suffix}"),
            uplink_uuid: Uuid::new_v4().to_string(),
            usb_name: format!("{PROFILE_PREFIX}-usb-{suffix}"),
            usb_uuid: Uuid::new_v4().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkBridgeRuntime {
    journal: BridgeJournal,
}

impl NetworkBridgeRuntime {
    pub fn activate(requested: &str, usb_interface: &str) -> Result<Self> {
        ensure_command("nmcli")?;
        ensure_command("ip")?;

        let interfaces = list_network_interfaces()?;
        let selected = select_bridge_candidate(&interfaces, requested)?;

        prepare_device_for_network_manager(usb_interface, "ethernet")?;

        Self::activate_physical_uplink(&selected.name, &selected.connection, usb_interface)
    }

    fn activate_physical_uplink(
        uplink: &str,
        original_connection: &str,
        usb_interface: &str,
    ) -> Result<Self> {
        if original_connection.is_empty() || original_connection == "--" {
            return Err(AppError::BadRequest(format!(
                "Ethernet interface {uplink} has no active NetworkManager connection"
            )));
        }

        reset_bridge_interface()?;
        let original_connection_uuid = active_connection_uuid(uplink)?;
        let ipv4_method = connection_value(&original_connection_uuid, "ipv4.method")?;
        if !matches!(ipv4_method.as_str(), "auto" | "manual") {
            return Err(AppError::BadRequest(format!(
                "Connection {original_connection} uses unsupported ipv4.method={ipv4_method}"
            )));
        }
        let ipv6_method = connection_value(&original_connection_uuid, "ipv6.method")?;
        if !matches!(ipv6_method.as_str(), "auto" | "disabled" | "ignore") {
            return Err(AppError::BadRequest(format!(
                "Connection {original_connection} uses unsupported ipv6.method={ipv6_method}"
            )));
        }
        let ipv4_metric = connection_value(&original_connection_uuid, "ipv4.route-metric")?;
        let ipv6_metric = connection_value(&original_connection_uuid, "ipv6.route-metric")?;
        let original_had_default_route = default_route(uplink).is_some();

        let mac_path = Path::new("/sys/class/net").join(uplink).join("address");
        let uplink_mac = fs::read_to_string(&mac_path)
            .map_err(|e| {
                AppError::Internal(format!("Failed to read {}: {}", mac_path.display(), e))
            })?
            .trim()
            .to_string();

        let profiles = TransactionProfiles::new();
        let journal = BridgeJournal {
            version: JOURNAL_VERSION,
            uplink: uplink.to_string(),
            existing_bridge: false,
            original_connection_uuid: Some(original_connection_uuid.clone()),
            bridge_profile_uuid: Some(profiles.bridge_uuid.clone()),
            uplink_profile_uuid: Some(profiles.uplink_uuid.clone()),
            usb_profile_uuid: profiles.usb_uuid.clone(),
        };
        write_journal(&journal)?;

        let prepare_result: Result<()> = (|| {
            create_bridge_interface(&uplink_mac)?;
            run_nmcli(&[
                "connection",
                "add",
                "type",
                "bridge",
                "ifname",
                BRIDGE_IF,
                "con-name",
                &profiles.bridge_name,
                "connection.uuid",
                &profiles.bridge_uuid,
            ])?;
            run_nmcli(&[
                "connection",
                "modify",
                &profiles.bridge_uuid,
                "connection.interface-name",
                BRIDGE_IF,
                "bridge.mac-address",
                &uplink_mac,
                "bridge.stp",
                "no",
                "ipv6.method",
                &ipv6_method,
                "connection.autoconnect",
                "no",
            ])?;
            configure_ipv4_profile(
                &original_connection_uuid,
                &profiles.bridge_uuid,
                &ipv4_method,
            )?;
            for (property, value) in [
                ("ipv4.route-metric", ipv4_metric.as_str()),
                ("ipv6.route-metric", ipv6_metric.as_str()),
            ] {
                if !value.is_empty() && value != "-1" {
                    run_nmcli(&[
                        "connection",
                        "modify",
                        &profiles.bridge_uuid,
                        property,
                        value,
                    ])?;
                }
            }
            run_nmcli(&[
                "connection",
                "add",
                "type",
                "ethernet",
                "ifname",
                uplink,
                "con-name",
                &profiles.uplink_name,
                "connection.uuid",
                &profiles.uplink_uuid,
                "master",
                BRIDGE_IF,
                "slave-type",
                "bridge",
                "connection.autoconnect",
                "no",
            ])?;
            run_nmcli(&[
                "connection",
                "add",
                "type",
                "ethernet",
                "ifname",
                usb_interface,
                "con-name",
                &profiles.usb_name,
                "connection.uuid",
                &profiles.usb_uuid,
                "master",
                BRIDGE_IF,
                "slave-type",
                "bridge",
                "connection.autoconnect",
                "no",
            ])?;
            Ok(())
        })();
        if let Err(error) = prepare_result {
            return Err(restore_or_combine(&journal, error));
        }

        let result = (|| {
            run_nmcli(&["connection", "down", "uuid", &original_connection_uuid])?;
            activate_connection(
                "bridge",
                &profiles.bridge_name,
                &profiles.bridge_uuid,
                Some(BRIDGE_IF),
            )?;
            activate_connection(
                "uplink",
                &profiles.uplink_name,
                &profiles.uplink_uuid,
                Some(uplink),
            )?;
            activate_connection(
                "USB",
                &profiles.usb_name,
                &profiles.usb_uuid,
                Some(usb_interface),
            )?;

            let deadline = Instant::now() + Duration::from_secs(35);
            while Instant::now() < deadline {
                if first_ipv4_address(BRIDGE_IF).is_some()
                    && (!original_had_default_route || default_route(BRIDGE_IF).is_some())
                {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }
            let address = first_ipv4_address(BRIDGE_IF).ok_or_else(|| {
                AppError::Internal(
                    "OTG bridge did not obtain an IPv4 address from upstream DHCP".to_string(),
                )
            })?;
            let route = default_route(BRIDGE_IF);
            if original_had_default_route && route.is_none() {
                return Err(AppError::Internal(
                    "OTG bridge did not obtain the original default route".to_string(),
                ));
            }
            if let Some(route) = route.as_deref() {
                if let Some(gateway) = gateway_from_route(route) {
                    if let Err(error) = run_command("ping", &["-c", "1", "-W", "2", gateway]) {
                        tracing::warn!(
                            "OTG bridge gateway ICMP diagnostic failed for {}: {}",
                            gateway,
                            error
                        );
                    }
                }
            }
            Ok(address)
        })();

        match result {
            Ok(_address) => Ok(Self { journal }),
            Err(error) => Err(restore_or_combine(&journal, error)),
        }
    }

    pub fn deactivate(&self) -> Result<()> {
        restore_from_journal(&self.journal)
    }

    pub fn recover_stale_transaction() -> Result<()> {
        let value = match fs::read_to_string(JOURNAL_PATH) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(AppError::Internal(format!(
                    "Failed to read OTG network recovery journal: {error}"
                )))
            }
        };
        match serde_json::from_str::<BridgeJournal>(&value) {
            Ok(journal) if journal.version == JOURNAL_VERSION => restore_from_journal(&journal),
            Ok(journal) => Err(AppError::Config(format!(
                "Unsupported OTG network recovery journal version {}",
                journal.version
            ))),
            Err(error) => Err(AppError::Config(format!(
                "Invalid OTG network recovery journal: {error}"
            ))),
        }
    }
}

pub fn list_network_interfaces() -> Result<Vec<NetworkInterfaceInfo>> {
    let devices = enumerate_network_manager_devices()?;
    Ok(bridge_candidates(devices, is_physical_network_interface))
}

fn enumerate_network_manager_devices() -> Result<Vec<NetworkManagerDevice>> {
    ensure_command("nmcli")?;
    let output = run_command(
        "nmcli",
        &[
            "-t",
            "--escape",
            "no",
            "-f",
            "DEVICE,TYPE,STATE,CONNECTION",
            "device",
            "status",
        ],
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut devices = parse_network_manager_devices(&text);
    for device in &mut devices {
        device.addresses = ipv4_addresses(&device.name);
        device.has_default_route = default_route(&device.name).is_some();
    }
    Ok(devices)
}

fn parse_network_manager_devices(text: &str) -> Vec<NetworkManagerDevice> {
    let mut devices = Vec::new();
    for line in text.lines() {
        let fields = line.splitn(4, ':').collect::<Vec<_>>();
        if fields.len() != 4 || fields[0].is_empty() {
            continue;
        }
        devices.push(NetworkManagerDevice {
            name: fields[0].to_string(),
            interface_type: fields[1].to_string(),
            state: fields[2].to_string(),
            connection: fields[3].to_string(),
            addresses: Vec::new(),
            has_default_route: false,
        });
    }
    devices
}

fn bridge_candidates(
    devices: Vec<NetworkManagerDevice>,
    is_physical: impl Fn(&str) -> bool,
) -> Vec<NetworkInterfaceInfo> {
    devices
        .into_iter()
        .filter(|device| {
            device.interface_type == "ethernet"
                && device.state == "connected"
                && !device.connection.is_empty()
                && device.connection != "--"
                && is_physical(&device.name)
        })
        .map(|device| NetworkInterfaceInfo {
            name: device.name,
            interface_type: device.interface_type,
            state: device.state,
            connection: device.connection,
            addresses: device.addresses,
            has_default_route: device.has_default_route,
            bridge_supported: true,
            reason: None,
        })
        .collect()
}

fn is_physical_network_interface(name: &str) -> bool {
    Path::new("/sys/class/net")
        .join(name)
        .join("device")
        .exists()
}

fn select_bridge_candidate<'a>(
    interfaces: &'a [NetworkInterfaceInfo],
    requested: &str,
) -> Result<&'a NetworkInterfaceInfo> {
    if requested.trim().is_empty() {
        return interfaces
            .iter()
            .max_by_key(|item| item.has_default_route)
            .ok_or_else(|| {
                AppError::Config(
                    "No connected physical NetworkManager Ethernet interface is available for OTG bridging"
                        .to_string(),
                )
            });
    }

    interfaces
        .iter()
        .find(|item| item.name == requested)
        .ok_or_else(|| {
            AppError::Config(format!(
                "Network interface {requested} is not a connected physical NetworkManager Ethernet interface"
            ))
        })
}

fn restore_from_journal(journal: &BridgeJournal) -> Result<()> {
    let mut errors = Vec::new();
    for (kind, profile_uuid) in [
        ("USB", Some(journal.usb_profile_uuid.as_str())),
        ("uplink", journal.uplink_profile_uuid.as_deref()),
        ("bridge", journal.bridge_profile_uuid.as_deref()),
    ] {
        let Some(profile_uuid) = profile_uuid else {
            continue;
        };
        if let Err(error) = delete_connection(profile_uuid) {
            errors.push(format!(
                "failed to remove owned {kind} profile {profile_uuid}: {error}"
            ));
        }
    }

    if !journal.existing_bridge {
        if let Err(error) = delete_bridge_interface() {
            errors.push(format!(
                "failed to remove owned bridge interface {BRIDGE_IF}: {error}"
            ));
        }
        if let Some(ref original_uuid) = journal.original_connection_uuid {
            if let Err(error) = run_nmcli(&[
                "connection",
                "up",
                "uuid",
                original_uuid,
                "ifname",
                &journal.uplink,
            ]) {
                errors.push(format!(
                    "failed to restore original profile {original_uuid}: {error}"
                ));
            }
        }
    }

    if !errors.is_empty() {
        return Err(AppError::Config(errors.join("; ")));
    }

    match fs::remove_file(JOURNAL_PATH) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Internal(format!(
            "Failed to remove OTG network recovery journal: {error}"
        ))),
    }
}

fn restore_or_combine(journal: &BridgeJournal, primary: AppError) -> AppError {
    match restore_from_journal(journal) {
        Ok(()) => primary,
        Err(rollback) => AppError::Config(format!("{primary}; bridge rollback failed: {rollback}")),
    }
}

fn reset_bridge_interface() -> Result<()> {
    for profile_uuid in connection_uuids()? {
        if connection_value(&profile_uuid, "connection.interface-name")? == BRIDGE_IF {
            tracing::warn!(
                "Removing NetworkManager profile {} bound to reserved interface {}",
                profile_uuid,
                BRIDGE_IF
            );
            run_nmcli(&["connection", "delete", "uuid", &profile_uuid])?;
        }
    }
    delete_bridge_interface()
}

fn create_bridge_interface(mac_address: &str) -> Result<()> {
    run_command("ip", &["link", "add", "name", BRIDGE_IF, "type", "bridge"])?;
    run_command(
        "ip",
        &["link", "set", "dev", BRIDGE_IF, "address", mac_address],
    )?;
    prepare_device_for_network_manager(BRIDGE_IF, "bridge")
}

fn delete_bridge_interface() -> Result<()> {
    if !Path::new("/sys/class/net").join(BRIDGE_IF).exists() {
        return Ok(());
    }
    run_command("ip", &["link", "delete", BRIDGE_IF, "type", "bridge"])?;
    Ok(())
}

fn prepare_device_for_network_manager(interface: &str, expected_type: &str) -> Result<()> {
    run_command("ip", &["link", "set", interface, "up"])?;

    let deadline = Instant::now() + NETWORK_MANAGER_DEVICE_WAIT_TIMEOUT;
    let mut requested_managed = false;
    while Instant::now() < deadline {
        match enumerate_network_manager_devices() {
            Ok(devices) => {
                if let Some(device) = devices.iter().find(|device| device.name == interface) {
                    if device.interface_type != expected_type {
                        return Err(AppError::BadRequest(format!(
                            "One-KVM interface {interface} has NetworkManager type {}, expected {expected_type}",
                            device.interface_type,
                        )));
                    }
                    if device.state != "unmanaged" {
                        return Ok(());
                    }
                    if !requested_managed {
                        tracing::info!(
                            "Marking One-KVM interface {} as managed by NetworkManager",
                            interface
                        );
                        run_nmcli(&["device", "set", interface, "managed", "yes"])?;
                        requested_managed = true;
                    }
                }
            }
            Err(error) => {
                tracing::debug!(
                    "Waiting for NetworkManager to discover One-KVM interface {}: {}",
                    interface,
                    error
                );
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(AppError::Internal(format!(
        "NetworkManager did not discover One-KVM {expected_type} interface {interface} within {} seconds",
        NETWORK_MANAGER_DEVICE_WAIT_TIMEOUT.as_secs()
    )))
}

fn activate_connection(kind: &str, name: &str, uuid: &str, interface: Option<&str>) -> Result<()> {
    let result = match interface {
        Some(interface) => run_nmcli(&["connection", "up", name, "ifname", interface]),
        None => run_nmcli(&["connection", "up", name]),
    };
    result.map_err(|error| {
        let target = interface
            .map(|value| format!(" on {value}"))
            .unwrap_or_default();
        AppError::Internal(format!(
            "Failed to activate One-KVM {kind} profile {name} ({uuid}){target}: {error}"
        ))
    })?;
    Ok(())
}

fn active_connection_uuid(interface: &str) -> Result<String> {
    let output = run_nmcli(&[
        "--escape",
        "no",
        "-g",
        "GENERAL.CON-UUID",
        "device",
        "show",
        interface,
    ])?;
    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() || uuid == "--" {
        return Err(AppError::BadRequest(format!(
            "Ethernet interface {interface} has no active NetworkManager profile UUID"
        )));
    }
    Ok(uuid)
}

fn connection_uuids() -> Result<Vec<String>> {
    let output = run_nmcli(&["-t", "--escape", "no", "-f", "UUID", "connection", "show"])?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect())
}

fn delete_connection(profile_uuid: &str) -> Result<()> {
    if !connection_uuids()?.iter().any(|uuid| uuid == profile_uuid) {
        return Ok(());
    }
    run_nmcli(&["connection", "delete", "uuid", profile_uuid])?;
    Ok(())
}

fn copy_connection_properties(source: &str, target: &str, properties: &[&str]) -> Result<()> {
    for property in properties {
        let Ok(value) = connection_value(source, property) else {
            tracing::debug!(
                "Skipping unsupported NetworkManager property {} while configuring OTG bridge",
                property
            );
            continue;
        };
        if value.is_empty() || value == "--" {
            continue;
        }
        run_nmcli(&["connection", "modify", target, property, &value])?;
    }
    Ok(())
}

fn configure_ipv4_profile(source: &str, target: &str, method: &str) -> Result<()> {
    match method {
        "auto" => {
            run_nmcli(&["connection", "modify", target, "ipv4.method", "auto"])?;
            copy_connection_properties(source, target, DHCP_IDENTITY_PROPERTIES)
        }
        "manual" => {
            let addresses = connection_value(source, "ipv4.addresses")?;
            if addresses.is_empty() || addresses == "--" {
                return Err(AppError::BadRequest(
                    "Static IPv4 profile has no ipv4.addresses value".to_string(),
                ));
            }
            let gateway = connection_value(source, "ipv4.gateway")?;
            if gateway.is_empty() || gateway == "--" {
                run_nmcli(&[
                    "connection",
                    "modify",
                    target,
                    "ipv4.method",
                    "manual",
                    "ipv4.addresses",
                    &addresses,
                ])?;
            } else {
                run_nmcli(&[
                    "connection",
                    "modify",
                    target,
                    "ipv4.method",
                    "manual",
                    "ipv4.addresses",
                    &addresses,
                    "ipv4.gateway",
                    &gateway,
                ])?;
            }
            copy_connection_properties(source, target, STATIC_IPV4_PROPERTIES)
        }
        _ => Err(AppError::BadRequest(format!(
            "Unsupported IPv4 method {method}"
        ))),
    }
}

fn write_journal(journal: &BridgeJournal) -> Result<()> {
    let path = Path::new(JOURNAL_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::Internal(format!("Failed to create {}: {}", parent.display(), e))
        })?;
    }
    let value = serde_json::to_vec(journal)
        .map_err(|e| AppError::Internal(format!("Failed to serialize bridge journal: {e}")))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, value)
        .map_err(|e| AppError::Internal(format!("Failed to write bridge recovery journal: {e}")))?;
    fs::rename(&temporary, path)
        .map_err(|e| AppError::Internal(format!("Failed to commit bridge recovery journal: {e}")))
}

fn connection_value(connection: &str, property: &str) -> Result<String> {
    let output = run_nmcli(&[
        "--escape",
        "no",
        "-g",
        property,
        "connection",
        "show",
        connection,
    ])?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn first_ipv4_address(interface: &str) -> Option<String> {
    ipv4_addresses(interface).into_iter().next()
}

fn ipv4_addresses(interface: &str) -> Vec<String> {
    let Ok(output) = Command::new("ip")
        .args(["-4", "-o", "address", "show", "dev", interface])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            fields
                .iter()
                .position(|field| *field == "inet")
                .and_then(|index| fields.get(index + 1))
                .map(|value| (*value).to_string())
        })
        .collect()
}

fn default_route(interface: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["-4", "route", "show", "default", "dev", interface])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::to_string)
}

fn gateway_from_route(route: &str) -> Option<&str> {
    let fields = route.split_whitespace().collect::<Vec<_>>();
    fields
        .windows(2)
        .find_map(|part| (part[0] == "via").then_some(part[1]))
}

fn ensure_command(name: &str) -> Result<()> {
    let status = Command::new(name).arg("--version").output();
    if status.is_err() {
        return Err(AppError::BadRequest(format!(
            "OTG bridge requires the {name} command"
        )));
    }
    Ok(())
}

fn run_nmcli(args: &[&str]) -> Result<Output> {
    run_command("nmcli", args)
}

fn run_command(command: &str, args: &[&str]) -> Result<Output> {
    let output = Command::new(command)
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .map_err(|e| {
            AppError::Internal(format!(
                "Failed to execute {command} {}: {e}",
                args.join(" ")
            ))
        })?;
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(AppError::Internal(format!(
        "{command} {} failed: {}",
        args.join(" "),
        if stderr.is_empty() { stdout } else { stderr }
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(
        name: &str,
        interface_type: &str,
        state: &str,
        connection: &str,
        has_default_route: bool,
    ) -> NetworkManagerDevice {
        NetworkManagerDevice {
            name: name.to_string(),
            interface_type: interface_type.to_string(),
            state: state.to_string(),
            connection: connection.to_string(),
            addresses: Vec::new(),
            has_default_route,
        }
    }

    #[test]
    fn bridge_journal_round_trip() {
        let journal = BridgeJournal {
            version: JOURNAL_VERSION,
            uplink: "eth0".to_string(),
            existing_bridge: false,
            original_connection_uuid: Some("original-uuid".to_string()),
            bridge_profile_uuid: Some("bridge-uuid".to_string()),
            uplink_profile_uuid: Some("uplink-uuid".to_string()),
            usb_profile_uuid: "usb-uuid".to_string(),
        };
        let value = serde_json::to_string(&journal).unwrap();
        let decoded: BridgeJournal = serde_json::from_str(&value).unwrap();
        assert_eq!(decoded.uplink, "eth0");
        assert_eq!(decoded.bridge_profile_uuid.as_deref(), Some("bridge-uuid"));
    }

    #[test]
    fn transaction_profiles_use_unique_names_and_uuids() {
        let first = TransactionProfiles::new();
        let second = TransactionProfiles::new();
        assert_ne!(first.bridge_name, second.bridge_name);
        assert_ne!(first.bridge_uuid, second.bridge_uuid);
        assert!(first.usb_name.starts_with(PROFILE_PREFIX));
        assert!(Uuid::parse_str(&first.usb_uuid).is_ok());
    }

    #[test]
    fn gateway_is_optional_diagnostic_data() {
        assert_eq!(
            gateway_from_route("default via 192.0.2.1 dev okvm-br0"),
            Some("192.0.2.1")
        );
        assert_eq!(gateway_from_route("default dev okvm-br0"), None);
    }

    #[test]
    fn dhcp_identity_properties_include_client_id_and_hostname() {
        assert!(DHCP_IDENTITY_PROPERTIES.contains(&"ipv4.dhcp-client-id"));
        assert!(DHCP_IDENTITY_PROPERTIES.contains(&"ipv4.dhcp-iaid"));
        assert!(DHCP_IDENTITY_PROPERTIES.contains(&"ipv4.dhcp-hostname"));
    }

    #[test]
    fn static_ipv4_properties_cover_dns_routes_and_policy() {
        assert!(STATIC_IPV4_PROPERTIES.contains(&"ipv4.dns"));
        assert!(STATIC_IPV4_PROPERTIES.contains(&"ipv4.routes"));
        assert!(STATIC_IPV4_PROPERTIES.contains(&"ipv4.route-table"));
        assert!(STATIC_IPV4_PROPERTIES.contains(&"ipv4.never-default"));
    }

    #[test]
    fn full_network_manager_enumeration_keeps_runtime_devices() {
        let devices = parse_network_manager_devices(
            "eth0:ethernet:connected:Wired connection 1\n\
             usb0:ethernet:disconnected:--\n\
             okvm-br0:bridge:unmanaged:--\n",
        );

        assert_eq!(
            devices
                .iter()
                .map(|device| device.name.as_str())
                .collect::<Vec<_>>(),
            ["eth0", "usb0", "okvm-br0"]
        );
    }

    #[test]
    fn bridge_candidates_only_keep_connected_physical_ethernet() {
        let devices = vec![
            device("eth0", "ethernet", "connected", "one-kvm-otg-uplink", false),
            device("wlx76012dc07213", "wifi", "connected", "Wi-Fi", true),
            device("okvm-br0", "bridge", "connected", "Bridge", true),
            device("usb0", "ethernet", "connected", "USB", false),
            device("lo", "loopback", "connected", "lo", false),
            device("bond0", "bond", "connected", "Bond", false),
            device("tun0", "tun", "connected", "Tunnel", false),
            device("veth0", "ethernet", "connected", "Virtual", false),
            device("eth1", "ethernet", "disconnected", "--", false),
            device("eth2", "ethernet", "connected", "--", false),
            device("eth3", "ethernet", "connected", "", false),
        ];

        let candidates = bridge_candidates(devices, |name| {
            matches!(name, "eth0" | "eth1" | "eth2" | "eth3")
        });

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "eth0");
        assert!(candidates[0].bridge_supported);
        assert_eq!(candidates[0].reason, None);
    }

    #[test]
    fn automatic_bridge_selection_prefers_default_route() {
        let candidates = bridge_candidates(
            vec![
                device("eth0", "ethernet", "connected", "Wired 1", false),
                device("eth1", "ethernet", "connected", "Wired 2", true),
            ],
            |_| true,
        );

        let selected = select_bridge_candidate(&candidates, "").unwrap();

        assert_eq!(selected.name, "eth1");
    }

    #[test]
    fn bridge_selection_reports_when_no_candidate_exists() {
        let error = select_bridge_candidate(&[], "").unwrap_err();

        assert!(matches!(error, AppError::Config(_)));
        assert!(error.to_string().contains("connected physical"));
    }
}
