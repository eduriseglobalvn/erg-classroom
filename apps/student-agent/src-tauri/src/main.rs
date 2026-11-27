#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Student agent. For now it also hosts the student runtime (one process, user session); the
//! SYSTEM service for privileged work is a later split. It owns every window the student sees:
//! the Show view, the lock screen and teacher messages.

use erg_proto::{AppPolicy, PowerAction, WebPolicy, DISCOVERY_PORT};
use erg_show::Frame;
use erg_student_core::{run_with, Config, Executor, Student};
use std::process::Command as Proc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};

struct Agent {
    app: AppHandle,
    show_channel: Mutex<Option<Channel<InvokeResponseBody>>>,
    msg_counter: AtomicUsize,
    apps: Mutex<Option<erg_policy::apps::AppBlocker>>,
    guard: Mutex<Option<erg_policy::apps::AppBlocker>>,
    /// The teacher’s own blocked websites and keywords (Command::SetGuard).
    blocklist: Mutex<erg_student_core::guard::Lists>,
    keywords: Arc<Mutex<erg_student_core::KeywordMonitor>>,
    /// Lesson info for the toolbar; `ends` is when the countdown reaches zero.
    lesson: Mutex<Lesson>,
    /// Running test: (id, JSON of paper + seconds).
    test: Mutex<Option<(u32, String)>>,
    /// Shared board: whether students may draw, and every stroke so far (a window opened late reads it).
    board: Mutex<(bool, Vec<erg_proto::BoardMsg>)>,
    exhibit: Mutex<Option<erg_broadcast::Handle>>,
    watch: Mutex<Option<erg_broadcast::Handle>>,
}

#[derive(Default, Clone, serde::Serialize)]
struct Lesson {
    title: String,
    objectives: Vec<String>,
    /// Seconds since the UNIX epoch when the lesson timer ends; 0 = no timer.
    ends_at: u64,
    stars: u16,
    /// The teacher is watching for listed words; the toolbar says so.
    monitoring: bool,
    /// The tutor made this student a group leader.
    leader: bool,
}

/// The teacher switched the student toolbar off (Command::Toolbar(false)); lesson updates must not bring it back.
static TOOLBAR_OFF: AtomicBool = AtomicBool::new(false);
const BAR_H: f64 = 40.0;

fn now_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Percent-encode everything except unreserved characters so teacher text cannot alter the URL.
fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

impl Agent {
    fn open_window(&self, label: &str, url: &str, title: &str, w: f64, h: f64) {
        if self.app.get_webview_window(label).is_some() {
            return;
        }
        let r = WebviewWindowBuilder::new(&self.app, label, WebviewUrl::App(url.into()))
            .title(title).inner_size(w, h).always_on_top(true).resizable(false).center().build();
        if let Err(e) = r {
            eprintln!("cannot open {label}: {e}");
        }
    }

    /// Show the toolbar (once) and tell it the current lesson.
    fn push_lesson(&self) {
        use tauri::Emitter;
        if TOOLBAR_OFF.load(Ordering::Relaxed) {
            return;
        }
        if self.app.get_webview_window("toolbar").is_none() {
            // top centre of the primary screen; the page sizes it to its content and folds it to a thin strip when the mouse leaves (toolbar_height)
            let width = self.app.primary_monitor().ok().flatten().map_or(1280.0, |m| m.size().width as f64 / m.scale_factor());
            let r = WebviewWindowBuilder::new(&self.app, "toolbar", WebviewUrl::App("index.html?view=toolbar".into()))
                .inner_size(480.0, BAR_H).position((width - 480.0) / 2.0, 0.0).decorations(false).shadow(false).always_on_top(true)
                .skip_taskbar(true).resizable(false).focused(false).build();
            if let Err(e) = r {
                eprintln!("cannot open toolbar: {e}");
                return;
            }
        }
        let l = self.lesson.lock().expect("lesson lock").clone();
        let _ = self.app.emit("lesson", l);
    }

