//! Tutor-side roster: listens for student beacons, keeps one authenticated QUIC control
//! channel per student, and fans commands out to a selection (or the whole class).
use erg_net::{discovery, tutor_endpoint, tutor_connect, RoomKey};
use erg_proto::{Beacon, Command, Event};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

pub mod assistant;
pub mod board;
pub mod classroom;
pub mod journal;
pub mod leaders;
pub mod recorder;
pub mod report;

pub const MAX_PARALLEL_TRANSFERS: usize = 10;
/// Students served by unicast at once when their multicast fails.
pub const MAX_SHOW_FALLBACK: usize = 20;
/// Largest H.264 access unit sent by unicast.
pub const MAX_SHOW_FRAME: usize = 2 << 20;

#[derive(Debug, Clone)]
pub enum RosterEvent {
    Online { id: Uuid, beacon: Beacon },
    Offline { id: Uuid },
    Student { id: Uuid, event: Event },
    /// A student pushed a file (Collect Work); saved at `path`.
    Collected { id: Uuid, path: std::path::PathBuf },
}

/// Largest unsent Show backlog per student; past it only keyframes get in (up to twice this).
pub const MAX_SHOW_BACKLOG: usize = 8 << 20;

/// Unicast Show backlog of one student (bytes queued but not yet written to its connection).
#[derive(Default)]
struct ShowQ {
    pending: std::sync::atomic::AtomicUsize,
    /// A frame was skipped: delta frames are useless until the next keyframe gets through.
    need_key: std::sync::atomic::AtomicBool,
}

impl ShowQ {
    /// Decide whether a frame may be queued; counts it when it may.
    fn admit(&self, keyframe: bool, len: usize) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        let backlog = self.pending.load(Relaxed);
        let ok = if keyframe { backlog <= 2 * MAX_SHOW_BACKLOG } else { !self.need_key.load(Relaxed) && backlog <= MAX_SHOW_BACKLOG };
        if ok {
            if keyframe { self.need_key.store(false, Relaxed); }
            self.pending.fetch_add(len, Relaxed);
        } else {
            self.need_key.store(true, Relaxed);
        }
        ok
    }
}

struct Entry {
    q: Arc<ShowQ>,
    beacon: Beacon,
    tx: mpsc::UnboundedSender<Command>,
    conn: Option<quinn::Connection>,
}

#[derive(Clone)]
pub struct Roster {
    inner: Arc<Mutex<HashMap<Uuid, Entry>>>,
    events: broadcast::Sender<RosterEvent>,
    /// Where files pushed by students are saved, in one sub-folder per host. None = refuse them.
    collect_dir: Arc<Mutex<Option<std::path::PathBuf>>>,
    /// Students that asked for the Show over their QUIC connection.
    fallback: Arc<Mutex<std::collections::HashSet<Uuid>>>,
    /// Signs privileged commands; `last_priv_ms` keeps the signing times strictly increasing.
    key: RoomKey,
    last_priv_ms: Arc<Mutex<u64>>,
    /// Only students whose beacon carries this room are connected. None = every room.
    room: Arc<Mutex<Option<String>>>,
}

impl Roster {
    /// Start listening for beacons on `discovery_port`. Students whose beacon carries a
    /// different room than `room` (when set) are ignored.
    pub fn start(discovery_port: u16, key: RoomKey, room: Option<String>) -> std::io::Result<Self> {
        let sock = discovery::listener(discovery_port)?;
        let (events, _) = broadcast::channel(1024);
        let me = Self { inner: Default::default(), events, collect_dir: Default::default(), fallback: Default::default(), key, last_priv_ms: Default::default(), room: Arc::new(Mutex::new(room)) };
        let r = me.clone();
        tokio::spawn(async move {
            let Ok(ep) = tutor_endpoint() else { return };
            while let Ok((b, from)) = discovery::next_beacon(&sock).await {
                if !r.in_room(&b.room) {
                    continue;
                }
                let known = r.inner.lock().expect("roster lock").contains_key(&b.student_id);
                if known {
                    continue;
                }
                let addr = SocketAddr::new(from.ip(), b.quic_port);
                r.spawn_session(ep.clone(), addr, b, key);
            }
        });
        Ok(me)
    }

