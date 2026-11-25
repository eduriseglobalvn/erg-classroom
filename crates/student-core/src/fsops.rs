//! Remote file manager and "collect work" helpers. Every path comes from the network, so each
//! operation refuses anything that is not an absolute path outside the protected system folders.
use erg_proto::DirEntry;
use std::path::{Component, Path, PathBuf};

pub const MAX_ENTRIES: usize = 5000;

/// Folders nobody may delete or rename from the network. Compared case-insensitively on path prefixes.
fn protected(p: &Path) -> bool {
    let s = p.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    let s = s.trim_end_matches('\\');
    // a drive root such as "c:" (other top-level folders like D:\Data are the teacher's to manage)
    let depth = s.split('\\').filter(|x| !x.is_empty()).count();
    if depth <= 1 {
        return true;
    }
    ["\\windows", "\\program files", "\\program files (x86)", "\\programdata", "\\system volume information", "\\$recycle.bin"]
        .iter()
        .any(|d| s.get(2..).is_some_and(|rest| rest == *d || rest.starts_with(&format!("{d}\\"))))
}

/// An absolute, normalised path without `..`; None for relative, UNC-device or odd paths.
pub fn clean_abs(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.len() > 1024 || path.chars().any(|c| c.is_control()) || path.starts_with("\\\\") {
        return None;
    }
    let p = PathBuf::from(path);
    if !p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return None;
    }
    Some(p)
}

pub fn list_dir(path: &str) -> Result<Vec<DirEntry>, String> {
    let p = clean_abs(path).ok_or("đường dẫn không hợp lệ")?;
    let mut out = vec![];
    for e in std::fs::read_dir(&p).map_err(|e| e.to_string())?.flatten().take(MAX_ENTRIES) {
        let Ok(m) = e.metadata() else { continue };
        out.push(DirEntry {
            name: e.file_name().to_string_lossy().chars().take(260).collect(),
            is_dir: m.is_dir(),
            size: if m.is_dir() { 0 } else { m.len() },
            modified_s: m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
        });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

pub fn make_dir(path: &str) -> Result<(), String> {
    let p = clean_abs(path).ok_or("đường dẫn không hợp lệ")?;
    if protected(&p) {
        return Err("thư mục hệ thống được bảo vệ".into());
    }
    std::fs::create_dir_all(p).map_err(|e| e.to_string())
}

pub fn delete(path: &str) -> Result<(), String> {
    let p = clean_abs(path).ok_or("đường dẫn không hợp lệ")?;
    if protected(&p) {
        return Err("thư mục hệ thống được bảo vệ".into());
    }
    let m = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    // A directory symlink/junction is removed as a link, never followed into its target.
    if m.is_dir() && !m.file_type().is_symlink() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p).or_else(|_| std::fs::remove_dir(&p)) }.map_err(|e| e.to_string())
}

pub fn rename(from: &str, to: &str) -> Result<(), String> {
    let (a, b) = (clean_abs(from).ok_or("đường dẫn không hợp lệ")?, clean_abs(to).ok_or("đường dẫn không hợp lệ")?);
    if protected(&a) || protected(&b) {
        return Err("thư mục hệ thống được bảo vệ".into());
    }
    if b.exists() {
        return Err("đích đã tồn tại".into());
    }
    std::fs::rename(a, b).map_err(|e| e.to_string())
}