    // destroy, not close: Alt+F4 on a lock window is refused in CloseRequested, the agent's own closes must still work
    fn close(&self, label: &str) {
        if let Some(w) = self.app.get_webview_window(label) {
            let _ = w.destroy();
        }
    }

    fn open_fullscreen(&self, label: &str, query: &str, translucent: bool) {
        self.open_on(label, query, translucent, None);
    }

    /// Full screen on the monitor at `at` (physical position), or on the primary one.
    fn open_on(&self, label: &str, query: &str, translucent: bool, at: Option<tauri::PhysicalPosition<i32>>) {
        if self.app.get_webview_window(label).is_some() {
            return;
        }
        let mut b = WebviewWindowBuilder::new(&self.app, label, WebviewUrl::App(format!("index.html?{query}").into()))
            .fullscreen(at.is_none())
            .visible(at.is_none())
            .always_on_top(true)
            .decorations(false)
            .skip_taskbar(true)
            .focused(true);
        if translucent {
            b = b.transparent(true).effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
        }
        match b.build() {
            Err(e) => eprintln!("cannot open {label}: {e}"),
            // physical position: the builder's logical one is off on a second screen with another scale
            Ok(w) => if let Some(p) = at {
                let _ = w.set_position(tauri::PhysicalPosition::new(p.x + 8, p.y + 8));
                let _ = w.set_fullscreen(true);
                let _ = w.show();
            },
        }
    }

    /// The lock covers every screen: "lock" on the primary one, "lock~N" on the others.
    fn lock_all(&self, query: &str, translucent: bool) {
        self.open_fullscreen("lock", query, translucent);
        let primary = self.app.primary_monitor().ok().flatten().map(|m| *m.position());
        for (i, m) in self.app.available_monitors().unwrap_or_default().iter().enumerate() {
            if Some(*m.position()) != primary {
                self.open_on(&format!("lock~{i}"), query, translucent, Some(*m.position()));
            }
        }
    }
}

/// "%USERPROFILE%\\Documents" -> "C:\\Users\\an\\Documents"; an unknown %NAME% stays as it is.
fn expand_env(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        match tail.find('%').and_then(|b| std::env::var(&tail[..b]).ok().map(|v| (b, v))) {
            Some((b, v)) => { out.push_str(&v); rest = &tail[b + 1..]; }
            None => { out.push('%'); rest = tail; }
        }
    }
    out + rest
}

