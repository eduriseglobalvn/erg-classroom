//! Privileged policy commands (web/app/usb/print/protect/power), signed by the tutor with the Room Key.
//! The student agent only forwards the signed blob to the SYSTEM service, which holds the key and runs
//! `Gate::admit` before doing anything: the agent (a process of the student's user) cannot originate one.
use erg_proto::{PrivAction, PrivRequest};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;

/// A request older or newer than this (against the service clock) is refused: bounds replay and tolerates clock skew.
pub const WINDOW_MS: u64 = 5 * 60 * 1000;
pub const MAX_BODY: usize = 32 * 1024;
const MAX_ITEMS: usize = 500;
const MAX_ITEM_LEN: usize = 260;

fn mac(key: &[u8; 32], issued: u64, body: &[u8]) -> [u8; 32] {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac key");
    m.update(b"erg-priv-v1\0");
    m.update(&issued.to_le_bytes());
    m.update(&(body.len() as u32).to_le_bytes());
    m.update(body);
    m.finalize().into_bytes().into()
}

/// Tutor side. `issued_ms` must grow with every call (the caller guarantees it); the Gate drops older ones.
pub fn sign(key: &[u8; 32], action: &PrivAction, issued_ms: u64) -> Result<PrivRequest, String> {
    check(action)?;
    let body = postcard::to_allocvec(action).map_err(|e| e.to_string())?;
    if body.len() > MAX_BODY {
        return Err("danh sách quá dài".into());
    }
    Ok(PrivRequest { issued: issued_ms, mac: mac(key, issued_ms, &body), body })
}

/// Which setting a request changes; a newer request of a kind replaces older ones, never the reverse.
fn kind(a: &PrivAction) -> u8 {
    match a {
        PrivAction::Web(..) => 0,
        PrivAction::App(..) => 1,
        PrivAction::Usb(_) => 2,
        PrivAction::Print(_) => 3,
        PrivAction::Protect(_) => 4,
        PrivAction::Power(_) => 5,
    }
}

/// Bounds on what a request may carry (the service re-checks content again when it applies it).
pub fn check(a: &PrivAction) -> Result<(), String> {
    let list_ok = |l: &[String]| l.len() <= MAX_ITEMS && l.iter().all(|s| s.len() <= MAX_ITEM_LEN && !s.chars().any(char::is_control));
    let ok = match a {
        PrivAction::Web(_, l) | PrivAction::App(_, l) => list_ok(l),
        PrivAction::Protect(p) => p.hide_drives.len() <= 26,
        _ => true,
    };
    ok.then_some(()).ok_or_else(|| "nội dung không hợp lệ".into())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    TooBig,
    BadSignature,
    OutsideWindow,
    Stale,
    Malformed,
}

/// Service side: remembers the newest request per kind so a recorded older one cannot be replayed.
#[derive(Default)]
pub struct Gate {
    last: HashMap<u8, u64>,
}

impl Gate {
    pub fn admit(&mut self, key: &[u8; 32], r: &PrivRequest, now_ms: u64) -> Result<PrivAction, Refused> {
        use subtle::ConstantTimeEq;
        if r.body.len() > MAX_BODY {
            return Err(Refused::TooBig);
        }
        if !bool::from(mac(key, r.issued, &r.body).ct_eq(&r.mac)) {
            return Err(Refused::BadSignature);
        }
        if r.issued.abs_diff(now_ms) > WINDOW_MS {
            return Err(Refused::OutsideWindow);
        }
        let a: PrivAction = postcard::from_bytes(&r.body).map_err(|_| Refused::Malformed)?;
        check(&a).map_err(|_| Refused::Malformed)?;
        let last = self.last.entry(kind(&a)).or_insert(0);
        if r.issued <= *last {
            return Err(Refused::Stale);
        }
        *last = r.issued;
        Ok(a)
    }
}

pub fn encode(r: &PrivRequest) -> Vec<u8> {
    postcard::to_allocvec(r).unwrap_or_default()
}

