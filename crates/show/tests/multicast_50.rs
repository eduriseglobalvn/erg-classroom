//! Real UDP multicast: one sender, 50 receivers on this host, frames paced at 30 fps.
use erg_show::{udp, Receiver, Sender};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn fifty_receivers_get_the_stream_over_multicast() {
    let Some(iface) = udp::default_interface() else {
        panic!("no non-loopback IPv4 interface: cannot test multicast here");
    };
    let group = Ipv4Addr::new(239, 255, 77, 2);
    let port = 47901;
    let key = [5u8; 16];
    let frames = 90u32; // 3 s at 30 fps
    let frame_len = 40_000; // ~9.6 Mbps: well above the planned 4 Mbps average

    let stop = Arc::new(AtomicBool::new(false));
    let handles: Vec<_> = (0..50)
        .map(|_| {
            let sock = udp::receiver(group, port, iface).expect("join");
            sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut rx = Receiver::new(&key, 1);
                let mut buf = [0u8; 2048];
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(n) = sock.recv(&mut buf) {
                        rx.push(&buf[..n]);
                    }
                }
                rx.stats
            })
        })
        .collect();

    let tx = udp::sender(iface).expect("sender");
    let dest = SocketAddrV4::new(group, port);
    let s = Sender::new(&key, 1, 20);
    let start = Instant::now();
    for f in 0..frames {
        let data = vec![(f % 251) as u8; frame_len];
        for p in s.packetize(f, f % 30 == 0, &data).unwrap() {
            tx.send_to(&p, dest).unwrap();
        }
        let next = start + Duration::from_millis(33 * (f as u64 + 1));
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    std::thread::sleep(Duration::from_millis(500));
    stop.store(true, Ordering::Relaxed);

    let stats: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let ok: Vec<u64> = stats.iter().map(|s| s.frames_ok).collect();
    let repaired: u64 = stats.iter().map(|s| s.frames_fec_repaired).sum();
    let worst = *ok.iter().min().unwrap();
    println!("frames_ok per receiver: min={worst} max={} of {frames}; fec-repaired frames total={repaired}", ok.iter().max().unwrap());
    assert!(worst >= (frames as u64) * 95 / 100, "worst receiver got only {worst}/{frames} frames");
}