    /// Also learn about students through an `erg-gateway` (rooms on other subnets). Safe to call
    /// several times with different gateways. The gateway only introduces students; the Room Key
    /// check on the QUIC connection still decides who can be controlled.
    pub fn add_gateway(&self, gateway: SocketAddr, key: RoomKey) {
        let r = self.clone();
        tokio::spawn(async move {
            let Ok(ep) = tutor_endpoint() else { return };
            let Ok(sock) = tokio::net::UdpSocket::bind("0.0.0.0:0").await else { return };
            let mut tick = tokio::time::interval(Duration::from_secs(10));
            let mut buf = [0u8; 2048];
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        // built every time: the room can change while running (`reset`)
                        let room = r.room.lock().expect("room lock").clone();
                        let hello = erg_proto::encode(&erg_proto::GatewayMsg::Subscribe { room }).expect("encode");
                        let _ = sock.send_to(&hello, gateway).await;
                    }
                    got = sock.recv_from(&mut buf) => {
                        let Ok((n, from)) = got else { continue };
                        if from != gateway { continue; } // only the gateway we subscribed to may introduce students
                        if let Ok(Some((erg_proto::GatewayMsg::Relayed { ip, beacon }, _))) = erg_proto::decode::<erg_proto::GatewayMsg>(&buf[..n]) {
                            if !r.in_room(&beacon.room) { continue; }
                            if r.inner.lock().expect("roster lock").contains_key(&beacon.student_id) { continue; }
                            let addr = SocketAddr::new(std::net::Ipv4Addr::from(ip).into(), beacon.quic_port);
                            r.spawn_session(ep.clone(), addr, beacon, key);
                        }
                    }
                }
            }
        });
    }

    fn in_room(&self, room: &str) -> bool {
        self.room.lock().expect("room lock").as_ref().is_none_or(|x| x == room)
    }

    /// Change room (None = every room) and disconnect everyone: students of the room reconnect on their next
    /// beacon. With the room unchanged this is "refresh the list of students".
    pub fn reset(&self, room: Option<String>) {
        *self.room.lock().expect("room lock") = room;
        self.fallback.lock().expect("fallback lock").clear();
        self.inner.lock().expect("roster lock").clear(); // dropping an entry closes its session
    }

    fn spawn_session(&self, ep: quinn::Endpoint, addr: SocketAddr, b: Beacon, key: RoomKey) {
        let (tx, mut rx) = mpsc::unbounded_channel::<Command>();
        let id = b.student_id;
        let q = Arc::new(ShowQ::default());
        // Reserve the slot first so repeated beacons do not start duplicate sessions.
        self.inner.lock().expect("roster lock").insert(id, Entry { q: q.clone(), beacon: b.clone(), tx, conn: None });
        let this = self.clone();
        tokio::spawn(async move {
            match tutor_connect(&ep, addr, &key).await {
                Ok((conn, mut ch)) => {
                    if let Some(e) = this.inner.lock().expect("roster lock").get_mut(&id) {
                        e.conn = Some(conn.clone());
                    }
                    // Collect Work: students push files on their own streams.
                    let (cconn, cthis, host) = (conn.clone(), this.clone(), b.hostname.clone());
                    tokio::spawn(async move {
                        while let Ok(rx) = cconn.accept_uni().await {
                            let Some(root) = cthis.collect_dir.lock().expect("collect lock").clone() else { continue };
                            let sub = erg_net::transfer::safe_name(&host).unwrap_or_else(|| "student".into());
                            let t = cthis.clone();
                            tokio::spawn(async move {
                                if let Ok(path) = erg_net::transfer::receive_file(rx, &root.join(sub)).await {
                                    let _ = t.events.send(RosterEvent::Collected { id, path });
                                }
                            });
                        }
                    });
                    let _ = this.events.send(RosterEvent::Online { id, beacon: b });
                    loop {
                        tokio::select! {
                            cmd = rx.recv() => match cmd {
                                Some(c) => {
                                    let sent = ch.send(&c).await.is_err();
                                    if let Command::ShowFrame { data, .. } = &c { q.pending.fetch_sub(data.len(), std::sync::atomic::Ordering::Relaxed); }
                                    if sent { break }
                                }
                                None => break,
                            },
                            ev = ch.recv() => match ev {
                                Ok(e) => {
                                    if matches!(e, Event::ShowFallbackRequest) {
                                        let mut f = this.fallback.lock().expect("fallback lock");
                                        if f.len() < MAX_SHOW_FALLBACK { f.insert(id); }
                                    } else if matches!(e, Event::ShowFallbackEnd) {
                                        this.fallback.lock().expect("fallback lock").remove(&id);
                                    }
                                    let _ = this.events.send(RosterEvent::Student { id, event: e }); }
                                Err(_) => break,
                            },
                        }
                    }
                    conn.close(0u32.into(), b"bye");
                    let _ = this.events.send(RosterEvent::Offline { id });
                }
                Err(e) => tracing::warn!("student {id} connect failed: {e}"),
            }
            // Drop the slot so a later beacon can reconnect after a short back-off.
            tokio::time::sleep(Duration::from_secs(2)).await;
            // only our own slot: after `reset` the student may already have a new session under the same id
            let mut map = this.inner.lock().expect("roster lock");
            if map.get(&id).is_some_and(|e| Arc::ptr_eq(&e.q, &q)) { map.remove(&id); }
        });
    }

    pub fn set_collect_dir(&self, dir: Option<std::path::PathBuf>) {
        *self.collect_dir.lock().expect("collect lock") = dir;
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RosterEvent> {
        self.events.subscribe()
    }

    pub fn online_ids(&self) -> Vec<Uuid> {
        self.inner.lock().expect("roster lock").keys().copied().collect()
    }

    /// Web/app/usb/print/protect/power commands leave the tutor only as `Command::Priv`, signed with the Room Key,
    /// so the student's SYSTEM service can tell them from anything the student's own processes could make up.
    /// None = the command is invalid (too long, bad characters) and is not sent.
    fn sign_privileged(&self, cmd: Command) -> Option<Command> {
        use erg_proto::PrivAction as P;
        let action = match cmd {
            Command::SetWebPolicy { mode, list } => P::Web(mode, list),
            Command::SetAppPolicy { mode, list } => P::App(mode, list),
            Command::SetUsbPolicy(p) => P::Usb(p),
            Command::SetPrintPolicy(p) => P::Print(p),
            Command::SetProtect(p) => P::Protect(p),
            Command::Power(p) => P::Power(p),
            other => return Some(other),
        };
        let issued = { let mut l = self.last_priv_ms.lock().ok()?; *l = erg_privcmd::now_ms().max(*l + 1); *l };
        erg_privcmd::sign(&self.key, &action, issued).ok().map(Command::Priv)
    }

    /// Send `cmd` to the given students, or to everyone when `ids` is None. Returns how many were addressed.
    pub fn send(&self, ids: Option<&[Uuid]>, cmd: Command) -> usize {
        let Some(cmd) = self.sign_privileged(cmd) else { return 0 };
        if matches!(cmd, Command::StopShow | Command::StartShow { .. }) {
            self.fallback.lock().expect("fallback lock").clear(); // a new or ended Show starts multicast-first again
        }
        let map = self.inner.lock().expect("roster lock");
        map.iter()
            .filter(|(id, _)| ids.is_none_or(|s| s.contains(id)))
            .filter(|(_, e)| e.tx.send(cmd.clone()).is_ok())
            .count()
    }

    /// Unicast one Show frame to the students that asked for the fallback. Returns how many got it.
    pub fn show_frame(&self, keyframe: bool, data: &[u8]) -> usize {
        if data.len() > MAX_SHOW_FRAME { return 0; }
        let ids: Vec<Uuid> = self.fallback.lock().expect("fallback lock").iter().copied().collect();
        let map = self.inner.lock().expect("roster lock");
        let cmd = Command::ShowFrame { keyframe, data: data.to_vec() };
        let mut n = 0;
        for e in ids.iter().filter_map(|id| map.get(id)) {
            if !e.q.admit(keyframe, data.len()) { continue; } // stalled student: skip, it resyncs on the next keyframe
            if e.tx.send(cmd.clone()).is_ok() { n += 1 } else { e.q.pending.fetch_sub(data.len(), std::sync::atomic::Ordering::Relaxed); }
        }
        n
    }

    /// Send one file to the given students (or all). At most `MAX_PARALLEL_TRANSFERS` run at once so a
    /// 50-machine class does not saturate the tutor's uplink. Returns one result per addressed student.
    pub async fn send_file(&self, ids: Option<&[Uuid]>, path: &std::path::Path) -> Vec<(Uuid, Result<(), String>)> {
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            return vec![];
        };
        let targets: Vec<(Uuid, quinn::Connection)> = {
            let map = self.inner.lock().expect("roster lock");
            map.iter()
                .filter(|(id, _)| ids.is_none_or(|s| s.contains(id)))
                .filter_map(|(id, e)| e.conn.clone().map(|c| (*id, c)))
                .collect()
        };
        let gate = Arc::new(tokio::sync::Semaphore::new(MAX_PARALLEL_TRANSFERS));
        let mut set = tokio::task::JoinSet::new();
        for (id, conn) in targets {
            let (gate, path, name) = (gate.clone(), path.to_path_buf(), name.clone());
            set.spawn(async move {
                let _permit = gate.acquire_owned().await.expect("semaphore open");
                (id, erg_net::transfer::send_file(&conn, &path, &name).await.map_err(|e| e.to_string()))
            });
        }
        let mut out = Vec::new();
        while let Some(r) = set.join_next().await {
            if let Ok(x) = r {
                out.push(x);
            }
        }
        out
    }

    pub fn beacon(&self, id: Uuid) -> Option<Beacon> {
        self.inner.lock().expect("roster lock").get(&id).map(|e| e.beacon.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stalled_student_backlog_is_bounded_and_resyncs_on_keyframe() {
        let q = ShowQ::default(); // nothing ever drains it: a stalled connection
        let f = 1 << 20;
        assert!(q.admit(true, f));
        let mut admitted = 1;
        while q.admit(false, f) { admitted += 1; assert!(admitted < 100, "unbounded"); }
        assert!(admitted * f <= MAX_SHOW_BACKLOG + f);
        assert!(!q.admit(false, 1)); // deltas stay out even if the backlog drained, they would decode wrongly
        q.pending.store(0, std::sync::atomic::Ordering::Relaxed);
        assert!(!q.admit(false, 1));
        assert!(q.admit(true, f)); // the latest keyframe always gets in once there is room
        assert!(q.admit(false, f)); // and deltas resume after it
        q.pending.store(3 * MAX_SHOW_BACKLOG, std::sync::atomic::Ordering::Relaxed);
        assert!(!q.admit(true, f)); // hard cap
    }

    #[tokio::test]
    async fn privileged_commands_leave_signed_and_increasing_everything_else_unchanged() {
        use erg_proto::{PowerAction, PrivAction, UsbPolicy};
        let key = [3u8; 32];
        let r = Roster::start(47890, key, None).unwrap();
        assert_eq!(r.sign_privileged(Command::Beep), Some(Command::Beep));
        let (a, b) = (r.sign_privileged(Command::Power(PowerAction::Logoff)), r.sign_privileged(Command::SetUsbPolicy(UsbPolicy::Allow)));
        let (Some(Command::Priv(a)), Some(Command::Priv(b))) = (a, b) else { panic!("not signed") };
        assert!(b.issued > a.issued, "strictly increasing even within one millisecond");
        let mut gate = erg_privcmd::Gate::default();
        assert_eq!(gate.admit(&key, &a, a.issued), Ok(PrivAction::Power(PowerAction::Logoff)));
        assert!(gate.admit(&[0; 32], &b, b.issued).is_err());
        assert!(r.sign_privileged(Command::SetWebPolicy { mode: erg_proto::WebPolicy::BlockList, list: vec!["a\nb".into()] }).is_none());
    }
}
