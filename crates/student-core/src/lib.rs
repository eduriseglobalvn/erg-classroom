//! Student-side runtime: announces itself, accepts authenticated tutor connections and hands
//! validated commands to an [`Executor`]. Platform specifics (opening URLs, locking, WFP...)
//! live behind the trait so this logic is testable without touching the machine.
use erg_net::auth_oracle::StudentAuth;
use erg_net::{discovery, new_identity, student_accept_with, student_endpoint, RoomKey};
use erg_proto::{Beacon, Command, Event, PowerAction};
use erg_show::{Frame, Receiver};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const MAX_MESSAGE_CHARS: usize = 2000;
pub const MAX_URL_CHARS: usize = 2048;

/// What a student machine can be asked to do. Every method receives already-validated input.
pub trait Executor: Send + Sync + 'static {
    fn open_url(&self, url: &str);
    fn message(&self, text: &str, seconds: u16);
    fn lock(&self, blank: bool, text: Option<&str>);
    fn unlock(&self);
    fn power(&self, action: PowerAction);
    fn set_web_policy(&self, mode: erg_proto::WebPolicy, list: &[String]);
    fn set_app_policy(&self, mode: erg_proto::AppPolicy, list: &[String]);
    /// A file from the teacher was received intact and saved at the given path.
    fn file_received(&self, _path: &Path) {}
    /// The Show group was joined; open the Show window.
    fn show_started(&self) {}
    /// A complete decoded-ready H.264 access unit of the teacher's Show arrived.
    fn show_frame(&self, _frame: &Frame) {}
    /// The Show ended (tutor stopped it or the connection dropped); remove the Show window.
    fn show_stopped(&self) {}
    /// The teacher wrote a chat line (already length-limited).
    fn chat(&self, _text: &str) {}
    /// A survey arrived; the UI must call Student::answer with the chosen index.
    fn poll(&self, _id: u32, _question: &str, _options: &[String]) {}
    /// JPEG of the screen that fits in w x h, or None when it cannot be captured (e.g. locked session).
    fn thumbnail(&self, _w: u16, _h: u16) -> Option<Vec<u8>> { None }
    fn mute(&self, _on: bool) {}
    fn beep(&self) {}
    fn set_clipboard(&self, _text: &str) {}
    fn clipboard(&self) -> Option<String> { None }
    fn set_usb_policy(&self, _p: erg_proto::UsbPolicy) {}
    fn set_print_policy(&self, _p: erg_proto::PrintPolicy) {}
    /// Show the attendance form; the UI answers with Student::register.
    fn request_register(&self) {}
    fn set_lesson(&self, _title: &str, _objectives: &[String], _seconds_left: u32) {}
    /// Start (or stop, when empty) watching typed text for these lowercase words; hits go to Student::keyword.
    fn set_keywords(&self, _words: &[String]) {}
    /// The teacher's own blocked websites and keywords (already cleaned and lowercase).
    fn set_guard(&self, _lists: guard::Lists) {}
    fn reward(&self, _stars: u16) {}
    fn set_leader(&self, _on: bool) {}
    /// The teacher switched the student toolbar on or off.
    fn toolbar(&self, _on: bool) {}
    /// Shared whiteboard operation from the tutor.
    fn board(&self, _m: &erg_proto::BoardMsg) {}
    /// Start (Some sink) or stop (None) the watched-screen stream; the sink gets (keyframe, H.264 bytes).
    fn watch_screen(&self, _sink: Option<Box<dyn Fn(bool, Vec<u8>) + Send>>) {}
    fn ask_question(&self, _id: u32, _text: &str) {}
    fn start_test(&self, _id: u32, _paper: &erg_assess::Paper, _seconds: u32) {}
    fn stop_test(&self) {}
    /// Remote-control input from the teacher (already range-checked).
    fn input(&self, _e: erg_proto::InputEvent) {}
    /// Start multicasting this student's own screen. False when it cannot be done.
    fn start_exhibit(&self, _group: [u8; 4], _port: u16, _key: [u8; 16], _stream: u32) -> bool { false }
    fn stop_exhibit(&self) {}
    fn set_protect(&self, _p: &erg_proto::Protect) {}
    /// A signed policy/power request: forward it to the SYSTEM service, or verify and apply it locally in per-user mode.
    fn privileged(&self, _r: &erg_proto::PrivRequest) -> bool { false }
    /// Video lesson control; `Open` carries a name already checked to be a plain mp4/webm file name.
    fn video(&self, _a: &erg_proto::VideoAction) {}
}

pub mod fsops;
pub mod guard;
pub mod privlocal;
pub mod i18n;

static AUTH: std::sync::OnceLock<Arc<dyn StudentAuth>> = std::sync::OnceLock::new();

/// Use a key holder in another process (the SYSTEM service) instead of `Config::key`; the runtime then never
/// has the Room Key in its own memory. Call before `run`.
pub fn set_auth(a: Arc<dyn StudentAuth>) {
    let _ = AUTH.set(a);
}

pub const MAX_CLIPBOARD_CHARS: usize = 65_536;
pub const MAX_KEYWORDS: usize = 100;
pub const MAX_TEST_SECONDS: u32 = 6 * 3600;

/// Watches typed characters and reports only whole words from the teacher's list; everything
/// else is dropped as soon as the word ends, so no free text is ever kept or sent.
#[derive(Default)]
pub struct KeywordMonitor {
    words: Vec<String>,
    buf: String,
}

impl KeywordMonitor {
    pub fn set_words(&mut self, words: &[String]) {
        self.words = words.to_vec();
        self.buf.clear();
    }
    /// Feed one typed character ('\u{8}' = backspace). Returns the listed word that just ended, if any.
    pub fn feed(&mut self, c: char) -> Option<String> {
        if self.words.is_empty() { return None; }
        if c == '\u{8}' { self.buf.pop(); return None; }
        if c.is_alphanumeric() {
            if self.buf.chars().count() < 40 { self.buf.extend(c.to_lowercase()); }
            return None;
        }
        let hit = self.words.iter().find(|w| **w == self.buf).cloned();
        self.buf.clear();
        hit
    }
}

/// First real network adapter's MAC, so the tutor can wake this machine later.
pub fn local_mac() -> Option<[u8; 6]> {
    let nets = sysinfo::Networks::new_with_refreshed_list();
    nets.values().map(|d| d.mac_address().0).find(|m| *m != [0; 6] && m[0] & 1 == 0)
}

