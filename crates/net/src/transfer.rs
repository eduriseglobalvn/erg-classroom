//! File transfer over one QUIC unidirectional stream per file: a framed [`FileHeader`], then the
//! raw bytes. The receiver treats the sender as untrusted: names are sanitised, size is capped,
//! the SHA-256 must match, and nothing is ever overwritten.
use crate::Error;
use erg_proto::{decode, encode, FileHeader, MAX_FILE_SIZE};
use quinn::{Connection, RecvStream};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHUNK: usize = 64 * 1024;

/// Windows device names that must never be used as file names (with or without extension).
fn is_reserved(stem: &str) -> bool {
    let s = stem.to_ascii_uppercase();
    matches!(s.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((s.starts_with("COM") || s.starts_with("LPT")) && s.len() == 4 && s.as_bytes()[3].is_ascii_digit() && s.as_bytes()[3] != b'0')
}

/// A plain file name or nothing: no directories, drives, devices or characters Windows rejects.
pub fn safe_name(name: &str) -> Option<String> {
    let bad = |c: char| c.is_control() || "\\/:*?\"<>|".contains(c);
    let ok = !name.is_empty()
        && name.chars().count() <= 200
        && !name.chars().any(bad)
        && name != "."
        && name != ".."
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !is_reserved(name.split('.').next().unwrap_or(""));
    ok.then(|| name.to_string())
}

/// Send `path` as `name` on a fresh stream.
pub async fn send_file(conn: &Connection, path: &Path, name: &str) -> Result<(), Error> {
    let mut f = tokio::fs::File::open(path).await?;
    let size = f.metadata().await?.len();
    if size > MAX_FILE_SIZE {
        return Err(Error::Connect("file too large".into()));
    }
    // Hash first (one extra read) so the header can carry the digest the receiver verifies.
    let mut h = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let header = FileHeader { name: name.to_string(), size, sha256: h.finalize().into() };
    let mut tx = conn.open_uni().await.map_err(|_| Error::Closed)?;
    tx.write_all(&encode(&header)?).await.map_err(|_| Error::Closed)?;
    let mut f = tokio::fs::File::open(path).await?;
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        tx.write_all(&buf[..n]).await.map_err(|_| Error::Closed)?;
    }
    tx.finish().map_err(|_| Error::Closed)?;
    // Wait until the receiver has read everything (or gave up) so the caller knows delivery ended.
    let _ = tx.stopped().await;
    Ok(())
}

/// Receive one file into `inbox`. Returns the final path. On any problem the partial file is removed.
pub async fn receive_file(mut rx: RecvStream, inbox: &Path) -> Result<PathBuf, Error> {
    let mut buf = Vec::new();
    let (header, used) = loop {
        if let Some((h, n)) = decode::<FileHeader>(&buf)? {
            break (h, n);
        }
        let mut chunk = [0u8; 4096];
        match rx.read(&mut chunk).await.map_err(|_| Error::Closed)? {
            Some(n) => buf.extend_from_slice(&chunk[..n]),
            None => return Err(Error::Closed),
        }
    };
    let name = safe_name(&header.name).ok_or(Error::AuthFailed)?;
    if header.size > MAX_FILE_SIZE {
        return Err(Error::AuthFailed);
    }
    tokio::fs::create_dir_all(inbox).await?;
    let (part, final_path) = unique_paths(inbox, &name).await?;
    let result = write_body(&mut rx, &part, &header, &buf[used..]).await;
    match result {
        Ok(()) => {
            tokio::fs::rename(&part, &final_path).await?;
            Ok(final_path)
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(e)
        }
    }
}

async fn write_body(rx: &mut RecvStream, part: &Path, h: &FileHeader, first: &[u8]) -> Result<(), Error> {
    let mut f = tokio::fs::OpenOptions::new().write(true).create_new(true).open(part).await?;
    let mut hash = Sha256::new();
    let mut got = 0u64;
    let mut take = |data: &[u8], got: &mut u64| -> Result<(), Error> {
        *got += data.len() as u64;
        if *got > h.size {
            return Err(Error::AuthFailed); // more bytes than announced
        }
        hash.update(data);
        Ok(())
    };
    if !first.is_empty() {
        take(first, &mut got)?;
        f.write_all(first).await?;
    }
    let mut chunk = vec![0u8; CHUNK];
    while got < h.size {
        let n = match tokio::time::timeout(std::time::Duration::from_secs(30), rx.read(&mut chunk)).await {
            Ok(r) => r.map_err(|_| Error::Closed)?,
            Err(_) => return Err(Error::Closed), // stalled sender
        };
        let Some(n) = n else { return Err(Error::Closed) };
        take(&chunk[..n], &mut got)?;
        f.write_all(&chunk[..n]).await?;
    }
    f.flush().await?;
    let digest: [u8; 32] = hash.finalize().into();
    if digest != h.sha256 {
        return Err(Error::AuthFailed);
    }
    Ok(())
}