pub fn decode(b: &[u8]) -> Option<PrivRequest> {
    postcard::from_bytes(b).ok()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Pipe-side handler: bytes in, one byte out (1 = done, 0 = refused); `H` is the liveness ping.
pub fn serve_bytes(key: &[u8; 32], gate: &mut Gate, req: &[u8], now_ms: u64, run: &mut dyn FnMut(PrivAction)) -> Vec<u8> {
    if req == b"H" {
        return b"K".to_vec();
    }
    match decode(req).map(|r| gate.admit(key, &r, now_ms)) {
        Some(Ok(a)) => {
            run(a);
            vec![1]
        }
        _ => vec![0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use erg_proto::{PowerAction, Protect, UsbPolicy, WebPolicy};

    const K: [u8; 32] = [9; 32];
    const NOW: u64 = 1_000_000_000;

    fn web() -> PrivAction {
        PrivAction::Web(WebPolicy::BlockList, vec!["a.vn".into()])
    }

    #[test]
    fn a_genuine_request_passes_once() {
        let mut g = Gate::default();
        let r = sign(&K, &web(), NOW).unwrap();
        assert_eq!(g.admit(&K, &r, NOW + 10), Ok(web()));
        assert_eq!(g.admit(&K, &r, NOW + 10), Err(Refused::Stale), "exact replay");
    }

    #[test]
    fn forged_edited_and_wrong_key_requests_fail() {
        let mut g = Gate::default();
        let r = sign(&K, &web(), NOW).unwrap();
        assert_eq!(g.admit(&[1; 32], &r, NOW), Err(Refused::BadSignature), "agent without the key cannot sign");
        let other = postcard::to_allocvec(&PrivAction::Power(PowerAction::Shutdown)).unwrap();
        assert_eq!(g.admit(&K, &PrivRequest { body: other, ..r.clone() }, NOW), Err(Refused::BadSignature));
        assert_eq!(g.admit(&K, &PrivRequest { issued: NOW + 1, ..r.clone() }, NOW), Err(Refused::BadSignature));
        assert_eq!(g.admit(&K, &PrivRequest { mac: [0; 32], ..r }, NOW), Err(Refused::BadSignature));
    }

    #[test]
    fn old_future_and_reordered_requests_are_refused() {
        let mut g = Gate::default();
        let old = sign(&K, &web(), NOW).unwrap();
        assert_eq!(g.admit(&K, &old, NOW + WINDOW_MS + 1), Err(Refused::OutsideWindow));
        let future = sign(&K, &web(), NOW + WINDOW_MS + 5).unwrap();
        assert_eq!(g.admit(&K, &future, NOW), Err(Refused::OutsideWindow));
        let (a, b) = (sign(&K, &PrivAction::Usb(UsbPolicy::BlockAll), NOW + 2).unwrap(), sign(&K, &PrivAction::Usb(UsbPolicy::Allow), NOW + 3).unwrap());
        assert!(g.admit(&K, &b, NOW + 5).is_ok());
        assert_eq!(g.admit(&K, &a, NOW + 5), Err(Refused::Stale), "a recorded older Usb request cannot undo the newer one");
        assert!(g.admit(&K, &sign(&K, &web(), NOW + 1).unwrap(), NOW + 5).is_ok(), "other kinds are independent");
    }

    #[test]
    fn oversize_content_is_refused_at_both_ends() {
        let long = vec!["x".repeat(MAX_ITEM_LEN + 1)];
        assert!(sign(&K, &PrivAction::App(erg_proto::AppPolicy::BlockList, long), NOW).is_err());
        assert!(sign(&K, &PrivAction::Protect(Protect { hide_drives: vec!['D'; 27], ..Default::default() }), NOW).is_err());
        assert!(sign(&K, &PrivAction::Web(WebPolicy::BlockList, vec!["a\nb".into()]), NOW).is_err());
        // a validly signed but hostile body is still bounded by the gate
        let body = postcard::to_allocvec(&PrivAction::Web(WebPolicy::BlockList, vec!["x".repeat(300)])).unwrap();
        let r = PrivRequest { issued: NOW, mac: mac(&K, NOW, &body), body };
        assert_eq!(Gate::default().admit(&K, &r, NOW), Err(Refused::Malformed));
        let big = PrivRequest { body: vec![0; MAX_BODY + 1], issued: NOW, mac: [0; 32] };
        assert_eq!(Gate::default().admit(&K, &big, NOW), Err(Refused::TooBig));
    }

    #[test]
    fn pipe_bytes_ping_garbage_and_run() {
        let mut g = Gate::default();
        let mut ran = vec![];
        let mut run = |a| ran.push(a);
        assert_eq!(serve_bytes(&K, &mut g, b"H", NOW, &mut run), b"K");
        for bad in [&b""[..], b"junk", &[0xff; 50]] {
            assert_eq!(serve_bytes(&K, &mut g, bad, NOW, &mut run), vec![0]);
        }
        let r = encode(&sign(&K, &PrivAction::Power(PowerAction::Logoff), NOW).unwrap());
        assert_eq!(serve_bytes(&K, &mut g, &r, NOW, &mut run), vec![1]);
        assert_eq!(serve_bytes(&K, &mut g, &r, NOW, &mut run), vec![0], "replay");
        assert_eq!(ran, vec![PrivAction::Power(PowerAction::Logoff)]);
    }
}