impl Executor for Agent {
    fn open_url(&self, url: &str) {
        // `url` is validated http(s) without whitespace and passed as one argv element (no shell).
        let _ = Proc::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).spawn();
    }

    fn run_app(&self, path: &str, args: &[String]) {
        let path = expand_env(path);
        // a program with its full path starts directly (no shell). Anything else goes through ShellExecute: a document or
        // folder opens with its program, and a bare "winword.exe" is found through App Paths (its arguments are dropped).
        let r = if path.to_ascii_lowercase().ends_with(".exe") && path.contains('\\') {
            Proc::new(&path).args(args).spawn()
        } else {
            Proc::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(&path).spawn()
        };
        if let Err(e) = r {
            eprintln!("cannot start {path}: {e}");
        }
    }

    fn poll(&self, id: u32, question: &str, options: &[String]) {
        let data = serde_json::json!({ "q": question, "o": options }).to_string();
        let url = format!("index.html?view=poll&id={id}&d={}", pct(&data));
        let r = WebviewWindowBuilder::new(&self.app, format!("poll-{id}"), WebviewUrl::App(url.into()))
            .title(erg_student_core::i18n::t("poll"))
            .inner_size(440.0, 120.0 + 40.0 * options.len() as f64)
            .always_on_top(true)
            .resizable(false)
            .center()
            .build();
        if let Err(e) = r {
            eprintln!("cannot open poll: {e}");
        }
    }
    // one chat window: a new line goes to the open one
    fn chat(&self, text: &str) {
        use tauri::Emitter;
        if let Some(w) = self.app.get_webview_window("chat") {
            if !text.is_empty() {
                let _ = w.emit("chat", text);
            }
            let _ = w.unminimize();
            let _ = w.set_focus();
            return;
        }
        let url = format!("index.html?view=chat&text={}", pct(text));
        let r = WebviewWindowBuilder::new(&self.app, "chat", WebviewUrl::App(url.into()))
            .title(erg_student_core::i18n::t("chat"))
            .inner_size(380.0, 470.0)
            .always_on_top(true)
            .resizable(false)
            .center()
            .build();
        if let Err(e) = r {
            eprintln!("cannot open chat: {e}");
        }
    }
    fn message(&self, text: &str, seconds: u16) {
        let n = self.msg_counter.fetch_add(1, Ordering::Relaxed);
        let url = format!("index.html?view=msg&s={seconds}&text={}", pct(text));
        // top right of the primary screen, under the toolbar
        let width = self.app.primary_monitor().ok().flatten().map_or(1280.0, |m| m.size().width as f64 / m.scale_factor());
        let r = WebviewWindowBuilder::new(&self.app, format!("msg-{n}"), WebviewUrl::App(url.into()))
            .inner_size(380.0, 190.0)
            .always_on_top(true)
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .position(width - 396.0, BAR_H + 12.0)
            .build();
        if let Err(e) = r {
            eprintln!("cannot open message: {e}");
        }
    }

    fn lock(&self, blank: bool, text: Option<&str>) {
        let q = format!("view=lock&blank={}&text={}", blank as u8, pct(text.unwrap_or("")));
        self.lock_all(&q, !blank);
    }
    fn unlock(&self) {
        for (label, w) in self.app.webview_windows() {
            if label == "lock" || label.starts_with("lock~") {
                let _ = w.destroy();
            }
        }
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
        let _ = c.spawn();
    }

    fn set_web_policy(&self, mode: WebPolicy, list: &[String]) {
        if service_mode() { return; }
        // HKLM needs administrator rights; works when the agent runs elevated, otherwise only logs
        match erg_policy::web::apply(mode, list) {
            Ok(()) => eprintln!("web policy {mode:?} applied ({} sites)", list.len()),
            Err(e) => eprintln!("web policy not applied: {e}"),
        }
        let mut g = self.guard.lock().expect("guard lock");
        if mode == WebPolicy::Unrestricted {
            *g = None;
        } else if g.is_none() {
            *g = Some(erg_policy::apps::AppBlocker::start(erg_policy::apps::browser_guard(), |n| eprintln!("ended unmanaged browser {n}")));
        }
    }
    fn set_app_policy(&self, mode: AppPolicy, list: &[String]) {
        if service_mode() { return; }
        match erg_policy::apps::Policy::new(mode, list.to_vec()) {
            Ok(p) => {
                let mut g = self.apps.lock().expect("apps lock");
                match g.as_ref() {
                    Some(b) => b.set_policy(p),
                    None => *g = Some(erg_policy::apps::AppBlocker::start(p, |n| eprintln!("ended blocked program {n}"))),
                }
            }
            Err(e) => eprintln!("app policy rejected: {e}"),
        }
    }

    fn mute(&self, on: bool) {
        if let Err(e) = erg_sysops::set_mute(on) { eprintln!("mute failed: {e}"); }
    }
    fn beep(&self) { erg_sysops::beep(); }
    fn set_clipboard(&self, text: &str) {
        if let Err(e) = erg_sysops::set_clipboard(text) { eprintln!("clipboard not set: {e}"); }
    }
    fn clipboard(&self) -> Option<String> { erg_sysops::clipboard() }
    fn set_usb_policy(&self, p: erg_proto::UsbPolicy) {
        if service_mode() { return; }
        if let Err(e) = erg_policy::usb::apply(p) { eprintln!("usb policy not applied (needs administrator): {e}"); }
    }
    fn set_print_policy(&self, p: erg_proto::PrintPolicy) {
        if service_mode() { return; }
        let failed = erg_policy::print::apply(p);
        if !failed.is_empty() { eprintln!("print policy {p:?}: not applied to {failed:?}"); }
    }
    fn set_protect(&self, p: &erg_proto::Protect) {
        if service_mode() { return; }
        if let Err(e) = erg_policy::protect::apply(p) { eprintln!("protect not applied: {e}"); }
    }
    fn privileged(&self, r: &erg_proto::PrivRequest) -> bool {
        if let Some(ok) = erg_authpipe::forward(erg_authpipe::PRIV_NAME, r) {
            return ok; // the service decides; this process cannot apply or originate it
        }
        LOCAL.get().is_some_and(|l| l.apply(self, r))
    }
    fn set_guard(&self, lists: erg_student_core::guard::Lists) {
        *self.blocklist.lock().expect("blocklist lock") = lists;
    }
    fn set_keywords(&self, words: &[String]) {
        self.keywords.lock().expect("kw lock").set_words(words);
        // The student must always see that typed words are being watched (PLAN.md section 3.2 point 6).
        let on = !words.is_empty();
        self.lesson.lock().expect("lesson lock").monitoring = on;
        self.push_lesson();
        if on {
            self.message(erg_student_core::i18n::t("keywords"), 15);
        }
    }
    fn input(&self, e: erg_proto::InputEvent) { erg_sysops::input(e); }
    fn start_exhibit(&self, group: [u8; 4], port: u16, key: [u8; 16], stream: u32) -> bool {
        self.stop_exhibit();
        match erg_broadcast::start(erg_broadcast::Source::Screen, erg_broadcast::Target { group, port, key, stream }) {
            Ok(h) => { *self.exhibit.lock().expect("exhibit lock") = Some(h); true }
            Err(e) => { eprintln!("exhibit not started: {e}"); false }
        }
    }
    fn video(&self, a: &erg_proto::VideoAction) {
        use erg_proto::VideoAction::*;
        use tauri::Emitter;
        match a {
            Open { name } => {
                let Ok(docs) = self.app.path().document_dir() else { return };
                let path = docs.join("ERG").join("Received").join(name);
                if !path.is_file() { eprintln!("video {name} was not received"); return; }
                self.close("video");
                self.open_fullscreen("video", &format!("view=video&p={}", pct(&path.to_string_lossy())), false);
            }
            Close => self.close("video"),
            Play => { let _ = self.app.emit("video", serde_json::json!({ "do": "play" })); }
            Pause => { let _ = self.app.emit("video", serde_json::json!({ "do": "pause" })); }
            Seek { secs } => { let _ = self.app.emit("video", serde_json::json!({ "do": "seek", "secs": secs })); }
        }
    }
    fn watch_screen(&self, sink: Option<Box<dyn Fn(bool, Vec<u8>) + Send>>) {
        if let Some(h) = self.watch.lock().expect("watch lock").take() { h.stop(); }
        if let Some(s) = sink {
            match erg_broadcast::watch::start(1280, s) {
                Ok(h) => *self.watch.lock().expect("watch lock") = Some(h),
                Err(e) => eprintln!("watch not started: {e}"),
            }
        }
    }
    fn stop_exhibit(&self) {
        if let Some(h) = self.exhibit.lock().expect("exhibit lock").take() { h.stop(); }
    }

    fn request_register(&self) {
        self.open_window("register", "index.html?view=register", erg_student_core::i18n::t("register"), 420.0, 230.0);
    }
    fn ask_question(&self, id: u32, text: &str) {
        self.open_window(&format!("question-{id}"), &format!("index.html?view=question&id={id}&text={}", pct(text)), erg_student_core::i18n::t("question"), 460.0, 260.0);
    }
    fn set_lesson(&self, title: &str, objectives: &[String], seconds_left: u32) {
        {
            let mut l = self.lesson.lock().expect("lesson lock");
            l.title = title.to_string();
            l.objectives = objectives.to_vec();
            l.ends_at = if seconds_left == 0 { 0 } else { now_s() + seconds_left as u64 };
        }
        self.push_lesson();
    }
    fn board(&self, m: &erg_proto::BoardMsg) {
        use erg_proto::BoardMsg as B;
        use tauri::Emitter;
        {
            let mut b = self.board.lock().expect("board lock");
            match m {
                B::Open { draw } => { b.0 = *draw; b.1.clear(); }
                B::Close => { b.0 = false; b.1.clear(); }
                B::Clear => b.1.clear(),
                B::Undo { id } => b.1.retain(|e| e.element_id() != Some(*id)),
                B::Scene { .. } => { b.1.retain(|e| !matches!(e, B::Scene { .. })); b.1.push(m.clone()); }
                _ => { if b.1.len() >= 5000 { b.1.remove(0); } b.1.push(m.clone()); }
            }
        }
        match m {
            B::Open { .. } => {
                if self.app.get_webview_window("board").is_none() {
                    let r = WebviewWindowBuilder::new(&self.app, "board", WebviewUrl::App("index.html?view=board".into()))
                        .title(erg_student_core::i18n::t("board")).inner_size(960.0, 600.0).always_on_top(true).center().build();
                    if let Err(e) = r { eprintln!("cannot open board: {e}"); }
                }
                let _ = self.app.emit("board", m);
            }
            B::Close => self.close("board"),
            _ => { let _ = self.app.emit("board", m); }
        }
    }
    fn toolbar(&self, on: bool) {
        TOOLBAR_OFF.store(!on, Ordering::Relaxed);
        if on { self.push_lesson() } else { self.close("toolbar") }
    }
    fn set_leader(&self, on: bool) {
        self.lesson.lock().expect("lesson lock").leader = on;
        self.push_lesson();
    }
    fn reward(&self, stars: u16) {
        self.lesson.lock().expect("lesson lock").stars = stars;
        self.push_lesson();
    }
    fn start_test(&self, id: u32, paper: &erg_assess::Paper, seconds: u32) {
        let data = serde_json::json!({ "paper": paper, "seconds": seconds }).to_string();
        // the paper can be large, so the window fetches it through `test_data` instead of the URL
        *self.test.lock().expect("test lock") = Some((id, data));
        self.open_fullscreen("test", &format!("view=test&id={id}"), false);
    }
    fn stop_test(&self) {
        *self.test.lock().expect("test lock") = None;
        self.close("test");
    }

    fn show_started(&self) {
        self.open_fullscreen("show", "view=show", false);
    }
    fn show_frame(&self, f: &Frame) {
        if let Some(ch) = self.show_channel.lock().expect("channel lock").as_ref() {
            let mut v = Vec::with_capacity(5 + f.data.len());
            v.push(f.keyframe as u8);
            v.extend_from_slice(&f.frame_no.to_le_bytes());
            v.extend_from_slice(&f.data);
            let _ = ch.send(InvokeResponseBody::Raw(v));
        }
    }
    #[cfg(windows)]
    fn thumbnail(&self, w: u16, h: u16) -> Option<Vec<u8>> {
        let mut cap = erg_capture::Capturer::new(0).ok()?;
        (0..15).find_map(|_| cap.grab(200).ok().flatten())?.thumbnail_jpeg(w, h)
    }
    fn show_stopped(&self) {
        *self.show_channel.lock().expect("channel lock") = None;
        self.close("show");
    }
}