/// `name`, `name (1).ext`, ... never an existing file. The `.part` name is reserved atomically
/// by `create_new` when the body is written.
async fn unique_paths(inbox: &Path, name: &str) -> Result<(PathBuf, PathBuf), Error> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 0..1000 {
        let candidate = if n == 0 { name.to_string() } else { format!("{stem} ({n}){ext}") };
        let fin = inbox.join(&candidate);
        let part = inbox.join(format!("{candidate}.part"));
        if !tokio::fs::try_exists(&fin).await? && !tokio::fs::try_exists(&part).await? {
            return Ok((part, fin));
        }
    }
    Err(Error::Io(std::io::Error::other("too many files with the same name")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{new_identity, student_endpoint, tutor_endpoint};

    #[test]
    fn hostile_names_are_rejected() {
        for bad in ["", ".", "..", "..\\evil.exe", "../evil", "a/b.txt", "C:\\x.txt", "C:x", "CON", "con.txt", "NUL.", "LPT1.log", "a.", "a ", "x\0y", "a*b", "a?b", "<a>", &"x".repeat(201)] {
            assert!(safe_name(bad).is_none(), "{bad:?} must be rejected");
        }
        for good in ["bai1.xlsx", "Bài tập số 1.docx", "a.b.c", "COM10.txt", "CONSOLE.txt", ".gitignore"] {
            assert_eq!(safe_name(good).as_deref(), Some(good));
        }
    }

    /// A connected pair; keeps endpoints alive for the test's duration.
    async fn pair() -> (Connection, Connection, Vec<quinn::Endpoint>) {
        let id = new_identity("erg-student").unwrap();
        let sep = student_endpoint(&id, "127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = sep.local_addr().unwrap();
        let s = sep.clone();
        let acc = tokio::spawn(async move { s.accept().await.unwrap().await.unwrap() });
        let tep = tutor_endpoint().unwrap();
        let tc = tep.connect(addr, "erg-student").unwrap().await.unwrap();
        (tc, acc.await.unwrap(), vec![sep, tep])
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("erg-transfer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn file_arrives_intact_and_is_never_overwritten() {
        let (tutor, student, _keep) = pair().await;
        let dir = tmp("ok");
        let src = dir.join("src.bin");
        let data: Vec<u8> = (0..5_000_000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
        std::fs::write(&src, &data).unwrap();
        let inbox = dir.join("inbox");

        for expected in ["bai1.bin", "bai1 (1).bin"] {
            let (name, inbox2) = ("bai1.bin", inbox.clone());
            let rx_task = tokio::spawn({
                let student = student.clone();
                async move { receive_file(student.accept_uni().await.unwrap(), &inbox2).await }
            });
            send_file(&tutor, &src, name).await.unwrap();
            let got = rx_task.await.unwrap().unwrap();
            assert_eq!(got.file_name().unwrap(), expected);
            assert_eq!(std::fs::read(&got).unwrap(), data);
        }
        assert!(std::fs::read_dir(&inbox).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".part")));
    }

    #[tokio::test]
    async fn hostile_name_and_corrupt_data_are_refused_and_leave_nothing_behind() {
        let (tutor, student, _keep) = pair().await;
        let dir = tmp("bad");
        let inbox = dir.join("inbox");

        // 1. Path traversal in the name.
        let src = dir.join("x.txt");
        std::fs::write(&src, b"hello").unwrap();
        let (i2, s2) = (inbox.clone(), student.clone());
        let r = tokio::spawn(async move { receive_file(s2.accept_uni().await.unwrap(), &i2).await });
        let _ = send_file(&tutor, &src, "..\\..\\evil.txt").await; // receiver may stop the stream early
        assert!(r.await.unwrap().is_err());

        // 2. Correct name but the bytes do not match the announced hash.
        let header = FileHeader { name: "ok.txt".into(), size: 5, sha256: [0; 32] };
        let (i2, s2) = (inbox.clone(), student.clone());
        let r = tokio::spawn(async move { receive_file(s2.accept_uni().await.unwrap(), &i2).await });
        let mut tx = tutor.open_uni().await.unwrap();
        tx.write_all(&encode(&header).unwrap()).await.unwrap();
        tx.write_all(b"hello").await.unwrap();
        tx.finish().unwrap();
        assert!(r.await.unwrap().is_err());

        // 3. More bytes than announced.
        let header = FileHeader { name: "big.txt".into(), size: 2, sha256: [0; 32] };
        let (i2, s2) = (inbox.clone(), student.clone());
        let r = tokio::spawn(async move { receive_file(s2.accept_uni().await.unwrap(), &i2).await });
        let mut tx = tutor.open_uni().await.unwrap();
        tx.write_all(&encode(&header).unwrap()).await.unwrap();
        tx.write_all(b"too long").await.unwrap();
        tx.finish().unwrap();
        assert!(r.await.unwrap().is_err());

        let left: Vec<_> = std::fs::read_dir(&inbox).map(|d| d.map(|e| e.unwrap().file_name()).collect()).unwrap_or_default();
        assert!(left.is_empty(), "files left behind: {left:?}");
        assert!(!dir.join("evil.txt").exists() && !dir.parent().unwrap().join("evil.txt").exists());
    }
}
