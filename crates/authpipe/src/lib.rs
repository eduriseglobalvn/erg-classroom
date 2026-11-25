//! Named-pipe transports (Windows only), served by the SYSTEM service.
//! * Room Key oracle (`serve`): the service holds the key and answers `erg_net::auth_oracle` requests; the
//!   student runtime in the user's session asks through the pipe and never sees the key. Any signed-in user may
//!   use it: the answers are only "is this the tutor's proof" (a yes/no) and the student's own proof.
//! * Privileged requests (`serve_priv`): the pipe is only open to SYSTEM, administrators and the SIDs of the users
//!   signed in at the console, and the service runs `erg_privcmd::Gate` on every request, so the agent can
//!   forward what the tutor signed but cannot make anything up.
#![cfg(windows)]
use erg_net::auth_oracle::{answer, Remote, StudentAuth, PING};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::IO::CancelIoEx;
use windows::Win32::System::Pipes::*;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub const DEFAULT_NAME: &str = r"\\.\pipe\ERGClassroomAuth";
/// Signed privileged requests (policy, power); see `serve_priv`.
pub const PRIV_NAME: &str = r"\\.\pipe\ERGClassroomPriv";
const MAX_REQUEST: usize = 128;
const MAX_PRIV_REQUEST: usize = erg_privcmd::MAX_BODY + 256;
/// A client that connects and then says nothing is cut off after this long.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);
/// Everyone signed in may talk to the key oracle; SYSTEM and administrators have full control.
const AUTH_SDDL: &str = "D:(A;;GRGW;;;AU)(A;;GA;;;SY)(A;;GA;;;BA)";

type Handler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// A SID string such as `S-1-5-21-1-2-3-1001`. Only these ever reach an SDDL string.
pub fn valid_sid(s: &str) -> bool {
    s.len() <= 100
        && s.len() > 4
        && s.starts_with("S-1-")
        && s[4..].chars().all(|c| c.is_ascii_digit() || c == '-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Pipe ACL: SYSTEM and administrators everything, each listed user read/write only. Invalid SIDs are dropped.
pub fn sddl_for_users(sids: &[String]) -> String {
    let mut s = String::from("D:(A;;GA;;;SY)(A;;GA;;;BA)");
    for sid in sids.iter().filter(|x| valid_sid(x)) {
        s.push_str(&format!("(A;;GRGW;;;{sid})"));
    }
    s
}

/// SID string of the user a token belongs to.
pub fn token_sid(token: HANDLE) -> Option<String> {
    unsafe {
        let mut need = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut need);
        if need == 0 {
            return None;
        }
        let mut buf = vec![0u64; (need as usize).div_ceil(8)]; // 8-byte aligned for TOKEN_USER
        GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), need, &mut need).ok()?;
        let tu = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut p = PWSTR::null();
        ConvertSidToStringSidW(tu.User.Sid, &mut p).ok()?;
        let out = p.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(p.0 as *mut _)));
        out
    }
}

/// SID of the user running this process.
pub fn current_user_sid() -> Option<String> {
    unsafe {
        let mut t = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut t).ok()?;
        let s = token_sid(t);
        let _ = CloseHandle(t);
        s
    }
}

pub struct Server {
    stop: Arc<AtomicBool>,
    name: String,
}

impl Server {
    /// Stop accepting; in-flight requests finish.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake();
    }

    /// Re-read the allowed users: the listening instance is replaced so a changed ACL takes effect.
    pub fn refresh(&self) {
        self.wake();
    }

    fn wake(&self) {
        let _ = call_with(&self.name, PING, 500); // wakes the blocked accept loop
    }
}

/// Serve `auth` on the pipe `name` from a background thread.
pub fn serve(name: &str, auth: Arc<dyn StudentAuth>) -> Server {
    serve_with(name, Arc::new(|| AUTH_SDDL.to_string()), MAX_REQUEST, Arc::new(move |req: &[u8]| answer(auth.as_ref(), req)))
}

/// Serve `handler` for privileged requests on `name`, open only to SYSTEM, administrators and the SIDs in `users`.
/// `handler` must refuse anything unsigned: the pipe ACL is the first gate, `Gate::admit` the second.
pub fn serve_priv(name: &str, users: Arc<Mutex<Vec<String>>>, handler: Handler) -> Server {
    serve_with(name, Arc::new(move || sddl_for_users(&users.lock().expect("users lock"))), MAX_PRIV_REQUEST, handler)
}

