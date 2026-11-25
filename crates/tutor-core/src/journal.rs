//! Session journal: one JSON line per event (student online/offline, help request, chat, poll answer),
//! so the teacher can review what happened in a lesson afterwards.
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Journal(Mutex<std::fs::File>);

impl Journal {
    /// Appends to `path`, creating it (and its folder) when missing.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        Ok(Self(Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(path)?)))
    }

    /// `detail` is untrusted student text; it is JSON-escaped and cut to 500 chars.
    pub fn log(&self, kind: &str, who: &str, detail: &str) {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let detail: String = detail.chars().take(500).collect();
        let line = format!("{{\"ts\":{ts},\"kind\":{},\"who\":{},\"detail\":{}}}\n", js(kind), js(who), js(&detail));
        if let Ok(mut f) = self.0.lock() {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_valid_one_per_event_and_hostile_text_cannot_break_out() {
        let p = std::env::temp_dir().join("erg-journal-test").join("j.jsonl");
        let _ = std::fs::remove_file(&p);
        let j = Journal::open(&p).unwrap();
        j.log("chat", "An", "x\",\"kind\":\"evil\n\u{1}\\");
        j.log("online", "Bình", "");
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2, "a newline in student text must not add a line");
        assert!(lines[0].contains(r#""detail":"x\",\"kind\":\"evil\n\u0001\\""#));
        assert!(lines[1].contains("\"who\":\"Bình\""));
        drop(j);
        let j2 = Journal::open(&p).unwrap(); // reopening appends
        j2.log("offline", "An", "");
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 3);
    }
}