/// The Show view registers the channel that receives binary frames (no base64/JSON overhead).
/// Student typed a reply in a chat window.
#[tauri::command]
fn chat_say(student: State<'_, Arc<Student>>, text: String) -> bool {
    student.say(&text)
}

/// Student toolbar: folded tab, bar, or bar with the objectives open, as wide as the page asks and centred at the
/// top of the screen. No width = the whole screen width.
#[tauri::command]
fn toolbar_height(window: tauri::WebviewWindow, height: f64, width: Option<f64>) {
    let Ok(Some(m)) = window.current_monitor() else { return };
    let k = m.scale_factor();
    let full = m.size().width as f64 / k;
    let w = width.unwrap_or(full).clamp(40.0, full);
    let _ = window.set_size(tauri::LogicalSize::new(w, height.clamp(4.0, 400.0)));
    let _ = window.set_position(tauri::LogicalPosition::new(m.position().x as f64 / k + (full - w) / 2.0, m.position().y as f64 / k));
}

/// (computer name, room, Windows user name) for the student toolbar.
#[tauri::command]
fn toolbar_info() -> (String, String, String) {
    (std::env::var("COMPUTERNAME").unwrap_or_default(), erg_student_core::load_settings().remove("room").unwrap_or_else(|| "default".into()), std::env::var("USERNAME").unwrap_or_default())
}

