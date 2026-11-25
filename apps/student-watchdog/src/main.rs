//! `erg-student-watchdog`: a Windows service (LocalSystem) that keeps the student agent alive in the
//! logged-in user's session. A student cannot end it without administrator rights, so killing the
//! agent only makes it come straight back.
//!
//!   erg-student-watchdog install <path-to-erg-student-agent.exe>   (administrator)
//!   erg-student-watchdog uninstall                                 (administrator)
//!   erg-student-watchdog console <path-to-agent>                   (foreground loop, for testing)
//!
//! The service itself never draws anything or talks to the network (session 0 cannot show windows);
//! it starts the agent with the user's token, holds the Room Key (pipe ERGClassroomAuth) and applies the tutor-signed
//! policy/power requests (pipe ERGClassroomPriv, log in %ProgramData%\ERGClassroom\priv.log).

/// What to do on one watchdog tick. Pure so it can be tested without Windows.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Launch(u32),
}

/// At most this many starts a minute: a crashing agent must not make the machine spin, yet a student who keeps
/// ending it in Task Manager must not win either (the loop looks every 2 s, so 30 is the most it could do).
pub const MAX_LAUNCHES: usize = 20;

/// `agent_sessions`: sessions where the agent is running. `active`: the console session with a
/// logged-in user (None at the logon screen). `launches_last_minute` is held against MAX_LAUNCHES.
pub fn decide(agent_sessions: &[u32], active: Option<u32>, launches_last_minute: usize) -> Action {
    match active {
        Some(s) if !agent_sessions.contains(&s) && launches_last_minute < MAX_LAUNCHES => Action::Launch(s),
        _ => Action::Nothing,
    }
}

/// 32 key bytes from the 64 hex characters of room.key.
pub fn parse_key(s: &str) -> Option<[u8; 32]> {
    let h = valid_key(s)?;
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(k)
}

/// The key as 64 hex characters, or None.
pub fn valid_key(s: &str) -> Option<&str> {
    let s = s.trim();
    (s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())).then_some(s)
}

/// `name=1` on a line of config.txt (case-insensitive name).
pub fn config_flag(text: &str, name: &str) -> bool {
    text.trim_start_matches('\u{feff}').lines().any(|l| l.split_once('=').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case(name) && v.trim() == "1"))
}

/// Terminal server mode: launch the agent in every active session that lacks one (throttled like `decide`).
pub fn decide_many(agent_sessions: &[u32], active: &[u32], launches_last_minute: usize) -> Vec<u32> {
    if launches_last_minute >= MAX_LAUNCHES {
        return vec![];
    }
    active.iter().copied().filter(|s| !agent_sessions.contains(s)).collect()
}

