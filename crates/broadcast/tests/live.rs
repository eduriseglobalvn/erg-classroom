//! The real pipeline: screen -> H.264 -> multicast -> Receiver, for both a full screen and one window.
#![cfg(windows)]
use erg_broadcast::{foreground_window, start, Source, Target};
use erg_show::{udp, Receiver};
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

/// Desktop Duplication allows few simultaneous captures of one screen, so these tests take turns.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn receive(source: Source, port: u16) -> (usize, usize, usize) {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let iface = udp::default_interface().expect("lan interface");
    let group = [239, 255, 77, 31];
    let key = [3u8; 16];
    let sock = udp::receiver(Ipv4Addr::from(group), port, iface).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    let h = start(source, Target { group, port, key, stream: 9 }).expect("start");
    let (mut rx, mut buf, mut frames, mut keyframes, mut bytes) = (Receiver::new(&key, 9), [0u8; 2048], 0, 0, 0);
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        if let Ok(n) = sock.recv(&mut buf) {
            if let Some(f) = rx.push(&buf[..n]) {
                frames += 1;
                keyframes += f.keyframe as usize;
                bytes += f.data.len();
            }
        }
    }
    assert!(h.running());
    h.stop();
    (frames, keyframes, bytes)
}

#[test]
fn full_screen_stream_arrives() {
    let (frames, keys, bytes) = receive(Source::Screen, 47950);
    println!("screen: {frames} frames, {keys} keyframes, {bytes} bytes");
    assert!(frames >= 1 && keys >= 1 && bytes > 1000);
}

#[test]
fn single_window_stream_arrives() {
    let Some(hw) = foreground_window() else { return }; // no foreground window (headless session)
    let (frames, keys, _) = receive(Source::Window(hw), 47951);
    println!("window: {frames} frames, {keys} keyframes");
    assert!(frames >= 1 && keys >= 1);
}

#[test]
fn a_missing_window_is_reported_before_anyone_joins() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let r = start(Source::Window(0x7FFF_FFFF), Target { group: [239, 255, 77, 32], port: 47952, key: [0; 16], stream: 1 });
    assert!(r.is_err());
}
