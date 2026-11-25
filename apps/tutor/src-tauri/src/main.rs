#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod show;

use erg_proto::{Command, DISCOVERY_PORT};
use erg_tutor_core::{Roster, RosterEvent};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Default)]
struct AppState {
    roster: Mutex<Option<Roster>>,
    show: Mutex<Option<show::ShowHandle>>,
    attendance: Mutex<erg_tutor_core::classroom::Attendance>,
    macs: Mutex<erg_tutor_core::classroom::MacBook>,
    hosts: Mutex<std::collections::HashMap<Uuid, String>>,
    recorder: Mutex<Option<erg_tutor_core::recorder::Recorder>>,
    leaders: Mutex<erg_tutor_core::leaders::Leaders>,
    assistant: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    room_key: Mutex<Option<[u8; 32]>>,
    board: Mutex<erg_tutor_core::board::Board>,
}

fn record_frame(app: &AppHandle, id: Uuid, jpeg: &[u8]) {
    let st = app.state::<AppState>();
    let host = st.hosts.lock().ok().and_then(|h| h.get(&id).cloned()).unwrap_or_else(|| id.to_string());
    let mut rec = st.recorder.lock();
    if let Ok(r) = rec.as_mut() {
        if let Some(r) = r.as_mut() {
            let _ = r.push(&host, jpeg);
        }
    }
}

/// Start recording the screen pictures of every student (Replay). Returns the folder.
#[tauri::command]
fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let dir = app.path().video_dir().or_else(|_| app.path().app_data_dir()).map_err(|e| e.to_string())?.join("ERG Replay").join(format!("lesson-{ts}"));
    let rec = erg_tutor_core::recorder::Recorder::start(&dir).map_err(|e| e.to_string())?;
    *state.recorder.lock().map_err(|e| e.to_string())? = Some(rec);
    Ok(dir.to_string_lossy().into_owned())
}

/// Stop recording; returns the path of the replay page.
#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let rec = state.recorder.lock().map_err(|e| e.to_string())?.take().ok_or("không có bản ghi nào đang chạy")?;
    rec.finish().map(|p| p.to_string_lossy().into_owned()).map_err(|e| e.to_string())
}

/// Files the UI may keep (lesson profile, seating layout, lesson plan...) live in one folder.
/// Names are restricted so the UI can never reach outside it.
fn doc_path(app: &AppHandle, name: &str) -> Result<std::path::PathBuf, String> {
    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("tên tệp không hợp lệ".into());
    }
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("docs").join(format!("{name}.json")))
}

/// Keep host names (for attendance) and MAC addresses (for Wake-on-LAN) of students that appear.
fn remember_host(app: &AppHandle, id: Uuid, beacon: &erg_proto::Beacon) {
    let st = app.state::<AppState>();
    if let Ok(mut h) = st.hosts.lock() {
        h.insert(id, beacon.hostname.clone());
    }
    let changed = st.macs.lock().map(|mut m| m.remember(&beacon.hostname, beacon.mac)).unwrap_or(false);
    if changed {
        if let (Ok(m), Ok(dir)) = (st.macs.lock(), app.path().app_data_dir()) {
            let _ = m.save(&dir.join("macs.json"));
        }
    }
}

