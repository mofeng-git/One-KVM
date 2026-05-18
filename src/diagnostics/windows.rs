use super::{DeviceInfo, DiskSpaceInfo, NetworkAddress};
use crate::error::{AppError, Result};
use crate::utils::hostname_uname;
use std::ffi::CStr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, FILETIME};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6,
};
use windows_sys::Win32::System::SystemInformation::{
    GetNativeSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, PROCESSOR_ARCHITECTURE_AMD64,
    PROCESSOR_ARCHITECTURE_ARM64, PROCESSOR_ARCHITECTURE_INTEL, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::GetSystemTimes;

pub fn get_disk_space(_path: &std::path::Path) -> Result<DiskSpaceInfo> {
    Err(AppError::Internal(
        "Disk space reporting is unavailable on Windows".to_string(),
    ))
}

pub fn get_device_info() -> DeviceInfo {
    let (memory_total, memory_used) = get_memory_usage();

    DeviceInfo {
        hostname: hostname_uname(),
        cpu_model: get_cpu_model(),
        cpu_usage: get_cpu_usage(),
        memory_total,
        memory_used,
        network_addresses: get_network_addresses(),
        serial_ports: crate::utils::list_serial_ports(),
    }
}

fn get_cpu_model() -> String {
    std::env::var("PROCESSOR_IDENTIFIER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(get_cpu_arch_label)
}

fn get_cpu_arch_label() -> String {
    let mut info = std::mem::MaybeUninit::<SYSTEM_INFO>::zeroed();
    unsafe {
        GetNativeSystemInfo(info.as_mut_ptr());
        let info = info.assume_init();
        match info.Anonymous.Anonymous.wProcessorArchitecture {
            PROCESSOR_ARCHITECTURE_AMD64 => "x86_64".to_string(),
            PROCESSOR_ARCHITECTURE_ARM64 => "aarch64".to_string(),
            PROCESSOR_ARCHITECTURE_INTEL => "x86".to_string(),
            _ => std::env::consts::ARCH.to_string(),
        }
    }
}

fn get_memory_usage() -> (u64, u64) {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 {
        return (0, 0);
    }

    (
        status.ullTotalPhys,
        status.ullTotalPhys.saturating_sub(status.ullAvailPhys),
    )
}

fn get_cpu_usage() -> f32 {
    static LAST_SAMPLE: OnceLock<Mutex<Option<CpuTimes>>> = OnceLock::new();

    let Some(current) = read_cpu_times() else {
        return 0.0;
    };
    let sample = LAST_SAMPLE.get_or_init(|| Mutex::new(None));
    let Ok(mut last) = sample.lock() else {
        return 0.0;
    };

    let (previous, current) = if let Some(previous) = last.replace(current) {
        (previous, current)
    } else {
        drop(last);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let Some(next) = read_cpu_times() else {
            return 0.0;
        };
        if let Ok(mut last) = sample.lock() {
            *last = Some(next);
        }
        (current, next)
    };

    let idle = current.idle.saturating_sub(previous.idle);
    let kernel = current.kernel.saturating_sub(previous.kernel);
    let user = current.user.saturating_sub(previous.user);
    let total = kernel.saturating_add(user);

    if total == 0 {
        return 0.0;
    }

    ((total.saturating_sub(idle)) as f64 * 100.0 / total as f64).clamp(0.0, 100.0) as f32
}

#[derive(Clone, Copy)]
struct CpuTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

fn read_cpu_times() -> Option<CpuTimes> {
    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = idle;
    let mut user = idle;

    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }

    Some(CpuTimes {
        idle: filetime_to_u64(idle),
        kernel: filetime_to_u64(kernel),
        user: filetime_to_u64(user),
    })
}

fn filetime_to_u64(time: FILETIME) -> u64 {
    ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64
}

fn get_network_addresses() -> Vec<NetworkAddress> {
    let mut buffer_len = 15_000u32;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;

    for _ in 0..2 {
        let mut buffer = vec![0u8; buffer_len as usize];
        let ret = unsafe {
            GetAdaptersAddresses(
                0,
                flags,
                std::ptr::null_mut(),
                buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut buffer_len,
            )
        };

        if ret == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if ret != ERROR_SUCCESS {
            return Vec::new();
        }

        let mut addresses = Vec::new();
        let mut adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !adapter.is_null() {
            let adapter_ref = unsafe { &*adapter };
            if adapter_ref.OperStatus != IfOperStatusUp {
                adapter = adapter_ref.Next;
                continue;
            }

            let interface = adapter_name(adapter_ref);
            let mut unicast = adapter_ref.FirstUnicastAddress;

            while !unicast.is_null() {
                let unicast_ref = unsafe { &*unicast };
                if let Some(ip) = sockaddr_to_ip(unicast_ref.Address.lpSockaddr) {
                    addresses.push(NetworkAddress {
                        interface: interface.clone(),
                        ip,
                    });
                }
                unicast = unicast_ref.Next;
            }

            adapter = adapter_ref.Next;
        }

        addresses.sort_by(|a, b| a.interface.cmp(&b.interface).then(a.ip.cmp(&b.ip)));
        addresses.dedup_by(|a, b| a.interface == b.interface && a.ip == b.ip);
        return addresses;
    }

    Vec::new()
}

fn adapter_name(adapter: &IP_ADAPTER_ADDRESSES_LH) -> String {
    unsafe {
        if !adapter.FriendlyName.is_null() {
            let mut len = 0usize;
            while *adapter.FriendlyName.add(len) != 0 {
                len += 1;
            }
            let name =
                String::from_utf16_lossy(std::slice::from_raw_parts(adapter.FriendlyName, len));
            if !name.trim().is_empty() {
                return name;
            }
        }

        if !adapter.AdapterName.is_null() {
            return CStr::from_ptr(adapter.AdapterName.cast())
                .to_string_lossy()
                .into_owned();
        }
    }

    "unknown".to_string()
}

fn sockaddr_to_ip(sockaddr: *const SOCKADDR) -> Option<String> {
    if sockaddr.is_null() {
        return None;
    }

    let family = unsafe { (*sockaddr).sa_family };
    match family {
        AF_INET => {
            let addr = unsafe { *(sockaddr as *const SOCKADDR_IN) };
            let bytes = unsafe { addr.sin_addr.S_un.S_addr.to_ne_bytes() };
            Some(Ipv4Addr::from(bytes).to_string())
        }
        AF_INET6 => {
            let addr = unsafe { *(sockaddr as *const SOCKADDR_IN6) };
            let bytes = unsafe { addr.sin6_addr.u.Byte };
            Some(Ipv6Addr::from(bytes).to_string())
        }
        _ => None,
    }
}
