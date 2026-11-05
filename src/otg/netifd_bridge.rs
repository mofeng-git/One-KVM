//! OpenWrt bridge transactions. Preserve logical interfaces and their L3 settings.
//! The recovery journal lives in /etc so committed UCI changes survive a reboot safely.
use super::bridge::{
    default_route, ensure_command, first_ipv4_address, run_command, NetworkInterfaceInfo,
};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const JOURNAL_PATH: &str = "/etc/one-kvm/otg-netifd-bridge.json";
const BRIDGE: &str = "okvm-br0";
const VERSION: u8 = 1;
type Config = BTreeMap<String, Map<String, Value>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Journal {
    version: u8,
    uplink: String,
    usb: String,
    bridge: String,
    had_default_route: bool,
    interface_devices: BTreeMap<String, String>,
    created_section: Option<String>,
    bridge_section: String,
    // Exact owned section options; unrelated user edits are preserved on recovery.
    created_options: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
enum Edit {
    Set(String, String),
    Delete(String),
    AddList(String, String),
    DelList(String, String),
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Config(message.into())
}
fn io_error(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("OpenWrt bridge: {error}"))
}
fn identifier(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn device_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 16
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}
fn option<'a>(section: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    section.get(key)?.as_str()
}
fn ports(section: &Map<String, Value>) -> Result<Vec<String>> {
    match section.get("ports") {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("Invalid UCI bridge port list"))
            })
            .collect(),
        Some(Value::String(value)) => Ok(value.split_whitespace().map(str::to_owned).collect()),
        _ => Err(invalid("Invalid UCI bridge ports")),
    }
}

pub(super) fn bridgeable_device(name: &str) -> bool {
    if !device_name(name) {
        return false;
    }
    let path = Path::new("/sys/class/net").join(name);
    path.join("bridge").is_dir()
        || (path.join("device").exists()
            && fs::read_to_string(path.join("type")).is_ok_and(|value| value.trim() == "1")
            && !path.join("wireless").exists()
            && !path.join("phy80211").exists())
}

