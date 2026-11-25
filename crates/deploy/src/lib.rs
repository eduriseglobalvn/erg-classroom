//! Remote Deploy: the tutor signs "install this package" with the Room Key; the student's SYSTEM service
//! (watchdog) checks the signature, the expiry and the file's SHA-256 before running the installer.
//! A student who can write into the spool folder still cannot make the service run anything the tutor
//! did not sign, because the signature covers the file's hash and the installer arguments.
use erg_proto::DeployRequest;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MAX_PACKAGE: u64 = 2 << 30;
pub const MAX_VALID_SECS: u64 = 24 * 3600;

pub fn sha256_file(p: &Path) -> std::io::Result<[u8; 32]> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let (mut h, mut buf) = (Sha256::new(), vec![0u8; 1 << 20]);
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

fn mac(key: &[u8; 32], name: &str, sha: &[u8; 32], args: &str, expires: u64) -> [u8; 32] {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac key");
    m.update(b"erg-deploy-v1\0");
    for part in [name.as_bytes(), sha.as_slice(), args.as_bytes(), &expires.to_le_bytes()] {
        m.update(&(part.len() as u32).to_le_bytes()); // length-prefixed so fields cannot slide into each other
        m.update(part);
    }
    m.finalize().into_bytes().into()
}

/// File names the service will run: a plain name ending in .msi or .exe.
pub fn plain_installer_name(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    (l.ends_with(".msi") || l.ends_with(".exe"))
        && name.len() <= 120
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || " ._-()".contains(c))
}

/// Silent-install switches: letters, digits and a few punctuation marks, no quoting or shell characters.
pub fn args_ok(a: &str) -> bool {
    a.len() <= 300 && a.chars().all(|c| c.is_ascii_alphanumeric() || " /-_=.:\\".contains(c))
}

pub fn sign(key: &[u8; 32], name: &str, sha: [u8; 32], args: &str, expires: u64) -> Result<DeployRequest, String> {
    if !plain_installer_name(name) {
        return Err("tên file phải là .msi hoặc .exe, chỉ chữ, số, khoảng trắng và . _ - ( )".into());
    }
    if !args_ok(args) {
        return Err("tham số cài đặt chứa ký tự không được phép".into());
    }
    Ok(DeployRequest { name: name.into(), sha256: sha, args: args.into(), expires, mac: mac(key, name, &sha, args, expires) })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    BadSignature,
    Expired,
    BadName,
    BadArgs,
    WrongFile,
}

/// `actual_sha` is the hash of the file the service itself copied; `now` is seconds since the epoch.
pub fn verify(key: &[u8; 32], r: &DeployRequest, actual_sha: &[u8; 32], now: u64) -> Result<(), Refused> {
    use subtle::ConstantTimeEq;
    if !plain_installer_name(&r.name) {
        return Err(Refused::BadName);
    }
    if !args_ok(&r.args) {
        return Err(Refused::BadArgs);
    }
    if !bool::from(mac(key, &r.name, &r.sha256, &r.args, r.expires).ct_eq(&r.mac)) {
        return Err(Refused::BadSignature);
    }
    if now > r.expires {
        return Err(Refused::Expired);
    }
    if !bool::from(actual_sha.ct_eq(&r.sha256)) {
        return Err(Refused::WrongFile);
    }
    Ok(())
}

