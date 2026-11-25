//! Real tutor roster + real student runtime: remote file manager, process kill and Collect Work.
use erg_proto::{Command, Event};
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

async fn next<T>(rx: &mut tokio::sync::broadcast::Receiver<RosterEvent>, f: impl Fn(RosterEvent) -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(v) = f(rx.recv().await.unwrap()) {
                break v;
            }
        }
    })
    .await
    .expect("timed out waiting for event")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_manager_and_collect_work_end_to_end() {
    let (key, port) = ([9u8; 32], 47840u16);
    let root = std::env::temp_dir().join("erg-remote-files");
    let _ = std::fs::remove_dir_all(&root);
    let docs = root.join("student").join("Documents");
    std::fs::create_dir_all(docs.join("Bai1")).unwrap();
    std::fs::write(docs.join("Bai1").join("hs.docx"), b"bai lam cua em").unwrap();
    std::fs::write(docs.join("Bai1").join("ghi-chu.txt"), b"x").unwrap();

    let roster = Roster::start(port, key, Some("t".into())).unwrap();
    roster.set_collect_dir(Some(root.join("collected")));
    let mut rx = roster.subscribe();
    let cfg = Config {
        key, room: "t".into(), hostname: "PC-01".into(), user: "hs".into(), discovery_port: port,
        browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: docs.join("ERG").join("Received"),
    };
    tokio::spawn(run(cfg, Arc::new(Quiet), None));
    let id = next(&mut rx, |e| if let RosterEvent::Online { id, .. } = e { Some(id) } else { None }).await;

    // list
    roster.send(Some(&[id]), Command::ListDir(docs.join("Bai1").to_str().unwrap().into()));
    let Event::DirListing { entries, .. } = next(&mut rx, |e| if let RosterEvent::Student { event: ev @ Event::DirListing { .. }, .. } = e { Some(ev) } else { None }).await else { unreachable!() };
    assert_eq!(entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["ghi-chu.txt", "hs.docx"]);

    // a forbidden operation is refused with a message, not executed
    roster.send(Some(&[id]), Command::DeletePath("C:\\Windows".into()));
    let ev = next(&mut rx, |e| if let RosterEvent::Student { event: ev @ Event::FsResult { .. }, .. } = e { Some(ev) } else { None }).await;
    assert!(matches!(ev, Event::FsResult { ok: false, .. }));

    // collect only *.docx
    roster.send(Some(&[id]), Command::CollectFiles { folder: "Bai1".into(), pattern: "*.docx".into() });
    let path = next(&mut rx, |e| if let RosterEvent::Collected { path, .. } = e { Some(path) } else { None }).await;
    assert_eq!(std::fs::read(&path).unwrap(), b"bai lam cua em");
    assert!(path.to_string_lossy().contains("PC-01"), "kept per host: {path:?}");
    assert!(!root.join("collected").join("PC-01").join("ghi-chu.txt").exists(), "pattern must filter");

    // the summary of the collect above arrives right after the file; drain it first
    let ev = next(&mut rx, |e| if let RosterEvent::Student { event: ev @ Event::FsResult { .. }, .. } = e { Some(ev) } else { None }).await;
    assert_eq!(ev, Event::FsResult { ok: true, message: "đã gửi 1/1 tệp".into() });

    // collecting outside Documents is refused
    roster.send(Some(&[id]), Command::CollectFiles { folder: "..".into(), pattern: "*".into() });
    let ev = next(&mut rx, |e| if let RosterEvent::Student { event: ev @ Event::FsResult { .. }, .. } = e { Some(ev) } else { None }).await;
    assert!(matches!(ev, Event::FsResult { ok: false, .. }));
}
