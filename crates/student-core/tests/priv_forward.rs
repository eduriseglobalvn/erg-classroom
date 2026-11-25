//! End to end: the tutor's signed policy command reaches a student runtime over QUIC, the student agent only
//! forwards it over a pipe, and the key-holding service side applies it. The agent itself holds no key
//! (Config::key is all zeros) and a plain, unsigned command from a tutor changes nothing on the service side.
#![cfg(windows)]
use erg_proto::{Command, UsbPolicy};
use erg_student_core::{run, Config, Executor};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The agent: forwards signed requests, applies nothing itself.
struct Forwarder(String);
impl Executor for Forwarder {
    fn open_url(&self, _: &str) {}
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: erg_proto::PowerAction) {}
    fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
    fn privileged(&self, r: &erg_proto::PrivRequest) -> bool {
        erg_authpipe::forward(&self.0, r).unwrap_or(false)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_signed_usb_command_travels_tutor_to_agent_to_service() {
    let (key, port) = ([31u8; 32], 47856u16);
    let pipe = format!(r"\\.\pipe\erg-priv-e2e-{}", std::process::id());
    let applied = Arc::new(Mutex::new(vec![]));
    let (gate, a2) = (Mutex::new(erg_privcmd::Gate::default()), applied.clone());
    let service = erg_authpipe::serve_priv(
        &pipe,
        Arc::new(Mutex::new(vec![erg_authpipe::current_user_sid().unwrap()])),
        Arc::new(move |req: &[u8]| erg_privcmd::serve_bytes(&key, &mut gate.lock().unwrap(), req, erg_privcmd::now_ms(), &mut |a| a2.lock().unwrap().push(a))),
    );
    let holder = erg_authpipe::serve(&format!("{pipe}-auth"), Arc::new(key));
    tokio::time::sleep(Duration::from_millis(200)).await;
    erg_student_core::set_auth(Arc::new(erg_authpipe::client(&format!("{pipe}-auth"))));
    let cfg = Config {
        key: [0u8; 32],
        room: "p".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
        browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()],
        inbox: std::env::temp_dir().join("erg-priv-e2e-inbox"),
    };
    tokio::spawn(run(cfg, Arc::new(Forwarder(pipe.clone())), None));

    let roster = erg_tutor_core::Roster::start(port, key, Some("p".into())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while roster.send(None, Command::SetUsbPolicy(UsbPolicy::ReadOnly)) == 0 {
        assert!(Instant::now() < deadline, "student never connected");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let until = Instant::now() + Duration::from_secs(10);
    while applied.lock().unwrap().is_empty() && Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(applied.lock().unwrap().contains(&erg_proto::PrivAction::Usb(UsbPolicy::ReadOnly)), "the service side never got the signed request");

    // the agent cannot make one up: a request signed with another key is refused by the service
    let forged = erg_privcmd::sign(&[1u8; 32], &erg_proto::PrivAction::Usb(UsbPolicy::Allow), erg_privcmd::now_ms() + 10).unwrap();
    assert_eq!(erg_authpipe::forward(&pipe, &forged), Some(false));
    assert!(!applied.lock().unwrap().contains(&erg_proto::PrivAction::Usb(UsbPolicy::Allow)));
    service.stop();
    holder.stop();
}
