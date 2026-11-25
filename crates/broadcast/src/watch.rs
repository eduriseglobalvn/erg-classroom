//! One-to-one screen watching: capture, shrink to at most `max_w` wide, encode H.264 and hand every
//! access unit to `sink`. The tutor's Watch/Control window gets them over the control connection.
use super::Handle;
use erg_capture::Capturer;
use erg_encode::H264Encoder;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FPS: u32 = 30;
const BITRATE: u32 = 4_000_000;
const KEYFRAME_EVERY: Duration = Duration::from_secs(1);

/// Nearest-neighbour resize of a BGRA picture.
pub fn scale_nearest(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = y * sh / dh;
        for x in 0..dw {
            let s = (sy * sw + x * sw / dw) * 4;
            out[(y * dw + x) * 4..(y * dw + x) * 4 + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// Output size: keep the aspect ratio, at most `max_w` wide, both sides even and at least 2.
pub fn fit(sw: usize, sh: usize, max_w: usize) -> (usize, usize) {
    let w = sw.min(max_w).max(2);
    let h = (sh * w / sw.max(1)).max(2);
    (w & !1, h & !1)
}

pub fn start(max_w: u32, sink: Box<dyn Fn(bool, Vec<u8>) + Send>) -> Result<Handle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let thread = std::thread::spawn(move || {
        let setup = || -> Result<_, String> {
            let mut cap = Capturer::new(0).map_err(|e| e.to_string())?;
            let first = (0..20).find_map(|_| cap.grab(250).transpose()).ok_or("no screen frame received")?.map_err(|e| e.to_string())?;
            let (w, h) = fit(first.width as usize, first.height as usize, max_w as usize);
            let enc = H264Encoder::new(w as u32, h as u32, FPS, BITRATE, FPS).map_err(|e| e.to_string())?;
            Ok((cap, first, enc, w, h))
        };
        let (mut cap, first, mut enc, w, h) = match setup() {
            Ok(v) => { let _ = ready_tx.send(Ok(())); v }
            Err(e) => { let _ = ready_tx.send(Err(e)); return; }
        };
        let (sw, sh) = (first.width as usize, first.height as usize);
        let mut last = scale_nearest(&first.bgra, sw, sh, w, h);
        // the teacher must see where the student (or their own remote control) points
        let pointer = || crate::pointer_pos().map(|(x, y)| (x * w as i32 / sw.max(1) as i32, y * h as i32 / sh.max(1) as i32));
        let mut shown_at = None;
        let (mut pending, mut last_key, mut last_sent) = (true, Instant::now() - KEYFRAME_EVERY, Instant::now() - Duration::from_secs(1));
        while !flag.load(Ordering::Relaxed) {
            if !pending {
                match cap.grab(1000 / FPS) {
                    Ok(Some(f)) if f.width as usize == sw && f.height as usize == sh => { last = scale_nearest(&f.bgra, sw, sh, w, h); pending = true; }
                    Ok(Some(_)) | Err(_) => break, // resolution changed or capture lost: the viewer reopens
                    // static screen: repeat the last picture only to keep keyframes coming for late joiners
                    Ok(None) if last_key.elapsed() >= KEYFRAME_EVERY => pending = true,
                    Ok(None) if pointer() != shown_at => pending = true, // only the pointer moved
                    Ok(None) => {}
                }
            }
            if !pending || last_sent.elapsed() < Duration::from_millis(1000 / FPS as u64) { continue; }
            pending = false;
            last_sent = Instant::now();
            let force = last_key.elapsed() >= KEYFRAME_EVERY;
            shown_at = pointer();
            let under = shown_at.map(|(x, y)| crate::draw_pointer(&mut last, w, h, x, y, 1));
            let units = enc.encode(&last, force);
            if let Some(u) = &under { crate::undo_pointer(&mut last, u); }
            let Ok(units) = units else { break };
            for u in units {
                if u.keyframe { last_key = Instant::now(); }
                sink(u.keyframe, u.data);
            }
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(Handle { stop, thread: Some(thread) }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Watch setup timed out".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_stay_even_and_keep_the_aspect_ratio() {
        assert_eq!(fit(1920, 1080, 1280), (1280, 720));
        assert_eq!(fit(800, 600, 1280), (800, 600), "small screens are not enlarged");
        assert_eq!(fit(1366, 768, 1280), (1280, 718));
        assert_eq!(fit(1, 1, 1280), (2, 2));
    }

    #[test]
    fn nearest_scaling_picks_the_covering_source_pixel() {
        let src: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect(); // 2x2
        assert_eq!(scale_nearest(&src, 2, 2, 4, 4).chunks(4).map(|p| p[0]).collect::<Vec<_>>(), [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 3, 3, 2, 2, 3, 3]);
    }

    /// Needs a real desktop: the first keyframe must arrive.
    #[test]
    fn live_screen_gives_a_keyframe() {
        let (tx, rx) = std::sync::mpsc::channel();
        let h = start(640, Box::new(move |k, d| { let _ = tx.send((k, d.len())); })).expect("watch start");
        let got = (0..60).find_map(|_| rx.recv_timeout(Duration::from_millis(500)).ok().filter(|(k, n)| *k && *n > 0));
        h.stop();
        assert!(got.is_some(), "no keyframe");
    }
}
