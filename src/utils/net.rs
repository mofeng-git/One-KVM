//! Networking helpers for binding sockets with explicit IPv6-only behavior.

use std::io;
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

use nix::sys::socket::{
    self, sockopt, AddressFamily, Backlog, SockFlag, SockProtocol, SockType, SockaddrIn,
    SockaddrIn6,
};

fn socket_addr_family(addr: &SocketAddr) -> AddressFamily {
    match addr {
        SocketAddr::V4(_) => AddressFamily::Inet,
        SocketAddr::V6(_) => AddressFamily::Inet6,
    }
}

/// Bind a TCP listener with IPv6-only set for IPv6 sockets.
pub fn bind_tcp_listener(addr: SocketAddr) -> io::Result<TcpListener> {
    let domain = socket_addr_family(&addr);
    let fd = socket::socket(
        domain,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        SockProtocol::Tcp,
    )
    .map_err(io::Error::from)?;

    socket::setsockopt(&fd, sockopt::ReuseAddr, &true).map_err(io::Error::from)?;

    if matches!(addr, SocketAddr::V6(_)) {
        socket::setsockopt(&fd, sockopt::Ipv6V6Only, &true).map_err(io::Error::from)?;
    }

    match addr {
        SocketAddr::V4(v4) => {
            let sockaddr = SockaddrIn::from(v4);
            socket::bind(fd.as_raw_fd(), &sockaddr).map_err(io::Error::from)?;
        }
        SocketAddr::V6(v6) => {
            let sockaddr = SockaddrIn6::from(v6);
            socket::bind(fd.as_raw_fd(), &sockaddr).map_err(io::Error::from)?;
        }
    }
    socket::listen(&fd, Backlog::MAXCONN).map_err(io::Error::from)?;

    let listener = unsafe { TcpListener::from_raw_fd(fd.into_raw_fd()) };
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Bind a UDP socket with IPv6-only set for IPv6 sockets.
pub fn bind_udp_socket(addr: SocketAddr) -> io::Result<UdpSocket> {
    let domain = socket_addr_family(&addr);
    let fd = socket::socket(
        domain,
        SockType::Datagram,
        SockFlag::SOCK_CLOEXEC,
        SockProtocol::Udp,
    )
    .map_err(io::Error::from)?;

    socket::setsockopt(&fd, sockopt::ReuseAddr, &true).map_err(io::Error::from)?;

    if matches!(addr, SocketAddr::V6(_)) {
        socket::setsockopt(&fd, sockopt::Ipv6V6Only, &true).map_err(io::Error::from)?;
    }

    match addr {
        SocketAddr::V4(v4) => {
            let sockaddr = SockaddrIn::from(v4);
            socket::bind(fd.as_raw_fd(), &sockaddr).map_err(io::Error::from)?;
        }
        SocketAddr::V6(v6) => {
            let sockaddr = SockaddrIn6::from(v6);
            socket::bind(fd.as_raw_fd(), &sockaddr).map_err(io::Error::from)?;
        }
    }

    let socket = unsafe { UdpSocket::from_raw_fd(fd.into_raw_fd()) };
    socket.set_nonblocking(true)?;
    Ok(socket)
}