fn plan(
    config: &Config,
    selected: &NetworkInterfaceInfo,
    usb: &str,
    mac: &str,
    existing_bridge: bool,
) -> Result<(Journal, Vec<Edit>)> {
    if !device_name(&selected.name)
        || !device_name(usb)
        || selected.name == usb
        || !identifier(&selected.connection)
    {
        return Err(invalid(
            "Invalid or identical OpenWrt uplink and USB interfaces",
        ));
    }
    let interface = config
        .get(&selected.connection)
        .ok_or_else(|| invalid("Selected UCI interface no longer exists"))?;
    if option(interface, ".type") != Some("interface")
        || option(interface, "device") != Some(selected.name.as_str())
        || interface.contains_key("ifname")
        || interface.contains_key("type")
    {
        return Err(invalid("OTG bridging requires a current UCI interface with a device option; legacy ifname/type configurations are unsupported"));
    }
    if !matches!(option(interface, "proto"), Some("dhcp" | "static")) {
        return Err(invalid(
            "OTG bridging supports DHCP and static Ethernet uplinks",
        ));
    }
    for section in config.values() {
        if option(section, ".type") == Some("interface")
            && option(section, "ifname")
                .is_some_and(|value| value.split_whitespace().any(|name| name == selected.name))
        {
            return Err(invalid("Another UCI interface uses the uplink through legacy ifname; migrate it to a device option first"));
        }
        if option(section, "device") == Some(usb)
            || option(section, "ifname").is_some_and(|v| v.split_whitespace().any(|p| p == usb))
            || (option(section, ".type") == Some("device")
                && ports(section)?.iter().any(|p| p == usb))
        {
            return Err(invalid(
                "USB interface already belongs to an OpenWrt network configuration",
            ));
        }
    }
    let mut journal = Journal {
        version: VERSION,
        uplink: selected.name.clone(),
        usb: usb.to_owned(),
        bridge: selected.name.clone(),
        had_default_route: selected.has_default_route,
        interface_devices: BTreeMap::new(),
        created_section: None,
        bridge_section: String::new(),
        created_options: Map::new(),
    };
    let mut edits = Vec::new();
    if existing_bridge {
        let matches: Vec<_> = config
            .iter()
            .filter(|(_, s)| {
                option(s, ".type") == Some("device")
                    && option(s, "name") == Some(selected.name.as_str())
            })
            .collect();
        if matches.len() != 1 || option(matches[0].1, "type") != Some("bridge") {
            return Err(invalid("Existing bridge must have one UCI device section"));
        }
        let (name, section) = matches[0];
        if !identifier(name)
            || matches!(
                option(section, "vlan_filtering"),
                Some("1" | "true" | "yes" | "on")
            )
            || config.values().any(|s| {
                option(s, ".type") == Some("bridge-vlan")
                    && option(s, "device") == Some(selected.name.as_str())
            })
        {
            return Err(invalid("VLAN-filtered bridges require an explicit USB VLAN configuration and are unsupported"));
        }
        // Preserve list semantics; a scalar ports option is not safe to edit with add_list.
        if section.get("ports").is_some_and(|v| !v.is_array()) {
            return Err(invalid("Bridge ports must use UCI list entries"));
        }
        journal.bridge_section = name.clone();
        edits.push(Edit::AddList(format!("{name}.ports"), usb.to_owned()));
    } else {
        if config.values().any(|s| option(s, "name") == Some(BRIDGE)) {
            return Err(invalid(
                "okvm-br0 already exists; refusing to replace another bridge",
            ));
        }
        if config.values().any(|s| {
            option(s, ".type") == Some("device")
                && ports(s).is_ok_and(|p| p.iter().any(|p| p == &selected.name))
        }) {
            return Err(invalid(
                "Selected uplink already belongs to a configured bridge",
            ));
        }
        let section = format!("one_kvm_otg_{}", Uuid::new_v4().simple());
        journal.bridge = BRIDGE.to_owned();
        journal.bridge_section = section.clone();
        journal.created_section = Some(section.clone());
        journal.created_options = json!({".type":"device", "name":BRIDGE, "type":"bridge", "stp":"0", "macaddr":mac, "ports":[selected.name, usb]}).as_object().unwrap().clone();
        edits.push(Edit::Set(section.clone(), "device".to_owned()));
        for key in ["name", "type", "stp", "macaddr"] {
            edits.push(Edit::Set(
                format!("{section}.{key}"),
                option(&journal.created_options, key).unwrap().to_owned(),
            ));
        }
        for port in [selected.name.as_str(), usb] {
            edits.push(Edit::AddList(format!("{section}.ports"), port.to_owned()));
        }
        for (name, section) in config {
            if option(section, ".type") != Some("interface")
                || option(section, "device") != Some(selected.name.as_str())
            {
                continue;
            }
            if !identifier(name)
                || section.contains_key("ifname")
                || section.contains_key("type")
                || !matches!(
                    option(section, "proto"),
                    Some("dhcp" | "static" | "dhcpv6" | "none")
                )
            {
                return Err(invalid("An additional uplink interface has an unsupported protocol or legacy configuration"));
            }
            journal
                .interface_devices
                .insert(name.clone(), selected.name.clone());
            edits.push(Edit::Set(format!("{name}.device"), BRIDGE.to_owned()));
        }
    }
    Ok((journal, edits))
}

