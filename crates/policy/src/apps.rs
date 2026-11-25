//! Application control: end programs the teacher has blocked.
//!
//! Limitation (by design, documented in PLAN.md): programs are closed shortly after they start,
//! they are not prevented from starting. Preventing launch needs a kernel driver or AppLocker.
use erg_proto::AppPolicy;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub mode: AppPolicy,
    /// Executable file names such as `notepad.exe`, compared case-insensitively.
    pub list: Vec<String>,
}

impl Policy {
    pub const UNRESTRICTED: Policy = Policy { mode: AppPolicy::Unrestricted, list: Vec::new() };

    /// An allow-list with no entries would end every program, which is never what a teacher wants.
    pub fn new(mode: AppPolicy, list: Vec<String>) -> Result<Self, &'static str> {
        if mode == AppPolicy::AllowListOnly && list.iter().all(|s| s.trim().is_empty()) {
            return Err("allow-list is empty");
        }
        let list = list.into_iter().map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
        Ok(Self { mode, list })
    }
}

/// What the watcher knows about one running process.
pub struct Candidate<'a> {
    pub name: &'a str,
    pub exe: Option<&'a Path>,
    /// This program, or started by it (for example the web view that draws the student's windows).
    pub ours: bool,
    /// Owned by the logged-in student. Other users' and system processes are never touched.
    pub same_user: bool,
}

/// Programs that must keep running for the desktop to work, even in allow-list mode.
const ESSENTIAL: &[&str] = &[
    "explorer.exe", "dwm.exe", "ctfmon.exe", "sihost.exe", "taskhostw.exe", "runtimebroker.exe",
    "shellexperiencehost.exe", "startmenuexperiencehost.exe", "searchhost.exe", "textinputhost.exe",
    "applicationframehost.exe", "conhost.exe", "svchost.exe", "msedgewebview2.exe", "widgets.exe",
    "unikey.exe", "evkey.exe", "unikeynt.exe",
];

pub fn should_kill(p: &Policy, c: &Candidate, system_root: &Path) -> bool {
    if c.ours || !c.same_user {
        return false;
    }
    let name = c.name.to_ascii_lowercase();
    if c.exe.is_some_and(|e| e.starts_with(system_root)) && p.mode != AppPolicy::BlockList {
        return false; // allow-list never ends Windows' own programs
    }
    let listed = p.list.contains(&name);
    match p.mode {
        AppPolicy::Unrestricted => false,
        AppPolicy::BlockList => listed,
        AppPolicy::AllowListOnly => !listed && !ESSENTIAL.contains(&name.as_str()),
    }
}

