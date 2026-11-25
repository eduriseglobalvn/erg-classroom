//! Wire types shared by tutor, student service and agent.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTO_VERSION: u16 = 1;
pub const DISCOVERY_PORT: u16 = 47800;
pub const DISCOVERY_GROUP: [u8; 4] = [239, 255, 77, 1];
/// Version of this build (the workspace version in Cargo.toml).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// "1.2.3" as one number (10203) for `Beacon::proto_ver`. Builds before 0.2.0 announce 1.
// shortcut: the beacon has no spare field and old Tutors drop a student that sends an unknown Event,
// so the app version rides in proto_ver (nobody ever checked it); majors above 6 need a real field.
pub fn version_code(v: &str) -> u16 {
    let mut p = v.split('.').map(|x| x.parse::<u16>().unwrap_or(0));
    let (a, b, c) = (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0));
    a.saturating_mul(10_000).saturating_add(b.min(99) * 100).saturating_add(c.min(99))
}

pub fn version_name(code: u16) -> String {
    if code < 200 { "0.1".into() } else { format!("{}.{}.{}", code / 10_000, code / 100 % 100, code % 100) }
}

pub const MAX_FRAME: usize = 8 * 1024 * 1024;

/// UDP beacon sent by each student every few seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Beacon {
    pub proto_ver: u16,
    pub student_id: Uuid,
    pub hostname: String,
    pub user: String,
    pub room: String,
    pub quic_port: u16,
    pub cert_fingerprint: [u8; 32],
    pub mac: Option<[u8; 6]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebPolicy { Unrestricted, AllowListOnly, BlockList, BlockAll }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppPolicy { Unrestricted, AllowListOnly, BlockList }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerAction { Shutdown, Reboot, Logoff }

/// First frame of every file-transfer stream; the raw file bytes follow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHeader {
    pub name: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

pub const MAX_FILE_SIZE: u64 = 8 << 30; // 8 GiB

/// Tutor -> student. New variants are appended only; unknown ones decode to an error and are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Auth { tutor_nonce: [u8; 16], proof: [u8; 32] },
    OpenUrl(String),
    RunApp { path: String, args: Vec<String> },
    Message { text: String, seconds: u16 },
    Lock { blank: bool, text: Option<String> },
    Unlock,
    SetWebPolicy { mode: WebPolicy, list: Vec<String> },
    SetAppPolicy { mode: AppPolicy, list: Vec<String> },
    Power(PowerAction),
    RequestThumbnail { w: u16, h: u16, interval_ms: u32 },
    /// Join a multicast Show. Variants are only ever appended so older students still decode the rest.
    StartShow { group: [u8; 4], port: u16, key: [u8; 16], stream: u32 },
    StopShow,
    /// Ask for a hardware/software snapshot; answered with Event::Inventory.
    RequestInventory,
    /// Chat line from the teacher.
    Chat(String),
    /// Quick survey: show `question` with `options`, the student picks one.
    Poll { id: u32, question: String, options: Vec<String> },
    /// Mute or unmute the student's speakers.
    Mute(bool),
    /// Short attention sound.
    Beep,
    /// Put text on the student's clipboard.
    SetClipboard(String),
    /// Ask for the student's clipboard text; answered with Event::Clipboard.
    RequestClipboard,
    SetUsbPolicy(UsbPolicy),
    SetPrintPolicy(PrintPolicy),
    /// Ask the student to type name and class (attendance); answered with Event::Register.
    RequestRegister,
    /// Lesson info shown on the student toolbar.
    SetLesson { title: String, objectives: Vec<String>, seconds_left: u32 },
    /// Keyboard monitoring: only these words are reported. Empty list turns it off.
    SetKeywords(Vec<String>),
    /// Reward stars shown on the student toolbar.
    Reward { stars: u16 },
    /// Oral question: students may answer with Event::Hand.
    AskQuestion { id: u32, text: String },
    /// Test paper (JSON, see tutor-core::testing) handed to the student.
    StartTest { id: u32, json: String, seconds: u32 },
    StopTest,
    /// Remote file manager (absolute paths on the student machine). Answered with Event::DirListing / Event::FsResult.
    ListDir(String),
    MakeDir(String),
    DeletePath(String),
    RenamePath { from: String, to: String },
    /// End every program of the student with this exe name.
    KillProcess(String),
    /// Remote control input, coordinates normalised to 0..=1 of the screen.
    Input(InputEvent),
    /// Push files matching `pattern` ("*.docx", "report*", "*") from `folder` (relative to the student's
    /// Documents) back to the teacher.
    CollectFiles { folder: String, pattern: String },
    /// Exhibit: this student multicasts their own screen (same wire format as a teacher Show).
    StartExhibit { group: [u8; 4], port: u16, key: [u8; 16], stream: u32 },
    StopExhibit,
    /// Protect: hide drives, block Control Panel / Run / Task Manager for the signed-in student.
    SetProtect(Protect),
    /// Remote command line (Tech Console). Students ignore it unless an administrator enabled it
    /// on that machine; the output comes back as Event::FsResult.
    Shell(String),
    /// Synchronised video lesson: the file was sent first with File Distribution, then played by name.
    Video(VideoAction),
    /// The tutor made this student a group leader (true) or took it back (false); only changes the toolbar.
    SetLeader(bool),
    /// Start/stop an H.264 stream of this student's screen back to the tutor (Event::Video).
    WatchScreen { on: bool },
    /// Install a package (sent to the student first as a file). Only the student's SYSTEM service acts on it,
    /// and only if the signature, expiry and file hash check out.
    Deploy(DeployRequest),
    /// Shared whiteboard: the tutor relays every stroke to the class.
    Board(BoardMsg),
    /// Unicast fallback of a Show: one H.264 access unit (Annex B) for a student that cannot get the multicast.
    ShowFrame { keyframe: bool, data: Vec<u8> },
    /// A policy/power change signed with the Room Key (see erg-privcmd). The agent only forwards it to the SYSTEM service.
    Priv(PrivRequest),
    /// Show (true) or remove (false) the student toolbar at the top of the screen.
    Toolbar(bool),
    /// The teacher's own blocked websites and keywords, on top of the built-in adult list. Replaces the previous lists.
    SetGuard { sites: Vec<String>, words: Vec<String> },
}

