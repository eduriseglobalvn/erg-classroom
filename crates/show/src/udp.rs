//! Multicast UDP sockets for the Show stream.
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

/// Local IPv4 the OS would use to reach the LAN. Connecting a UDP socket sends nothing.
pub fn default_interface() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?; // TEST-NET-1: only used to pick a route
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Sender bound to `iface`; TTL 1 keeps the stream inside the local subnet.
pub fn sender(iface: Ipv4Addr) -> io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.bind(&SocketAddrV4::new(iface, 0).into())?;
    s.set_multicast_if_v4(&iface)?;
    s.set_multicast_ttl_v4(1)?;
    s.set_multicast_loop_v4(true)?; // lets a tutor preview its own stream and enables single-host tests
    s.set_send_buffer_size(4 << 20)?;
    Ok(s.into())
}

/// Receiver joined to `group:port` on `iface`. A large buffer absorbs a keyframe burst.
pub fn receiver(group: Ipv4Addr, port: u16, iface: Ipv4Addr) -> io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    s.set_recv_buffer_size(8 << 20)?;
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    s.join_multicast_v4(&group, &iface)?;
    Ok(s.into())
}
