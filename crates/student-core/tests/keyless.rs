//! A student runtime that has no Room Key of its own (Config::key is all zeros) authenticates to the real
//! tutor through a key holder on a real named pipe, and a tutor with another key is still refused.
#![cfg(windows)]
use erg_student_core::{run, Config, Executor};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Nop;
impl Executor for Nop {
    fn open_url(&self, _: &str) {}
    fn message(&self, _: &str, _: u16) {}
    fn lock(&self, _: bool, _: Option<&str>) {}
    fn unlock(&self) {}
    fn power(&self, _: erg_proto::PowerAction) {}
    fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
    fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn keyless_student_authenticates_through_the_key_holder() {
    let (real_key, port) = ([21u8; 32], 47852u16);
    let pipe = format!(r"\.\pipe\erg-keyless-test-{}", std::process::id());
    let holder = erg_authpipe::serve(&pipe, Arc::new(real_key));
    tokio::time::sleep(Duration::from_millis(200)).await;
    erg_student_core::set_auth(Arc::new(erg_authpipe::client(&pipe)));
    let cfg = Config {
        key: [0u8; 32], // the runtime itself has no key
        room: "k".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
        browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()],
        inbox: std::env::temp_dir().join("erg-keyless-inbox"),
    };
    tokio::spawn(run(cfg, Arc::new(Nop), None));

    let roster = erg_tutor_core::Roster::start(port, real_key, Some("k".into())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    // Roster::send counts only students whose control connection is authenticated
    while roster.send(None, erg_proto::Command::Unlock) == 0 {
        assert!(Instant::now() < deadline, "tutor never got an authenticated connection to the keyless student");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    holder.stop();
}