fn restore_plan(config: &Config, journal: &Journal) -> Result<Vec<Edit>> {
    if journal.version != VERSION
        || !identifier(&journal.bridge_section)
        || !device_name(&journal.uplink)
        || !device_name(&journal.usb)
        || !device_name(&journal.bridge)
    {
        return Err(invalid("Invalid OpenWrt bridge recovery journal"));
    }
    let mut edits = Vec::new();
    for (name, original) in &journal.interface_devices {
        if !identifier(name) {
            return Err(invalid("Invalid recovery interface name"));
        }
        let section = config.get(name).ok_or_else(|| {
            invalid(format!(
                "Recovery interface {name} was removed; journal retained"
            ))
        })?;
        match option(section, "device") {
            Some(value) if value == original => {},
            Some(value) if value == journal.bridge => edits.push(Edit::Set(format!("{name}.device"), original.clone())),
            _ => return Err(invalid(format!("Interface {name} was changed externally; refusing to overwrite it; journal retained"))),
        }
    }
    if let Some(name) = &journal.created_section {
        if name != &journal.bridge_section || !name.starts_with("one_kvm_otg_") {
            return Err(invalid("Invalid owned bridge section"));
        }
        if let Some(section) = config.get(name) {
            let actual: Map<_, _> = section
                .iter()
                .filter(|(key, _)| !key.starts_with('.') || key.as_str() == ".type")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if actual != journal.created_options {
                return Err(invalid(
                    "Owned bridge was changed externally; refusing to delete it; journal retained",
                ));
            }
            if config.iter().any(|(name, s)| {
                !journal.interface_devices.contains_key(name)
                    && option(s, "device") == Some(journal.bridge.as_str())
            }) {
                return Err(invalid(
                    "Another interface now uses the owned bridge; journal retained",
                ));
            }
            edits.push(Edit::Delete(name.clone()));
        }
    } else {
        let section = config
            .get(&journal.bridge_section)
            .ok_or_else(|| invalid("Existing bridge was removed; journal retained"))?;
        if option(section, "name") != Some(journal.bridge.as_str())
            || option(section, "type") != Some("bridge")
        {
            return Err(invalid("Existing bridge was replaced; journal retained"));
        }
        if ports(section)?.contains(&journal.usb) {
            edits.push(Edit::DelList(
                format!("{}.ports", journal.bridge_section),
                journal.usb.clone(),
            ));
        }
    }
    Ok(edits)
}

trait Control {
    fn config(&mut self) -> Result<Config>;
    fn apply(&mut self, original: &Config, edits: &[Edit]) -> Result<()>;
    fn reload(&mut self) -> Result<()>;
    fn wait(&mut self, journal: &Journal, active: bool) -> Result<()>;
    fn save(&mut self, journal: &Journal) -> Result<()>;
    fn clear(&mut self) -> Result<()>;
}

fn activate_with(
    control: &mut impl Control,
    selected: &NetworkInterfaceInfo,
    usb: &str,
    mac: &str,
    existing_bridge: bool,
) -> Result<Journal> {
    let config = control.config()?;
    let (journal, edits) = plan(&config, selected, usb, mac, existing_bridge)?;
    control.save(&journal)?; // Durable recovery precedes every configuration change.
    let result = control
        .apply(&config, &edits)
        .and_then(|()| control.reload())
        .and_then(|()| control.wait(&journal, true));
    if let Err(primary) = result {
        return Err(match restore_with(control, &journal) {
            Ok(()) => primary,
            Err(rollback) => invalid(format!(
                "{primary}; OpenWrt bridge rollback failed: {rollback}"
            )),
        });
    }
    Ok(journal)
}

fn restore_with(control: &mut impl Control, journal: &Journal) -> Result<()> {
    let config = control.config()?;
    let edits = restore_plan(&config, journal)?;
    if !edits.is_empty() {
        control.apply(&config, &edits)?;
    }
    control.reload()?;
    control.wait(journal, false)?;
    control.clear()
}

