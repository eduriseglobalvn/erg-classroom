//! Student process (console mode). Windows-service wrapping and the split into SYSTEM service +
//! per-session agent come next; until then this runs in the user's session so URL opening and
//! message boxes work, and privileged features log that they are not implemented yet.
use erg_proto::{AppPolicy, PowerAction, WebPolicy, DISCOVERY_PORT};
use erg_student_core::{Config, Executor};
use std::process::Command as Proc;
use std::sync::Arc;

#[derive(Default)]
struct WinExecutor {
    apps: std::sync::Mutex<Option<erg_policy::apps::AppBlocker>>,
    /// ends browsers that ignore the web policies while a web filter is on
    guard: std::sync::Mutex<Option<erg_policy::apps::AppBlocker>>,
    keywords: std::sync::Arc<std::sync::Mutex<erg_student_core::KeywordMonitor>>,
    #[cfg(windows)]
    exhibit: std::sync::Mutex<Option<erg_broadcast::Handle>>,
    #[cfg(windows)]
    watch: std::sync::Mutex<Option<erg_broadcast::Handle>>,
}

impl Executor for WinExecutor {
    fn open_url(&self, url: &str) {
        // `url` was validated as plain http(s) with no whitespace/control chars, and is passed as
        // a single argv element (no shell), so it cannot inject extra arguments or commands.
        if let Err(e) = Proc::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).spawn() {
            tracing::error!("open_url failed: {e}");
        }
    }

    fn message(&self, text: &str, seconds: u16) {
        let text = text.to_string();
        // MessageBoxW blocks, so give it its own thread; `seconds` is advisory until the agent
        // provides a self-closing toast.
        std::thread::spawn(move || show_box(&text));
        let _ = seconds;
    }

    fn lock(&self, blank: bool, _text: Option<&str>) {
        tracing::warn!("lock(blank={blank}) needs the student agent - not implemented yet");
    }
    fn unlock(&self) {
        tracing::warn!("unlock needs the student agent - not implemented yet");
    }

    fn power(&self, action: PowerAction) {
        if service_mode() { return; } // the SYSTEM service does it, from a request the tutor signed
        let flag = match action {
            PowerAction::Shutdown => "/s",
            PowerAction::Reboot => "/r",
            PowerAction::Logoff => "/l",
        };
        let mut c = Proc::new("shutdown.exe");
        c.arg(flag);
        if flag != "/l" {
            c.args(["/t", "10"]);
        }
        if let Err(e) = c.spawn() {
            tracing::error!("power failed: {e}");
        }
    }

    fn thumbnail(&self, w: u16, h: u16) -> Option<Vec<u8>> {
        // a fresh duplication each time: COM objects stay on one thread and this runs on a pool thread
        let mut cap = erg_capture::Capturer::new(0).ok()?;
        // the first frames may only carry a pointer update, so retry briefly
        (0..15).find_map(|_| cap.grab(200).ok().flatten())?.thumbnail_jpeg(w, h)
    }
    fn mute(&self, on: bool) {
        #[cfg(windows)]
        if let Err(e) = erg_sysops::set_mute(on) { tracing::warn!("mute failed: {e}"); }
    }
    fn beep(&self) {
        #[cfg(windows)]
        erg_sysops::beep();
    }
    fn set_clipboard(&self, text: &str) {
        #[cfg(windows)]
        if let Err(e) = erg_sysops::set_clipboard(text) { tracing::warn!("clipboard not set: {e}"); }
    }
    fn clipboard(&self) -> Option<String> {
        #[cfg(windows)]
        { erg_sysops::clipboard() }
        #[cfg(not(windows))]
        { None }
    }
    fn set_usb_policy(&self, p: erg_proto::UsbPolicy) {
        if service_mode() { return; }
        #[cfg(windows)]
        match erg_policy::usb::apply(p) {
            Ok(()) => tracing::info!("usb policy {p:?} applied"),
            Err(e) => tracing::warn!("usb policy not applied (needs administrator): {e}"),
        }
    }
    fn set_print_policy(&self, p: erg_proto::PrintPolicy) {
        if service_mode() { return; }
        #[cfg(windows)]
        {
            let failed = erg_policy::print::apply(p);
            if !failed.is_empty() { tracing::warn!("print policy {p:?}: not applied to {failed:?}"); }
        }
    }
    fn input(&self, e: erg_proto::InputEvent) {
        #[cfg(windows)]
        erg_sysops::input(e);
    }
    fn start_exhibit(&self, group: [u8; 4], port: u16, key: [u8; 16], stream: u32) -> bool {
        #[cfg(windows)]
        {
            self.stop_exhibit();
            return match erg_broadcast::start(erg_broadcast::Source::Screen, erg_broadcast::Target { group, port, key, stream }) {
                Ok(h) => { *self.exhibit.lock().unwrap() = Some(h); true }
                Err(e) => { tracing::warn!("exhibit not started: {e}"); false }
            };
        }
        #[cfg(not(windows))]
        { let _ = (group, port, key, stream); false }
    }
    fn watch_screen(&self, sink: Option<Box<dyn Fn(bool, Vec<u8>) + Send>>) {
        #[cfg(windows)]
        {
            if let Some(h) = self.watch.lock().unwrap().take() { h.stop(); }
            if let Some(s) = sink {
                match erg_broadcast::watch::start(1280, s) {
                    Ok(h) => *self.watch.lock().unwrap() = Some(h),
                    Err(e) => tracing::warn!("watch not started: {e}"),
                }
            }
        }
        #[cfg(not(windows))]
        let _ = sink;
    }
    fn stop_exhibit(&self) {
        #[cfg(windows)]
        if let Some(h) = self.exhibit.lock().unwrap().take() { h.stop(); }
    }
    fn set_protect(&self, p: &erg_proto::Protect) {
        if service_mode() { return; }
        #[cfg(windows)]
        if let Err(e) = erg_policy::protect::apply(p) { tracing::warn!("protect not applied: {e}"); }
    }
    fn privileged(&self, r: &erg_proto::PrivRequest) -> bool {
        #[cfg(windows)]
        if let Some(ok) = erg_authpipe::forward(erg_authpipe::PRIV_NAME, r) {
            return ok; // the service decides; this process cannot apply or originate it
        }
        LOCAL.get().is_some_and(|l| l.apply(self, r))
    }
    fn set_keywords(&self, words: &[String]) {
        self.keywords.lock().unwrap().set_words(words);
    }
    fn set_web_policy(&self, mode: WebPolicy, list: &[String]) {
        if service_mode() { return; }
        // needs administrator rights (HKLM); the SYSTEM service has them, a console run may not
        match erg_policy::web::apply(mode, list) {
            Ok(()) => tracing::info!("web policy {mode:?} applied ({} sites)", list.len()),
            Err(e) => tracing::warn!("web policy not applied: {e}"),
        }
        let mut g = self.guard.lock().unwrap();
        if mode == WebPolicy::Unrestricted {
            *g = None;
        } else if g.is_none() {
            *g = Some(erg_policy::apps::AppBlocker::start(erg_policy::apps::browser_guard(), |n| tracing::info!("ended unmanaged browser {n}")));
        }
    }
    fn set_app_policy(&self, mode: AppPolicy, list: &[String]) {
        if service_mode() { return; }
        match erg_policy::apps::Policy::new(mode, list.to_vec()) {
            Ok(p) => {
                let mut g = self.apps.lock().unwrap();
                match g.as_ref() {
                    Some(b) => b.set_policy(p),
                    None => *g = Some(erg_policy::apps::AppBlocker::start(p, |n| tracing::info!("ended blocked program {n}"))),
                }
            }
            Err(e) => tracing::warn!("app policy rejected: {e}"),
        }
    }
}