/// Where files from the teacher land.
fn inbox_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into())).join("Documents").join("ERG").join("Received")
}

/// "My work" on the toolbar: shows the received-files folder in Explorer.
#[tauri::command]
fn open_inbox() {
    let dir = inbox_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
}

/// The student starts a chat with the teacher from the toolbar.
#[tauri::command]
fn open_chat(agent: State<'_, Arc<Agent>>) {
    // not on this thread: a window created inside a synchronous command deadlocks the whole program on Windows
    let a = agent.inner().clone();
    std::thread::spawn(move || a.chat(""));
}

#[tauri::command]
fn lesson_state(agent: State<'_, Arc<Agent>>) -> Lesson {
    agent.lesson.lock().expect("lesson lock").clone()
}

/// "en" when config.txt says `lang=en`, otherwise "vi".
#[tauri::command]
fn ui_lang() -> String {
    erg_student_core::i18n::ui_lang().into()
}

/// What a freshly opened board window needs: (students may draw, strokes so far).
#[tauri::command]
fn board_state(agent: State<'_, Arc<Agent>>) -> (bool, Vec<erg_proto::BoardMsg>) {
    agent.board.lock().expect("board lock").clone()
}

#[tauri::command]
fn board_emit(student: State<'_, Arc<Student>>, msg: erg_proto::BoardMsg) -> bool {
    student.board_emit(msg)
}

