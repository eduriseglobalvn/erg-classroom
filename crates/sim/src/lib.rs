//! Headless fake students for load tests: each one listens on its own QUIC port, announces
//! itself, authenticates the tutor, and records every command it receives.
use erg_net::{discovery, new_identity, student_accept, student_endpoint, RoomKey};
use erg_proto::{Beacon, Command, Event, PROTO_VERSION};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Default)]
pub struct Counters {
    pub connected: AtomicUsize,
    pub commands: AtomicUsize,
}

/// Spawn `count` students. Beacons go to `announce_to` (unicast) so tests do not depend on
/// multicast routing; pass `None` to use multicast + broadcast on `discovery_port`.
pub async fn spawn_students(
    count: usize,
    key: RoomKey,
    room: &str,
    discovery_port: u16,
    announce_to: Option<SocketAddr>,
) -> Result<Arc<Counters>, erg_net::Error> {
    let counters = Arc::new(Counters::default());
    for i in 0..count {
        let id = new_identity("erg-student")?;
        let ep = student_endpoint(&id, "0.0.0.0:0".parse().expect("addr"))?;
        let port = ep.local_addr()?.port();
        let beacon = Beacon {
            proto_ver: PROTO_VERSION,
            student_id: Uuid::new_v4(),
            hostname: format!("SIM-{i:03}"),
            user: format!("hs{i}"),
            room: room.to_string(),
            quic_port: port,
            cert_fingerprint: id.fingerprint,
            mac: None,
        };
        let c = counters.clone();
        tokio::spawn(async move {
            loop {
                let r = match announce_to {
                    Some(a) => discovery::announce_to(&beacon, a).await,
                    None => discovery::announce(&beacon, discovery_port).await,
                };
                let _ = r;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        tokio::spawn(async move {
            while let Some(incoming) = ep.accept().await {
                let c = c.clone();
                tokio::spawn(async move {
                    let Ok(conn) = incoming.await else { return };
                    let Ok(mut ch) = student_accept(&conn, &key).await else { return };
                    c.connected.fetch_add(1, Ordering::SeqCst);
                    while let Ok(cmd) = ch.recv().await {
                        c.commands.fetch_add(1, Ordering::SeqCst);
                        if let Command::RequestThumbnail { .. } = cmd {
                            let _ = ch.send(&Event::Thumbnail(vec![0xFF, 0xD8, 0xFF, 0xD9])).await;
                        }
                    }
                    c.connected.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
    }
    Ok(counters)
}