fn serve_with(name: &str, sddl: Arc<dyn Fn() -> String + Send + Sync>, max: usize, handler: Handler) -> Server {
    let stop = Arc::new(AtomicBool::new(false));
    let (flag, n) = (stop.clone(), name.to_string());
    std::thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            let Some(pipe) = create(&n, &sddl(), max as u32) else {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            };
            // blocks until a client connects (ERROR_PIPE_CONNECTED means one already did)
            let connected = unsafe { ConnectNamedPipe(pipe, None) };
            if connected.is_err() && unsafe { windows::Win32::Foundation::GetLastError() } != windows::Win32::Foundation::ERROR_PIPE_CONNECTED {
                unsafe { let _ = CloseHandle(pipe); }
                continue;
            }
            let h = handler.clone();
            let raw = pipe.0 as usize;
            std::thread::spawn(move || handle(HANDLE(raw as *mut _), h.as_ref(), max));
        }
    });
    Server { stop, name: name.to_string() }
}

fn create(name: &str, sddl: &str, buf: u32) -> Option<HANDLE> {
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(&HSTRING::from(sddl), SDDL_REVISION_1, &mut sd, None).ok()?;
        let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: false.into() };
        let h = CreateNamedPipeW(
            &HSTRING::from(name), PIPE_ACCESS_DUPLEX, PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES, 256, buf.max(256), 0, Some(&sa),
        );
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        (!h.is_invalid()).then_some(h)
    }
}

fn handle(pipe: HANDLE, handler: &(dyn Fn(&[u8]) -> Vec<u8> + Send + Sync), max: usize) {
    unsafe {
        let done = Arc::new(AtomicBool::new(false));
        let (d2, raw) = (done.clone(), pipe.0 as usize);
        std::thread::spawn(move || {
            std::thread::sleep(CLIENT_TIMEOUT);
            if !d2.load(Ordering::Relaxed) {
                let _ = CancelIoEx(HANDLE(raw as *mut _), None); // unblocks a silent client's ReadFile
            }
        });
        let mut buf = vec![0u8; max];
        let mut n = 0u32;
        // a message longer than `max` fails with ERROR_MORE_DATA and gets no answer
        if ReadFile(pipe, Some(&mut buf), Some(&mut n), None).is_ok() {
            let reply = handler(&buf[..n as usize]);
            let mut w = 0u32;
            let _ = WriteFile(pipe, Some(&reply), Some(&mut w), None);
            let _ = FlushFileBuffers(pipe);
        }
        done.store(true, Ordering::Relaxed);
        let _ = DisconnectNamedPipe(pipe);
        let _ = CloseHandle(pipe);
    }
}

pub fn call_with(name: &str, req: &[u8], timeout_ms: u32) -> Option<Vec<u8>> {
    let mut out = [0u8; 64];
    let mut n = 0u32;
    unsafe {
        let ok = CallNamedPipeW(
            PCWSTR(HSTRING::from(name).as_ptr()), Some(req.as_ptr() as *const _), req.len() as u32,
            Some(out.as_mut_ptr() as *mut _), out.len() as u32, &mut n, timeout_ms,
        )
        .as_bool();
        if !ok {
            return None;
        }
    }
    Some(out[..n as usize].to_vec())
}

/// True when a key holder answers on this pipe (so the student runtime needs no key file).
pub fn available(name: &str) -> bool {
    call_with(name, PING, 1000).is_some_and(|a| a == b"K")
}

/// A `StudentAuth` that asks the key holder on the pipe.
pub fn client(name: &str) -> impl StudentAuth {
    let n = name.to_string();
    Remote(move |req: &[u8]| call_with(&n, req, 3000))
}