/// `*` wildcards only, case-insensitive: "*.docx", "bai*", "*".
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.to_lowercase().chars().collect(), name.to_lowercase().chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if pi < p.len() && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// Files directly inside `<documents>/<folder>` that match `pattern`. `folder` must stay inside Documents.
pub fn collectable(documents: &Path, folder: &str, pattern: &str) -> Result<Vec<PathBuf>, String> {
    let rel = Path::new(folder);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) && !folder.is_empty() {
        return Err("thư mục phải nằm trong Tài liệu".into());
    }
    let dir = documents.join(rel);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.metadata().is_ok_and(|m| m.is_file()))
        .filter(|e| erg_net::transfer::safe_name(&e.file_name().to_string_lossy()).is_some() && glob_match(pattern, &e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    v.sort();
    v.truncate(200);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("erg-fsops-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn paths_must_be_absolute_and_free_of_dotdot() {
        for bad in ["", "rel\\x", "..\\x", "C:\\a\\..\\b", "\\\\server\\share", "\\\\.\\C:", "C:\\a\u{0}b"] {
            assert!(clean_abs(bad).is_none(), "{bad:?}");
        }
        assert!(clean_abs("C:\\Users\\hs\\Documents").is_some());
    }

    #[test]
    fn system_folders_and_drive_roots_are_protected() {
        for p in ["C:\\", "C:", "C:\\Windows", "c:\\windows\\system32", "C:\\Program Files\\App", "C:\\ProgramData"] {
            assert!(protected(Path::new(p)), "{p}");
        }
        for p in ["C:\\Users\\hs\\Documents", "D:\\Data\\bai1", "C:\\Windowsx\\a"] {
            assert!(!protected(Path::new(p)), "{p}");
        }
        assert!(delete("C:\\Windows").is_err());
        assert!(delete("C:\\").is_err());
        assert!(rename("C:\\Users\\a", "C:\\Windows\\b").is_err());
    }

    #[test]
    fn file_manager_lists_creates_renames_and_deletes() {
        let d = tmp("fm");
        let s = |p: &Path| p.to_str().unwrap().to_string();
        std::fs::write(d.join("b.txt"), "12345").unwrap();
        std::fs::write(d.join("A.txt"), "1").unwrap();
        make_dir(&s(&d.join("sub"))).unwrap();
        let l = list_dir(&s(&d)).unwrap();
        assert_eq!(l.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["sub", "A.txt", "b.txt"], "folders first, then by name");
        assert_eq!(l[2].size, 5);
        rename(&s(&d.join("b.txt")), &s(&d.join("c.txt"))).unwrap();
        assert!(rename(&s(&d.join("c.txt")), &s(&d.join("A.txt"))).is_err(), "never overwrite");
        delete(&s(&d.join("c.txt"))).unwrap();
        delete(&s(&d.join("sub"))).unwrap();
        assert!(list_dir(&s(&d.join("sub"))).is_err());
        assert!(delete(&s(&d.join("nothing"))).is_err());
    }

    #[test]
    fn glob_handles_stars_and_case() {
        assert!(glob_match("*.docx", "Bai1.DOCX"));
        assert!(glob_match("bai*", "bai1.xlsx"));
        assert!(glob_match("*", "x"));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("*.docx", "bai.doc"));
        assert!(!glob_match("a*b", "abc"));
        assert!(glob_match("", ""));
    }

    #[test]
    fn collect_stays_inside_documents_and_skips_odd_names() {
        let d = tmp("collect");
        std::fs::create_dir_all(d.join("Bai")).unwrap();
        std::fs::write(d.join("Bai").join("a.docx"), "x").unwrap();
        std::fs::write(d.join("Bai").join("b.txt"), "x").unwrap();
        std::fs::write(d.join("Bai").join("CON.docx"), "x").unwrap();
        let got = collectable(&d, "Bai", "*.docx").unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("a.docx"));
        assert!(collectable(&d, "..", "*").is_err());
        assert!(collectable(&d, "Bai\\..\\..", "*").is_err());
        assert!(collectable(&d, "C:\\Windows", "*").is_err());
        assert_eq!(collectable(&d, "", "*.zzz").unwrap().len(), 0);
    }
}

/// End the student's own programs called `name` (an exe file name, no path). Never touches other
/// users' or system processes, nor this program. Returns how many were ended.
pub fn kill_by_name(name: &str) -> Result<usize, String> {
    use sysinfo::{ProcessesToUpdate, System};
    let n = name.trim().to_ascii_lowercase();
    if erg_net::transfer::safe_name(&n).is_none() || !n.ends_with(".exe") {
        return Err("tên chương trình không hợp lệ".into());
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_user(sysinfo::UpdateKind::OnlyIfNotSet),
    );
    let me = sysinfo::get_current_pid().map_err(|e| e.to_string())?;
    let my_user = sys.process(me).and_then(|p| p.user_id().cloned()).ok_or("không xác định được người dùng")?;
    let mut ended = 0;
    for (pid, p) in sys.processes() {
        if *pid != me && p.user_id() == Some(&my_user) && p.name().to_string_lossy().to_ascii_lowercase() == n && p.kill() {
            ended += 1;
        }
    }
    Ok(ended)
}

