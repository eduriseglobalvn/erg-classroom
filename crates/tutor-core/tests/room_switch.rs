use erg_tutor_core::Roster;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// `reset` to another room drops the students and keeps them out; back to their room (also the "refresh" case) they return.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reset_leaves_the_room_and_comes_back() {
    let key = [5u8; 32];
    let port = 47833;
    let roster = Roster::start(port, key, Some("a".into())).expect("roster");
    let sim = erg_sim::spawn_students(3, key, "a", port, Some(format!("127.0.0.1:{port}").parse().unwrap())).await.expect("sim");
    let connected = |n: usize| {
        let c = &sim.connected;
        async move {
            for _ in 0..300 {
                if c.load(Ordering::SeqCst) >= n { return true; }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            false
        }
    };
    assert!(connected(3).await, "all three join room a");

    roster.reset(Some("b".into()));
    assert!(roster.online_ids().is_empty());
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(roster.online_ids().is_empty(), "students of room a stay out of room b");
    assert_eq!(sim.connected.load(Ordering::SeqCst), 0, "their sessions were closed");

    roster.reset(Some("a".into()));
    assert!(connected(3).await, "all three come back");
    assert_eq!(roster.online_ids().len(), 3);
}
