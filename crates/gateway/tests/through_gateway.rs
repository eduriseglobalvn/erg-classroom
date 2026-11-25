//! A real student that only knows the gateway's address is found and commanded by a tutor that
//! never hears the student's own beacons (different discovery port = "other subnet").
use erg_proto::Command;
use erg_student_core::{run, Config, Executor};
use erg_tutor_core::{Roster, RosterEvent};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Rec(Mutex<Vec<String>>);
impl Executor for Rec {
    fn open_url(&self, u: &str) { self.0.lock().unwrap().push(u.into()) }
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: erg_proto::PowerAction) {}
    fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn student_behind_the_gateway_is_discovered_and_controlled() {
    let key = [11u8; 32];
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(erg_gateway::run("127.0.0.1:0".parse().unwrap(), Some(tx)));
    let gw = rx.await.unwrap();

    // the student announces to the gateway only; its LAN discovery port has no listener at all
    let rec = Arc::new(Rec::default());
    let cfg = Config {
        key, room: "r1".into(), hostname: "PC-GW".into(), user: "hs".into(), discovery_port: 47861,
        browse_addrs: vec![gw], inbox: std::env::temp_dir().join("erg-gw-inbox"),
    };
    let ex: Arc<dyn Executor> = rec.clone();
    tokio::spawn(run(cfg, ex, None));

    // the tutor listens on a different port, so LAN beacons never reach it
    let roster = Roster::start(47862, key, Some("r1".into())).unwrap();
    let mut events = roster.subscribe();
    roster.add_gateway(gw, key);
    let id = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let RosterEvent::Online { id, beacon } = events.recv().await.unwrap() {
                assert_eq!(beacon.hostname, "PC-GW");
                break id;
            }
        }
    })
    .await
    .expect("student never found through the gateway");

    roster.send(Some(&[id]), Command::OpenUrl("https://example.com/gw".into()));
    for _ in 0..50 {
        if !rec.0.lock().unwrap().is_empty() { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(rec.0.lock().unwrap().as_slice(), ["https://example.com/gw"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tutor_for_another_room_does_not_see_the_student() {
    let key = [12u8; 32];
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(erg_gateway::run("127.0.0.1:0".parse().unwrap(), Some(tx)));
    let gw = rx.await.unwrap();
    let cfg = Config {
        key, room: "r1".into(), hostname: "PC-X".into(), user: "hs".into(), discovery_port: 47863,
        browse_addrs: vec![gw], inbox: std::env::temp_dir().join("erg-gw-inbox2"),
    };
    tokio::spawn(run(cfg, Arc::new(Rec::default()), None));
    let roster = Roster::start(47864, key, Some("r2".into())).unwrap();
    let mut events = roster.subscribe();
    roster.add_gateway(gw, key);
    let seen = tokio::time::timeout(Duration::from_secs(8), async { loop { if let RosterEvent::Online { .. } = events.recv().await.unwrap() { break; } } }).await;
    assert!(seen.is_err(), "a student of room r1 showed up for a tutor of room r2");
}