#[cfg(test)]
mod proc_tests {
    use super::*;

    #[test]
    fn kill_ends_only_matching_programs_of_this_user() {
        use std::process::{Command, Stdio};
        let mut victim = Command::new("findstr.exe").arg("x").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        let mut other = Command::new("sort.exe").stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(kill_by_name("FINDSTR.EXE").unwrap() >= 1);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(victim.try_wait().unwrap().is_some(), "target still running");
        assert!(other.try_wait().unwrap().is_none(), "unrelated program was ended");
        let _ = other.kill();
        for bad in ["", "..\\x.exe", "C:\\x.exe", "notepad", "a/b.exe", "con.exe"] {
            assert!(kill_by_name(bad).is_err(), "{bad:?}");
        }
        assert_eq!(kill_by_name("no-such-program-xyz.exe").unwrap(), 0);
    }
}

pub const SHELL_TIMEOUT_S: u64 = 30;
pub const SHELL_MAX_OUTPUT: usize = 64 * 1024;

/// Remote command line for the Tech Console. Off unless `ERG_ALLOW_SHELL=1` is set for the
/// student process by an administrator; every use is logged. Runs with the student's own rights
/// (never more), is killed after 30 s, and its output is cut at 64 KiB.
pub fn run_shell(cmd: &str) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    if std::env::var("ERG_ALLOW_SHELL").as_deref() != Ok("1") {
        return Err("dòng lệnh từ xa đang tắt trên máy này".into());
    }
    if cmd.trim().is_empty() || cmd.len() > 2000 || cmd.contains('\0') {
        return Err("lệnh không hợp lệ".into());
    }
    tracing::warn!("remote shell: {cmd}");
    let mut child = Command::new("cmd.exe")
        .args(["/d", "/s", "/c", cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let read = |mut s: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = s.by_ref().take(SHELL_MAX_OUTPUT as u64).read_to_end(&mut v);
            // keep draining so the child never blocks on a full pipe
            let _ = std::io::copy(&mut s, &mut std::io::sink());
            v
        })
    };
    let (o, e) = (read(Box::new(child.stdout.take().unwrap())), read(Box::new(child.stderr.take().unwrap())));
    let end = std::time::Instant::now() + std::time::Duration::from_secs(SHELL_TIMEOUT_S);
    let mut timed_out = false;
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if std::time::Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mut out = String::from_utf8_lossy(&o.join().unwrap_or_default()).into_owned();
    out.push_str(&String::from_utf8_lossy(&e.join().unwrap_or_default()));
    if timed_out {
        out.push_str("\n[đã dừng sau 30 giây]");
    }
    Ok(out)
}

#[cfg(test)]
mod shell_tests {
    use super::*;

    // Environment variables are process-wide, so one test covers off, on and bad input in order.
    #[test]
    fn shell_is_off_by_default_and_bounded_when_enabled() {
        std::env::remove_var("ERG_ALLOW_SHELL");
        assert!(run_shell("echo hi").unwrap_err().contains("tắt"));
        std::env::set_var("ERG_ALLOW_SHELL", "1");
        assert_eq!(run_shell("echo hello").unwrap().trim(), "hello");
        assert!(run_shell("echo oops 1>&2").unwrap().contains("oops"), "stderr is returned too");
        assert!(run_shell("   ").is_err() && run_shell(&"x".repeat(3000)).is_err());
        let big = run_shell("for /L %i in (1,1,30000) do @echo 0123456789abcdef0123456789abcdef").unwrap();
        assert!(big.len() <= SHELL_MAX_OUTPUT + 8, "output not capped: {}", big.len());
        std::env::remove_var("ERG_ALLOW_SHELL");
    }
}