#[tauri::command]
fn save_doc(app: AppHandle, name: String, json: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| e.to_string())?;
    let p = doc_path(&app, &name)?;
    std::fs::create_dir_all(p.parent().expect("has parent")).map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_doc(app: AppHandle, name: String) -> Result<Option<String>, String> {
    match std::fs::read_to_string(doc_path(&app, &name)?) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Let `leader` message/lock/unlock `members` for `minutes` (1..=240); the leader's toolbar shows the buttons.
#[tauri::command]
fn grant_leader(state: State<'_, AppState>, leader: String, members: Vec<String>, minutes: u64) -> Result<(), String> {
    let leader: Uuid = leader.parse().map_err(|_| "bad leader id")?;
    let members: Vec<Uuid> = members.iter().filter_map(|m| m.parse().ok()).collect();
    let ttl = std::time::Duration::from_secs(minutes.clamp(1, 240) * 60);
    state.leaders.lock().map_err(|e| e.to_string())?.grant(leader, &members, ttl, std::time::Instant::now());
    if let Some(r) = state.roster.lock().map_err(|e| e.to_string())?.as_ref() {
        r.send(Some(&[leader]), Command::SetLeader(true));
    }
    Ok(())
}

#[tauri::command]
fn revoke_leader(state: State<'_, AppState>, leader: String) -> Result<(), String> {
    let leader: Uuid = leader.parse().map_err(|_| "bad leader id")?;
    state.leaders.lock().map_err(|e| e.to_string())?.revoke(leader);
    if let Some(r) = state.roster.lock().map_err(|e| e.to_string())?.as_ref() {
        r.send(Some(&[leader]), Command::SetLeader(false));
    }
    Ok(())
}

/// Teacher-side shared whiteboard operation: checked, kept for late joiners, sent to the class.
#[tauri::command]
fn board_send(state: State<'_, AppState>, msg: erg_proto::BoardMsg) -> Result<bool, String> {
    let relay = state.board.lock().map_err(|e| e.to_string())?.apply(msg, erg_tutor_core::board::Who::Tutor);
    let Some(m) = relay else { return Ok(false) };
    if let Some(r) = state.roster.lock().map_err(|e| e.to_string())?.as_ref() {
        r.send(None, Command::Board(m));
    }
    Ok(true)
}

/// Start the phone page (Tutor Assistant); returns the address to open on the phone. A new token each time.
#[tauri::command]
async fn assistant_start(state: State<'_, AppState>) -> Result<String, String> {
    use rand::RngCore;
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("Chưa kết nối lớp học")?;
    if let Some(old) = state.assistant.lock().map_err(|e| e.to_string())?.take() { old.abort(); }
    let mut raw = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut raw);
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let (addr, task) = erg_tutor_core::assistant::serve("0.0.0.0:47900".parse().map_err(|_| "addr")?, token.clone(), std::sync::Arc::new(roster)).await.map_err(|e| format!("Không mở được cổng 47900: {e}"))?;
    *state.assistant.lock().map_err(|e| e.to_string())? = Some(tauri::async_runtime::JoinHandle::Tokio(task));
    // the address other devices can reach: the interface used for the default route
    let ip = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s| { s.connect("192.0.2.1:9")?; s.local_addr() }).map(|a| a.ip().to_string()).unwrap_or_else(|_| "IP-may-nay".into());
    Ok(format!("http://{ip}:{}/t/{token}/", addr.port()))
}

#[tauri::command]
fn assistant_stop(state: State<'_, AppState>) {
    if let Ok(mut a) = state.assistant.lock() { if let Some(t) = a.take() { t.abort(); } }
}

/// Wake the named hosts (all remembered ones when empty) with Wake-on-LAN.
#[tauri::command]
fn wake(state: State<'_, AppState>, hosts: Vec<String>) -> Result<usize, String> {
    Ok(state.macs.lock().map_err(|e| e.to_string())?.wake(&hosts, std::net::Ipv4Addr::BROADCAST))
}

#[tauri::command]
fn attendance_csv(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.attendance.lock().map_err(|e| e.to_string())?.csv())
}

/// Save text the teacher exported (CSV / HTML report) to a path chosen in the save dialog.
#[tauri::command]
fn write_text(path: String, text: String) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Open the drawing layer on the teacher's screen: `annotate` (transparent) or `whiteboard` (white page).
/// The screen capture used by Show includes this window, so students see the strokes.
#[tauri::command]
fn open_overlay(app: AppHandle, mode: String) -> Result<(), String> {
    if mode != "annotate" && mode != "whiteboard" {
        return Err("chế độ không hợp lệ".into());
    }
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.close();
    }
    tauri::WebviewWindowBuilder::new(&app, "overlay", tauri::WebviewUrl::App(format!("index.html?view=overlay&mode={mode}").into()))
        .title("Vẽ")
        .fullscreen(true)
        .transparent(mode == "annotate")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn close_overlay(app: AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.close();
    }
}

static KEYSHOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static KEYSHOW_HOOK: std::sync::Once = std::sync::Once::new();

/// Keyshow: show the characters the teacher types in the drawing layer. The hook runs for the
/// life of the app but only forwards characters while this is switched on.
#[tauri::command]
fn keyshow(app: AppHandle, on: bool) {
    use std::sync::atomic::Ordering;
    KEYSHOW.store(on, Ordering::Relaxed);
    #[cfg(windows)]
    KEYSHOW_HOOK.call_once(|| {
        erg_sysops::keys::start(move |c| {
            if KEYSHOW.load(Ordering::Relaxed) {
                let _ = app.emit("keyshow", c.to_string());
            }
        });
    });
}