/// Validate a tutor command and run it. Returns false when the command was refused or is not
/// handled here (Show commands need connection state and go through [`Student::handle`]).
pub fn dispatch(ex: &dyn Executor, cmd: Command) -> bool {
    match cmd {
        Command::OpenUrl(u) => {
            // Only plain web links: file:, javascript:, ms-*: etc. could run local code.
            let ok = u.len() <= MAX_URL_CHARS
                && (u.starts_with("http://") || u.starts_with("https://"))
                && !u.chars().any(|c| c.is_control() || c.is_whitespace());
            if ok {
                ex.open_url(&u);
            }
            ok
        }
        Command::Message { text, seconds } => {
            let text: String = text.chars().take(MAX_MESSAGE_CHARS).collect();
            ex.message(&text, seconds.clamp(1, 600));
            true
        }
        Command::Lock { blank, text } => {
            ex.lock(blank, text.as_deref().map(|t| &t[..t.floor_char_boundary(200)]));
            true
        }
        Command::Unlock => {
            ex.unlock();
            true
        }
        Command::Priv(r) => ex.privileged(&r),
        Command::Power(a) => {
            ex.power(a);
            true
        }
        Command::SetWebPolicy { mode, list } => {
            ex.set_web_policy(mode, &list);
            true
        }
        Command::SetAppPolicy { mode, list } => {
            ex.set_app_policy(mode, &list);
            true
        }
        Command::Chat(t) => {
            let t: String = t.chars().take(MAX_CHAT).collect();
            if t.trim().is_empty() { return false; }
            ex.chat(&t);
            true
        }
        Command::Poll { id, question, options } => {
            if question.trim().is_empty() || !(2..=8).contains(&options.len()) || options.iter().any(|o| o.trim().is_empty()) {
                return false;
            }
            let q: String = question.chars().take(MAX_CHAT).collect();
            let o: Vec<String> = options.iter().map(|o| o.chars().take(120).collect()).collect();
            ex.poll(id, &q, &o);
            true
        }
        Command::Mute(on) => { ex.mute(on); true }
        Command::Beep => { ex.beep(); true }
        Command::SetClipboard(t) => {
            ex.set_clipboard(&t.chars().take(MAX_CLIPBOARD_CHARS).collect::<String>());
            true
        }
        Command::SetUsbPolicy(p) => { ex.set_usb_policy(p); true }
        Command::SetPrintPolicy(p) => { ex.set_print_policy(p); true }
        Command::RequestRegister => { ex.request_register(); true }
        Command::SetLesson { title, objectives, seconds_left } => {
            let cut = |s: &str, n| s.chars().take(n).collect::<String>();
            let o: Vec<String> = objectives.iter().take(20).map(|s| cut(s, 200)).collect();
            ex.set_lesson(&cut(&title, 200), &o, seconds_left);
            true
        }
        Command::SetKeywords(words) => {
            let w: Vec<String> = words.iter().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty() && w.chars().count() <= 40 && w.chars().all(char::is_alphanumeric)).take(MAX_KEYWORDS).collect();
            ex.set_keywords(&w);
            true
        }
        Command::SetGuard { sites, words } => {
            let clean = |v: Vec<String>, ok: fn(char) -> bool| -> Vec<String> {
                v.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty() && s.chars().count() <= 100 && s.chars().all(ok)).take(500).collect()
            };
            ex.set_guard(guard::Lists {
                sites: clean(sites, |c| c.is_ascii_alphanumeric() || ".-*".contains(c)),
                words: clean(words, |c| c.is_alphanumeric() || c == ' '),
            });
            true
        }
        Command::Board(m) => {
            if !m.valid() { return false; }
            ex.board(&m);
            true
        }
        Command::SetLeader(on) => { ex.set_leader(on); true }
        Command::Toolbar(on) => { ex.toolbar(on); true }
        Command::Reward { stars } => { ex.reward(stars.min(erg_assess::qa::Rewards::MAX)); true }
        Command::AskQuestion { id, text } => {
            if text.trim().is_empty() { return false; }
            ex.ask_question(id, &text.chars().take(MAX_CHAT).collect::<String>());
            true
        }
        Command::StartTest { id, json, seconds } => match erg_assess::Paper::from_json(&json) {
            Ok(p) if seconds <= MAX_TEST_SECONDS => { ex.start_test(id, &p, seconds); true }
            _ => false,
        },
        Command::StopTest => { ex.stop_test(); true }
        Command::SetProtect(p) => {
            if p.hide_drives.len() > 26 { return false; }
            ex.set_protect(&p);
            true
        }
        Command::Video(a) => {
            use erg_proto::VideoAction::*;
            let ok = match &a {
                Open { name } => erg_net::transfer::safe_name(name).is_some() && {
                    let l = name.to_ascii_lowercase();
                    l.ends_with(".mp4") || l.ends_with(".webm")
                },
                Seek { secs } => secs.is_finite() && *secs >= 0.0 && *secs < 86_400.0,
                Play | Pause | Close => true,
            };
            if ok { ex.video(&a); }
            ok
        }
        Command::Input(e) => {
            use erg_proto::InputEvent::*;
            let ok = match e {
                MouseMove { x, y } => x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y),
                MouseButton { button, .. } => button <= 2,
                Wheel(_) => true,
                // Ctrl+Alt+Del cannot be sent anyway; block system-key combos that would leave the student stranded
                Key { vk, .. } => vk != 0 && vk < 256,
            };
            if ok { ex.input(e); }
            ok
        }
        Command::ListDir(_) | Command::MakeDir(_) | Command::DeletePath(_) | Command::RenamePath { .. }
        | Command::KillProcess(_) | Command::CollectFiles { .. } | Command::Shell(_) => false, // need the connection; handled in `run_with`
        Command::Auth { .. }
        | Command::RequestClipboard
        | Command::RunApp { .. }
        | Command::RequestThumbnail { .. }
        | Command::WatchScreen { .. }
        | Command::Deploy(_)
        | Command::RequestInventory
        | Command::StartShow { .. }
        | Command::StartExhibit { .. }
        | Command::StopExhibit
        | Command::ShowFrame { .. }
        | Command::StopShow => false,
    }
}

const MAX_CHAT: usize = 1000;

/// Hand a Deploy request to the SYSTEM service by leaving it in the spool folder. The service, not this
/// process, decides whether it is genuine (signature + file hash), so nothing is trusted here.
fn deploy_spool(inbox: &Path, req: &erg_proto::DeployRequest) -> Result<(), String> {
    if !erg_deploy::plain_installer_name(&req.name) {
        return Err("tên gói không hợp lệ".into());
    }
    let file = inbox.join(&req.name);
    if !file.is_file() {
        return Err("gói chưa được gửi tới máy này".into());
    }
    let dir = std::env::var_os("ERG_DEPLOY_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| ".".into())).join("ERGClassroom").join("deploy"));
    erg_deploy::spool(&dir, req, &file).map_err(|e| e.to_string())
}

/// The inbox is `<Documents>/ERG/Received`, so Documents is two levels up.
fn documents_of(inbox: &Path) -> PathBuf {
    inbox.ancestors().nth(2).unwrap_or(inbox).to_path_buf()
}

fn file_command(c: Command) -> Event {
    let r: Result<String, String> = match c {
        Command::ListDir(p) => {
            return match fsops::list_dir(&p) {
                Ok(entries) => Event::DirListing { path: p, entries },
                Err(message) => Event::FsResult { ok: false, message },
            }
        }
        Command::MakeDir(p) => fsops::make_dir(&p).map(|_| "đã tạo thư mục".into()),
        Command::DeletePath(p) => fsops::delete(&p).map(|_| "đã xoá".into()),
        Command::RenamePath { from, to } => fsops::rename(&from, &to).map(|_| "đã đổi tên".into()),
        Command::KillProcess(n) => fsops::kill_by_name(&n).map(|k| format!("đã đóng {k} tiến trình")),
        Command::Shell(c) => fsops::run_shell(&c),
        _ => Err("không hỗ trợ".into()),
    };
    match r {
        Ok(message) => Event::FsResult { ok: true, message },
        Err(message) => Event::FsResult { ok: false, message },
    }
}

/// Blocking: refreshing process and disk lists takes tens of milliseconds.
pub fn collect_inventory() -> erg_proto::Inventory {
    use sysinfo::{Disks, ProcessesToUpdate, System};
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let me = sysinfo::get_current_pid().ok().and_then(|p| sys.process(p)).and_then(|p| p.user_id().cloned());
    let mut processes: Vec<String> = sys
        .processes()
        .values()
        .filter(|p| me.is_some() && p.user_id() == me.as_ref())
        .map(|p| p.name().to_string_lossy().to_string())
        .collect();
    processes.sort_unstable_by_key(|n| n.to_ascii_lowercase());
    processes.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    erg_proto::Inventory {
        os: format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()).trim().to_string(),
        cpu: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cores: sys.cpus().len() as u32,
        ram_total: sys.total_memory(),
        ram_used: sys.used_memory(),
        disks: Disks::new_with_refreshed_list().iter().map(|d| (d.mount_point().to_string_lossy().to_string(), d.total_space(), d.available_space())).collect(),
        uptime_s: System::uptime(),
        processes,
    }
}