/// Settings that need more rights than the student's own user; carried inside a signed [`PrivRequest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrivAction {
    Web(WebPolicy, Vec<String>),
    App(AppPolicy, Vec<String>),
    Usb(UsbPolicy),
    Print(PrintPolicy),
    Protect(Protect),
    Power(PowerAction),
}

/// `body` is a postcard-encoded [`PrivAction`]; `mac` covers `issued` (ms since the epoch) and `body`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivRequest {
    pub issued: u64,
    pub body: Vec<u8>,
    pub mac: [u8; 32],
}

/// Shared whiteboard operation. Coordinates are 0..=65535 of the board width/height; `color` is 0xRRGGBB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BoardMsg {
    /// Show the board; `draw` lets students draw too.
    Open { draw: bool },
    Close,
    Clear,
    Stroke { id: u64, color: u32, width: u8, pts: Vec<(u16, u16)> },
    /// Line, rectangle, ellipse or arrow from corner/point `a` to `b`.
    Shape { id: u64, color: u32, width: u8, kind: ShapeKind, a: (u16, u16), b: (u16, u16) },
    /// Text at `at`; `size` is the font height in board pixels (board is 960x540).
    Text { id: u64, color: u32, size: u8, at: (u16, u16), text: String },
    /// Remove one element (the author's own, or any when the tutor asks).
    Undo { id: u64 },
    /// Whole Excalidraw scene (elements JSON) from the tutor; a higher `version` replaces the previous one. Appended last: wire-compatible.
    Scene { version: u64, json: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
    Arrow,
}

pub const BOARD_MAX_POINTS: usize = 2000;
pub const BOARD_MAX_TEXT: usize = 200;
pub const BOARD_MAX_SCENE: usize = 2 * 1024 * 1024;