/// Read a test paper the teacher picked (capped, so a wrong file cannot flood memory).
#[tauri::command]
fn load_text(path: String) -> Result<String, String> {
    let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if len > erg_assess::testing::MAX_PAPER_JSON as u64 {
        return Err("tệp quá lớn".into());
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Save the zoomed student screen (a `data:image/jpeg;base64,` URL) into Pictures\ERG Snapshots.
#[tauri::command]
fn save_snapshot(app: AppHandle, data_url: String) -> Result<String, String> {
    use base64::Engine;
    let b64 = data_url.strip_prefix("data:image/jpeg;base64,").ok_or("không phải ảnh JPEG")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| e.to_string())?;
    let dir = app.path().picture_dir().map_err(|e| e.to_string())?.join("ERG Snapshots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let file = dir.join(format!("snapshot-{ts}.jpg"));
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().into_owned())
}

/// Folder where students' pushed files (Collect Work) are saved, one sub-folder per host.
#[tauri::command]
fn set_collect_dir(state: State<'_, AppState>, dir: String) -> Result<(), String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    roster.set_collect_dir(Some(std::path::PathBuf::from(dir)));
    Ok(())
}

/// The paper JSON to put in `Command::StartTest`: answer keys removed, ordered lists shuffled by `seed`
/// (use the test id as the seed, and the same value when grading).
#[tauri::command]
fn make_student_paper(paper: String, seed: u64) -> Result<String, String> {
    let p = erg_assess::Paper::from_json(&paper)?;
    serde_json::to_string(&p.student_view(seed)).map_err(|e| e.to_string())
}

/// Grade one student's answers (JSON `Vec<Answer>`, given on the student copy) against the full paper.
#[tauri::command]
fn grade_test(paper: String, answers: String, seed: u64) -> Result<erg_assess::Score, String> {
    let p = erg_assess::Paper::from_json(&paper)?;
    let a: Vec<erg_assess::Answer> = serde_json::from_str(&answers).map_err(|e| e.to_string())?;
    Ok(erg_assess::grade(&p, &p.unmap(seed, &a)))
}

#[tauri::command]
fn test_report_csv(rows: Vec<(String, erg_assess::Score)>) -> String {
    erg_assess::testing::report_csv(&rows)
}

/// The lesson journal as a printable HTML page.
#[tauri::command]
fn journal_html(title: String, objectives: Vec<String>) -> Result<String, String> {
    let d = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    Ok(erg_tutor_core::report::html(&title, &objectives, &erg_tutor_core::report::read_tail(&std::path::Path::new(&d).join("ERGClassroom").join("journal.jsonl"), 5000)))
}

/// Newest `limit` (max 2000) journal events, oldest first, for the journal dialog.
#[tauri::command]
fn journal_list(limit: usize) -> Result<Vec<erg_tutor_core::report::Entry>, String> {
    let d = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    Ok(erg_tutor_core::report::read_tail(&std::path::Path::new(&d).join("ERGClassroom").join("journal.jsonl"), limit.min(2000)))
}

#[derive(Serialize, Clone)]
struct StudentDto {
    id: String,
    hostname: String,
    user: String,
    room: String,
    /// App version the student announced ("0.1" = any build before 0.2.0).
    version: String,
    /// Older than this Tutor: offer the update.
    outdated: bool,
    /// Before 0.2.0: drops the connection on a command it does not know, so it only gets the old ones.
    legacy: bool,
}

/// Room Key shared with every student of the school. Read from `ERG_ROOM_KEY` (64 hex chars) or
/// from `room.key` in the app config dir; without either, the built-in key that students get when installed without ROOMKEY.
fn load_room_key(app: &AppHandle) -> Result<[u8; 32], String> {
    let parse = |s: &str| -> Option<[u8; 32]> {
        let s = s.trim();
        if s.len() != 64 {
            return None;
        }
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(k)
    };
    if let Some(k) = std::env::var("ERG_ROOM_KEY").ok().and_then(|v| parse(&v)) {
        return Ok(k);
    }
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(std::fs::read_to_string(dir.join("room.key")).ok().and_then(|v| parse(&v)).unwrap_or(erg_net::DEFAULT_ROOM_KEY))
}

/// Room this tutor teaches in (`room.txt` in the config folder). None = every room that shares the Room Key.
#[tauri::command]
fn get_room(app: AppHandle) -> Option<String> {
    let text = std::fs::read_to_string(app.path().app_config_dir().ok()?.join("room.txt")).ok()?;
    Some(text.trim().to_string()).filter(|r| !r.is_empty())
}

/// Saves the room (empty = all rooms) and reconnects: everyone is dropped and the students of that room come back
/// on their next beacon. With the room unchanged this is Refresh.
#[tauri::command]
fn set_room(app: AppHandle, state: State<'_, AppState>, room: String) -> Result<(), String> {
    let room = room.trim();
    if room.len() > 200 || room.contains(['\r', '\n']) { return Err("room name: one line, at most 200 characters".into()); }
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("room.txt"), room).map_err(|e| e.to_string())?;
    if let Some(r) = state.roster.lock().map_err(|e| e.to_string())?.as_ref() {
        r.reset(Some(room.to_string()).filter(|r| !r.is_empty()));
    }
    Ok(())
}