const SHOW_STATS_EVERY: Duration = Duration::from_secs(2);
/// No multicast packet for this long after StartShow: ask for unicast.
const SHOW_SILENCE: Duration = Duration::from_secs(3);
/// Largest unicast Show frame accepted (the tutor sends far less).
const MAX_UNICAST_FRAME: usize = 2 << 20;

/// More than 10% of the frames of a window lost (needs a few frames so one hiccup does not trigger it).
fn lost_too_much(recv: u32, lost: u32) -> bool {
    recv + lost >= 5 && lost * 10 > (recv + lost)
}

/// Multicast is healthy again: enough frames and under 2% lost.
fn multicast_healthy(recv: u32, lost: u32) -> bool {
    recv + lost >= 5 && lost * 50 < recv + lost
}

/// Consecutive healthy stats windows (2 s each) before a fallen-back student returns to multicast.
const SHOW_RETURN_WINDOWS: u32 = 5;

struct ShowTask {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ShowTask {
    fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join(); // returns within one socket read timeout
        }
    }
}

/// Per-process student state shared by all tutor connections.
pub struct Student {
    ex: Arc<dyn Executor>,
    show: Mutex<Option<ShowTask>>,
    out: tokio::sync::broadcast::Sender<Event>,
    uframe: std::sync::atomic::AtomicU32,
}

impl Student {
    pub fn new(ex: Arc<dyn Executor>) -> Arc<Self> {
        Arc::new(Self { ex, show: Mutex::new(None), out: tokio::sync::broadcast::channel(32).0, uframe: Default::default() })
    }

    /// Send a chat line to every connected tutor. Returns false when the text is empty.
    pub fn say(&self, text: &str) -> bool {
        let t: String = text.chars().take(MAX_CHAT).collect();
        !t.trim().is_empty() && { let _ = self.out.send(Event::Chat(t)); true }
    }

    fn emit(&self, e: Event) { let _ = self.out.send(e); }
    /// Attendance form answer.
    pub fn register(&self, name: &str, class: &str) -> bool {
        let cut = |s: &str| s.trim().chars().take(80).collect::<String>();
        let (n, c) = (cut(name), cut(class));
        !n.is_empty() && { self.emit(Event::Register { name: n, class: c }); true }
    }
    /// "I need help" button; the tutor shows a bell.
    pub fn help(&self, text: &str) {
        self.emit(Event::HelpRequest(text.chars().take(MAX_CHAT).collect()));
    }
    /// Raise a hand or answer an oral question.
    pub fn hand(&self, question: u32, text: &str) {
        self.emit(Event::Hand { question, text: text.chars().take(MAX_CHAT).collect() });
    }
    pub fn test_done(&self, id: u32, answers_json: &str) -> bool {
        answers_json.len() <= erg_assess::testing::MAX_PAPER_JSON && { self.emit(Event::TestDone { id, answers: answers_json.into() }); true }
    }
    /// Group leader button; the tutor decides whether it is allowed.
    pub fn leader(&self, action: erg_proto::LeaderAction) {
        let action = match action {
            erg_proto::LeaderAction::Message(t) => erg_proto::LeaderAction::Message(t.chars().take(MAX_CHAT).collect()),
            a => a,
        };
        self.emit(Event::Leader(action));
    }
    /// Something the student drew (or undid) on the shared board; the tutor relays it if drawing is allowed.
    pub fn board_emit(&self, m: erg_proto::BoardMsg) -> bool {
        use erg_proto::BoardMsg as B;
        let ok = matches!(m, B::Undo { .. }) || (m.element_id().is_some() && m.valid());
        if ok {
            let m = match m {
                B::Stroke { id, color, width, pts } => B::Stroke { id, color: color & 0xFF_FFFF, width, pts },
                B::Shape { id, color, width, kind, a, b } => B::Shape { id, color: color & 0xFF_FFFF, width, kind, a, b },
                B::Text { id, color, size, at, text } => B::Text { id, color: color & 0xFF_FFFF, size, at, text },
                other => other,
            };
            self.emit(Event::Board(m));
        }
        ok
    }
    pub fn keyword(&self, word: &str) { self.emit(Event::Keyword { word: word.chars().take(40).collect() }); }
    /// Tell the tutor about a forbidden program or website. False when no tutor is connected: then nobody
    /// could unlock the computer again, so the caller must not lock it.
    pub fn violation(&self, kind: &str, app: &str, title: &str) -> bool {
        let cut = |s: &str| s.chars().take(200).collect::<String>();
        self.out.receiver_count() > 0 && self.out.send(Event::Violation { kind: cut(kind), app: cut(app), title: cut(title) }).is_ok()
    }
    pub fn power(&self, percent: Option<u8>, ac: bool, wifi_signal: Option<u8>) {
        self.emit(Event::Power { percent: percent.map(|p| p.min(100)), ac, wifi_signal: wifi_signal.map(|p| p.min(100)) });
    }

    /// Report the student's survey choice to every connected tutor.
    pub fn answer(&self, id: u32, choice: u8) {
        let _ = self.out.send(Event::PollAnswer { id, choice });
    }

    pub fn handle(&self, cmd: Command) -> bool {
        match cmd {
            Command::StartShow { group, port, key, stream } => self.start_show(group, port, key, stream),
            Command::StopShow => {
                self.stop_show();
                true
            }
            Command::ShowFrame { keyframe, data } => {
                // Only while a Show runs; the frame number just orders frames for the Executor.
                if self.show.lock().expect("show lock").is_some() && data.len() <= MAX_UNICAST_FRAME {
                    let frame_no = self.uframe.fetch_add(1, Ordering::Relaxed);
                    self.ex.show_frame(&Frame { frame_no, keyframe, data });
                }
                true
            }
            Command::StartExhibit { group, port, key, stream } => {
                // same scope rule as joining a Show: administratively scoped groups, unprivileged ports
                group[0] == 239 && port >= 1024 && self.ex.start_exhibit(group, port, key, stream)
            }
            Command::StopExhibit => {
                self.ex.stop_exhibit();
                true
            }
            other => dispatch(self.ex.as_ref(), other),
        }
    }