/// Agent side: hand a tutor-signed request to the SYSTEM service. `None` = no service on the machine (per-user
/// mode); `Some(false)` = the service refused it or could not be reached in time.
pub fn forward(name: &str, r: &erg_proto::PrivRequest) -> Option<bool> {
    if !available(name) {
        return None;
    }
    Some(call_with(name, &erg_privcmd::encode(r), 5000).is_some_and(|a| a == [1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use erg_proto::{PowerAction, PrivAction, PrivRequest, UsbPolicy};

    fn pipe_name() -> String {
        format!(r"\\.\pipe\erg-auth-test-{:016x}", rand::random::<u64>())
    }

    #[test]
    fn a_keyless_client_gets_proofs_and_verdicts_through_a_real_pipe() {
        let (name, key) = (pipe_name(), [5u8; 32]);
        let server = serve(&name, Arc::new(key));
        std::thread::sleep(Duration::from_millis(200));
        assert!(available(&name));
        let c = client(&name);
        let (ex, n, tn) = ([1u8; 32], [2u8; 16], [3u8; 16]);
        assert_eq!(c.prove_student(&ex, &tn, &n), key.prove_student(&ex, &tn, &n), "same proof as the key holder would make");
        let tutor = erg_net::auth_oracle::request_verify(&ex, &n, &tn, &[0; 32]);
        assert_eq!(call_with(&name, &tutor, 2000), Some(vec![0]), "a made-up tutor proof is refused");
        // garbage and oversize requests get an empty answer and do not break the server
        assert_eq!(call_with(&name, b"junk", 2000), Some(vec![]));
        assert!(available(&name));
        server.stop();
    }

    #[test]
    fn a_client_that_connects_and_stays_silent_does_not_block_others() {
        let name = pipe_name();
        let server = serve(&name, Arc::new([6u8; 32]));
        std::thread::sleep(Duration::from_millis(200));
        // open the pipe and never write
        let silent = std::fs::OpenOptions::new().read(true).write(true).open(&name).expect("connect");
        for _ in 0..3 {
            assert!(available(&name), "other clients are still served");
        }
        drop(silent);
        server.stop();
    }

    #[test]
    fn nobody_listening_means_not_available() {
        assert!(!available(&pipe_name()));
    }

    #[test]
    fn sids_are_validated_before_they_reach_an_sddl_string() {
        assert!(valid_sid("S-1-5-21-1004336348-1177238915-682003330-512"));
        for bad in ["", "S-1-", "S-1-5-18)(A;;GA;;;WD", "S-1-5--1", "S-1-5-1-", "X-1-5-18", "S-1-5-a", "S-1-5-18\n"] {
            assert!(!valid_sid(bad), "{bad:?}");
        }
        let acl = sddl_for_users(&["S-1-5-21-1-2-3-1001".into(), "S-1-5-18)(A;;GA;;;WD".into()]);
        assert_eq!(acl, "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;S-1-5-21-1-2-3-1001)");
        assert_eq!(sddl_for_users(&[]), "D:(A;;GA;;;SY)(A;;GA;;;BA)", "no signed-in user: only SYSTEM and administrators");
    }

    #[test]
    fn the_current_user_sid_can_be_read_and_makes_a_valid_acl() {
        let me = current_user_sid().expect("own SID");
        assert!(valid_sid(&me), "{me}");
        assert!(create(&pipe_name(), &sddl_for_users(&[me]), 256).is_some(), "Windows accepts the generated ACL");
    }

    /// The pipe ACL is what keeps other users out: with this user's SID listed the request goes through the
    /// Gate, with only SYSTEM listed this (non-system) user cannot even open the pipe.
    #[test]
    fn only_the_listed_user_reaches_the_priv_service_and_the_gate_still_decides() {
        let key = [8u8; 32];
        let make = |users: Vec<String>, name: &str| {
            let gate = Mutex::new(erg_privcmd::Gate::default());
            let seen = Arc::new(Mutex::new(vec![]));
            let s2 = seen.clone();
            let h: Handler = Arc::new(move |req: &[u8]| {
                erg_privcmd::serve_bytes(&key, &mut gate.lock().unwrap(), req, erg_privcmd::now_ms(), &mut |a| s2.lock().unwrap().push(a))
            });
            (serve_priv(name, Arc::new(Mutex::new(users)), h), seen)
        };

        let (name, me) = (pipe_name(), current_user_sid().unwrap());
        let (server, seen) = make(vec![me.clone()], &name);
        std::thread::sleep(Duration::from_millis(200));
        assert!(available(&name), "the listed user reaches the service");
        let ok = erg_privcmd::sign(&key, &PrivAction::Usb(UsbPolicy::NoExecute), erg_privcmd::now_ms()).unwrap();
        assert_eq!(forward(&name, &ok), Some(true));
        assert_eq!(forward(&name, &ok), Some(false), "replayed");
        let forged = erg_privcmd::sign(&[1; 32], &PrivAction::Power(PowerAction::Shutdown), erg_privcmd::now_ms() + 5).unwrap();
        assert_eq!(forward(&name, &forged), Some(false), "an agent without the Room Key cannot make a request");
        assert_eq!(forward(&name, &PrivRequest { issued: 1, body: vec![1, 2, 3], mac: [0; 32] }), Some(false));
        assert_eq!(*seen.lock().unwrap(), vec![PrivAction::Usb(UsbPolicy::NoExecute)], "only the signed request ran");
        server.stop();

        // someone else's SID only: this user is locked out by the ACL (S-1-5-18 = SYSTEM, whose ACE is always there)
        let name2 = pipe_name();
        let (_server2, seen2) = make(vec![], &name2);
        std::thread::sleep(Duration::from_millis(200));
        assert!(!available(&name2), "a user who is not listed cannot open the pipe");
        assert_eq!(forward(&name2, &ok), None);
        assert!(seen2.lock().unwrap().is_empty());
    }
}