pub struct AppBlocker {
    policy: Arc<Mutex<Policy>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AppBlocker {
    /// `on_blocked` is called with the program name each time one is ended (for a "blocked" notice).
    pub fn start(policy: Policy, on_blocked: impl Fn(&str) + Send + 'static) -> Self {
        Self::start_in(policy, on_blocked, false)
    }

    /// For the SYSTEM service: acts on the programs of the signed-in users (any account except SYSTEM/LocalService/
    /// NetworkService and the service itself) instead of those of the account it runs as.
    pub fn start_for_users(policy: Policy, on_blocked: impl Fn(&str) + Send + 'static) -> Self {
        Self::start_in(policy, on_blocked, true)
    }

    fn start_in(policy: Policy, on_blocked: impl Fn(&str) + Send + 'static, service: bool) -> Self {
        let policy = Arc::new(Mutex::new(policy));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (p, s) = (policy.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut sys = System::new();
            let me = Pid::from_u32(std::process::id());
            let root = std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
            while !s.load(std::sync::atomic::Ordering::Relaxed) {
                let pol = p.lock().expect("policy lock").clone();
                if pol.mode != AppPolicy::Unrestricted {
                    sys.refresh_processes_specifics(
                        ProcessesToUpdate::All,
                        true,
                        sysinfo::ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet).with_user(UpdateKind::OnlyIfNotSet),
                    );
                    let my_user = sys.process(me).and_then(|x| x.user_id()).cloned();
                    for (pid, proc) in sys.processes() {
                        let ours = *pid == me || is_descendant(&sys, *pid, me);
                        let same_user = if service {
                            proc.user_id().is_some_and(|u| proc.user_id() != my_user.as_ref() && is_student_account(&u.to_string()))
                        } else {
                            my_user.is_some() && proc.user_id() == my_user.as_ref()
                        };
                        let name = proc.name().to_string_lossy().to_string();
                        let c = Candidate { name: &name, exe: proc.exe(), ours, same_user };
                        if should_kill(&pol, &c, &root) && proc.kill() {
                            on_blocked(&name);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        Self { policy, stop, thread: Some(thread) }
    }

    pub fn set_policy(&self, p: Policy) {
        *self.policy.lock().expect("policy lock") = p;
    }
}

impl Drop for AppBlocker {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn is_descendant(sys: &System, mut pid: Pid, ancestor: Pid) -> bool {
    for _ in 0..32 {
        match sys.process(pid).and_then(|p| p.parent()) {
            Some(parent) if parent == ancestor => return true,
            Some(parent) => pid = parent,
            None => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cand<'a>(name: &'a str, exe: Option<&'a Path>) -> Candidate<'a> {
        Candidate { name, exe, ours: false, same_user: true }
    }
    const ROOT: &str = "C:\\Windows";

    #[test]
    fn empty_allow_list_is_refused() {
        assert!(Policy::new(AppPolicy::AllowListOnly, vec![]).is_err());
        assert!(Policy::new(AppPolicy::AllowListOnly, vec!["  ".into()]).is_err());
        assert!(Policy::new(AppPolicy::BlockList, vec![]).is_ok());
    }

    #[test]
    fn block_list_matches_names_case_insensitively() {
        let p = Policy::new(AppPolicy::BlockList, vec!["Game.EXE".into()]).unwrap();
        assert!(should_kill(&p, &cand("game.exe", None), Path::new(ROOT)));
        assert!(should_kill(&p, &cand("GAME.exe", None), Path::new(ROOT)));
        assert!(!should_kill(&p, &cand("word.exe", None), Path::new(ROOT)));
    }

    #[test]
    fn allow_list_spares_essentials_windows_and_ourselves() {
        let p = Policy::new(AppPolicy::AllowListOnly, vec!["winword.exe".into()]).unwrap();
        let sys32 = PathBuf::from("C:\\Windows\\System32\\notepad.exe");
        let game = PathBuf::from("C:\\Games\\game.exe");
        let r = Path::new(ROOT);
        assert!(!should_kill(&p, &cand("winword.exe", None), r)); // listed
        assert!(!should_kill(&p, &cand("explorer.exe", None), r)); // essential
        assert!(!should_kill(&p, &cand("notepad.exe", Some(&sys32)), r)); // inside Windows
        assert!(should_kill(&p, &cand("game.exe", Some(&game)), r)); // everything else
        assert!(!should_kill(&p, &Candidate { ours: true, ..cand("game.exe", Some(&game)) }, r));
        assert!(!should_kill(&p, &Candidate { same_user: false, ..cand("game.exe", Some(&game)) }, r));
    }

    #[test]
    fn unrestricted_never_kills() {
        assert!(!should_kill(&Policy::UNRESTRICTED, &cand("game.exe", None), Path::new(ROOT)));
    }

    fn running(name: &str) -> bool {
        let mut s = System::new();
        s.refresh_processes(ProcessesToUpdate::All, true);
        s.processes().values().any(|p| p.name().to_string_lossy().eq_ignore_ascii_case(name))
    }

    /// Live: a blocked program that really runs is ended within a couple of seconds, other programs are left alone.
    #[cfg(windows)]
    #[test]
    fn blocker_ends_a_blocked_program_and_spares_others() {
        use std::process::{Command, Stdio};
        let spawn = |exe: &str, args: &[&str]| Command::new(exe).args(args).stdout(Stdio::null()).spawn().unwrap();
        // started through cmd so it is not our descendant (descendants are exempt)
        spawn("cmd.exe", &["/c", "start", "/b", "ping.exe", "-n", "120", "127.0.0.1"]).wait().ok(); // cmd exits at once, ping lives on
        let mut other = Command::new("findstr.exe").arg("x").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap(); // waits on stdin
        // under parallel test load ping may not exist yet; without this wait the "died" check passes vacuously
        let up = std::time::Instant::now() + Duration::from_secs(10);
        while !running("ping.exe") && std::time::Instant::now() < up {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(running("ping.exe"), "test setup: ping did not start");
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let s2 = seen.clone();
        let blocker = AppBlocker::start(Policy::new(AppPolicy::BlockList, vec!["ping.exe".into()]).unwrap(), move |n| {
            s2.lock().unwrap().push(n.to_string())
        });
        let end = std::time::Instant::now() + Duration::from_secs(5);
        let mut died = false;
        while std::time::Instant::now() < end {
            if !running("ping.exe") {
                died = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let other_alive = other.try_wait().unwrap().is_none();
        drop(blocker);
        let _ = Command::new("taskkill").args(["/f","/im","ping.exe"]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = other.kill();
        assert!(died, "blocked program was not ended");
        assert!(other_alive, "an unrelated program was ended");
        assert!(seen.lock().unwrap().iter().any(|n| n.eq_ignore_ascii_case("ping.exe")));
    }
}

/// Browsers that do not read the Chrome / Edge / Firefox policies we write, so they would bypass a
/// web filter. While a web filter is on they are ended like any blocked program.
pub const UNMANAGED_BROWSERS: &[&str] = &[
    "opera.exe", "opera_gx.exe", "brave.exe", "tor.exe", "vivaldi.exe", "chromium.exe", "librewolf.exe", "waterfox.exe", "palemoon.exe", "yandex.exe",
];

/// Policy that ends the unmanaged browsers (used next to the web filter, independent of the teacher's app policy).
pub fn browser_guard() -> Policy {
    Policy::new(AppPolicy::BlockList, UNMANAGED_BROWSERS.iter().map(|s| s.to_string()).collect()).expect("block list is never empty")
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn guard_ends_unmanaged_browsers_only() {
        let g = browser_guard();
        let c = |name: &'static str| Candidate { name, exe: Some(Path::new("C:\\Users\\hs\\AppData\\x.exe")), ours: false, same_user: true };
        let root = Path::new("C:\\Windows");
        assert!(should_kill(&g, &c("Brave.exe"), root));
        assert!(should_kill(&g, &c("tor.exe"), root));
        for ok in ["chrome.exe", "msedge.exe", "firefox.exe", "winword.exe"] {
            assert!(!should_kill(&g, &c(ok), root), "{ok}");
        }
    }
}

/// A SID that belongs to a person's account, not to Windows itself (SYSTEM, LocalService, NetworkService).
pub fn is_student_account(sid: &str) -> bool {
    sid.starts_with("S-1-5-21-") || sid.starts_with("S-1-12-1-") // domain/local accounts, Microsoft/Entra accounts
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn service_scope_covers_people_not_windows_accounts() {
        assert!(is_student_account("S-1-5-21-1-2-3-1001"));
        assert!(is_student_account("S-1-12-1-1-2-3-4"));
        for sys in ["S-1-5-18", "S-1-5-19", "S-1-5-20", "S-1-5-80-1-2-3-4-5", "S-1-5-90-0-1", ""] {
            assert!(!is_student_account(sys), "{sys}");
        }
    }
}