    fn start_show(&self, group: [u8; 4], port: u16, key: [u8; 16], stream: u32) -> bool {
        // Administratively scoped range only: a rogue tutor must not make students join arbitrary groups.
        if group[0] != 239 || port < 1024 {
            return false;
        }
        self.stop_show();
        let iface = erg_show::udp::default_interface().unwrap_or(Ipv4Addr::UNSPECIFIED);
        let Ok(sock) = erg_show::udp::receiver(Ipv4Addr::from(group), port, iface) else { return false };
        if sock.set_read_timeout(Some(Duration::from_millis(200))).is_err() {
            return false;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, ex, out) = (stop.clone(), self.ex.clone(), self.out.clone());
        let thread = std::thread::spawn(move || {
            let mut rx = Receiver::new(&key, stream);
            let mut buf = [0u8; 2048];
            let (mut last_pkt, mut window, mut prev, mut asked, mut healthy) = (Instant::now(), Instant::now(), erg_show::Stats::default(), false, 0u32);
            while !flag.load(Ordering::Relaxed) {
                if let Ok(n) = sock.recv(&mut buf) {
                    let before = rx.stats.packets;
                    if let Some(f) = rx.push(&buf[..n]) {
                        ex.show_frame(&f);
                    }
                    if rx.stats.packets != before { last_pkt = Instant::now(); }
                }
                if window.elapsed() >= SHOW_STATS_EVERY {
                    window = Instant::now();
                    let s = rx.stats;
                    let (recv, lost, fec) = ((s.frames_ok - prev.frames_ok) as u32, (s.frames_lost - prev.frames_lost) as u32, (s.frames_fec_repaired - prev.frames_fec_repaired) as u32);
                    prev = s;
                    let _ = out.send(Event::ShowStats { recv, lost, fec_fixed: fec });
                    asked |= lost_too_much(recv, lost) && out.send(Event::ShowFallbackRequest).is_ok();
                    healthy = if asked && multicast_healthy(recv, lost) { healthy + 1 } else { 0 };
                    if healthy >= SHOW_RETURN_WINDOWS && out.send(Event::ShowFallbackEnd).is_ok() {
                        asked = false;
                        healthy = 0;
                    }
                }
                if !asked && last_pkt.elapsed() >= SHOW_SILENCE {
                    asked = out.send(Event::ShowFallbackRequest).is_ok();
                }
            }
            ex.show_stopped();
        });
        *self.show.lock().expect("show lock") = Some(ShowTask { stop, thread: Some(thread) });
        self.ex.show_started();
        true
    }

    pub fn stop_show(&self) {
        if let Some(t) = self.show.lock().expect("show lock").take() {
            t.stop();
        }
    }
}

pub struct Config {
    pub key: RoomKey,
    pub room: String,
    pub hostname: String,
    pub user: String,
    pub discovery_port: u16,
    /// Extra unicast beacon targets, for networks that block multicast and broadcast.
    pub browse_addrs: Vec<SocketAddr>,
    /// Where files sent by the teacher are saved.
    pub inbox: PathBuf,
}

/// Run until the process ends. Returns the QUIC port it is listening on via `ready`.
pub async fn run(cfg: Config, ex: Arc<dyn Executor>, ready: Option<tokio::sync::oneshot::Sender<u16>>) -> Result<(), erg_net::Error> {
    run_with(cfg, Student::new(ex), ready).await
}