#[tauri::command]
fn leader_action(student: State<'_, Arc<Student>>, kind: String, text: String) {
    use erg_proto::LeaderAction as A;
    match kind.as_str() {
        "message" => student.leader(A::Message(text)),
        "lock" => student.leader(A::Lock),
        "unlock" => student.leader(A::Unlock),
        _ => {}
    }
}

#[tauri::command]
fn help_request(student: State<'_, Arc<Student>>, text: String) {
    student.help(&text);
}

#[tauri::command]
fn register(student: State<'_, Arc<Student>>, name: String, class: String) -> bool {
    student.register(&name, &class)
}

#[tauri::command]
fn hand(student: State<'_, Arc<Student>>, question: u32, text: String) {
    student.hand(question, &text);
}

/// (paper + seconds) JSON of the running test, so the test window can render it.
#[tauri::command]
fn test_data(agent: State<'_, Arc<Agent>>, id: u32) -> Option<String> {
    agent.test.lock().expect("test lock").as_ref().filter(|t| t.0 == id).map(|t| t.1.clone())
}

#[tauri::command]
fn test_done(agent: State<'_, Arc<Agent>>, student: State<'_, Arc<Student>>, id: u32, answers: String) -> bool {
    let ok = student.test_done(id, &answers);
    if ok {
        *agent.test.lock().expect("test lock") = None;
        agent.close("test");
    }
    ok
}

#[tauri::command]
fn poll_answer(student: State<'_, Arc<Student>>, id: u32, choice: u8) {
    student.answer(id, choice);
}

#[tauri::command]
fn show_subscribe(agent: State<'_, Arc<Agent>>, on_frame: Channel<InvokeResponseBody>) {
    *agent.show_channel.lock().expect("channel lock") = Some(on_frame);
}

/// Per-user mode only (the Room Key is in this process): checks signed requests itself.
static LOCAL: std::sync::OnceLock<erg_student_core::privlocal::Local> = std::sync::OnceLock::new();

