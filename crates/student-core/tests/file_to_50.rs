use erg_student_core::{run, Config, Executor};
use erg_proto::{AppPolicy, PowerAction, WebPolicy};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Counter(AtomicUsize);
impl Executor for Counter {
    fn open_url(&self, _: &str) {}
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: PowerAction) {}
    fn set_web_policy(&self, _: WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: AppPolicy, _: &[String]) {}
    fn file_received(&self, _: &Path) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tutor_sends_a_file_to_50_real_student_runtimes() {
    let (key, port) = ([11u8; 32], 47850u16);
    let root: PathBuf = std::env::temp_dir().join(format!("erg-file50-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let counter = Arc::new(Counter::default());
    for i in 0..50 {
        let cfg = Config {
            key,
            room: "f".into(),
            hostname: format!("PC-{i:02}"),
            user: format!("hs{i}"),
            discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()],
            inbox: root.join(format!("inbox{i:02}")),
        };
        tokio::spawn(run(cfg, counter.clone(), None));
    }
    let roster = erg_tutor_core::Roster::start(port, key, Some("f".into())).unwrap();

    let src = root.join("de-kiem-tra.bin");
    let data: Vec<u8> = (0..8_000_000u32).map(|i| (i.wrapping_mul(2246822519) >> 23) as u8).collect();
    std::fs::write(&src, &data).unwrap();

    // Wait until all 50 have authenticated (their control connection is up).
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut results = Vec::new();
    while Instant::now() < deadline {
        results = roster.send_file(None, &src).await;
        if results.len() == 50 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        // Partial rounds would re-deliver to those already served; discard what the early round wrote.
        for e in std::fs::read_dir(&root).unwrap().flatten() {
            if e.file_name().to_string_lossy().starts_with("inbox") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        counter.0.store(0, Ordering::SeqCst);
    }
    assert_eq!(results.len(), 50, "only {} students were connected", results.len());
    let failed: Vec<_> = results.iter().filter(|(_, r)| r.is_err()).collect();
    assert!(failed.is_empty(), "failed transfers: {failed:?}");

    for _ in 0..100 {
        if counter.0.load(Ordering::SeqCst) == 50 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(counter.0.load(Ordering::SeqCst), 50, "students reporting a received file");
    for i in 0..50 {
        let got = std::fs::read(root.join(format!("inbox{i:02}")).join("de-kiem-tra.bin")).unwrap_or_default();
        assert!(got == data, "student {i} has a wrong or missing file ({} bytes)", got.len());
    }
    let _ = std::fs::remove_dir_all(&root);
}