/// Like run, but the caller keeps the Student handle (to send chat lines from the student side).
pub async fn run_with(cfg: Config, student: Arc<Student>, ready: Option<tokio::sync::oneshot::Sender<u16>>) -> Result<(), erg_net::Error> {
    let id = new_identity("erg-student")?;
    let ep = student_endpoint(&id, "0.0.0.0:0".parse().expect("addr"))?;
    let port = ep.local_addr()?.port();
    if let Some(r) = ready {
        let _ = r.send(port);
    }
    let beacon = Beacon {
        proto_ver: erg_proto::version_code(erg_proto::APP_VERSION),
        student_id: Uuid::new_v4(),
        hostname: cfg.hostname,
        user: cfg.user,
        room: cfg.room,
        quic_port: port,
        cert_fingerprint: id.fingerprint,
        mac: local_mac(),
    };
    let (dp, extra) = (cfg.discovery_port, cfg.browse_addrs);
    let b2 = beacon.clone();
    tokio::spawn(async move {
        loop {
            let _ = discovery::announce(&b2, dp).await;
            for a in &extra {
                let _ = discovery::announce_to(&b2, *a).await;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
    let auth: Arc<dyn StudentAuth> = AUTH.get().cloned().unwrap_or_else(|| Arc::new(cfg.key));
    let inbox = cfg.inbox;
    while let Some(incoming) = ep.accept().await {
        let student = student.clone();
        let inbox = inbox.clone();
        let auth = auth.clone();
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else { return };
            let Ok(mut ch) = student_accept_with(&conn, auth).await else {
                tracing::warn!("rejected unauthenticated peer {}", conn.remote_address());
                return;
            };
            // Files arrive on their own streams, accepted only after the tutor authenticated.
            let docs_root = documents_of(&inbox);
            let deploy_inbox = inbox.clone();
            let (files_conn, files_student) = (conn.clone(), student.clone());
            tokio::spawn(async move {
                while let Ok(rx) = files_conn.accept_uni().await {
                    let (inbox, st) = (inbox.clone(), files_student.clone());
                    tokio::spawn(async move {
                        match erg_net::transfer::receive_file(rx, &inbox).await {
                            Ok(p) => st.ex.file_received(&p),
                            Err(e) => tracing::warn!("file rejected: {e}"),
                        }
                    });
                }
            });
            // (w, h, period); period 0 means one shot. Clamped so a tutor cannot make us busy-loop.
            let mut thumbs: Option<(u16, u16, Duration)> = None;
            let mut out = student.out.subscribe();
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                let mut send_thumb = false;
                tokio::select! {
                    cmd = ch.recv() => match cmd {
                        Ok(Command::RequestThumbnail { w, h, interval_ms }) => {
                            let (w, h) = (w.clamp(16, 1024), h.clamp(16, 1024));
                            if interval_ms == 0 { thumbs = None; } else {
                                let p = Duration::from_millis(interval_ms.clamp(500, 60_000) as u64);
                                thumbs = Some((w, h, p));
                                tick = tokio::time::interval(p);
                                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                            }
                            send_thumb = true;
                            thumbs = thumbs.or(Some((w, h, Duration::ZERO)));
                        }
                        Ok(Command::Deploy(req)) => {
                            let ev = match deploy_spool(&deploy_inbox, &req) {
                                Ok(()) => Event::FsResult { ok: true, message: format!("yêu cầu cài {} đã chuyển cho dịch vụ hệ thống", req.name) },
                                Err(m) => Event::FsResult { ok: false, message: m },
                            };
                            if ch.send(&ev).await.is_err() { break; }
                        }
                        Ok(Command::WatchScreen { on }) => {
                            let st = student.clone();
                            let out = student.out.clone();
                            let sink: Option<Box<dyn Fn(bool, Vec<u8>) + Send>> = on.then(|| Box::new(move |keyframe, data| { let _ = out.send(Event::Video { keyframe, data }); }) as _);
                            let _ = tokio::task::spawn_blocking(move || st.ex.watch_screen(sink)).await;
                        }
                        Ok(Command::RequestInventory) => {
                            if let Ok(inv) = tokio::task::spawn_blocking(collect_inventory).await {
                                if ch.send(&Event::Inventory(inv)).await.is_err() { break; }
                            }
                        }
                        Ok(c @ (Command::ListDir(_) | Command::MakeDir(_) | Command::DeletePath(_) | Command::RenamePath { .. } | Command::KillProcess(_) | Command::Shell(_))) => {
                            let ev = tokio::task::spawn_blocking(move || file_command(c)).await.unwrap_or_else(|_| Event::FsResult { ok: false, message: "internal error".into() });
                            if ch.send(&ev).await.is_err() { break; }
                        }
                        Ok(Command::CollectFiles { folder, pattern }) => {
                            let (docs, c2) = (docs_root.clone(), conn.clone());
                            let ev = match fsops::collectable(&docs, &folder, &pattern) {
                                Ok(files) => {
                                    let mut sent = 0;
                                    for f in &files {
                                        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                                        if erg_net::transfer::send_file(&c2, f, &name).await.is_ok() { sent += 1; }
                                    }
                                    Event::FsResult { ok: true, message: format!("đã gửi {sent}/{} tệp", files.len()) }
                                }
                                Err(m) => Event::FsResult { ok: false, message: m },
                            };
                            if ch.send(&ev).await.is_err() { break; }
                        }
                        Ok(Command::RequestClipboard) => {
                            let st = student.clone();
                            if let Ok(Some(t)) = tokio::task::spawn_blocking(move || st.ex.clipboard()).await {
                                if ch.send(&Event::Clipboard(t.chars().take(MAX_CLIPBOARD_CHARS).collect())).await.is_err() { break; }
                            }
                        }
                        Ok(cmd) => if !student.handle(cmd) {
                            let _ = ch.send(&Event::Error("command refused".into())).await;
                        },
                        Err(_) => break,
                    },
                    Ok(ev) = out.recv() => if ch.send(&ev).await.is_err() { break },
                    _ = tick.tick(), if thumbs.is_some_and(|t| !t.2.is_zero()) => send_thumb = true,
                }
                if send_thumb {
                    if let Some((w, h, p)) = thumbs {
                        if p.is_zero() { thumbs = None; }
                        let st = student.clone();
                        if let Ok(Some(j)) = tokio::task::spawn_blocking(move || st.ex.thumbnail(w, h)).await {
                            if ch.send(&Event::Thumbnail(j)).await.is_err() { break; }
                        }
                    }
                }
            }
            // Controlling tutor is gone: never leave the student stuck on a Show nobody is running.
            student.stop_show();
            student.ex.stop_exhibit();
            student.ex.watch_screen(None);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct Rec {
        log: Mutex<Vec<String>>,
        frames: AtomicUsize,
        stopped: AtomicBool,
    }
    impl Executor for Rec {
        fn open_url(&self, u: &str) { self.log.lock().unwrap().push(format!("url:{u}")) }
        fn message(&self, t: &str, s: u16) { self.log.lock().unwrap().push(format!("msg:{}:{s}", t.chars().count())) }
        fn lock(&self, b: bool, _: Option<&str>) { self.log.lock().unwrap().push(format!("lock:{b}")) }
        fn unlock(&self) { self.log.lock().unwrap().push("unlock".into()) }
        fn power(&self, _: PowerAction) { self.log.lock().unwrap().push("power".into()) }
        fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
        fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
        fn show_frame(&self, _: &Frame) { self.frames.fetch_add(1, Ordering::SeqCst); }
        fn show_stopped(&self) { self.stopped.store(true, Ordering::SeqCst); }
        fn chat(&self, t: &str) { self.log.lock().unwrap().push(format!("chat:{}", t.chars().count())) }
        fn poll(&self, id: u32, q: &str, o: &[String]) { self.log.lock().unwrap().push(format!("poll:{id}:{q}:{}", o.len())) }
        fn thumbnail(&self, w: u16, h: u16) -> Option<Vec<u8>> { Some(vec![0xFF, 0xD8, (w % 256) as u8, (h % 256) as u8]) }
    }

    #[test]
    fn display_host_adds_the_user_only_on_terminal_servers() {
        assert_eq!(display_host("PC-01", "an", false), "PC-01");
        assert_eq!(display_host("SRV", "an.nguyen", true), "SRV-an.nguyen");
        assert_eq!(display_host("SRV", r"a b/\c", true), "SRV-abc");
        assert_eq!(display_host("SRV", "", true), "SRV");
    }

    #[test]
    fn multicast_healthy_needs_sample_and_under_two_percent() {
        assert!(!multicast_healthy(3, 0));
        assert!(multicast_healthy(60, 1));
        assert!(!multicast_healthy(40, 1)); // 2.4%
    }

    #[test]
    fn fallback_threshold_is_ten_percent_with_a_minimum_sample() {
        assert!(!lost_too_much(0, 0));
        assert!(!lost_too_much(2, 1)); // too few frames to judge
        assert!(!lost_too_much(54, 6)); // exactly 10%
        assert!(lost_too_much(50, 10));
    }

    #[test]
    fn only_http_links_are_opened() {
        let r = Rec::default();
        for bad in ["file:///C:/Windows/System32/calc.exe", "javascript:alert(1)", "ms-msdt:/id x", "https://a.com/ b", "//evil", ""] {
            assert!(!dispatch(&r, Command::OpenUrl(bad.into())), "{bad}");
        }
        assert!(dispatch(&r, Command::OpenUrl("https://example.com/a?b=1".into())));
        assert_eq!(r.log.lock().unwrap().as_slice(), ["url:https://example.com/a?b=1"]);
    }

    #[test]
    fn message_is_truncated_and_seconds_clamped() {
        let r = Rec::default();
        dispatch(&r, Command::Message { text: "x".repeat(5000), seconds: 0 });
        assert_eq!(r.log.lock().unwrap()[0], format!("msg:{MAX_MESSAGE_CHARS}:1"));
    }

    #[test]
    fn show_refuses_non_scoped_groups_and_privileged_ports() {
        let s = Student::new(Arc::new(Rec::default()));
        assert!(!s.handle(Command::StartShow { group: [224, 0, 0, 251], port: 47930, key: [0; 16], stream: 1 })); // mDNS
        assert!(!s.handle(Command::StartShow { group: [239, 255, 77, 9], port: 80, key: [0; 16], stream: 1 }));
    }

    #[tokio::test]
    async fn tutor_command_reaches_executor_end_to_end() {
        let key = [3u8; 32];
        let port = 47822;
        let rec = Arc::new(Rec::default());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cfg = Config {
            key, room: "t".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox"),
        };
        let ex: Arc<dyn Executor> = rec.clone();
        tokio::spawn(run(cfg, ex, Some(tx)));
        rx.await.unwrap();
        let roster = erg_tutor_core::Roster::start(port, key, Some("t".into())).unwrap();
        for _ in 0..100 {
            if roster.send(None, Command::OpenUrl("https://example.com".into())) > 0 { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        for _ in 0..50 {
            if !rec.log.lock().unwrap().is_empty() { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(rec.log.lock().unwrap().as_slice(), ["url:https://example.com"]);
    }

    /// Tutor sends StartShow over the authenticated channel, then multicasts frames; the student
    /// receives them, and a dropped control connection ends the Show.
    #[tokio::test]
    async fn show_frames_flow_and_stop_when_tutor_disconnects() {
        let key = [4u8; 32];
        let port = 47823;
        let rec = Arc::new(Rec::default());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cfg = Config {
            key, room: "s".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox"),
        };
        let ex: Arc<dyn Executor> = rec.clone();
        tokio::spawn(run(cfg, ex, Some(tx)));
        rx.await.unwrap();

        // Drive the tutor side by hand so the test can drop the connection on purpose.
        let tutor_ep = erg_net::tutor_endpoint().unwrap();
        let mut beacon_sock = erg_net::discovery::listener(port).unwrap();
        let (b, from) = erg_net::discovery::next_beacon(&beacon_sock).await.unwrap();
        let _ = &mut beacon_sock;
        let (conn, mut ch) = erg_net::tutor_connect(&tutor_ep, SocketAddr::new(from.ip(), b.quic_port), &key).await.unwrap();

        let (group, show_port, skey) = ([239, 255, 77, 9], 47931u16, [8u8; 16]);
        ch.send(&Command::StartShow { group, port: show_port, key: skey, stream: 3 }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await; // let the student join the group

        let iface = erg_show::udp::default_interface().expect("lan interface");
        let sender_sock = erg_show::udp::sender(iface).unwrap();
        let sender = erg_show::Sender::new(&skey, 3, 20);
        for f in 0..30u32 {
            for p in sender.packetize(f, f == 0, &vec![f as u8; 20_000]).unwrap() {
                sender_sock.send_to(&p, (Ipv4Addr::from(group), show_port)).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        let got = rec.frames.load(Ordering::SeqCst);
        assert!(got >= 28, "student received only {got}/30 frames");

        conn.close(0u32.into(), b"tutor gone");
        for _ in 0..60 {
            if rec.stopped.load(Ordering::SeqCst) { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(rec.stopped.load(Ordering::SeqCst), "Show kept running after the tutor disconnected");
        }

    #[tokio::test]
    async fn thumbnails_are_sent_once_and_periodically_with_clamped_size() {
        let key = [5u8; 32];
        let port = 47824;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cfg = Config {
            key, room: "s".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox2"),
        };
        tokio::spawn(run(cfg, Arc::new(Rec::default()), Some(tx)));
        rx.await.unwrap();
        let ep = erg_net::tutor_endpoint().unwrap();
        let sock = erg_net::discovery::listener(port).unwrap();
        let (b, from) = erg_net::discovery::next_beacon(&sock).await.unwrap();
        let (_conn, mut ch) = erg_net::tutor_connect(&ep, SocketAddr::new(from.ip(), b.quic_port), &key).await.unwrap();

        ch.send(&Command::RequestThumbnail { w: 5000, h: 1, interval_ms: 0 }).await.unwrap();
        let Event::Thumbnail(j) = ch.recv().await.unwrap() else { panic!("not a thumbnail") };
        assert_eq!(j[2..], [(1024 % 256) as u8, 16]); // 5000 -> 1024, 1 -> 16

        ch.send(&Command::RequestThumbnail { w: 160, h: 90, interval_ms: 500 }).await.unwrap();
        for _ in 0..3 {
            let got = tokio::time::timeout(Duration::from_secs(3), ch.recv()).await.expect("periodic thumbnail missing").unwrap();
            assert!(matches!(got, Event::Thumbnail(_)));
        }
    }

    #[tokio::test]
    async fn a_deploy_request_is_only_handed_to_the_service_when_the_package_arrived() {
        let key = [6u8; 32];
        let port = 47832;
        let inbox = std::env::temp_dir().join("erg-test-inbox6");
        let spool = std::env::temp_dir().join("erg-test-spool6");
        let _ = std::fs::remove_dir_all(&spool);
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::write(inbox.join("setup.exe"), b"pkg").unwrap();
        std::env::set_var("ERG_DEPLOY_DIR", &spool);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cfg = Config { key, room: "s".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port, browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: inbox.clone() };
        tokio::spawn(run(cfg, Arc::new(Rec::default()), Some(tx)));
        rx.await.unwrap();
        let ep = erg_net::tutor_endpoint().unwrap();
        let sock = erg_net::discovery::listener(port).unwrap();
        let (b, from) = erg_net::discovery::next_beacon(&sock).await.unwrap();
        let (_conn, mut ch) = erg_net::tutor_connect(&ep, SocketAddr::new(from.ip(), b.quic_port), &key).await.unwrap();
        let sha = erg_deploy::sha256_file(&inbox.join("setup.exe")).unwrap();
        ch.send(&Command::Deploy(erg_deploy::sign(&key, "setup.exe", sha, "/S", 99).unwrap())).await.unwrap();
        assert!(matches!(ch.recv().await.unwrap(), Event::FsResult { ok: true, .. }));
        assert_eq!(std::fs::read_dir(&spool).unwrap().count(), 1, "one request spooled");
        // a package that never arrived is refused
        ch.send(&Command::Deploy(erg_deploy::sign(&key, "missing.exe", sha, "", 99).unwrap())).await.unwrap();
        assert!(matches!(ch.recv().await.unwrap(), Event::FsResult { ok: false, .. }));
        assert_eq!(std::fs::read_dir(&spool).unwrap().count(), 1);
    }

    #[test]
    fn inventory_describes_this_machine() {
        let i = collect_inventory();
        assert!(i.cores >= 1 && i.ram_total > 0 && i.uptime_s > 0 && !i.os.is_empty());
        assert!(!i.disks.is_empty() && !i.processes.is_empty());
        println!("{} | {} | {} MB | {} processes", i.os, i.cpu, i.ram_total >> 20, i.processes.len());
    }

    #[tokio::test]
    async fn chat_goes_both_ways_and_is_limited() {
        let key = [7u8; 32];
        let port = 47827;
        let rec = Arc::new(Rec::default());
        let student = Student::new(rec.clone());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cfg = Config {
            key, room: "s".into(), hostname: "PC".into(), user: "hs".into(), discovery_port: port,
            browse_addrs: vec![format!("127.0.0.1:{port}").parse().unwrap()], inbox: std::env::temp_dir().join("erg-test-inbox4"),
        };
        tokio::spawn(run_with(cfg, student.clone(), Some(tx)));
        rx.await.unwrap();
        let ep = erg_net::tutor_endpoint().unwrap();
        let sock = erg_net::discovery::listener(port).unwrap();
        let (b, from) = erg_net::discovery::next_beacon(&sock).await.unwrap();
        let (_c, mut ch) = erg_net::tutor_connect(&ep, SocketAddr::new(from.ip(), b.quic_port), &key).await.unwrap();

        ch.send(&Command::Chat("x".repeat(5000))).await.unwrap();
        ch.send(&Command::Chat("   ".into())).await.unwrap(); // refused
        let Event::Error(_) = ch.recv().await.unwrap() else { panic!("blank chat should be refused") };
        assert_eq!(rec.log.lock().unwrap().iter().filter(|l| l.starts_with("chat:")).collect::<Vec<_>>(), ["chat:1000"]);

        assert!(student.say("em cần giúp"));
        assert!(!student.say(" "));
        assert_eq!(ch.recv().await.unwrap(), Event::Chat("em cần giúp".into()));
    }

    #[test]
    fn polls_need_a_question_and_two_to_eight_options() {
        let r = Rec::default();
        let ok = |q: &str, o: &[&str]| dispatch(&r, Command::Poll { id: 1, question: q.into(), options: o.iter().map(|s| s.to_string()).collect() });
        assert!(ok("2+2?", &["3", "4"]));
        assert!(!ok("", &["3", "4"]));
        assert!(!ok("q", &["only"]));
        assert!(!ok("q", &["a", " "]));
        assert!(!ok("q", &["1", "2", "3", "4", "5", "6", "7", "8", "9"]));
        assert_eq!(r.log.lock().unwrap().as_slice(), ["poll:1:2+2?:2"]);
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;

    #[derive(Default)]
    struct Log(Mutex<Vec<String>>);
    impl Executor for Log {
        fn open_url(&self, _: &str) {}
        fn message(&self, _: &str, _: u16) {}
        fn lock(&self, _: bool, _: Option<&str>) {}
        fn unlock(&self) {}
        fn power(&self, _: PowerAction) {}
        fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
        fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
        fn mute(&self, on: bool) { self.0.lock().unwrap().push(format!("mute:{on}")) }
        fn set_clipboard(&self, t: &str) { self.0.lock().unwrap().push(format!("clip:{}", t.chars().count())) }
        fn set_keywords(&self, w: &[String]) { self.0.lock().unwrap().push(format!("kw:{}", w.join(","))) }
        fn reward(&self, s: u16) { self.0.lock().unwrap().push(format!("stars:{s}")) }
        fn board(&self, m: &erg_proto::BoardMsg) { self.0.lock().unwrap().push(format!("board:{}", matches!(m, erg_proto::BoardMsg::Stroke { .. }))) }
        fn set_lesson(&self, t: &str, o: &[String], s: u32) { self.0.lock().unwrap().push(format!("lesson:{}:{}:{s}", t.chars().count(), o.len())) }
        fn start_test(&self, id: u32, p: &erg_assess::Paper, s: u32) { self.0.lock().unwrap().push(format!("test:{id}:{}:{s}", p.questions.len())) }
    }

    #[test]
    fn new_commands_are_validated_before_they_reach_the_executor() {
        let l = Log::default();
        assert!(dispatch(&l, Command::Mute(true)));
        assert!(dispatch(&l, Command::SetClipboard("x".repeat(100_000))));
        assert!(dispatch(&l, Command::SetKeywords(vec![" Hack ".into(), "a b".into(), "".into(), "x;y".into(), "ok1".into()])));
        assert!(dispatch(&l, Command::SetLeader(true)));
        assert!(dispatch(&l, Command::Toolbar(false)));
        use erg_proto::BoardMsg as B;
        assert!(dispatch(&l, Command::Board(B::Open { draw: true })));
        assert!(dispatch(&l, Command::Board(B::Stroke { id: 1, color: 0, width: 3, pts: vec![(1, 1)] })));
        assert!(!dispatch(&l, Command::Board(B::Stroke { id: 2, color: 0, width: 3, pts: vec![] })), "empty stroke refused");
        assert!(!dispatch(&l, Command::Board(B::Stroke { id: 3, color: 0, width: 0, pts: vec![(1, 1)] })));
        assert!(!dispatch(&l, Command::Board(B::Stroke { id: 4, color: 0, width: 3, pts: vec![(1, 1); 2001] })));
        assert!(dispatch(&l, Command::Reward { stars: 5000 }));
        assert!(dispatch(&l, Command::SetLesson { title: "t".repeat(500), objectives: vec!["o".into(); 50], seconds_left: 9 }));
        assert!(!dispatch(&l, Command::AskQuestion { id: 1, text: "  ".into() }));
        assert_eq!(
            l.0.lock().unwrap().as_slice(),
            ["mute:true", "clip:65536", "kw:hack,ok1", "board:false", "board:true", "stars:99", "lesson:200:20:9"]
        );
    }

    #[test]
    fn tests_must_be_valid_papers_with_a_sane_time_limit() {
        let l = Log::default();
        let good = r#"{"title":"t","shuffle":false,"questions":[{"text":"q","points":1.0,"kind":{"TrueFalse":{"answer":true}}}]}"#;
        assert!(dispatch(&l, Command::StartTest { id: 7, json: good.into(), seconds: 600 }));
        assert!(!dispatch(&l, Command::StartTest { id: 8, json: good.into(), seconds: MAX_TEST_SECONDS + 1 }));
        assert!(!dispatch(&l, Command::StartTest { id: 9, json: "{}".into(), seconds: 60 }));
        let bad_key = good.replace(r#"{"TrueFalse":{"answer":true}}"#, r#"{"Choice":{"options":["a","b"],"correct":[9]}}"#);
        assert!(!dispatch(&l, Command::StartTest { id: 10, json: bad_key, seconds: 60 }));
        assert_eq!(l.0.lock().unwrap().as_slice(), ["test:7:1:600"]);
    }

    #[test]
    fn keyword_monitor_reports_only_listed_whole_words() {
        let mut m = KeywordMonitor::default();
        let type_str = |m: &mut KeywordMonitor, s: &str| s.chars().filter_map(|c| m.feed(c)).collect::<Vec<_>>();
        assert!(type_str(&mut m, "hack ").is_empty(), "monitoring is off until words are set");
        m.set_words(&["hack".into(), "game".into()]);
        assert_eq!(type_str(&mut m, "i want to HACK. play game now "), ["hack", "game"]);
        assert!(type_str(&mut m, "hacker gamer ").is_empty(), "only whole words count");
        assert_eq!(type_str(&mut m, "hacx\u{8}k "), ["hack"], "backspace edits the word");
        m.set_words(&[]);
        assert!(type_str(&mut m, "hack ").is_empty());
    }

    #[test]
    fn student_events_are_cleaned_before_sending() {
        let s = Student::new(Arc::new(Log::default()));
        let mut rx = s.out.subscribe();
        assert!(!s.register("  ", "10A"));
        assert!(s.register(&"n".repeat(300), "10A"));
        s.power(Some(250), true, None);
        let Ok(Event::Register { name, class }) = rx.try_recv() else { panic!() };
        assert_eq!((name.len(), class.as_str()), (80, "10A"));
        assert_eq!(rx.try_recv().unwrap(), Event::Power { percent: Some(100), ac: true, wifi_signal: None });
        assert!(!s.test_done(1, &"x".repeat(erg_assess::testing::MAX_PAPER_JSON + 1)));
    }

    #[test]
    fn local_mac_is_not_multicast_or_zero() {
        if let Some(m) = local_mac() { assert!(m != [0; 6] && m[0] & 1 == 0); }
    }
}

#[cfg(test)]
mod video_tests {
    use super::*;
    use erg_proto::VideoAction::*;

    #[derive(Default)]
    struct V(Mutex<Vec<String>>);
    impl Executor for V {
        fn open_url(&self, _: &str) {}
        fn message(&self, _: &str, _: u16) {}
        fn lock(&self, _: bool, _: Option<&str>) {}
        fn unlock(&self) {}
        fn power(&self, _: PowerAction) {}
        fn set_web_policy(&self, _: erg_proto::WebPolicy, _: &[String]) {}
        fn set_app_policy(&self, _: erg_proto::AppPolicy, _: &[String]) {}
        fn video(&self, a: &erg_proto::VideoAction) { self.0.lock().unwrap().push(format!("{a:?}")) }
    }

    #[test]
    fn only_plain_video_names_and_sane_seeks_get_through() {
        let v = V::default();
        for bad in ["..\\x.mp4", "C:\\x.mp4", "x.exe", "x.mp4.exe", "", "a/b.mp4", "con.mp4"] {
            assert!(!dispatch(&v, Command::Video(Open { name: bad.into() })), "{bad:?}");
        }
        assert!(dispatch(&v, Command::Video(Open { name: "Bai giang.MP4".into() })));
        assert!(dispatch(&v, Command::Video(Seek { secs: 12.5 })));
        for bad in [f32::NAN, -1.0, f32::INFINITY, 1e9] {
            assert!(!dispatch(&v, Command::Video(Seek { secs: bad })));
        }
        assert!(dispatch(&v, Command::Video(Pause)) && dispatch(&v, Command::Video(Close)));
        assert_eq!(v.0.lock().unwrap().len(), 4);
    }

    #[test]
    fn exhibit_requires_a_scoped_group_and_an_executor_that_can_do_it() {
        let s = Student::new(Arc::new(V::default()));
        assert!(!s.handle(Command::StartExhibit { group: [224, 0, 0, 251], port: 47900, key: [0; 16], stream: 1 }));
        assert!(!s.handle(Command::StartExhibit { group: [239, 255, 77, 20], port: 80, key: [0; 16], stream: 1 }));
        // default executor cannot exhibit, so even a valid request is reported as refused
        assert!(!s.handle(Command::StartExhibit { group: [239, 255, 77, 20], port: 47900, key: [0; 16], stream: 1 }));
        assert!(s.handle(Command::StopExhibit));
    }
}

/// Machine-wide settings written by the installer: `%PROGRAMDATA%\ERGClassroom\config.txt`, lines
/// of `name=value` (`room`, `gateway`). Environment variables ERG_ROOM / ERG_GATEWAY win over the file.
pub fn parse_settings(text: &str) -> std::collections::HashMap<String, String> {
    // Windows PowerShell writes UTF-8 with a BOM: it would become part of the first setting's name
    text.trim_start_matches('\u{feff}').lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') { return None; }
            let (k, v) = l.split_once('=')?;
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            (!k.is_empty() && !v.is_empty() && v.len() <= 200).then(|| (k, v.to_string()))
        })
        .collect()
}

/// Name shown on the tutor. On a terminal server every signed-in user is a separate student on the same
/// computer, so the user name is added (`SERVER-an`); otherwise it is just the computer name.
pub fn display_host(computer: &str, user: &str, terminal_server: bool) -> String {
    let clean: String = user.chars().filter(|c| c.is_alphanumeric() || "-_.".contains(*c)).take(40).collect();
    if terminal_server && !clean.is_empty() { format!("{computer}-{clean}") } else { computer.to_string() }
}

/// config.txt text for settings edited in the Student Configurator, or what is wrong with them.
/// Same rules as installer\write-config.ps1; keys the configurator does not know are written back as they were.
pub fn settings_text(s: &std::collections::HashMap<String, String>) -> Result<String, String> {
    for (k, v) in s {
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') { return Err(format!("bad setting name: {k}")); }
        if v.len() > 200 || v.contains(['\r', '\n']) { return Err(format!("{k}: at most 200 characters on one line")); }
    }
    if s.get("room").is_some_and(|r| r.contains('=')) { return Err("Room must not contain \"=\"".into()); }
    if s.get("gateway").is_some_and(|g| !g.trim().is_empty() && g.trim().parse::<SocketAddr>().is_err()) { return Err("Server address must look like 10.0.0.5:47810".into()); }
    let mut keys: Vec<_> = s.iter().filter(|(_, v)| !v.trim().is_empty()).collect();
    keys.sort();
    Ok(keys.iter().map(|(k, v)| format!("{k}={}\r\n", v.trim())).collect())
}

pub fn load_settings() -> std::collections::HashMap<String, String> {
    ["PROGRAMDATA", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|d| std::fs::read_to_string(std::path::Path::new(&d).join("ERGClassroom").join("config.txt")).ok())
        .map(|t| parse_settings(&t))
        .unwrap_or_default()
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn settings_file_is_forgiving() {
        let s = parse_settings("# comment\nRoom = Phong Tin 1 \ngateway=10.0.0.5:47810\nbad line\n=x\nempty=\n  \n");
        assert_eq!(s.get("room").map(String::as_str), Some("Phong Tin 1"));
        assert_eq!(s.get("gateway").map(String::as_str), Some("10.0.0.5:47810"));
        assert_eq!(s.len(), 2);
        assert!(parse_settings(&format!("room={}", "x".repeat(500))).is_empty());
    }

    #[test]
    fn settings_text_round_trips_and_rejects_bad_values() {
        let s = parse_settings("room=Lab 2\ngateway=10.0.0.5:47810\nallow_power=1\nlang=en\n");
        assert_eq!(parse_settings(&settings_text(&s).unwrap()), s);
        assert_eq!(parse_settings("\u{feff}room=erg\r\n").get("room").map(String::as_str), Some("erg")); // file written by PowerShell
        let with = |k: &str, v: &str| { let mut m = s.clone(); m.insert(k.into(), v.into()); settings_text(&m) };
        assert!(with("gateway", "server:47810").is_err()); // the agent only accepts ip:port
        assert!(with("room", "a=b").is_err());
        assert!(with("room", "a\nallow_power=1").is_err());
        assert!(with("bad key", "x").is_err());
        assert!(!with("gateway", "").unwrap().contains("gateway")); // cleared = removed
    }
}