/// With the SYSTEM service installed, policy and power changes happen there, only from signed requests.
fn service_mode() -> bool {
    erg_authpipe::available(erg_authpipe::PRIV_NAME)
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

/// The student never invents a Room Key: without the school's key it cannot be controlled.
fn load_key() -> Result<[u8; 32], String> {
    // machine-wide location first, then the per-user one made by installer\install-student.ps1
    let from_file = ["PROGRAMDATA", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|d| std::fs::read_to_string(format!("{d}\\ERGClassroom\\room.key")).ok());
    // the watchdog hands the key over in the environment; drop it so apps started later do not inherit it
    let from_env = std::env::var("ERG_ROOM_KEY").ok();
    std::env::remove_var("ERG_ROOM_KEY");
    let hex = from_env
        .or(from_file)
        .ok_or("ERG_ROOM_KEY or ERGClassroom\\room.key (in %PROGRAMDATA% or %LOCALAPPDATA%) is required")?;
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

/// The config.txt in use (machine-wide first, as load_settings reads it); a new one goes to %PROGRAMDATA%.
fn config_file() -> std::path::PathBuf {
    let files: Vec<_> = ["PROGRAMDATA", "LOCALAPPDATA"].iter().filter_map(|v| std::env::var(v).ok())
        .map(|d| std::path::Path::new(&d).join("ERGClassroom").join("config.txt")).collect();
    files.iter().find(|f| f.exists()).or(files.first()).cloned().unwrap_or_else(|| "config.txt".into())
}

/// Student Configurator (`--config`): (settings, file they are saved in, a Room Key or key holder is present).
#[tauri::command]
fn config_load() -> (std::collections::HashMap<String, String>, String, bool) {
    (erg_student_core::load_settings(), config_file().display().to_string(), key_or_holder().is_ok())
}

#[tauri::command]
fn config_save(settings: std::collections::HashMap<String, String>) -> Result<(), String> {
    let text = erg_student_core::settings_text(&settings)?;
    let f = config_file();
    // the folder is writable by administrators only (installer ACL)
    std::fs::write(&f, text).map_err(|e| format!("Cannot write {}: {e}. Run the configurator as administrator.", f.display()))
}

/// The agent has no console when the service starts it: fatal errors go to %LOCALAPPDATA%\ERGClassroom\agent.log.
fn note(line: &str) {
    use std::io::Write;
    eprintln!("{line}");
    let Ok(dir) = std::env::var("LOCALAPPDATA").map(|d| std::path::Path::new(&d).join("ERGClassroom")) else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("agent.log")) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{now} {line}");
    }
}

