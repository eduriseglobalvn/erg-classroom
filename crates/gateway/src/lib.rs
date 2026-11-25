//! Name server for schools whose rooms sit on different subnets (multicast / broadcast do not
//! cross routers). Students send their normal beacon to the gateway; tutors subscribe and are
//! sent every fresh beacon together with the student's address. It only introduces machines:
//! control still runs tutor <-> student over QUIC with the Room Key, so a gateway sees host names
//! but cannot command anybody.
//!
//! shortcut: UDP only, no NAT traversal and no relaying of control traffic. Students must be
//! directly reachable from the tutor (routed LAN / VPN). Upgrade to a QUIC relay for the Internet.
use erg_proto::{decode, encode, Beacon, GatewayMsg};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use uuid::Uuid;

const BEACON_TTL: Duration = Duration::from_secs(15);
const SUBSCRIBER_TTL: Duration = Duration::from_secs(40);
pub const MAX_BEACONS: usize = 5000;
pub const MAX_SUBSCRIBERS: usize = 100;

#[derive(Default)]
pub struct Table {
    beacons: HashMap<Uuid, (Beacon, [u8; 4], Instant)>,
    subscribers: HashMap<SocketAddr, (Option<String>, Instant)>,
}

impl Table {
    /// Record a datagram. Returns false when it was ignored (junk, IPv6 source, table full).
    pub fn accept(&mut self, pkt: &[u8], from: SocketAddr, now: Instant) -> bool {
        if let Ok(Some((GatewayMsg::Subscribe { room }, _))) = decode::<GatewayMsg>(pkt) {
            if self.subscribers.len() >= MAX_SUBSCRIBERS && !self.subscribers.contains_key(&from) {
                return false;
            }
            self.subscribers.insert(from, (room, now));
            return true;
        }
        if let Ok(Some((b, _))) = decode::<Beacon>(pkt) {
            let SocketAddr::V4(v4) = from else { return false };
            if self.beacons.len() >= MAX_BEACONS && !self.beacons.contains_key(&b.student_id) {
                return false;
            }
            self.beacons.insert(b.student_id, (b, v4.ip().octets(), now));
            return true;
        }
        false
    }

    pub fn expire(&mut self, now: Instant) {
        self.beacons.retain(|_, (_, _, t)| now.duration_since(*t) < BEACON_TTL);
        self.subscribers.retain(|_, (_, t)| now.duration_since(*t) < SUBSCRIBER_TTL);
    }

    /// (subscriber, datagram) pairs to send now.
    pub fn outgoing(&self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut v = vec![];
        for (sub, (room, _)) in &self.subscribers {
            for (b, ip, _) in self.beacons.values() {
                if room.as_ref().is_some_and(|r| *r != b.room) {
                    continue;
                }
                if let Ok(p) = encode(&GatewayMsg::Relayed { ip: *ip, beacon: b.clone() }) {
                    v.push((*sub, p));
                }
            }
        }
        v
    }

    pub fn counts(&self) -> (usize, usize) {
        (self.beacons.len(), self.subscribers.len())
    }
}

/// Serve until the task is dropped. Returns the bound address through `ready`.
pub async fn run(bind: SocketAddr, ready: Option<tokio::sync::oneshot::Sender<SocketAddr>>) -> std::io::Result<()> {
    let sock = UdpSocket::bind(bind).await?;
    if let Some(r) = ready {
        let _ = r.send(sock.local_addr()?);
    }
    let mut table = Table::default();
    let mut buf = [0u8; 2048];
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            r = sock.recv_from(&mut buf) => {
                let (n, from) = r?;
                table.accept(&buf[..n], from, Instant::now());
            }
            _ = tick.tick() => {
                table.expire(Instant::now());
                for (to, pkt) in table.outgoing() {
                    let _ = sock.send_to(&pkt, to).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beacon(room: &str) -> Beacon {
        Beacon { proto_ver: 1, student_id: Uuid::new_v4(), hostname: "PC".into(), user: "u".into(), room: room.into(), quic_port: 1234, cert_fingerprint: [0; 32], mac: None }
    }
    fn from(ip: [u8; 4]) -> SocketAddr { SocketAddr::from((ip, 5000)) }

    #[test]
    fn subscribers_get_fresh_beacons_filtered_by_room_and_stale_ones_expire() {
        let (mut t, now) = (Table::default(), Instant::now());
        assert!(t.accept(&encode(&beacon("A")).unwrap(), from([10, 0, 1, 5]), now));
        assert!(t.accept(&encode(&beacon("B")).unwrap(), from([10, 0, 2, 5]), now));
        assert!(t.accept(&encode(&GatewayMsg::Subscribe { room: Some("A".into()) }).unwrap(), from([10, 0, 9, 9]), now));
        let out = t.outgoing();
        assert_eq!(out.len(), 1);
        let Ok(Some((GatewayMsg::Relayed { ip, beacon: b }, _))) = decode::<GatewayMsg>(&out[0].1) else { panic!() };
        assert_eq!((ip, b.room.as_str()), ([10, 0, 1, 5], "A"));

        t.accept(&encode(&GatewayMsg::Subscribe { room: None }).unwrap(), from([10, 0, 9, 8]), now);
        assert_eq!(t.outgoing().len(), 1 + 2, "a subscriber without a room filter gets both rooms");

        t.expire(now + Duration::from_secs(20));
        assert_eq!(t.counts(), (0, 2), "beacons expire before subscribers do");
        t.expire(now + Duration::from_secs(60));
        assert_eq!(t.counts(), (0, 0));
    }

    #[test]
    fn junk_and_floods_are_ignored() {
        let (mut t, now) = (Table::default(), Instant::now());
        assert!(!t.accept(b"hello", from([1, 1, 1, 1]), now));
        assert!(!t.accept(&[], from([1, 1, 1, 1]), now));
        assert!(!t.accept(&encode(&beacon("A")).unwrap(), "[::1]:1".parse().unwrap(), now), "IPv6 sources cannot be relayed");
        for i in 0..MAX_SUBSCRIBERS + 10 {
            t.accept(&encode(&GatewayMsg::Subscribe { room: None }).unwrap(), from([10, 1, (i / 200) as u8, (i % 200) as u8]), now);
        }
        assert_eq!(t.counts().1, MAX_SUBSCRIBERS);
        for _ in 0..MAX_BEACONS + 10 {
            t.accept(&encode(&beacon("A")).unwrap(), from([10, 0, 0, 1]), now);
        }
        assert_eq!(t.counts().0, MAX_BEACONS);
    }
}