#[tauri::command]
async fn start_roster(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut slot = state.roster.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Ok(()); // already running (e.g. webview reload)
    }
    let key = load_room_key(&app)?;
    if let Ok(dir) = app.path().app_data_dir() {
        *state.macs.lock().map_err(|e| e.to_string())? = erg_tutor_core::classroom::MacBook::load(&dir.join("macs.json"));
    }
    *state.room_key.lock().map_err(|e| e.to_string())? = Some(key);
    let roster = Roster::start(DISCOVERY_PORT, key, get_room(app.clone())).map_err(|e| e.to_string())?;
    // rooms on other subnets: ERG_GATEWAY=host:47810 (or the first line of gateway.txt in the config folder)
    let gw = std::env::var("ERG_GATEWAY").ok().or_else(|| app.path().app_config_dir().ok().and_then(|d| std::fs::read_to_string(d.join("gateway.txt")).ok()));
    if let Some(a) = gw.and_then(|s| s.lines().next().and_then(|l| l.trim().parse().ok())) {
        roster.add_gateway(a, key);
    }
    let journal = std::env::var("LOCALAPPDATA").ok().and_then(|d| erg_tutor_core::journal::Journal::open(&std::path::Path::new(&d).join("ERGClassroom").join("journal.jsonl")).ok());
    let log = move |kind: &str, who: &str, detail: &str| { if let Some(j) = &journal { j.log(kind, who, detail); } };
    let mut rx = roster.subscribe();
    let r2 = roster.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            match ev {
                RosterEvent::Online { id, beacon } => {
                    log("online", &format!("{} ({})", beacon.user, beacon.hostname), &id.to_string());
                    remember_host(&app, id, &beacon);
                    // live monitor grid: a small JPEG every 3 s
                    r2.send(Some(&[id]), Command::RequestThumbnail { w: 480, h: 270, interval_ms: 1000 });
                    // a student who joins while the shared board is open gets the board as it is now
                    let replay = app.state::<AppState>().board.lock().map(|b| b.replay()).unwrap_or_default();
                    for m in replay { r2.send(Some(&[id]), Command::Board(m)); }
                    let _ = app.emit("student-online", StudentDto {
                        id: id.to_string(),
                        hostname: beacon.hostname,
                        user: beacon.user,
                        room: beacon.room,
                        version: erg_proto::version_name(beacon.proto_ver),
                        outdated: beacon.proto_ver < erg_proto::version_code(erg_proto::APP_VERSION),
                        legacy: beacon.proto_ver < 200,
                    });
                }
                RosterEvent::Offline { id } => {
                    log("offline", &id.to_string(), "");
                    let _ = app.emit("student-offline", serde_json::json!({ "id": id.to_string() }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::HelpRequest(t) } => {
                    log("help", &id.to_string(), &t);
                    let _ = app.emit("student-help", serde_json::json!({ "id": id.to_string() }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Thumbnail(j) } => {
                    use base64::Engine;
                    record_frame(&app, id, &j);
                    let jpeg = base64::engine::general_purpose::STANDARD.encode(j);
                    let _ = app.emit("student-thumb", serde_json::json!({ "id": id.to_string(), "jpeg": jpeg }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Inventory(inv) } => {
                    let _ = app.emit("student-inventory", serde_json::json!({ "id": id.to_string(), "inv": inv }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Chat(t) } => {
                    log("chat", &id.to_string(), &t);
                    let _ = app.emit("student-chat", serde_json::json!({ "id": id.to_string(), "text": t }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::PollAnswer { id: poll, choice } } => {
                    log("poll", &id.to_string(), &format!("poll {poll} choice {choice}"));
                    let _ = app.emit("student-poll-answer", serde_json::json!({ "id": id.to_string(), "poll": poll, "choice": choice }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Register { name, class } } => {
                    let host = app.state::<AppState>().hosts.lock().ok().and_then(|h| h.get(&id).cloned()).unwrap_or_else(|| id.to_string());
                    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
                    if let Ok(mut a) = app.state::<AppState>().attendance.lock() { a.record(&host, &name, &class, ts); }
                    log("register", &host, &format!("{name} / {class}"));
                    let _ = app.emit("student-register", serde_json::json!({ "id": id.to_string(), "name": name, "class": class }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Power { percent, ac, wifi_signal } } => {
                    let _ = app.emit("student-power", serde_json::json!({ "id": id.to_string(), "percent": percent, "ac": ac, "wifi": wifi_signal }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Keyword { word } } => {
                    log("keyword", &id.to_string(), &word);
                    let _ = app.emit("student-keyword", serde_json::json!({ "id": id.to_string(), "word": word }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Violation { kind, app: prog, title } } => {
                    log("violation", &id.to_string(), &format!("{kind}: {prog} | {title}"));
                    let _ = app.emit("student-violation", serde_json::json!({ "id": id.to_string(), "kind": kind, "app": prog, "title": title }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Video { keyframe, data } } => {
                    use base64::Engine;
                    let _ = app.emit("student-video", serde_json::json!({ "id": id.to_string(), "key": keyframe, "data": base64::engine::general_purpose::STANDARD.encode(data) }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Board(m) } => {
                    let relay = app.state::<AppState>().board.lock().ok().and_then(|mut b| b.apply(m, erg_tutor_core::board::Who::Student(id)));
                    if let Some(m) = relay {
                        let _ = app.emit("board", &m);
                        r2.send(None, Command::Board(m));
                    }
                }
                RosterEvent::Student { id, event: erg_proto::Event::Leader(action) } => {
                    let allowed = app.state::<AppState>().leaders.lock().ok().and_then(|mut l| l.authorize(id, &action, std::time::Instant::now()));
                    match allowed {
                        Some((targets, cmd)) => {
                            log("leader", &id.to_string(), &format!("{action:?} -> {} machines", targets.len()));
                            r2.send(Some(&targets), cmd);
                        }
                        None => log("leader-denied", &id.to_string(), &format!("{action:?}")),
                    }
                }
                RosterEvent::Student { id, event: erg_proto::Event::Hand { question, text } } => {
                    log("hand", &id.to_string(), &format!("q{question}: {text}"));
                    let _ = app.emit("student-hand", serde_json::json!({ "id": id.to_string(), "question": question, "text": text, "at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::TestDone { id: test, answers } } => {
                    log("test", &id.to_string(), &format!("test {test} submitted"));
                    let _ = app.emit("student-test-done", serde_json::json!({ "id": id.to_string(), "test": test, "answers": answers }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::DirListing { path, entries } } => {
                    let _ = app.emit("student-dir", serde_json::json!({ "id": id.to_string(), "path": path, "entries": entries }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::FsResult { ok, message } } => {
                    let _ = app.emit("student-fs", serde_json::json!({ "id": id.to_string(), "ok": ok, "message": message }));
                }
                RosterEvent::Collected { id, path } => {
                    log("collected", &id.to_string(), &path.to_string_lossy());
                    let _ = app.emit("student-collected", serde_json::json!({ "id": id.to_string(), "path": path.to_string_lossy() }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::Clipboard(t) } => {
                    let _ = app.emit("student-clipboard", serde_json::json!({ "id": id.to_string(), "text": t }));
                }
                RosterEvent::Student { id, event: erg_proto::Event::ShowStats { recv, lost, fec_fixed } } => {
                    let _ = app.emit("student-show-stats", serde_json::json!({ "id": id.to_string(), "recv": recv, "lost": lost, "fecFixed": fec_fixed }));
                }
                RosterEvent::Student { .. } => {}
            }
        }
    });
    *slot = Some(roster);
    Ok(())
}

/// `ids = None` addresses the whole class. Returns how many students were addressed.
#[tauri::command]
fn send_command(state: State<'_, AppState>, ids: Option<Vec<String>>, cmd: Command) -> Result<usize, String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    let parsed: Option<Vec<Uuid>> = match ids {
        Some(v) => Some(v.iter().map(|s| Uuid::parse_str(s).map_err(|e| e.to_string())).collect::<Result<_, _>>()?),
        None => None,
    };
    Ok(roster.send(parsed.as_deref(), cmd))
}

/// Start a Show: begin capturing and multicasting first, then tell students to join.
#[tauri::command]
async fn start_show(state: State<'_, AppState>, ids: Option<Vec<String>>, window: Option<bool>) -> Result<usize, String> {
    if window == Some(true) {
        // "Show application": give the teacher a few seconds to bring the window to the front.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        return do_start_show_src(&state, ids, show::foreground_window().map(show::Source::Window).ok_or("không có cửa sổ nào ở phía trước")?).await;
    }
    do_start_show(&state, ids).await
}

/// Exhibit: `exhibitor` multicasts their own screen and every other student watches it.
#[tauri::command]
async fn start_exhibit(state: State<'_, AppState>, exhibitor: String) -> Result<usize, String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    let who = Uuid::parse_str(&exhibitor).map_err(|e| e.to_string())?;
    let (key, stream): ([u8; 16], u32) = (rand::random(), rand::random());
    if roster.send(Some(&[who]), Command::StartExhibit { group: show::GROUP, port: show::PORT, key, stream }) == 0 {
        return Err("học sinh không trực tuyến".into());
    }
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await; // let the exhibitor start sending
    let others: Vec<Uuid> = roster.online_ids().into_iter().filter(|i| *i != who).collect();
    Ok(roster.send(Some(&others), Command::StartShow { group: show::GROUP, port: show::PORT, key, stream }))
}

#[tauri::command]
fn stop_exhibit(state: State<'_, AppState>) -> Result<(), String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    roster.send(None, Command::StopExhibit);
    roster.send(None, Command::StopShow);
    Ok(())
}

async fn do_start_show(state: &AppState, ids: Option<Vec<String>>) -> Result<usize, String> {
    do_start_show_src(state, ids, show::Source::Screen).await
}

async fn do_start_show_src(state: &AppState, ids: Option<Vec<String>>, source: show::Source) -> Result<usize, String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    if let Some(old) = state.show.lock().map_err(|e| e.to_string())?.take() {
        old.stop();
    }
    let (key, stream): ([u8; 16], u32) = (rand::random(), rand::random());
    let tap_roster = roster.clone();
    let handle = tauri::async_runtime::spawn_blocking(move || show::start_tap(key, stream, source, Box::new(move |k, d| { tap_roster.show_frame(k, d); })))
        .await
        .map_err(|e| e.to_string())??;
    *state.show.lock().map_err(|e| e.to_string())? = Some(handle);
    let parsed: Option<Vec<Uuid>> = match ids {
        Some(v) => Some(v.iter().map(|s| Uuid::parse_str(s).map_err(|e| e.to_string())).collect::<Result<_, _>>()?),
        None => None,
    };
    Ok(roster.send(parsed.as_deref(), Command::StartShow { group: show::GROUP, port: show::PORT, key, stream }))
}

/// Send one file to the selected students (or everyone). Returns (delivered, failed).
/// File Distribution: one folder of this computer (empty path = the user's home), folders first. Returns the real path and (name, is_dir, size, modified).
#[derive(Serialize)]
struct LocalEntry { name: String, is_dir: bool, size: u64, modified_s: u64 }
#[tauri::command]
fn local_dir(path: String) -> Result<(String, Vec<LocalEntry>), String> {
    let dir = if path.trim().is_empty() { std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into()) } else { path.trim().to_string() };
    let mut list: Vec<LocalEntry> = std::fs::read_dir(&dir).map_err(|e| format!("{dir}: {e}"))?
        .filter_map(|e| {
            let e = e.ok()?;
            let m = e.metadata().ok()?;
            let modified_s = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            Some(LocalEntry { name: e.file_name().to_string_lossy().into_owned(), is_dir: m.is_dir(), size: m.len(), modified_s })
        })
        .collect();
    list.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok((dir, list))
}

#[tauri::command]
async fn send_file(state: State<'_, AppState>, ids: Option<Vec<String>>, path: String) -> Result<(usize, usize), String> {
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    let path = std::path::PathBuf::from(path);
    if !path.is_file() {
        return Err("không phải file hợp lệ".into());
    }
    let parsed: Option<Vec<Uuid>> = match ids {
        Some(v) => Some(v.iter().map(|s| Uuid::parse_str(s).map_err(|e| e.to_string())).collect::<Result<_, _>>()?),
        None => None,
    };
    let results = roster.send_file(parsed.as_deref(), &path).await;
    let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
    Ok((ok, results.len() - ok))
}

/// Remote Deploy: send an .msi/.exe to the students and ask their SYSTEM service to install it silently.
/// The request is signed with the Room Key (it covers the file's hash and the arguments) and valid for `minutes`.
#[tauri::command]
async fn deploy(state: State<'_, AppState>, ids: Option<Vec<String>>, path: String, args: String, minutes: u64) -> Result<(usize, usize), String> {
    let key = state.room_key.lock().map_err(|e| e.to_string())?.ok_or("Chưa kết nối lớp học")?;
    let p = std::path::PathBuf::from(&path);
    let name = p.file_name().and_then(|n| n.to_str()).ok_or("tên file không hợp lệ")?.to_string();
    let sha = erg_deploy::sha256_file(&p).map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let req = erg_deploy::sign(&key, &name, sha, args.trim(), now + minutes.clamp(1, 240) * 60)?;
    let roster = state.roster.lock().map_err(|e| e.to_string())?.clone().ok_or("roster not started")?;
    let parsed: Option<Vec<Uuid>> = match ids {
        Some(v) => Some(v.iter().map(|s| Uuid::parse_str(s).map_err(|e| e.to_string())).collect::<Result<_, _>>()?),
        None => None,
    };
    let results = roster.send_file(parsed.as_deref(), &p).await;
    // only machines that really got the file are asked to install it
    let got: Vec<Uuid> = results.iter().filter(|(_, r)| r.is_ok()).map(|(id, _)| *id).collect();
    if !got.is_empty() {
        roster.send(Some(&got), Command::Deploy(req));
    }
    Ok((got.len(), results.len() - got.len()))
}

/// Version of this Tutor (the workspace version in Cargo.toml).
#[tauri::command]
fn app_version() -> &'static str {
    erg_proto::APP_VERSION
}

/// Opens the project's GitHub Releases page in the default browser (fixed address: nothing from the page is passed on).
#[tauri::command]
fn open_releases() -> Result<(), String> {
    std::process::Command::new("explorer.exe").arg(RELEASES_PAGE).spawn().map(|_| ()).map_err(|e| e.to_string())
}

const RELEASES_PAGE: &str = "https://github.com/eduriseglobalvn/erg-classroom/releases/latest";

#[tauri::command]
async fn stop_show(state: State<'_, AppState>) -> Result<(), String> {
    do_stop_show(&state).await
}

async fn do_stop_show(state: &AppState) -> Result<(), String> {
    let handle = state.show.lock().map_err(|e| e.to_string())?.take();
    if let Some(h) = handle {
        tauri::async_runtime::spawn_blocking(move || h.stop()).await.map_err(|e| e.to_string())?;
    }
    // Students that were never in the Show treat StopShow as a no-op, so address everyone.
    if let Some(r) = state.roster.lock().map_err(|e| e.to_string())?.clone() {
        r.send(None, Command::StopShow);
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            // Debug builds only: lets an automated test run one Show without clicking the UI.
            #[cfg(debug_assertions)]
            if let Some(secs) = std::env::var("ERG_DEV_AUTOSHOW").ok().and_then(|v| v.parse::<u64>().ok()) {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    let st = handle.state::<AppState>();
                    eprintln!("autoshow start: {:?}", do_start_show(&st, None).await);
                    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                    eprintln!("autoshow stop: {:?}", do_stop_show(&st).await);
                });
            }
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_room, set_room, local_dir, start_recording, stop_recording, open_overlay, close_overlay, keyshow, start_roster, board_send, deploy, assistant_start, assistant_stop, grant_leader, revoke_leader, send_command, start_show, start_exhibit, stop_exhibit, stop_show, send_file, save_doc, load_doc, wake, attendance_csv, grade_test, make_student_paper, test_report_csv, write_text, set_collect_dir, load_text, save_snapshot, journal_html, journal_list, app_version, open_releases])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
