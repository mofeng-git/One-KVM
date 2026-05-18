use super::{DeviceInfo, DiskSpaceInfo, NetworkAddress};
use crate::error::{AppError, Result};
use crate::utils::hostname_uname;

pub fn get_disk_space(path: &std::path::Path) -> Result<DiskSpaceInfo> {
    let stat = nix::sys::statvfs::statvfs(path)
        .map_err(|e| AppError::Internal(format!("Failed to get disk space: {}", e)))?;

    let block_size = stat.block_size() as u64;
    let total = stat.blocks() as u64 * block_size;
    let available = stat.blocks_available() as u64 * block_size;
    let used = total - available;

    Ok(DiskSpaceInfo {
        total,
        available,
        used,
    })
}

pub fn get_device_info() -> DeviceInfo {
    let mem_info = get_meminfo();

    DeviceInfo {
        hostname: hostname_uname(),
        cpu_model: get_cpu_model(),
        cpu_usage: get_cpu_usage(),
        memory_total: mem_info.total,
        memory_used: mem_info.total.saturating_sub(mem_info.available),
        network_addresses: get_network_addresses(),
        serial_ports: crate::utils::list_serial_ports(),
    }
}

fn get_cpu_model() -> String {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok();

    if let Some(model) = parse_cpu_model_from_cpuinfo_content(cpuinfo.as_deref()) {
        return model;
    }

    if let Some(model) = read_device_tree_model() {
        return model;
    }

    if let Some(content) = cpuinfo.as_deref() {
        let cores = content
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count();
        if cores > 0 {
            return format!("{} {}C", std::env::consts::ARCH, cores);
        }
    }

    std::env::consts::ARCH.to_string()
}

fn parse_cpu_model_from_cpuinfo_content(content: Option<&str>) -> Option<String> {
    let content = content?;

    content
        .lines()
        .find(|line| line.starts_with("model name") || line.starts_with("Model"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_device_tree_model() -> Option<String> {
    std::fs::read("/proc/device-tree/model")
        .ok()
        .and_then(|bytes| parse_device_tree_model_bytes(bytes.as_slice()))
}

fn parse_device_tree_model_bytes(bytes: &[u8]) -> Option<String> {
    let model = String::from_utf8_lossy(bytes)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string();

    if model.is_empty() {
        None
    } else {
        Some(model)
    }
}

static CPU_PREV_STATS: std::sync::OnceLock<std::sync::Mutex<(u64, u64)>> =
    std::sync::OnceLock::new();

fn get_cpu_usage() -> f32 {
    let content = match std::fs::read_to_string("/proc/stat") {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    let cpu_line = match content.lines().next() {
        Some(line) if line.starts_with("cpu ") => line,
        _ => return 0.0,
    };

    let parts: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .take(8)
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() < 4 {
        return 0.0;
    }

    let idle = parts[3] + parts.get(4).unwrap_or(&0);
    let total: u64 = parts.iter().sum();

    let prev_mutex = CPU_PREV_STATS.get_or_init(|| std::sync::Mutex::new((0, 0)));
    let mut prev = prev_mutex.lock().unwrap();
    let (prev_idle, prev_total) = *prev;

    let idle_delta = idle.saturating_sub(prev_idle);
    let total_delta = total.saturating_sub(prev_total);
    *prev = (idle, total);

    if total_delta == 0 {
        return 0.0;
    }

    let usage = 100.0 * (1.0 - (idle_delta as f64 / total_delta as f64));
    usage as f32
}

struct MemInfo {
    total: u64,
    available: u64,
}

fn get_meminfo() -> MemInfo {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => {
            return MemInfo {
                total: 0,
                available: 0,
            }
        }
    };

    let mut total = 0u64;
    let mut available = 0u64;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(kb) = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
            {
                total = kb * 1024;
            }
        } else if line.starts_with("MemAvailable:") {
            if let Some(kb) = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
            {
                available = kb * 1024;
            }
        }

        if total > 0 && available > 0 {
            break;
        }
    }

    MemInfo { total, available }
}

fn get_network_addresses() -> Vec<NetworkAddress> {
    let all_addrs = match nix::ifaddrs::getifaddrs() {
        Ok(addrs) => addrs,
        Err(_) => return Vec::new(),
    };

    let mut up_ifaces = std::collections::HashSet::new();
    let net_dir = match std::fs::read_dir("/sys/class/net") {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    for entry in net_dir.flatten() {
        let iface_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        if iface_name == "lo" {
            continue;
        }

        let operstate_path = entry.path().join("operstate");
        let is_up = std::fs::read_to_string(&operstate_path)
            .map(|s| s.trim() == "up")
            .unwrap_or(false);

        if is_up {
            up_ifaces.insert(iface_name);
        }
    }

    let mut addresses = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ifaddr in all_addrs {
        let iface_name = &ifaddr.interface_name;
        if iface_name == "lo" || !up_ifaces.contains(iface_name) {
            continue;
        }

        if let Some(addr) = ifaddr.address {
            if let Some(sockaddr_in) = addr.as_sockaddr_in() {
                let ip = sockaddr_in.ip();
                if ip.is_loopback() {
                    continue;
                }
                let ip_str = ip.to_string();
                if seen.insert((iface_name.clone(), ip_str.clone())) {
                    addresses.push(NetworkAddress {
                        interface: iface_name.clone(),
                        ip: ip_str,
                    });
                }
            } else if let Some(sockaddr_in6) = addr.as_sockaddr_in6() {
                let ip = sockaddr_in6.ip();
                if ip.is_loopback() || ip.is_unspecified() || ip.is_unicast_link_local() {
                    continue;
                }
                let ip_str = ip.to_string();
                if seen.insert((iface_name.clone(), ip_str.clone())) {
                    addresses.push(NetworkAddress {
                        interface: iface_name.clone(),
                        ip: ip_str,
                    });
                }
            }
        }
    }

    addresses
}

#[cfg(test)]
mod tests {
    use super::{parse_cpu_model_from_cpuinfo_content, parse_device_tree_model_bytes};

    #[test]
    fn parse_cpu_model_from_model_name_field() {
        let input = "processor\t: 0\nmodel name\t: Intel(R) Xeon(R)\n";
        assert_eq!(
            parse_cpu_model_from_cpuinfo_content(input),
            Some("Intel(R) Xeon(R)".to_string())
        );
    }

    #[test]
    fn parse_cpu_model_from_model_field() {
        let input = "processor\t: 0\nModel\t\t: Raspberry Pi 4 Model B Rev 1.4\n";
        assert_eq!(
            parse_cpu_model_from_cpuinfo_content(input),
            Some("Raspberry Pi 4 Model B Rev 1.4".to_string())
        );
    }

    #[test]
    fn parse_device_tree_model_trimmed() {
        let input = b"Onething OEC Box\0\n";
        assert_eq!(
            parse_device_tree_model_bytes(input),
            Some("Onething OEC Box".to_string())
        );
    }
}