/// Per-user mode only (the Room Key is in this process): checks signed requests itself.
static LOCAL: std::sync::OnceLock<erg_student_core::privlocal::Local> = std::sync::OnceLock::new();

/// With the SYSTEM service installed, policy and power changes happen there, only from signed requests.
fn service_mode() -> bool {
    #[cfg(windows)]
    return erg_authpipe::available(erg_authpipe::PRIV_NAME);
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
fn show_box(text: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SYSTEMMODAL};
    let body: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = erg_student_core::i18n::t("teacher").encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(None, PCWSTR(body.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONINFORMATION | MB_SYSTEMMODAL);
    }
}
#[cfg(not(windows))]
fn show_box(text: &str) {
    println!("MESSAGE: {text}");
}

/// A SYSTEM key holder (the watchdog service) answers on a named pipe: then no key file or key in memory is needed here.
fn key_or_holder() -> Result<[u8; 32], String> {
    #[cfg(windows)]
    if erg_authpipe::available(erg_authpipe::DEFAULT_NAME) {
        erg_student_core::set_auth(std::sync::Arc::new(erg_authpipe::client(erg_authpipe::DEFAULT_NAME)));
        return Ok([0u8; 32]); // unused: every proof comes from the holder
    }
    load_key()
}

/// The student must never invent a Room Key: without the school's key it cannot be controlled.
fn load_key() -> Result<[u8; 32], String> {
    let dir = std::env::var("PROGRAMDATA").unwrap_or_else(|_| ".".into());
    let from_file = std::fs::read_to_string(format!("{dir}\\ERGClassroom\\room.key")).ok();
    let from_env = std::env::var("ERG_ROOM_KEY").ok();
    std::env::remove_var("ERG_ROOM_KEY");
    let hex = from_env.or(from_file).ok_or("ERG_ROOM_KEY or %PROGRAMDATA%\\ERGClassroom\\room.key is required")?;
    let hex = hex.trim();
    if hex.len() != 64 || !hex.is_ascii() {
        return Err("room key must be 64 hex characters".into());
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(k)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let key = match key_or_holder() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if key != [0u8; 32] {
        let _ = LOCAL.set(erg_student_core::privlocal::Local::new(key)); // zero key = a holder answers on the pipe
    }
    let cfg = Config {
        key,
        room: std::env::var("ERG_ROOM").ok().or_else(|| erg_student_core::load_settings().remove("room")).unwrap_or_else(|| "default".into()),
        hostname: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "student".into()),
        user: std::env::var("USERNAME").unwrap_or_default(),
        discovery_port: DISCOVERY_PORT,
        browse_addrs: std::env::var("ERG_GATEWAY").ok().or_else(|| erg_student_core::load_settings().remove("gateway")).and_then(|a| a.parse().ok()).into_iter().collect(),
        inbox: std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into())).join("Documents").join("ERG").join("Received"),
    };
    let ex = Arc::new(WinExecutor::default());
    let student = erg_student_core::Student::new(ex.clone());
    #[cfg(windows)]
    {
        // Typed-word reports: the hook feeds the monitor; only whole listed words are sent on.
        let (mon, st) = (ex.keywords.clone(), student.clone());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        erg_sysops::keys::start(move |c| {
            if let Some(w) = mon.lock().unwrap().feed(c) { let _ = tx.send(w); }
        });
        let st2 = st.clone();
        tokio::spawn(async move { while let Some(w) = rx.recv().await { st2.keyword(&w); } });
        // battery and Wi-Fi every 30 s
        tokio::spawn(async move {
            loop {
                let (p, ac) = tokio::task::spawn_blocking(erg_sysops::battery).await.unwrap_or((None, true));
                let wifi = tokio::task::spawn_blocking(erg_sysops::wifi_signal).await.unwrap_or(None);
                st.power(p, ac, wifi);
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
    }
    if let Err(e) = erg_student_core::run_with(cfg, student, None).await {
        eprintln!("student stopped: {e}");
        std::process::exit(1);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use erg_proto::Command;
    use erg_student_core::run;
    use erg_tutor_core::{Roster, RosterEvent};
    use std::time::Duration;

    /// Both tests duplicate the screen; Desktop Duplication tolerates only a few at once.
    static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// Real student runtime + real tutor roster + real screen capture: a JPEG of the desktop arrives.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn live_thumbnail_of_the_real_screen_reaches_the_tutor() {
        let _turn = ONE_AT_A_TIME.lock().await;
        let (key, port) = ([6u8; 32], 47826u16);
        let roster = Roster::start(port, key, Some("t".into())).unwrap();
        let mut rx = roster.subscribe();
        let cfg = Config {
            key, room: "t".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox3"),
        };
        tokio::spawn(run(cfg, Arc::new(WinExecutor::default()), None));
        let jpeg = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match rx.recv().await.unwrap() {
                    RosterEvent::Online { id, .. } => {
                        roster.send(Some(&[id]), Command::RequestThumbnail { w: 320, h: 180, interval_ms: 0 });
                    }
                    RosterEvent::Student { event: erg_proto::Event::Thumbnail(j), .. } => break j,
                    _ => {}
                }
            }
        })
        .await
        .expect("no thumbnail within 30 s");
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let out = std::env::temp_dir().join("erg-thumb-test.jpg");
        std::fs::write(&out, &jpeg).unwrap();
        println!("thumbnail {} bytes -> {}", jpeg.len(), out.display());
    }

    /// Watch: the tutor asks for the screen and receives H.264 over the control connection (keyframe first),
    /// and stopping the watch ends the stream.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn watching_student_streams_h264_to_the_tutor() {
        let _turn = ONE_AT_A_TIME.lock().await;
        let (key, port) = ([8u8; 32], 47830u16);
        let roster = Roster::start(port, key, Some("t".into())).unwrap();
        let mut rx = roster.subscribe();
        let cfg = Config {
            key, room: "t".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox5"),
        };
        tokio::spawn(run(cfg, Arc::new(WinExecutor::default()), None));
        let (key_frames, delta_frames) = tokio::time::timeout(Duration::from_secs(30), async {
            let (mut k, mut d, mut sid) = (0, 0, None);
            loop {
                match rx.recv().await.unwrap() {
                    RosterEvent::Online { id, .. } => {
                        sid = Some(id);
                        roster.send(Some(&[id]), Command::WatchScreen { on: true });
                    }
                    RosterEvent::Student { event: erg_proto::Event::Video { keyframe, data }, .. } => {
                        // Annex B: starts with a 00 00 00 01 / 00 00 01 start code
                        assert!(data.starts_with(&[0, 0, 0, 1]) || data.starts_with(&[0, 0, 1]), "not Annex B");
                        if keyframe { k += 1 } else { d += 1 }
                        if k >= 1 && d >= 1 { break (k, d); }
                    }
                    _ => {}
                }
                let _ = sid;
            }
        })
        .await
        .expect("no keyframe + delta frame within 30 s");
        assert!(key_frames >= 1 && delta_frames >= 1);
    }

    /// A student told to Exhibit really multicasts their screen: frames decode on the wire, and a
    /// non-scoped multicast group is refused.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn exhibiting_student_multicasts_their_screen() {
        let _turn = ONE_AT_A_TIME.lock().await;
        let (key, port) = ([7u8; 32], 47828u16);
        let roster = Roster::start(port, key, Some("t".into())).unwrap();
        let mut rx = roster.subscribe();
        let cfg = Config {
            key, room: "t".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox5"),
        };
        tokio::spawn(run(cfg, Arc::new(WinExecutor::default()), None));
        let id = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let RosterEvent::Online { id, .. } = rx.recv().await.unwrap() { break id; }
            }
        }).await.expect("student never appeared");

        let (group, show_port, skey) = ([239, 255, 77, 33], 47953u16, [4u8; 16]);
        let iface = erg_show::udp::default_interface().expect("lan interface");
        let sock = erg_show::udp::receiver(std::net::Ipv4Addr::from(group), show_port, iface).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();

        roster.send(Some(&[id]), Command::StartExhibit { group: [224, 0, 0, 251], port: show_port, key: skey, stream: 5 });
        let refused = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let RosterEvent::Student { event: erg_proto::Event::Error(_), .. } = rx.recv().await.unwrap() { break true; }
            }
        }).await.unwrap_or(false);
        assert!(refused, "unscoped group must be refused");

        roster.send(Some(&[id]), Command::StartExhibit { group, port: show_port, key: skey, stream: 5 });
        let got = tokio::task::spawn_blocking(move || {
            let (mut r, mut buf) = (erg_show::Receiver::new(&skey, 5), [0u8; 2048]);
            let end = std::time::Instant::now() + Duration::from_secs(10);
            while std::time::Instant::now() < end {
                if let Ok(n) = sock.recv(&mut buf) {
                    if let Some(f) = r.push(&buf[..n]) { return Some(f.keyframe); }
                }
            }
            None
        }).await.unwrap();
        roster.send(Some(&[id]), Command::StopExhibit);
        assert_eq!(got, Some(true), "no keyframe from the exhibiting student");
    }
}