impl BoardMsg {
    /// Id of a drawable element (stroke, shape, text).
    pub fn element_id(&self) -> Option<u64> {
        match self {
            BoardMsg::Stroke { id, .. } | BoardMsg::Shape { id, .. } | BoardMsg::Text { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// Size limits for drawable elements; other messages are always fine here.
    pub fn valid(&self) -> bool {
        match self {
            BoardMsg::Stroke { pts, width, .. } => !pts.is_empty() && pts.len() <= BOARD_MAX_POINTS && (1..=64).contains(width),
            BoardMsg::Shape { width, .. } => (1..=64).contains(width),
            BoardMsg::Scene { json, .. } => !json.is_empty() && json.len() <= BOARD_MAX_SCENE,
            BoardMsg::Text { text, size, .. } => !text.trim().is_empty() && text.chars().count() <= BOARD_MAX_TEXT && (8..=96).contains(size),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployRequest {
    pub name: String,
    pub sha256: [u8; 32],
    pub args: String,
    /// Seconds since the UNIX epoch.
    pub expires: u64,
    pub mac: [u8; 32],
}

/// What a group leader may ask for; the tutor checks the grant before anything is sent to the group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LeaderAction {
    Message(String),
    Lock,
    Unlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VideoAction {
    /// File name inside the student's inbox (mp4 / webm only).
    Open { name: String },
    Play,
    Pause,
    Seek { secs: f32 },
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: f32, y: f32 },
    /// 0 = left, 1 = right, 2 = middle
    MouseButton { button: u8, down: bool },
    Wheel(i16),
    Key { vk: u16, down: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since 1970, 0 when unknown.
    pub modified_s: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbPolicy { Allow, ReadOnly, BlockAll, NoExecute }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintPolicy { Allow, Pause, BlockAll }

/// Student -> tutor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Challenge { nonce: [u8; 16] },
    Ready { proto_ver: u16, proof: [u8; 32] },
    Status { user: String, fg_app: String, fg_title: String, locked: bool },
    Thumbnail(Vec<u8>),
    HelpRequest(String),
    Error(String),
    Inventory(Inventory),
    /// Chat line from the student.
    Chat(String),
    PollAnswer { id: u32, choice: u8 },
    Clipboard(String),
    /// Attendance entry typed by the student.
    Register { name: String, class: String },
    /// Battery percentage (None for desktops), AC state and Wi-Fi signal 0-100.
    Power { percent: Option<u8>, ac: bool, wifi_signal: Option<u8> },
    /// A word from the teacher's keyword list was typed.
    Keyword { word: String },
    /// Student raised a hand / answered an oral question.
    Hand { question: u32, text: String },
    /// Finished test answers (JSON).
    TestDone { id: u32, answers: String },
    DirListing { path: String, entries: Vec<DirEntry> },
    /// Result of a file-manager or process command.
    FsResult { ok: bool, message: String },
    /// A group leader asks the tutor to act on the group.
    Leader(LeaderAction),
    /// One H.264 access unit of the watched screen (Annex B).
    Video { keyframe: bool, data: Vec<u8> },
    /// A stroke drawn by a student on the shared board.
    Board(BoardMsg),
    /// Show reception over the last ~2 s: frames received, frames lost, frames rebuilt with parity.
    ShowStats { recv: u32, lost: u32, fec_fixed: u32 },
    /// No usable multicast: please send the Show frames over this connection.
    ShowFallbackRequest,
    /// Multicast works again (loss < 2% for ~10 s): stop sending this student the unicast Show.
    ShowFallbackEnd,
    /// The student opened something forbidden (`kind`: "adult" website or system "tool") and locked itself with the warning screen.
    Violation { kind: String, app: String, title: String },
}

/// Machine snapshot for the tutor Tech Console. Sizes are bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub os: String,
    pub cpu: String,
    pub cores: u32,
    pub ram_total: u64,
    pub ram_used: u64,
    /// (mount point, total, free)
    pub disks: Vec<(String, u64, u64)>,
    pub uptime_s: u64,
    /// Sorted, de-duplicated program names of the logged-in user.
    pub processes: Vec<String>,
}

pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>, postcard::Error> {
    let body = postcard::to_allocvec(v)?;
    let mut out = (body.len() as u32).to_le_bytes().to_vec();
    out.extend(body);
    Ok(out)
}

/// Decode one length-prefixed frame; returns the value and bytes consumed, or None if incomplete.
pub fn decode<T: for<'a> Deserialize<'a>>(buf: &[u8]) -> Result<Option<(T, usize)>, postcard::Error> {
    if buf.len() < 4 { return Ok(None); }
    let n = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if n > MAX_FRAME { return Err(postcard::Error::DeserializeBadEncoding); }
    if buf.len() < 4 + n { return Ok(None); }
    Ok(Some((postcard::from_bytes(&buf[4..4 + n])?, 4 + n)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_codes_order_like_versions() {
        assert_eq!(version_code("0.2.0"), 200);
        assert!(version_code("0.10.1") > version_code("0.9.9") && version_code("1.0.0") > version_code("0.99.99"));
        assert_eq!(version_name(version_code("1.2.3")), "1.2.3");
        assert_eq!(version_name(1), "0.1");
        assert!(version_code(APP_VERSION) >= 200);
    }
    #[test]
    fn command_roundtrip_and_partial() {
        let c = Command::SetWebPolicy { mode: WebPolicy::AllowListOnly, list: vec!["a.vn".into()] };
        let b = encode(&c).unwrap();
        assert_eq!(decode::<Command>(&b[..b.len() - 1]).unwrap(), None);
        let (d, used) = decode::<Command>(&b).unwrap().unwrap();
        assert_eq!((d, used), (c, b.len()));
    }
    #[test]
    fn oversize_frame_rejected() {
        let mut b = ((MAX_FRAME + 1) as u32).to_le_bytes().to_vec();
        b.extend([0; 8]);
        assert!(decode::<Command>(&b).is_err());
    }
}

/// Datagrams between tutors / students and an `erg-gateway` (name server for routed networks).
/// Students simply send their normal Beacon to the gateway; tutors subscribe and get Relayed back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GatewayMsg {
    Subscribe { room: Option<String> },
    Relayed { ip: [u8; 4], beacon: Beacon },
}

/// Desktop restrictions for the signed-in student (Protect). All are per-user policies, so no
/// administrator rights are needed and nothing outlives the user's profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protect {
    pub no_control_panel: bool,
    /// Drive letters hidden in Explorer, e.g. ['D', 'E'].
    pub hide_drives: Vec<char>,
    pub no_run_dialog: bool,
    pub no_task_manager: bool,
}