struct System;
impl Control for System {
    fn config(&mut self) -> Result<Config> {
        if Path::new("/var/run/uci/network").exists() {
            return Err(invalid("A runtime network UCI override is active; OTG bridge configuration cannot safely replace it"));
        }
        let pending = run_command("uci", &["-q", "changes", "network"])?;
        if !pending.stdout.is_empty() {
            return Err(invalid(
                "Save or discard pending network UCI changes before changing the OTG bridge",
            ));
        }
        let output = run_command("ubus", &["call", "uci", "get", r#"{"config":"network"}"#])?;
        let value: Value = serde_json::from_slice(&output.stdout).map_err(io_error)?;
        serde_json::from_value(value.get("values").cloned().ok_or_else(|| {
            invalid("Invalid ubus UCI network response; rpcd UCI service is required")
        })?)
        .map_err(io_error)
    }
    fn apply(&mut self, original: &Config, edits: &[Edit]) -> Result<()> {
        if &self.config()? != original {
            return Err(invalid(
                "Network configuration changed concurrently; retry the bridge operation",
            ));
        }
        let path = Path::new("/etc/config/network");
        if path.is_symlink() {
            return Err(invalid("Symlinked network configuration is unsupported"));
        }
        let before = fs::read(path).map_err(io_error)?;
        let directory = tempfile::tempdir().map_err(io_error)?;
        let copy = directory.path().join("network");
        fs::write(&copy, &before).map_err(io_error)?;
        let delta = directory.path().join("delta");
        fs::create_dir(&delta).map_err(io_error)?;
        let confdir = directory
            .path()
            .to_str()
            .ok_or_else(|| invalid("Invalid UCI temporary directory"))?;
        let savedir = delta
            .to_str()
            .ok_or_else(|| invalid("Invalid UCI delta directory"))?;
        for edit in edits {
            let (command, argument) = match edit {
                Edit::Set(key, value) => ("set", format!("network.{key}={value}")),
                Edit::Delete(key) => ("delete", format!("network.{key}")),
                Edit::AddList(key, value) => ("add_list", format!("network.{key}={value}")),
                Edit::DelList(key, value) => ("del_list", format!("network.{key}={value}")),
            };
            run_command(
                "uci",
                &["-q", "-c", confdir, "-t", savedir, command, &argument],
            )?;
        }
        run_command(
            "uci",
            &["-q", "-c", confdir, "-t", savedir, "commit", "network"],
        )?;
        let mut replacement =
            tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(io_error)?;
        replacement
            .as_file()
            .set_permissions(fs::metadata(path).map_err(io_error)?.permissions())
            .map_err(io_error)?;
        replacement
            .write_all(&fs::read(&copy).map_err(io_error)?)
            .map_err(io_error)?;
        replacement.as_file().sync_all().map_err(io_error)?;
        if fs::read(path).map_err(io_error)? != before || &self.config()? != original {
            return Err(invalid(
                "Network configuration changed concurrently; original configuration was preserved",
            ));
        }
        replacement.persist(path).map_err(io_error)?;
        fs::File::open(path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(io_error)
    }
    fn reload(&mut self) -> Result<()> {
        run_command("ubus", &["call", "network", "reload"]).map(|_| ())
    }
    fn wait(&mut self, journal: &Journal, active: bool) -> Result<()> {
        let interface = if active {
            &journal.bridge
        } else {
            &journal.uplink
        };
        let deadline = Instant::now() + Duration::from_secs(35);
        while Instant::now() < deadline {
            let members = !active
                || (Path::new("/sys/class/net")
                    .join(interface)
                    .join("brif")
                    .join(&journal.usb)
                    .exists()
                    && (journal.created_section.is_none()
                        || Path::new("/sys/class/net")
                            .join(interface)
                            .join("brif")
                            .join(&journal.uplink)
                            .exists()));
            if members
                && first_ipv4_address(interface).is_some()
                && (!journal.had_default_route || default_route(interface).is_some())
            {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(invalid(format!("OpenWrt interface {interface} did not restore its IPv4 address/default route or bridge ports in time")))
    }
    fn save(&mut self, journal: &Journal) -> Result<()> {
        let path = Path::new(JOURNAL_PATH);
        if path.exists() {
            return Err(invalid("An OpenWrt bridge recovery journal already exists; recover it before activating another bridge"));
        }
        fs::create_dir_all(path.parent().unwrap()).map_err(io_error)?;
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(io_error)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(io_error)?;
        file.write_all(&serde_json::to_vec(journal).map_err(io_error)?)
            .map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        file.persist_noclobber(path).map_err(io_error)?;
        fs::File::open(path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(io_error)
    }
    fn clear(&mut self) -> Result<()> {
        match fs::remove_file(JOURNAL_PATH) {
            Ok(()) => fs::File::open(Path::new(JOURNAL_PATH).parent().unwrap())
                .and_then(|f| f.sync_all())
                .map_err(io_error),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error(error)),
        }
    }
}

pub(super) fn activate(selected: &NetworkInterfaceInfo, usb: &str) -> Result<Journal> {
    for name in ["uci", "ubus", "ip"] {
        ensure_command(name)?;
    }
    if !bridgeable_device(&selected.name) {
        return Err(invalid("OpenWrt bridge uplink must be Ethernet or an existing bridge; ordinary Wi-Fi clients cannot be bridged"));
    }
    let existing_bridge = Path::new("/sys/class/net")
        .join(&selected.name)
        .join("bridge")
        .is_dir();
    if existing_bridge
        && fs::read_to_string(
            Path::new("/sys/class/net")
                .join(&selected.name)
                .join("bridge/vlan_filtering"),
        )
        .is_ok_and(|value| value.trim() == "1")
    {
        return Err(invalid(
            "VLAN-filtered bridges require an explicit USB VLAN configuration and are unsupported",
        ));
    }
    if !existing_bridge && Path::new("/sys/class/net").join(BRIDGE).exists() {
        return Err(invalid(
            "okvm-br0 already exists; refusing to replace another bridge",
        ));
    }
    let usb_path = Path::new("/sys/class/net").join(usb);
    if !device_name(usb) || !usb_path.is_dir() || usb_path.join("master").exists() {
        return Err(invalid(
            "USB network interface is unavailable or already enslaved",
        ));
    }
    let mac = fs::read_to_string(
        Path::new("/sys/class/net")
            .join(&selected.name)
            .join("address"),
    )
    .map_err(io_error)?;
    activate_with(&mut System, selected, usb, mac.trim(), existing_bridge)
}

pub(super) fn deactivate(journal: &Journal) -> Result<()> {
    restore_with(&mut System, journal)
}
pub(super) fn recover() -> Result<()> {
    let contents = match fs::read(JOURNAL_PATH) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    };
    let journal: Journal = serde_json::from_slice(&contents).map_err(io_error)?;
    restore_with(&mut System, &journal)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        config: Config,
        journal: Option<Journal>,
        reload_failures: usize,
        fail_active_wait: bool,
        saved_before_apply: bool,
    }
    impl Control for Fake {
        fn config(&mut self) -> Result<Config> {
            Ok(self.config.clone())
        }
        fn apply(&mut self, original: &Config, edits: &[Edit]) -> Result<()> {
            assert_eq!(original, &self.config);
            assert!(self.journal.is_some());
            self.saved_before_apply = true;
            for edit in edits {
                match edit {
                    Edit::Set(key, value) => match key.split_once('.') {
                        Some((name, key)) => {
                            self.config
                                .get_mut(name)
                                .unwrap()
                                .insert(key.to_owned(), json!(value));
                        }
                        None => {
                            self.config.insert(
                                key.clone(),
                                json!({".type":value}).as_object().unwrap().clone(),
                            );
                        }
                    },
                    Edit::Delete(key) => {
                        self.config.remove(key);
                    }
                    Edit::AddList(key, value) => {
                        let (name, key) = key.split_once('.').unwrap();
                        self.config
                            .get_mut(name)
                            .unwrap()
                            .entry(key.to_owned())
                            .or_insert(json!([]))
                            .as_array_mut()
                            .unwrap()
                            .push(json!(value));
                    }
                    Edit::DelList(key, value) => {
                        let (name, key) = key.split_once('.').unwrap();
                        self.config
                            .get_mut(name)
                            .unwrap()
                            .get_mut(key)
                            .unwrap()
                            .as_array_mut()
                            .unwrap()
                            .retain(|v| v != value);
                    }
                }
            }
            Ok(())
        }
        fn reload(&mut self) -> Result<()> {
            if self.reload_failures > 0 {
                self.reload_failures -= 1;
                return Err(invalid("simulated reload failure"));
            }
            Ok(())
        }
        fn wait(&mut self, _: &Journal, active: bool) -> Result<()> {
            if active && self.fail_active_wait {
                Err(invalid("simulated DHCP timeout"))
            } else {
                Ok(())
            }
        }
        fn save(&mut self, journal: &Journal) -> Result<()> {
            assert!(self.journal.is_none());
            self.journal = Some(journal.clone());
            Ok(())
        }
        fn clear(&mut self) -> Result<()> {
            self.journal = None;
            Ok(())
        }
    }
    fn fixture(proto: &str) -> (Fake, NetworkInterfaceInfo) {
        let config: Config = serde_json::from_value(json!({
            "lan":{".type":"interface", "device":"eth0", "proto":proto, "ipaddr":"192.0.2.5", "dns":["192.0.2.1"], "gateway":"192.0.2.1"},
            "lan6":{".type":"interface", "device":"eth0", "proto":"dhcpv6"},
            "wan":{".type":"interface", "device":"eth1", "proto":"dhcp"}
        })).unwrap();
        let selected = NetworkInterfaceInfo {
            name: "eth0".into(),
            connection: "lan".into(),
            interface_type: "netifd".into(),
            state: "connected".into(),
            addresses: vec!["192.0.2.5/24".into()],
            has_default_route: true,
            bridge_supported: true,
            reason: None,
        };
        (
            Fake {
                config,
                journal: None,
                reload_failures: 0,
                fail_active_wait: false,
                saved_before_apply: false,
            },
            selected,
        )
    }
    #[test]
    fn ethernet_transaction_keeps_static_ip_dns_routes_ipv6_and_other_networks() {
        let (mut fake, selected) = fixture("static");
        let original = fake.config.clone();
        let journal =
            activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).unwrap();
        assert!(fake.saved_before_apply);
        for name in ["lan", "lan6"] {
            let mut expected = original[name].clone();
            expected.insert("device".into(), json!(BRIDGE));
            assert_eq!(fake.config[name], expected);
        }
        assert_eq!(fake.config["wan"], original["wan"]);
        assert_eq!(
            fake.config[&journal.bridge_section]["macaddr"],
            "02:00:00:00:00:01"
        );
        fake.config
            .get_mut("wan")
            .unwrap()
            .insert("metric".into(), json!("50"));
        restore_with(&mut fake, &journal).unwrap();
        assert_eq!(fake.config["lan"], original["lan"]);
        assert_eq!(fake.config["lan6"], original["lan6"]);
        assert_eq!(fake.config["wan"]["metric"], "50");
        assert!(fake.journal.is_none());
        assert!(!fake.config.contains_key(&journal.bridge_section));
    }
    #[test]
    fn existing_bridge_only_adds_and_removes_usb_preserving_new_ports() {
        let (mut fake, mut selected) = fixture("dhcp");
        selected.name = "br-lan".into();
        fake.config
            .get_mut("lan")
            .unwrap()
            .insert("device".into(), json!("br-lan"));
        fake.config.insert("cfg123".into(), json!({".type":"device", "name":"br-lan", "type":"bridge", "ports":["eth0"], "stp":"1"}).as_object().unwrap().clone());
        let journal =
            activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", true).unwrap();
        assert!(journal.created_section.is_none());
        assert_eq!(fake.config["cfg123"]["ports"], json!(["eth0", "usb0"]));
        fake.config
            .get_mut("cfg123")
            .unwrap()
            .get_mut("ports")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(json!("eth2"));
        restore_with(&mut fake, &journal).unwrap();
        assert_eq!(fake.config["cfg123"]["ports"], json!(["eth0", "eth2"]));
        assert_eq!(fake.config["cfg123"]["stp"], "1");
        assert_eq!(fake.config["lan"]["device"], "br-lan");
    }
    #[test]
    fn reload_failure_and_dhcp_timeout_restore_original_before_returning_error() {
        for reload_failure in [true, false] {
            let (mut fake, selected) = fixture("dhcp");
            let original = fake.config.clone();
            fake.reload_failures = usize::from(reload_failure);
            fake.fail_active_wait = !reload_failure;
            assert!(
                activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).is_err()
            );
            assert_eq!(fake.config, original);
            assert!(fake.journal.is_none());
        }
    }
    #[test]
    fn failed_rollback_keeps_journal_and_can_be_retried() {
        let (mut fake, selected) = fixture("dhcp");
        let original = fake.config.clone();
        fake.reload_failures = 2;
        let error =
            activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).unwrap_err();
        assert!(error.to_string().contains("rollback failed"));
        let journal = fake.journal.clone().unwrap();
        restore_with(&mut fake, &journal).unwrap();
        assert_eq!(fake.config, original);
        assert!(fake.journal.is_none());
    }
    #[test]
    fn reboot_recovery_accepts_serialized_journal_and_already_restored_config() {
        let (mut fake, selected) = fixture("dhcp");
        let original = fake.config.clone();
        let journal =
            activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).unwrap();
        let recovered: Journal =
            serde_json::from_slice(&serde_json::to_vec(&journal).unwrap()).unwrap();
        restore_with(&mut fake, &recovered).unwrap();
        fake.journal = Some(recovered.clone());
        restore_with(&mut fake, &recovered).unwrap();
        assert_eq!(fake.config, original);
    }
    #[test]
    fn externally_changed_interface_or_owned_bridge_is_never_overwritten() {
        for change_interface in [true, false] {
            let (mut fake, selected) = fixture("dhcp");
            let journal =
                activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).unwrap();
            let (name, key, value) = if change_interface {
                ("lan", "device", "eth2")
            } else {
                (journal.bridge_section.as_str(), "stp", "1")
            };
            fake.config
                .get_mut(name)
                .unwrap()
                .insert(key.into(), json!(value));
            let before = fake.config.clone();
            assert!(restore_with(&mut fake, &journal).is_err());
            assert_eq!(fake.config, before);
            assert!(fake.journal.is_some());
        }
    }
    #[test]
    fn rejects_ppp_legacy_and_busy_usb_before_writing_journal() {
        for kind in 0..3 {
            let (mut fake, selected) = fixture(if kind == 0 { "pppoe" } else { "dhcp" });
            if kind == 1 {
                fake.config
                    .get_mut("lan")
                    .unwrap()
                    .insert("ifname".into(), json!("eth0"));
            }
            if kind == 2 {
                fake.config
                    .get_mut("wan")
                    .unwrap()
                    .insert("device".into(), json!("usb0"));
            }
            let before = fake.config.clone();
            assert!(
                activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", false).is_err()
            );
            assert_eq!(fake.config, before);
            assert!(fake.journal.is_none());
        }
    }
    #[test]
    fn rejects_vlan_filtered_bridge_without_mutations() {
        let (mut fake, mut selected) = fixture("dhcp");
        selected.name = "br-lan".into();
        fake.config
            .get_mut("lan")
            .unwrap()
            .insert("device".into(), json!("br-lan"));
        fake.config.insert(
            "bridge".into(),
            json!({".type":"device", "name":"br-lan", "type":"bridge", "ports":["eth0"]})
                .as_object()
                .unwrap()
                .clone(),
        );
        fake.config.insert(
            "vlan".into(),
            json!({".type":"bridge-vlan", "device":"br-lan", "vlan":"1"})
                .as_object()
                .unwrap()
                .clone(),
        );
        assert!(activate_with(&mut fake, &selected, "usb0", "02:00:00:00:00:01", true).is_err());
        assert!(fake.journal.is_none());
    }
}
