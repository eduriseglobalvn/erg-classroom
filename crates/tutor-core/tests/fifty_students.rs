use erg_proto::Command;
use erg_tutor_core::Roster;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

async fn wait_for(mut f: impl FnMut() -> bool, secs: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tutor_connects_to_50_students_and_broadcasts() {
    let key = [9u8; 32];
    let port = 47811;
    let roster = Roster::start(port, key, Some("t".into())).expect("roster");
    let sim = erg_sim::spawn_students(50, key, "t", port, Some(format!("127.0.0.1:{port}").parse().unwrap()))
        .await
        .expect("sim");

    assert!(wait_for(|| sim.connected.load(Ordering::SeqCst) == 50, 30).await, "all 50 authenticate");

    let sent = roster.send(None, Command::OpenUrl("https://example.com".into()));
    assert_eq!(sent, 50);
    assert!(wait_for(|| sim.commands.load(Ordering::SeqCst) == 50, 10).await, "all 50 receive the command");

    // Selection: only three students.
    let some: Vec<_> = roster.online_ids().into_iter().take(3).collect();
    assert_eq!(roster.send(Some(&some), Command::Unlock), 3);
    assert!(wait_for(|| sim.commands.load(Ordering::SeqCst) == 53, 10).await);
}
