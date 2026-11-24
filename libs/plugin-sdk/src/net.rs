use std::{
    io,
    net::{IpAddr, SocketAddr, TcpListener, UdpSocket},
};

use socket2::{Domain, Protocol, Socket, Type};

pub fn bind_socket_addr(bind: &str, port: u16) -> Result<SocketAddr, std::net::AddrParseError> {
    bind.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port))
}

fn bind(address: SocketAddr, kind: Type, protocol: Protocol) -> io::Result<Socket> {
    let socket = Socket::new(Domain::for_address(address), kind, Some(protocol))?;
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    if address.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.set_nonblocking(true)?;
    socket.bind(&address.into())?;
    Ok(socket)
}

pub fn bind_tcp_listener(address: SocketAddr) -> io::Result<TcpListener> {
    let socket = bind(address, Type::STREAM, Protocol::TCP)?;
    socket.listen(128)?;
    Ok(socket.into())
}

pub fn bind_udp_socket(address: SocketAddr) -> io::Result<UdpSocket> {
    Ok(bind(address, Type::DGRAM, Protocol::UDP)?.into())
}