/// What the service runs: program and arguments for a verified, already copied installer.
pub fn command_line(r: &DeployRequest, copy: &Path) -> (String, Vec<String>) {
    let extra = r.args.split_whitespace().map(str::to_string);
    if r.name.to_ascii_lowercase().ends_with(".msi") {
        let mut v = vec!["/i".into(), copy.to_string_lossy().into_owned(), "/qn".into(), "/norestart".into()];
        v.extend(extra);
        ("msiexec.exe".into(), v)
    } else {
        (copy.to_string_lossy().into_owned(), extra.collect())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Spooled {
    req: DeployRequest,
    /// Where the student runtime stored the received file.
    path: PathBuf,
}

/// Student side: leave a request for the service. Anyone may write here; only signed ones get run.
pub fn spool(dir: &Path, req: &DeployRequest, received: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let id: u64 = rand::random();
    let json = serde_json::to_vec(&Spooled { req: req.clone(), path: received.to_path_buf() }).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(format!("{id:016x}.req")), json)
}

/// Service side: handle every pending request once. `run` starts the installer and returns its exit code.
/// Returns one log line per request.
pub fn process(spool_dir: &Path, work_dir: &Path, key: &[u8; 32], now: u64, run: &dyn Fn(&str, &[String]) -> Result<i32, String>) -> Vec<String> {
    let mut log = vec![];
    let Ok(rd) = std::fs::read_dir(spool_dir) else { return log };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("req") {
            continue;
        }
        let data = std::fs::read(&p);
        let _ = std::fs::remove_file(&p); // a request is handled at most once, whatever happens
        let line = (|| -> Result<String, String> {
            let data = data.map_err(|e| e.to_string())?;
            if data.len() > 8192 {
                return Err("request too large".into());
            }
            let s: Spooled = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            if s.req.expires > now + MAX_VALID_SECS {
                return Err("validity too long".into());
            }
            // copy first: what is hashed and run is our private copy, not a file the student can still change
            std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
            let meta = std::fs::metadata(&s.path).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() > MAX_PACKAGE {
                return Err("package missing or too large".into());
            }
            if !plain_installer_name(&s.req.name) {
                return Err(format!("{:?}", Refused::BadName));
            }
            let copy = work_dir.join(format!("{:016x}-{}", rand::random::<u64>(), s.req.name));
            std::fs::copy(&s.path, &copy).map_err(|e| e.to_string())?;
            let result = (|| {
                let sha = sha256_file(&copy).map_err(|e| e.to_string())?;
                verify(key, &s.req, &sha, now).map_err(|r| format!("refused: {r:?}"))?;
                let (prog, args) = command_line(&s.req, &copy);
                let code = run(&prog, &args)?;
                Ok(format!("{}: exit code {code}", s.req.name))
            })();
            let _ = std::fs::remove_file(&copy);
            result
        })();
        log.push(line.unwrap_or_else(|e| format!("not run: {e}")));
    }
    log
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const KEY: [u8; 32] = [9; 32];

    fn setup(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("erg-deploy-{tag}"));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["spool", "work", "recv"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        (root.join("spool"), root.join("work"), root.join("recv"))
    }

    #[test]
    fn signature_covers_name_hash_args_and_expiry() {
        let sha = [1u8; 32];
        let r = sign(&KEY, "app.msi", sha, "ALLUSERS=1", 1000).unwrap();
        assert_eq!(verify(&KEY, &r, &sha, 999), Ok(()));
        assert_eq!(verify(&[8; 32], &r, &sha, 999), Err(Refused::BadSignature), "other key");
        assert_eq!(verify(&KEY, &r, &sha, 1001), Err(Refused::Expired));
        assert_eq!(verify(&KEY, &r, &[2; 32], 999), Err(Refused::WrongFile), "swapped file");
        for tampered in [
            DeployRequest { args: "ALLUSERS=2".into(), ..r.clone() },
            DeployRequest { name: "evil.msi".into(), ..r.clone() },
            DeployRequest { expires: 5000, ..r.clone() },
            DeployRequest { sha256: [2; 32], ..r.clone() },
        ] {
            assert_eq!(verify(&KEY, &tampered, &tampered.sha256, 999), Err(Refused::BadSignature));
        }
        assert!(sign(&KEY, "..\\evil.exe", sha, "", 1).is_err());
        assert!(sign(&KEY, "a.bat", sha, "", 1).is_err());
        assert!(sign(&KEY, "a.exe", sha, "/S & calc", 1).is_err());
    }

    #[test]
    fn command_lines() {
        let r = sign(&KEY, "x.msi", [0; 32], "A=1 B=2", 1).unwrap();
        let (p, a) = command_line(&r, Path::new("C:\\w\\x.msi"));
        assert_eq!((p.as_str(), a.join(" ")), ("msiexec.exe", "/i C:\\w\\x.msi /qn /norestart A=1 B=2".to_string()));
        let r = sign(&KEY, "setup.exe", [0; 32], "/S", 1).unwrap();
        assert_eq!(command_line(&r, Path::new("C:\\w\\setup.exe")).1, ["/S"]);
    }

    #[test]
    fn the_service_runs_only_what_the_tutor_signed() {
        let (spool_d, work, recv) = setup("process");
        let good = recv.join("setup.exe");
        std::fs::write(&good, b"genuine installer").unwrap();
        let sha = sha256_file(&good).unwrap();
        let req = sign(&KEY, "setup.exe", sha, "/S", 2000).unwrap();
        let ran: RefCell<Vec<String>> = RefCell::new(vec![]);
        let run = |p: &str, a: &[String]| -> Result<i32, String> {
            // the installer the service runs is its own copy with the original content
            let content = std::fs::read(p).map_err(|e| e.to_string())?;
            ran.borrow_mut().push(format!("{}|{}", String::from_utf8_lossy(&content), a.join(" ")));
            Ok(0)
        };

        // genuine request
        spool(&spool_d, &req, &good).unwrap();
        let log = process(&spool_d, &work, &KEY, 1000, &run);
        assert_eq!(log, ["setup.exe: exit code 0"]);
        assert_eq!(ran.borrow().as_slice(), ["genuine installer|/S"]);
        assert!(std::fs::read_dir(&spool_d).unwrap().next().is_none(), "request consumed");
        assert!(std::fs::read_dir(&work).unwrap().next().is_none(), "private copy removed");

        // student swaps the file but reuses the signed request
        std::fs::write(&good, b"malware").unwrap();
        spool(&spool_d, &req, &good).unwrap();
        // student forges a request for their own file
        let evil = recv.join("evil.exe");
        std::fs::write(&evil, b"malware").unwrap();
        let forged = DeployRequest { name: "evil.exe".into(), sha256: sha256_file(&evil).unwrap(), args: "".into(), expires: 2000, mac: [0; 32] };
        spool(&spool_d, &forged, &evil).unwrap();
        // genuine but expired
        std::fs::write(&good, b"genuine installer").unwrap();
        spool(&spool_d, &req, &good).unwrap();
        let log = process(&spool_d, &work, &KEY, 3000, &run);
        assert_eq!(log.len(), 3);
        assert!(log.iter().all(|l| l.starts_with("not run: refused")), "{log:?}");
        assert_eq!(ran.borrow().len(), 1, "nothing else was started");

        // garbage and oversized requests are dropped
        std::fs::write(spool_d.join("junk.req"), b"not json").unwrap();
        std::fs::write(spool_d.join("big.req"), vec![b'{'; 9000]).unwrap();
        std::fs::write(spool_d.join("ignored.txt"), b"x").unwrap();
        assert_eq!(process(&spool_d, &work, &KEY, 1000, &run).len(), 2);
        assert!(spool_d.join("ignored.txt").exists());
    }

    #[test]
    fn a_validity_longer_than_a_day_is_refused() {
        let (spool_d, work, recv) = setup("long");
        let f = recv.join("a.exe");
        std::fs::write(&f, b"x").unwrap();
        let req = sign(&KEY, "a.exe", sha256_file(&f).unwrap(), "", 10 + MAX_VALID_SECS + 100).unwrap();
        spool(&spool_d, &req, &f).unwrap();
        let log = process(&spool_d, &work, &KEY, 10, &|_, _| Ok(0));
        assert_eq!(log, ["not run: validity too long"]);
    }
}
