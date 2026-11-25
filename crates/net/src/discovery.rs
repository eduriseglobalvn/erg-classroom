//! UDP beacon: students announce themselves via multicast + subnet broadcast; tutors listen.
use erg_proto::{decode, encode, Beacon, DISCOVERY_GROUP, DISCOVERY_PORT};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;

/// Send one beacon to `port` via multicast and broadcast. Either path may be filtered by the
/// network, so success on one of them is enough.
pub async fn announce(b: &Beacon, port: u16) -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.set_broadcast(true)?;
    let pkt = encode(b).map_err(|e| std::io::Error::other(e.to_string()))?;
    let grp = SocketAddrV4::new(Ipv4Addr::from(DISCOVERY_GROUP), port);
    let bc = SocketAddrV4::new(Ipv4Addr::BROADCAST, port);
    let a = sock.send_to(&pkt, grp).await;
    let c = sock.send_to(&pkt, bc).await;
    a.map(|_| ()).or_else(|_| c.map(|_| ()))
}

/// Unicast beacon to a fixed address (the "browse address" list for networks that block multicast).
pub async fn announce_to(b: &Beacon, to: SocketAddr) -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let pkt = encode(b).map_err(|e| std::io::Error::other(e.to_string()))?;
    sock.send_to(&pkt, to).await.map(|_| ())
}

pub fn announce_default(b: &Beacon) -> impl std::future::Future<Output = std::io::Result<()>> + '_ {
    announce(b, DISCOVERY_PORT)
}

pub fn listener(port: u16) -> std::io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    s.set_nonblocking(true)?;
    s.bind(&SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).into())?;
    let _ = s.join_multicast_v4(&Ipv4Addr::from(DISCOVERY_GROUP), &Ipv4Addr::UNSPECIFIED);
    UdpSocket::from_std(s.into())
}

pub async fn next_beacon(sock: &UdpSocket) -> std::io::Result<(Beacon, SocketAddr)> {
    let mut buf = [0u8; 2048];
    loop {
        let (n, from) = sock.recv_from(&mut buf).await?;
        if let Ok(Some((b, _))) = decode::<Beacon>(&buf[..n]) {
            return Ok((b, from));
        }
    }
}
