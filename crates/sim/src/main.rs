use std::sync::atomic::Ordering;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(50);
    let key = [7u8; 32]; // dev key; real deployments use a per-school Room Key
    let c = erg_sim::spawn_students(count, key, "sim", erg_proto::DISCOVERY_PORT, None)
        .await
        .expect("spawn");
    println!("{count} simulated students running (Ctrl+C to stop)");
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!(
            "connected={} commands={}",
            c.connected.load(Ordering::SeqCst),
            c.commands.load(Ordering::SeqCst)
        );
    }
}
