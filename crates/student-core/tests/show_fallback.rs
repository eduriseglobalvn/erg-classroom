//! A student that never receives the multicast (nobody sends on its group) reports stats, asks for the
//! unicast fallback after ~3 s, and then gets the Show frames over its QUIC connection.
use erg_proto::{Command, Event};
use erg_show::Frame;
use erg_student_core::{run, Config, Executor};
use erg_tutor_core::{Roster, RosterEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec(AtomicUsize);
impl Executor for Rec {
    fn open_url(&self, _: &str) {}
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: erg_proto::PowerAction) {}
    fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
    fn show_frame(&self, f: &Frame) {
        if f.data == [0, 0, 0, 1, 0x65] { self.0.fetch_add(1, Ordering::SeqCst); }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn silent_multicast_falls_back_to_unicast_frames() {
    let (key, port) = ([31u8; 32], 47861u16);
    let rec = Arc::new(Rec::default());
    let cfg = Config {
        key, room: "f".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
        browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-fallback-inbox"),
    };
    tokio::spawn(run(cfg, rec.clone(), None));
    let roster = Roster::start(port, key, Some("f".into())).unwrap();
    let mut events = roster.subscribe();
    let deadline = Instant::now() + Duration::from_secs(20);
    while roster.send(None, Command::StartShow { group: [239, 255, 77, 11], port: 47932, key: [1; 16], stream: 1 }) == 0 {
        assert!(Instant::now() < deadline, "student never connected");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let (mut stats, mut asked) = (false, false);
    while !(stats && asked) {
        let ev = tokio::time::timeout(Duration::from_secs(10), events.recv()).await.expect("no event in 10 s").unwrap();
        match ev {
            RosterEvent::Student { event: Event::ShowStats { .. }, .. } => stats = true,
            RosterEvent::Student { event: Event::ShowFallbackRequest, .. } => asked = true,
            _ => {}
        }
    }
    // The tutor registered the request; frames now flow by unicast (and an oversized one is dropped).
    assert_eq!(roster.show_frame(true, &vec![0; erg_tutor_core::MAX_SHOW_FRAME + 1]), 0);
    assert_eq!(roster.show_frame(true, &[0, 0, 0, 1, 0x65]), 1);
    for _ in 0..50 {
        if rec.0.load(Ordering::SeqCst) > 0 { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(rec.0.load(Ordering::SeqCst), 1);

    // A new Show starts multicast-first again.
    roster.send(None, Command::StopShow);
    assert_eq!(roster.show_frame(true, &[0, 0, 0, 1, 0x65]), 0);
}

/// After falling back, the multicast starts flowing cleanly: ~10 s later the student ends the fallback
/// and the tutor stops unicasting to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clean_multicast_returns_student_to_multicast() {
    let (key, port, show_port, group) = ([32u8; 32], 47862u16, 47933u16, [239u8, 255, 77, 12]);
    let cfg = Config {
        key, room: "g".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
        browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-fallback-inbox2"),
    };
    tokio::spawn(run(cfg, Arc::new(Rec::default()), None));
    let roster = Roster::start(port, key, Some("g".into())).unwrap();
    let mut events = roster.subscribe();
    let deadline = Instant::now() + Duration::from_secs(20);
    while roster.send(None, Command::StartShow { group, port: show_port, key: [1; 16], stream: 1 }) == 0 {
        assert!(Instant::now() < deadline, "student never connected");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(10), events.recv()).await.expect("no fallback request").unwrap();
        if matches!(ev, RosterEvent::Student { event: Event::ShowFallbackRequest, .. }) { break; }
    }
    assert_eq!(roster.show_frame(true, &[0, 0, 0, 1, 0x65]), 1);

    // Now the multicast works: 10 loopless-loss frames per second.
    let iface = erg_show::udp::default_interface().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
    let sock = erg_show::udp::sender(iface).unwrap();
    let tx = erg_show::Sender::new(&[1; 16], 1, 20);
    let sender = tokio::task::spawn_blocking(move || {
        for n in 0..200u32 {
            for p in tx.packetize(n, n % 30 == 0, &[0, 0, 0, 1, 0x65, 1, 2, 3]).unwrap() {
                let _ = sock.send_to(&p, (std::net::Ipv4Addr::from(group), show_port));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(25), events.recv()).await.expect("no ShowFallbackEnd").unwrap();
        if matches!(ev, RosterEvent::Student { event: Event::ShowFallbackEnd, .. }) { break; }
    }
    assert_eq!(roster.show_frame(true, &[0, 0, 0, 1, 0x65]), 0);
    sender.abort();
}
