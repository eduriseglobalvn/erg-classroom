//! Capture the screen (or one window), encode H.264 and multicast it with FEC + encryption.
//! Used by the tutor (Show) and by a student chosen to Exhibit their screen.
//!
//! shortcut: a window source crops the desktop picture to the window's rectangle (it does not use
//! Windows.Graphics.Capture), so windows covering it show up. Upgrade when that matters.
#![cfg(windows)]
use erg_capture::Capturer;
use erg_encode::H264Encoder;
use erg_show::{udp, Sender};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowRect, IsWindow};

pub mod watch;

const FPS: u32 = 60;
const BITRATE: u32 = 8_000_000;
/// A keyframe at least this often lets late joiners (or anyone who lost a frame) recover.
const KEYFRAME_EVERY: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Screen,
    /// A window handle (HWND as isize). The Show ends when the window closes.
    Window(isize),
}

/// The window the user is working in right now (for "Show application").
pub fn foreground_window() -> Option<isize> {
    let h = unsafe { GetForegroundWindow() };
    (!h.0.is_null()).then_some(h.0 as isize)
}

pub struct Target {
    pub group: [u8; 4],
    pub port: u16,
    pub key: [u8; 16],
    pub stream: u32,
}

pub struct Handle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Handle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
    /// True while the worker still runs (it ends by itself when the source disappears).
    pub fn running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}

impl Drop for Handle {
    /// A forgotten handle must not leave the screen being captured and sent forever.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn window_rect(h: isize) -> Option<RECT> {
    let hw = HWND(h as *mut _);
    unsafe {
        if !IsWindow(Some(hw)).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        GetWindowRect(hw, &mut r).ok()?;
        Some(r)
    }
}

/// Desktop Duplication pictures do not contain the mouse pointer, so it is painted in: a plain arrow
/// ('#' black edge, '.' white), whatever shape the real pointer has.
const ARROW: [&[u8; 12]; 18] = [
    b"#           ", b"##          ", b"#.#         ", b"#..#        ", b"#...#       ", b"#....#      ",
    b"#.....#     ", b"#......#    ", b"#.......#   ", b"#........#  ", b"#.........# ", b"#......#####",
    b"#...#..#    ", b"#..# #..#   ", b"#.#  #..#   ", b"##    #..#  ", b"#     #..#  ", b"       ##   ",
];