#[cfg(windows)]
mod win {
    use super::{decide, Action};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use windows::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId, WTSQueryUserToken};
    use windows::Win32::System::Threading::*;

    pub const NAME: &str = "ERGStudent";

    /// Stops the deploy thread when `watch` returns.
    struct Finish(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Finish {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Sessions in which a process with this exe file name runs.
    fn agent_sessions(exe: &str) -> Vec<u32> {
        let mut out = vec![];
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            if Process32FirstW(snap, &mut e).is_ok() {
                loop {
                    let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
                    if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                        let mut s = 0u32;
                        if ProcessIdToSessionId(e.th32ProcessID, &mut s).is_ok() {
                            out.push(s);
                        }
                    }
                    if Process32NextW(snap, &mut e).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    /// Start `agent` in `session` with that session's user token on the interactive desktop.
    fn launch(agent: &Path, session: u32) -> Result<(), String> {
        let mut cmd: Vec<u16> = format!("\"{}\"", agent.display()).encode_utf16().chain(std::iter::once(0)).collect();
        let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let mut token = HANDLE::default();
            WTSQueryUserToken(session, &mut token).map_err(|e| format!("WTSQueryUserToken: {e}"))?;
            let mut env = std::ptr::null_mut();
            let have_env = CreateEnvironmentBlock(&mut env, Some(token), false).is_ok();
            let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, lpDesktop: PWSTR(desktop.as_mut_ptr()), ..Default::default() };
            let mut pi = PROCESS_INFORMATION::default();
            let r = CreateProcessAsUserW(
                Some(token), None, Some(PWSTR(cmd.as_mut_ptr())), None, None, false,
                CREATE_UNICODE_ENVIRONMENT,
                if have_env { Some(env) } else { None }, None, &si, &mut pi,
            );
            if have_env {
                let _ = DestroyEnvironmentBlock(env);
            }
            let _ = CloseHandle(token);
            r.map_err(|e| format!("CreateProcessAsUserW: {e}"))?;
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
        }
        Ok(())
    }

    /// `terminal_server=1` in config.txt: one student per signed-in user session instead of one per computer.
    fn terminal_server() -> bool {
        std::fs::read_to_string(data_dir().join("config.txt")).is_ok_and(|t| super::config_flag(&t, "terminal_server"))
    }

    /// (session id, SID) of the user in each session the agent runs in: the console session, or every active one in terminal-server mode.
    fn session_users() -> Vec<(u32, String)> {
        let sessions = if terminal_server() {
            active_sessions()
        } else {
            let c = unsafe { WTSGetActiveConsoleSessionId() };
            if c != 0xFFFF_FFFF && c != 0 { vec![c] } else { vec![] }
        };
        sessions
            .into_iter()
            .filter_map(|s| unsafe {
                let mut t = HANDLE::default();
                WTSQueryUserToken(s, &mut t).ok()?;
                let sid = erg_authpipe::token_sid(t);
                let _ = CloseHandle(t);
                sid.map(|x| (s, x))
            })
            .collect()
    }

    fn log_line(file: &str, line: &str) {
        use std::io::Write;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join(file)) {
            let _ = writeln!(f, "{now} {line}");
        }
    }

    /// Privileged policy requests: the agent forwards what the tutor signed over \\.\pipe\ERGClassroomPriv, this
    /// service (SYSTEM) checks the signature and applies it. The pipe is open only to SYSTEM, administrators and
    /// the signed-in users' SIDs, which are refreshed every 2 s.
    fn priv_loop(done: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        use std::sync::atomic::Ordering;
        use std::sync::{Arc, Mutex};
        while !done.load(Ordering::Relaxed) {
            let Some(k) = std::fs::read_to_string(data_dir().join("room.key")).ok().and_then(|s| super::parse_key(&s)) else {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            };
            // power requests are always honoured (they are signed with the Room Key); allow_power in config.txt is no longer read
            let runner = erg_policy::runner::Runner::new(session_users, true);
            let gate = Mutex::new(erg_privcmd::Gate::default());
            let users = Arc::new(Mutex::new(session_users().into_iter().map(|u| u.1).collect::<Vec<_>>()));
            let handler = Arc::new(move |req: &[u8]| {
                let mut log = |a: erg_proto::PrivAction| {
                    let r = runner.run(a.clone());
                    log_line("priv.log", &format!("{:?} -> {}", a, r.map_or_else(|e| format!("FAILED {e}"), |_| "ok".into())));
                };
                let now = erg_privcmd::now_ms();
                let out = erg_privcmd::serve_bytes(&k, &mut gate.lock().expect("gate lock"), req, now, &mut log);
                if out == [0] {
                    // silent refusals made "Shutdown does nothing" impossible to diagnose
                    let skew = erg_privcmd::decode(req).map_or(0, |r| r.issued.abs_diff(now) / 1000);
                    log_line("priv.log", &format!("request REFUSED: wrong Room Key, replayed, or clocks differ too much (tutor and this PC differ by {skew} s, limit 300)"));
                }
                out
            });
            let server = erg_authpipe::serve_priv(erg_authpipe::PRIV_NAME, users.clone(), handler);
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(2));
                let now: Vec<String> = session_users().into_iter().map(|u| u.1).collect();
                let mut cur = users.lock().expect("users lock");
                if *cur != now {
                    *cur = now;
                    drop(cur);
                    server.refresh();
                }
            }
            server.stop();
            return;
        }
    }

    /// Ids of the active user sessions (session 0 is the services session and is skipped).
    fn active_sessions() -> Vec<u32> {
        use windows::Win32::System::RemoteDesktop::{WTSActive, WTSEnumerateSessionsW, WTSFreeMemory, WTS_SESSION_INFOW};
        let mut out = vec![];
        unsafe {
            let (mut p, mut n) = (windows::core::PWSTR::null(), 0u32);
            if WTSEnumerateSessionsW(None, 0, 1, &mut p as *mut _ as *mut *mut WTS_SESSION_INFOW, &mut n).is_ok() {
                let list = std::slice::from_raw_parts(p.0 as *const WTS_SESSION_INFOW, n as usize);
                out = list.iter().filter(|s| s.State == WTSActive && s.SessionId != 0).map(|s| s.SessionId).collect();
                WTSFreeMemory(p.0 as *mut _);
            }
        }
        out
    }

    fn data_dir() -> PathBuf {
        agent_path_file().parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// Hold the Room Key for the students: answer proofs on the pipe so the agent never needs the key.
    /// room.key may appear after the service starts (installer order), so keep trying until it parses.
    fn auth_loop(done: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        use std::sync::atomic::Ordering;
        while !done.load(Ordering::Relaxed) {
            if let Some(k) = std::fs::read_to_string(data_dir().join("room.key")).ok().and_then(|s| super::parse_key(&s)) {
                let server = erg_authpipe::serve(erg_authpipe::DEFAULT_NAME, std::sync::Arc::new(k));
                while !done.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(1));
                }
                server.stop();
                return;
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }

    /// Remote Deploy: install packages the tutor signed. Runs on its own thread because installers take minutes.
    fn deploy_loop(done: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        use std::sync::atomic::Ordering;
        let (spool, work) = (data_dir().join("deploy"), data_dir().join("deploy-work"));
        let _ = std::fs::create_dir_all(&spool);
        let _ = std::fs::create_dir_all(&work);
        // students may leave requests; only SYSTEM and administrators may touch the copies being installed
        let _ = std::process::Command::new("icacls.exe").arg(&spool).args(["/grant", "*S-1-5-32-545:(OI)(CI)M"]).output();
        let _ = std::process::Command::new("icacls.exe").arg(&work).args(["/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F"]).output();
        let run = |prog: &str, args: &[String]| -> Result<i32, String> {
            let mut c = std::process::Command::new(prog).args(args).spawn().map_err(|e| e.to_string())?;
            let start = Instant::now();
            loop {
                if let Some(s) = c.try_wait().map_err(|e| e.to_string())? { return Ok(s.code().unwrap_or(-1)); }
                if start.elapsed() > Duration::from_secs(30 * 60) { let _ = c.kill(); return Err("timed out after 30 minutes".into()); }
                std::thread::sleep(Duration::from_secs(1));
            }
        };
        while !done.load(Ordering::Relaxed) {
            let key = std::fs::read_to_string(data_dir().join("room.key")).ok().and_then(|s| super::parse_key(&s));
            if let Some(k) = key {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                for line in erg_deploy::process(&spool, &work, &k, now, &run) {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join("deploy.log")) {
                        let _ = writeln!(f, "{now} {line}");
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }

    /// Loop until `stop` returns true.
    pub fn watch(agent: &Path, stop: &dyn Fn() -> bool) {
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let d2 = done.clone();
        std::thread::spawn(move || deploy_loop(d2));
        let d3 = done.clone();
        std::thread::spawn(move || auth_loop(d3));
        let d4 = done.clone();
        std::thread::spawn(move || priv_loop(d4));
        let _guard = Finish(done);
        let exe = agent.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let mut launches: Vec<Instant> = vec![];
        while !stop() {
            launches.retain(|t| t.elapsed() < Duration::from_secs(60));
            let running = agent_sessions(&exe);
            let todo: Vec<u32> = if terminal_server() {
                super::decide_many(&running, &active_sessions(), launches.len())
            } else {
                let console = unsafe { WTSGetActiveConsoleSessionId() };
                // 0xFFFFFFFF = no console session; session 0 = services only, no user to show windows to
                let active = (console != 0xFFFF_FFFF && console != 0).then_some(console);
                match decide(&running, active, launches.len()) { Action::Launch(s) => vec![s], Action::Nothing => vec![] }
            };
            for s in todo {
                // a service has no console: without this file nobody can tell why the student program is not running
                match launch(agent, s) {
                    Ok(()) => log_line("watchdog.log", &format!("started {} in session {s}", agent.display())),
                    Err(e) => log_line("watchdog.log", &format!("cannot start {} in session {s}: {e}", agent.display())),
                }
                launches.push(Instant::now());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    fn agent_path_file() -> PathBuf {
        PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".into())).join("ERGClassroom").join("agent.path")
    }

    pub fn install(agent: &str) -> Result<(), String> {
        use windows_service::service::*;
        use windows_service::service_manager::*;
        let agent = std::fs::canonicalize(agent).map_err(|e| format!("agent not found: {e}"))?;
        std::fs::create_dir_all(agent_path_file().parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(agent_path_file(), agent.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE).map_err(|e| format!("need administrator rights: {e}"))?;
        let info = ServiceInfo {
            name: OsString::from(NAME),
            display_name: OsString::from("ERG Classroom Student"),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: std::env::current_exe().map_err(|e| e.to_string())?,
            launch_arguments: vec![OsString::from("service")],
            dependencies: vec![],
            account_name: None, // LocalSystem
            account_password: None,
        };
        let svc = mgr.create_service(&info, ServiceAccess::START | ServiceAccess::CHANGE_CONFIG).map_err(|e| e.to_string())?;
        // restart the watchdog itself if it ever crashes
        let _ = std::process::Command::new("sc.exe").args(["failure", NAME, "reset=", "60", "actions=", "restart/2000/restart/2000/restart/2000"]).status();
        svc.start::<&str>(&[]).map_err(|e| e.to_string())
    }

    pub fn uninstall() -> Result<(), String> {
        use windows_service::service::*;
        use windows_service::service_manager::*;
        let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(|e| e.to_string())?;
        let svc = mgr.open_service(NAME, ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS).map_err(|e| e.to_string())?;
        let _ = svc.stop();
        svc.delete().map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(agent_path_file());
        Ok(())
    }

    windows_service::define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use windows_service::service::*;
        use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let Ok(status) = service_control_handler::register(NAME, move |c| match c {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                s2.store(true, Ordering::Relaxed);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }) else { return };
        let set = |state, accepted| {
            let _ = status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS, current_state: state, controls_accepted: accepted,
                exit_code: ServiceExitCode::Win32(0), checkpoint: 0, wait_hint: Duration::from_secs(5), process_id: None,
            });
        };
        set(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN);
        match std::fs::read_to_string(agent_path_file()) {
            Ok(p) => watch(Path::new(p.trim()), &|| stop.load(Ordering::Relaxed)),
            Err(e) => log_line("watchdog.log", &format!("no agent path configured ({}): {e}", agent_path_file().display())),
        }
        set(ServiceState::Stopped, ServiceControlAccept::empty());
    }

    pub fn run_service() -> Result<(), String> {
        windows_service::service_dispatcher::start(NAME, ffi_service_main).map_err(|e| e.to_string())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    {
        let r = match args.get(1).map(String::as_str) {
            Some("install") => win::install(args.get(2).map(String::as_str).unwrap_or("")),
            Some("uninstall") => win::uninstall(),
            Some("service") => win::run_service(),
            Some("console") => {
                win::watch(std::path::Path::new(args.get(2).map(String::as_str).unwrap_or("")), &|| false);
                Ok(())
            }
            _ => Err("usage: erg-student-watchdog install <agent.exe> | uninstall | console <agent.exe>".into()),
        };
        if let Err(e) = r {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        eprintln!("Windows only");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launches_only_when_a_user_is_logged_in_and_the_agent_is_missing_from_that_session() {
        assert_eq!(decide(&[], Some(1), 0), Action::Launch(1));
        assert_eq!(decide(&[1], Some(1), 0), Action::Nothing, "already running");
        assert_eq!(decide(&[2], Some(1), 0), Action::Launch(1), "another user's agent does not count");
        assert_eq!(decide(&[], None, 0), Action::Nothing, "nobody logged in");
    }

    #[test]
    fn terminal_server_mode_launches_one_agent_per_session() {
        assert_eq!(decide_many(&[2], &[2, 3, 5], 0), vec![3, 5]);
        assert_eq!(decide_many(&[2, 3], &[2, 3], 0), Vec::<u32>::new());
        assert_eq!(decide_many(&[], &[2], MAX_LAUNCHES), Vec::<u32>::new(), "crash loop throttled");
    }

    #[test]
    fn config_flags_are_exact() {
        let t = "room=A\r\nAllow_Power = 1\r\nterminal_server=0\r\n";
        assert!(config_flag(t, "allow_power"));
        assert!(!config_flag(t, "terminal_server"));
        assert!(!config_flag(t, "power"), "no prefix matching");
    }

    #[test]
    fn key_bytes_are_decoded() {
        assert_eq!(parse_key(&format!("{}
", "0f".repeat(32))), Some([15u8; 32]));
        assert_eq!(parse_key("zz"), None);
    }

    #[test]
    fn only_a_64_hex_key_is_passed_on() {
        assert!(valid_key(&"a".repeat(64)).is_some());
        assert!(valid_key(&format!(" {}
", "F".repeat(64))).is_some());
        assert!(valid_key(&"a".repeat(63)).is_none());
        assert!(valid_key(&format!("{}g", "a".repeat(63))).is_none());
    }

    #[test]
    fn a_crash_loop_is_throttled() {
        assert_eq!(decide(&[], Some(1), MAX_LAUNCHES - 1), Action::Launch(1));
        assert_eq!(decide(&[], Some(1), MAX_LAUNCHES), Action::Nothing);
    }
}