fn main() {
    // the configurator edits config.txt and must work on a machine that has no Room Key yet: no agent is started
    let configure = std::env::args().any(|a| a == "--config");
    let key = if configure { [0u8; 32] } else { key_or_holder().unwrap_or_else(|e| {
        note(&e);
        std::process::exit(2);
    }) };
    if !configure && key != [0u8; 32] {
        let _ = LOCAL.set(erg_student_core::privlocal::Local::new(key)); // zero key = a holder answers on the pipe
    }
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![toolbar_height, toolbar_info, open_inbox, open_chat, show_subscribe, chat_say, poll_answer, lesson_state, board_state, board_emit, ui_lang, leader_action, help_request, register, hand, test_data, test_done, config_load, config_save])
        .setup(move |app| {
            if configure {
                WebviewWindowBuilder::new(app, "config", WebviewUrl::App("index.html?view=config".into()))
                    .title("ERG Classroom Student Configurator").inner_size(820.0, 600.0).min_inner_size(640.0, 420.0).center().build()?;
                return Ok(());
            }
            let agent = Arc::new(Agent {
                app: app.handle().clone(),
                show_channel: Mutex::new(None),
                msg_counter: AtomicUsize::new(0),
                apps: Mutex::new(None),
                guard: Mutex::new(None),
                blocklist: Default::default(),
                keywords: Default::default(),
                lesson: Mutex::new(Lesson::default()),
                test: Mutex::new(None),
                board: Mutex::new((false, vec![])),
                exhibit: Mutex::new(None),
                watch: Mutex::new(None),
            });
            let student = Student::new(agent.clone());
            // Typed words: only whole words from the teacher's list are ever reported.
            #[cfg(windows)]
            {
                let (mon, st) = (agent.keywords.clone(), student.clone());
                erg_sysops::keys::start(move |c| {
                    let hit = mon.lock().expect("kw lock").feed(c);
                    if let Some(w) = hit { st.keyword(&w); }
                });
                // Forbidden website or system tool in front: warning lock + alert to the teacher. Nothing is closed,
                // so the teacher sees it after unlocking.
                let (st, ag) = (student.clone(), agent.clone());
                std::thread::spawn(move || {
                    let me = std::env::current_exe().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase())).unwrap_or_default();
                    let mut tab: (String, Option<String>) = Default::default(); // (title, address) of the browser tab last looked at
                    let mut last = String::new(); // the window already reported: after Unlock it is in front again and must not lock again
                    let mut shown = (String::new(), String::new()); // what the Tutor was last told is in front
                    let user = std::env::var("USERNAME").unwrap_or_default();
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let Some((exe, title)) = erg_sysops::foreground() else { continue };
                        if exe.to_lowercase() == me { continue; } // our own lock screen, toolbar, messages
                        // reading the address bar walks the browser's controls: only when the tab title changed
                        let address = if erg_student_core::guard::is_browser(&exe) {
                            if tab.0 != title { tab = (title.clone(), erg_sysops::foreground_address()); }
                            tab.1.as_deref()
                        } else {
                            None
                        };
                        let front = (exe.clone(), address.and_then(erg_student_core::guard::host_of).unwrap_or_else(|| title.clone()));
                        if front != shown {
                            st.status(&user, &front.0, &front.1);
                            shown = front;
                        }
                        let kind = erg_student_core::guard::check(&exe, &title, address, &ag.blocklist.lock().expect("blocklist lock"));
                        match kind {
                            Some(kind) if title != last => {
                                if st.violation(kind, &exe, &title) {
                                    let q = format!("view=lock&warn={kind}&app={}&text={}", pct(&exe), pct(&title));
                                    ag.lock_all(&q, false);
                                    last = title;
                                }
                            }
                            Some(_) => {}
                            None => last.clear(),
                        }
                    }
                });
                let st = student.clone();
                std::thread::spawn(move || loop {
                    let (p, ac) = erg_sysops::battery();
                    st.power(p, ac, erg_sysops::wifi_signal());
                    std::thread::sleep(std::time::Duration::from_secs(30));
                });
            }
            app.manage(student.clone());
            app.manage(agent.clone());
            agent.push_lesson(); // the student toolbar is there from the start, folded
            // Room, server and language are read once at start: when the Configurator saves config.txt, start again by
            // ourselves (the watchdog cannot do it: it sees the open Configurator as a running agent).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let at_start = erg_student_core::load_settings();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if erg_student_core::load_settings() != at_start {
                        handle.restart();
                    }
                }
            });
            let cfg = Config {
                key,
                room: std::env::var("ERG_ROOM").ok().or_else(|| erg_student_core::load_settings().remove("room")).unwrap_or_else(|| "default".into()),
                hostname: erg_student_core::display_host(
                    &std::env::var("COMPUTERNAME").unwrap_or_else(|_| "student".into()),
                    &std::env::var("USERNAME").unwrap_or_default(),
                    erg_student_core::load_settings().get("terminal_server").is_some_and(|v| v == "1"),
                ),
                user: std::env::var("USERNAME").unwrap_or_default(),
                discovery_port: DISCOVERY_PORT,
                browse_addrs: std::env::var("ERG_GATEWAY").ok().or_else(|| erg_student_core::load_settings().remove("gateway")).and_then(|a| a.parse().ok()).into_iter().collect(),
        inbox: inbox_dir(),
            };
            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_with(cfg, student, None).await {
                    note(&format!("student runtime stopped: {e}"));
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building the student agent");
    // The agent has no permanent window: closing the Show/lock window must not end the process.
    app.run(move |_, event| match event {
        RunEvent::ExitRequested { api, code, .. } if code.is_none() && !configure => api.prevent_exit(),
        // Alt+F4 must not end a lock, a test or the teacher's show: only the teacher (Agent::close) ends them
        RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. }
            if label == "show" || label == "test" || label.starts_with("lock") => api.prevent_close(),
        _ => {}
    });
}
