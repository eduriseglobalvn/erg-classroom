//! The licence's seat limit: a student over the limit is left out, and comes in when seats are added.
use erg_student_core::{run, Config, Executor};
use erg_tutor_core::{Roster, RosterEvent};
use std::sync::Arc;
use std::time::Duration;

struct Quiet;
impl Executor for Quiet {
    fn open_url(&self, _: &str) {}
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: erg_proto::PowerAction) {}
    fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
}

async fn wait(rx: &mut tokio::sync::broadcast::Receiver<RosterEvent>, seen: &mut (usize, usize), want: (usize, usize)) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while *seen != want {
            match rx.recv().await.unwrap() {
                RosterEvent::Online { .. } => seen.0 += 1,
                RosterEvent::Refused { waiting } => seen.1 = waiting,
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out at {seen:?}, want {want:?}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_student_over_the_seat_limit_waits_until_there_is_room() {
    let (key, port) = ([7u8; 32], 47866u16);
    let roster = Roster::start(port, key, Some("s".into())).unwrap();
    roster.set_seats(1);
    let mut rx = roster.subscribe();
    for host in ["PC-01", "PC-02"] {
        let cfg = Config {
            key, room: "s".into(), hostname: host.into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-seats"),
        };
        tokio::spawn(run(cfg, Arc::new(Quiet), None));
    }
    let mut seen = (0, 0); // (students online, students reported waiting)
    wait(&mut rx, &mut seen, (1, 1)).await;
    assert_eq!(roster.refused(), 1);
    roster.set_seats(2);
    wait(&mut rx, &mut seen, (2, 1)).await;
    assert_eq!(roster.online_ids().len(), 2);
    assert_eq!(roster.refused(), 0);
}
