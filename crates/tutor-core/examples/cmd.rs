//! Dev tool: wait for one student, send commands given as JSON arguments, wait, exit.
//! `ERG_ROOM_KEY=<64 hex> cargo run -p erg-tutor-core --example cmd -- '{"Lock":{"blank":false,"text":"hi"}}' 5 '"Unlock"'`
//! A bare number means "sleep that many seconds before the next command".
use erg_proto::Command;
use erg_tutor_core::{Roster, RosterEvent};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let hex = std::env::var("ERG_ROOM_KEY").expect("ERG_ROOM_KEY");
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex key");
    }
    let roster = Roster::start(erg_proto::DISCOVERY_PORT, key, None).expect("roster");
    let mut rx = roster.subscribe();
    loop {
        if let RosterEvent::Online { .. } = rx.recv().await.expect("events") {
            break;
        }
    }
    for a in std::env::args().skip(1) {
        if let Ok(s) = a.parse::<u64>() {
            tokio::time::sleep(Duration::from_secs(s)).await;
        } else {
            let c: Command = serde_json::from_str(&a).expect("command json");
            println!("sent to {}", roster.send(None, c));
        }
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
}
