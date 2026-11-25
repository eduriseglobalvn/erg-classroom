//! Lesson journal as a self-contained HTML page (print it to PDF from the browser/WebView).
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct Entry {
    pub ts: u64,
    pub kind: String,
    pub who: String,
    pub detail: String,
}

/// Parse journal.jsonl; damaged lines are skipped, never fatal.
pub fn parse(jsonl: &str) -> Vec<Entry> {
    jsonl.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// The newest `max_lines` entries of the journal at `path`. Reads at most the last 4 MiB (a runaway
/// file cannot exhaust memory); the cut first line and damaged lines are skipped; missing file = empty.
pub fn read_tail(path: &std::path::Path, max_lines: usize) -> Vec<Entry> {
    use std::io::{Read, Seek, SeekFrom};
    const CAP: u64 = 4 << 20;
    let Ok(mut f) = std::fs::File::open(path) else { return vec![] };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(CAP);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_err() || f.take(CAP).read_to_end(&mut buf).is_err() {
        return vec![];
    }
    let text = String::from_utf8_lossy(&buf);
    let body = if start > 0 { text.split_once('\n').map_or("", |x| x.1) } else { &text };
    let mut v = parse(body);
    v.drain(..v.len().saturating_sub(max_lines));
    v
}

fn esc(s: &str) -> String {
    s.chars().fold(String::new(), |mut o, c| {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
        o
    })
}

fn clock(ms: u64) -> String {
    let s = ms / 1000 % 86_400; // UTC time of day; the page says so
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// `title` and every journal field are escaped: student text must not inject markup.
pub fn html(title: &str, objectives: &[String], entries: &[Entry]) -> String {
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for e in entries {
        *counts.entry(e.kind.as_str()).or_default() += 1;
    }
    let mut o = format!(
        "<!doctype html><meta charset=utf-8><title>{t}</title><style>body{{font:14px 'Segoe UI',sans-serif;margin:2em}}\
         table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:4px 8px;text-align:left}}</style>\
         <h1>{t}</h1>",
        t = esc(title)
    );
    if !objectives.is_empty() {
        o.push_str("<h2>Mục tiêu</h2><ul>");
        for g in objectives {
            o.push_str(&format!("<li>{}</li>", esc(g)));
        }
        o.push_str("</ul>");
    }
    o.push_str("<h2>Tổng kết</h2><ul>");
    for (k, n) in &counts {
        o.push_str(&format!("<li>{}: {n}</li>", esc(k)));
    }
    o.push_str("</ul><h2>Diễn biến (giờ UTC)</h2><table><tr><th>Giờ<th>Loại<th>Ai<th>Chi tiết</tr>");
    for e in entries {
        o.push_str(&format!("<tr><td>{}<td>{}<td>{}<td>{}</tr>", clock(e.ts), esc(&e.kind), esc(&e.who), esc(&e.detail)));
    }
    o.push_str("</table>");
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tail_caps_lines_skips_garbage_and_survives_missing_or_huge_files() {
        let d = std::env::temp_dir().join("erg-readtail-test");
        std::fs::create_dir_all(&d).unwrap();
        assert!(read_tail(&d.join("nope.jsonl"), 10).is_empty());
        let p = d.join("j.jsonl");
        let mut s = String::new();
        for i in 0..5 {
            s += &format!("{{\"ts\":{i},\"kind\":\"chat\",\"who\":\"A\",\"detail\":\"x\"}}\n");
            if i == 2 { s += "garbage\n{\"ts\":\n"; }
        }
        std::fs::write(&p, &s).unwrap();
        let e = read_tail(&p, 3);
        assert_eq!(e.iter().map(|x| x.ts).collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(read_tail(&p, 99).len(), 5);
        // >4 MiB: only the tail is read and the half-cut first line is dropped
        let big = format!("{}{s}", "{\"ts\":9,\"kind\":\"k\",\"who\":\"w\",\"detail\":\"".to_string() + &"y".repeat(5 << 20) + "\"}\n");
        std::fs::write(&p, big).unwrap();
        assert_eq!(read_tail(&p, 99).len(), 5);
    }

    #[test]
    fn report_escapes_student_text_and_skips_bad_lines() {
        let log = "{\"ts\":3661000,\"kind\":\"chat\",\"who\":\"PC-1\",\"detail\":\"<script>alert(1)</script>\"}\nnot json\n\
                   {\"ts\":1,\"kind\":\"help\",\"who\":\"PC-2\",\"detail\":\"a&b\"}\n";
        let e = parse(log);
        assert_eq!(e.len(), 2);
        let h = html("Bài <1>", &["Mục tiêu \"x\"".into()], &e);
        assert!(!h.contains("<script>"));
        assert!(h.contains("&lt;script&gt;") && h.contains("a&amp;b") && h.contains("Bài &lt;1&gt;"));
        assert!(h.contains("01:01:01"));
        assert!(h.contains("<li>chat: 1</li>"));
    }
}