/// Where the pointer is on the primary display, in pixels.
pub fn pointer_pos() -> Option<(i32, i32)> {
    let mut p = windows::Win32::Foundation::POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

/// Paint the arrow with its tip at (x, y), enlarged `k` times, into a `w`x`h` BGRA picture.
/// Returns what it covered, for `undo_pointer`: the clean picture is needed again for the next frame.
pub fn draw_pointer(bgra: &mut [u8], w: usize, h: usize, x: i32, y: i32, k: usize) -> Vec<(usize, [u8; 4])> {
    let mut saved = vec![];
    for (row, line) in ARROW.iter().enumerate() {
        for (col, c) in line.iter().enumerate() {
            let color = match c { b'#' => [0, 0, 0, 255], b'.' => [255, 255, 255, 255], _ => continue };
            for (dy, dx) in (0..k).flat_map(|dy| (0..k).map(move |dx| (dy, dx))) {
                let (px, py) = (x + (col * k + dx) as i32, y + (row * k + dy) as i32);
                if px < 0 || py < 0 || px as usize >= w || py as usize >= h { continue; }
                let i = (py as usize * w + px as usize) * 4;
                saved.push((i, [bgra[i], bgra[i + 1], bgra[i + 2], bgra[i + 3]]));
                bgra[i..i + 4].copy_from_slice(&color);
            }
        }
    }
    saved
}

pub fn undo_pointer(bgra: &mut [u8], saved: &[(usize, [u8; 4])]) {
    for (i, p) in saved { bgra[*i..*i + 4].copy_from_slice(p); }
}

/// Copy the `w`x`h` rectangle at (x, y) out of a BGRA picture; anything outside stays black.
pub fn crop(src: &[u8], sw: usize, sh: usize, x: i32, y: i32, w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    for row in 0..h {
        let sy = y + row as i32;
        if sy < 0 || sy as usize >= sh {
            continue;
        }
        let x0 = x.max(0) as usize;
        let x1 = ((x + w as i32).max(0) as usize).min(sw);
        if x0 >= x1 {
            continue;
        }
        let dst = (x0 as i32 - x) as usize;
        let n = (x1 - x0) * 4;
        let s = (sy as usize * sw + x0) * 4;
        out[(row * w + dst) * 4..(row * w + dst) * 4 + n].copy_from_slice(&src[s..s + n]);
    }
    out
}

/// Start streaming. Setup runs on the worker thread (COM objects stay on one thread); its result
/// comes back here so a failure is reported before anyone is told to join.
/// Called with every encoded access unit (keyframe flag, Annex B data); used for the unicast fallback.
pub type Tap = Box<dyn Fn(bool, &[u8]) + Send>;

pub fn start(source: Source, t: Target) -> Result<Handle, String> {
    start_tap(source, t, None)
}

pub fn start_tap(source: Source, t: Target, tap: Option<Tap>) -> Result<Handle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let thread = std::thread::spawn(move || {
        let setup = || -> Result<_, String> {
            let iface = udp::default_interface().ok_or("no network interface found")?;
            let mut cap = Capturer::new(0).map_err(|e| e.to_string())?;
            let first = (0..20).find_map(|_| cap.grab(250).transpose()).ok_or("no screen frame received")?.map_err(|e| e.to_string())?;
            let (w, h) = match source {
                Source::Screen => (first.width & !1, first.height & !1), // H.264 needs even dimensions
                Source::Window(hw) => {
                    let r = window_rect(hw).ok_or("window not found")?;
                    (((r.right - r.left).clamp(64, first.width as i32) as u32) & !1, ((r.bottom - r.top).clamp(64, first.height as i32) as u32) & !1)
                }
            };
            let enc = H264Encoder::new(w, h, FPS, BITRATE, FPS * 2).map_err(|e| e.to_string())?;
            let sock = udp::sender(iface).map_err(|e| e.to_string())?;
            Ok((cap, first, enc, sock, w, h))
        };
        let (mut cap, first, mut enc, sock, w, h) = match setup() {
            Ok(v) => {
                let _ = ready_tx.send(Ok(()));
                v
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
                return;
            }
        };
        let sender = Sender::new(&t.key, t.stream, 20);
        let dest = SocketAddrV4::new(Ipv4Addr::from(t.group), t.port);
        let (sw, sh) = (first.width as usize, first.height as usize);
        let cut = |bgra: &[u8]| match source {
            Source::Screen => crop(bgra, sw, sh, 0, 0, w as usize, h as usize),
            Source::Window(hw) => match window_rect(hw) {
                Some(r) => crop(bgra, sw, sh, r.left, r.top, w as usize, h as usize),
                None => Vec::new(),
            },
        };
        // pointer position inside the picture being sent
        let pointer = || {
            let (x, y) = pointer_pos()?;
            match source {
                Source::Screen => Some((x, y)),
                Source::Window(hw) => window_rect(hw).map(|r| (x - r.left, y - r.top)),
            }
        };
        let frame_time = Duration::from_millis(1000 / FPS as u64);
        let (mut shown_at, mut last_sent) = (None, Instant::now() - frame_time);
        let (mut frame_no, mut last_key) = (0u32, Instant::now() - KEYFRAME_EVERY);
        let mut last = cut(&first.bgra);
        let mut pending = Some(last.clone());
        while !flag.load(Ordering::Relaxed) {
            let img = match pending.take() {
                Some(i) => Some(i),
                None => match cap.grab(1000 / FPS) {
                    Ok(Some(f)) if f.width == first.width && f.height == first.height => Some(cut(&f.bgra)),
                    Ok(Some(_)) | Err(_) => break, // resolution changed or capture lost: end the Show
                    // A window that moves is not a "new desktop frame" on a static screen, so re-cut it too.
                    Ok(None) if matches!(source, Source::Window(_)) => Some(cut(&last_desktop(&mut cap, &first))),
                    // Static screen: re-send the last picture only when a keyframe is due.
                    Ok(None) if last_key.elapsed() >= KEYFRAME_EVERY => Some(last.clone()),
                    // only the pointer moved (the API reports that without a picture): same picture, new pointer
                    Ok(None) if pointer() != shown_at && last_sent.elapsed() >= frame_time => Some(last.clone()),
                    Ok(None) => None,
                },
            };
            let Some(img) = img else { continue };
            if img.is_empty() {
                break; // the shown window was closed
            }
            let mut img = img;
            let force = last_key.elapsed() >= KEYFRAME_EVERY;
            shown_at = pointer();
            last_sent = Instant::now();
            let under = shown_at.map(|(x, y)| draw_pointer(&mut img, w as usize, h as usize, x, y, (h as usize / 900).max(1)));
            let units = enc.encode(&img, force);
            if let Some(u) = &under { undo_pointer(&mut img, u); }
            let Ok(units) = units else { break };
            for u in units {
                if u.keyframe {
                    last_key = Instant::now();
                }
                if let Some(f) = &tap {
                    f(u.keyframe, &u.data);
                }
                if let Ok(pkts) = sender.packetize(frame_no, u.keyframe, &u.data) {
                    for p in pkts {
                        let _ = sock.send_to(&p, dest);
                    }
                }
                frame_no = frame_no.wrapping_add(1);
            }
            last = img;
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(Handle { stop, thread: Some(thread) }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Show setup timed out".into()),
    }
}

/// shortcut: with a static desktop the duplication API returns no new picture, so a window source
/// keeps showing its last desktop picture; a window dragged on an otherwise idle screen lags.
fn last_desktop(_cap: &mut Capturer, first: &erg_capture::Frame) -> Vec<u8> {
    first.bgra.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_is_painted_clipped_and_undone() {
        let (w, h) = (16usize, 16usize);
        let clean = vec![7u8; w * h * 4];
        let mut pic = clean.clone();
        let saved = draw_pointer(&mut pic, w, h, 10, 10, 1); // partly outside: must not panic
        assert_eq!(&pic[(10 * w + 10) * 4..(10 * w + 10) * 4 + 3], &[0, 0, 0]); // the tip is the black edge
        assert_eq!(&pic[(12 * w + 11) * 4..(12 * w + 11) * 4 + 3], &[255, 255, 255]);
        assert!(draw_pointer(&mut pic.clone(), w, h, -50, 40, 2).is_empty());
        undo_pointer(&mut pic, &saved);
        assert_eq!(pic, clean);
    }

    #[test]
    fn crop_pads_outside_the_picture_with_black() {
        // 4x2 picture, each pixel's B channel = its index + 1
        let src: Vec<u8> = (0..8u8).flat_map(|i| [i + 1, 0, 0, 255]).collect();
        let full = crop(&src, 4, 2, 0, 0, 4, 2);
        assert_eq!(full, src);
        let shifted = crop(&src, 4, 2, -1, 0, 4, 2); // one column hangs off the left edge
        assert_eq!(&shifted[0..4], &[0, 0, 0, 0]);
        assert_eq!(shifted[4], 1);
        let below = crop(&src, 4, 2, 0, 1, 2, 2); // second row missing
        assert_eq!(below[0], 5);
        assert!(below[8..].iter().all(|b| *b == 0));
        assert!(crop(&src, 4, 2, 10, 10, 2, 2).iter().all(|b| *b == 0));
        assert!(crop(&src, 4, 2, 3, 0, 4, 1)[4..].iter().all(|b| *b == 0), "right overhang is black");
    }
}
