#![allow(clippy::missing_safety_doc)]

use std::ffi::CString;
use std::mem::{size_of, zeroed};
use std::os::raw::{c_char, c_int, c_uint, c_void};

#[repr(C)]
pub struct ifaddrs {
    pub ifa_next: *mut ifaddrs,
    pub ifa_name: *mut c_char,
    pub ifa_flags: c_uint,
    pub ifa_addr: *mut libc::sockaddr,
    pub ifa_netmask: *mut libc::sockaddr,
    pub ifa_ifu: *mut libc::sockaddr,
    pub ifa_data: *mut c_void,
}

#[repr(C)]
struct AddrNode {
    ifa: ifaddrs,
    name: CString,
    addr: libc::sockaddr_in,
    next: *mut AddrNode,
}

fn sockaddr_to_ipv4(addr: libc::sockaddr) -> Option<std::net::Ipv4Addr> {
    if addr.sa_family as c_int != libc::AF_INET {
        return None;
    }

    unsafe {
        let sin = &*(&addr as *const libc::sockaddr as *const libc::sockaddr_in);
        Some(std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)))
    }
}

fn query_ipv4(iface_name: &str) -> Option<libc::sockaddr_in> {
    let name = CString::new(iface_name).ok()?;
    if name.as_bytes().len() >= libc::IFNAMSIZ {
        return None;
    }

    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
        if fd < 0 {
            return None;
        }

        let mut request: libc::ifreq = zeroed();
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            request.ifr_name.as_mut_ptr(),
            name.as_bytes_with_nul().len(),
        );

        let request_code = libc::SIOCGIFADDR.try_into().ok()?;
        let rc = libc::ioctl(fd, request_code, &mut request);
        libc::close(fd);
        if rc < 0 {
            return None;
        }

        let addr = request.ifr_ifru.ifru_addr;
        if addr.sa_family as c_int != libc::AF_INET {
            return None;
        }

        let mut sin: libc::sockaddr_in = zeroed();
        std::ptr::copy_nonoverlapping(
            &addr as *const libc::sockaddr as *const u8,
            &mut sin as *mut libc::sockaddr_in as *mut u8,
            size_of::<libc::sockaddr_in>(),
        );
        Some(sin)
    }
}

#[no_mangle]
pub unsafe extern "C" fn getifaddrs(addrs: *mut *mut ifaddrs) -> c_int {
    if addrs.is_null() {
        return -1;
    }
    *addrs = std::ptr::null_mut();

    let net_dir = match std::fs::read_dir("/sys/class/net") {
        Ok(dir) => dir,
        Err(_) => return -1,
    };

    let mut head: *mut AddrNode = std::ptr::null_mut();
    let mut tail: *mut AddrNode = std::ptr::null_mut();

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
        if !is_up {
            continue;
        }

        let Some(addr) = query_ipv4(&iface_name) else {
            continue;
        };
        let ip = sockaddr_to_ipv4(unsafe {
            std::mem::transmute::<libc::sockaddr_in, libc::sockaddr>(addr)
        });
        if ip
            .map(|ip| ip.is_loopback() || ip.is_unspecified())
            .unwrap_or(true)
        {
            continue;
        }

        let name = match CString::new(iface_name) {
            Ok(name) => name,
            Err(_) => continue,
        };

        let mut node = Box::new(AddrNode {
            ifa: ifaddrs {
                ifa_next: std::ptr::null_mut(),
                ifa_name: std::ptr::null_mut(),
                ifa_flags: 0,
                ifa_addr: std::ptr::null_mut(),
                ifa_netmask: std::ptr::null_mut(),
                ifa_ifu: std::ptr::null_mut(),
                ifa_data: std::ptr::null_mut(),
            },
            name,
            addr,
            next: std::ptr::null_mut(),
        });

        node.ifa.ifa_name = node.name.as_ptr() as *mut c_char;
        node.ifa.ifa_addr = &mut node.addr as *mut libc::sockaddr_in as *mut libc::sockaddr;
        node.ifa.ifa_ifu = std::ptr::null_mut();
        node.ifa.ifa_netmask = std::ptr::null_mut();
        node.ifa.ifa_flags = (libc::IFF_UP | libc::IFF_RUNNING) as c_uint;

        let raw = Box::into_raw(node);
        if head.is_null() {
            head = raw;
        } else {
            (*tail).next = raw;
            (*tail).ifa.ifa_next = raw as *mut ifaddrs;
        }
        tail = raw;
    }

    *addrs = if head.is_null() {
        std::ptr::null_mut()
    } else {
        head as *mut ifaddrs
    };
    0
}

#[no_mangle]
pub unsafe extern "C" fn freeifaddrs(addrs: *mut ifaddrs) {
    let mut current = addrs as *mut AddrNode;
    while !current.is_null() {
        let next = (*current).next;
        drop(Box::from_raw(current));
        current = next;
    }
}
