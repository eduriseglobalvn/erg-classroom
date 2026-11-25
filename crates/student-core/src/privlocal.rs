//! Per-user mode (no SYSTEM service): the agent holds the Room Key itself, so it checks a signed request and
//! then applies it through its own executor. With the service installed this is never used; the agent
//! forwards the request instead and cannot apply or forge anything.
use crate::Executor;
use erg_proto::{PrivAction, PrivRequest};
use std::sync::Mutex;

pub struct Local {
    key: [u8; 32],
    gate: Mutex<erg_privcmd::Gate>,
}

impl Local {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key, gate: Default::default() }
    }

    pub fn apply(&self, ex: &dyn Executor, r: &PrivRequest) -> bool {
        let admitted = self.gate.lock().expect("gate lock").admit(&self.key, r, erg_privcmd::now_ms());
        match admitted {
            Ok(a) => {
                run(ex, a);
                true
            }
            Err(e) => {
                tracing::warn!("signed request refused: {e:?}");
                false
            }
        }
    }
}

fn run(ex: &dyn Executor, a: PrivAction) {
    match a {
        PrivAction::Web(m, l) => ex.set_web_policy(m, &l),
        PrivAction::App(m, l) => ex.set_app_policy(m, &l),
        PrivAction::Usb(p) => ex.set_usb_policy(p),
        PrivAction::Print(p) => ex.set_print_policy(p),
        PrivAction::Protect(p) => ex.set_protect(&p),
        PrivAction::Power(p) => ex.power(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use erg_proto::{UsbPolicy, WebPolicy};

    #[derive(Default)]
    struct Rec(Mutex<Vec<String>>);
    impl Executor for Rec {
        fn open_url(&self, _: &str) {}
        fn unlock(&self) {}
        fn message(&self, _: &str, _: u16) {}
        fn lock(&self, _: bool, _: Option<&str>) {}
        fn power(&self, a: erg_proto::PowerAction) { self.0.lock().unwrap().push(format!("{a:?}")); }
        fn set_web_policy(&self, m: WebPolicy, l: &[String]) { self.0.lock().unwrap().push(format!("web {m:?} {l:?}")); }
        fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
        fn set_usb_policy(&self, p: UsbPolicy) { self.0.lock().unwrap().push(format!("usb {p:?}")); }
    }

    #[test]
    fn only_requests_signed_with_this_key_are_applied() {
        let (k, ex, l) = ([4u8; 32], Rec::default(), Local::new([4u8; 32]));
        let now = erg_privcmd::now_ms();
        let good = erg_privcmd::sign(&k, &PrivAction::Usb(UsbPolicy::BlockAll), now).unwrap();
        assert!(l.apply(&ex, &good));
        assert!(!l.apply(&ex, &good), "replay");
        let forged = erg_privcmd::sign(&[5u8; 32], &PrivAction::Web(WebPolicy::BlockAll, vec![]), now + 1).unwrap();
        assert!(!l.apply(&ex, &forged));
        assert_eq!(*ex.0.lock().unwrap(), vec!["usb BlockAll".to_string()]);
    }
}
