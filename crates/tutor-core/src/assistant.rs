//! Tutor Assistant: a tiny web page for the teacher's phone (add it to the home screen).
//! It lists the students and offers Lock / Unlock / Message for the whole class, nothing else.
//! The secret token is part of the URL and is shown only to the teacher; plain HTTP on the classroom
//! LAN is the accepted limit (shortcut: no TLS, so use it on a trusted Wi-Fi only).
use erg_proto::Command;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub trait Api: Send + Sync {
    /// (host name, user) of every online student.
    fn students(&self) -> Vec<(String, String)>;
    /// Send to the whole class; returns how many machines got it.
    fn send(&self, cmd: Command) -> usize;
}

impl Api for crate::Roster {
    fn students(&self) -> Vec<(String, String)> {
        self.online_ids().into_iter().filter_map(|id| self.beacon(id)).map(|b| (b.hostname, b.user)).collect()
    }
    fn send(&self, cmd: Command) -> usize {
        crate::Roster::send(self, None, cmd)
    }
}

pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

fn reply(status: u16, content_type: &'static str, body: impl Into<String>) -> Reply {
    Reply { status, content_type, body: body.into() }
}

/// Comparison that does not stop at the first differing byte.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

const PAGE: &str = r##"<!doctype html><html lang=vi><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<link rel=manifest href=manifest.json><meta name=theme-color content="#0f6cbd"><title>ERG Trợ lý</title>
<style>body{font:16px system-ui,sans-serif;margin:0;padding:12px}button{font:inherit;padding:14px;margin:4px 0;width:100%;border:0;border-radius:8px;background:#0f6cbd;color:#fff}
button.g{background:#555}li{padding:4px 0}#m{width:100%;box-sizing:border-box;padding:10px;font:inherit}</style>
<h2>Lớp học: <span id=n>…</span> máy</h2>
<button onclick="c('lock')">🔒 Khoá cả lớp</button><button class=g onclick="c('unlock')">🔓 Mở khoá</button>
<input id=m placeholder="Tin nhắn cho cả lớp"><button onclick="c('message',m.value)">✉ Gửi tin nhắn</button>
<p id=r></p><ul id=l></ul>
<script>
async function load(){try{const s=await (await fetch('students')).json();n.textContent=s.length;l.innerHTML=s.map(x=>'<li>'+x.replace(/[<>&]/g,'')+'</li>').join('')}catch(e){}}
async function c(cmd,text){const r=await fetch('cmd',{method:'POST',body:JSON.stringify({cmd,text:text||''})});const j=await r.json();document.getElementById('r').textContent=r.ok?'Đã gửi tới '+j.sent+' máy':'Lỗi: '+j.error}
load();setInterval(load,5000)
</script></html>"##;

const MANIFEST: &str = r##"{"name":"ERG Trợ lý giáo viên","short_name":"ERG","start_url":".","display":"standalone","background_color":"#ffffff","theme_color":"#0f6cbd","icons":[]}"##;

/// `target` is the request path, `body` the (already length-limited) request body.
pub fn handle(method: &str, target: &str, body: &str, token: &str, api: &dyn Api) -> Reply {
    let path = target.split('?').next().unwrap_or("");
    let Some(rest) = path.strip_prefix("/t/") else { return reply(404, "text/plain", "not found") };
    let (given, tail) = rest.split_once('/').unwrap_or((rest, ""));
    if !same(given, token) {
        return reply(404, "text/plain", "not found"); // same answer as an unknown page: no hint that a token exists
    }
    match (method, tail) {
        ("GET", "") => reply(200, "text/html; charset=utf-8", PAGE),
        ("GET", "manifest.json") => reply(200, "application/manifest+json", MANIFEST),
        ("GET", "students") => {
            let names: Vec<String> = api.students().into_iter().map(|(h, u)| if u.is_empty() { h } else { format!("{h} · {u}") }).collect();
            reply(200, "application/json", serde_json::to_string(&names).unwrap_or_else(|_| "[]".into()))
        }
        ("POST", "cmd") => {
            #[derive(serde::Deserialize)]
            struct Req {
                cmd: String,
                #[serde(default)]
                text: String,
            }
            let Ok(r) = serde_json::from_str::<Req>(body) else { return reply(400, "application/json", r#"{"error":"bad request"}"#) };
            let cmd = match r.cmd.as_str() {
                "lock" => Command::Lock { blank: false, text: Some("Giáo viên đã khoá máy".into()) },
                "unlock" => Command::Unlock,
                "message" if !r.text.trim().is_empty() => Command::Message { text: r.text.chars().take(500).collect(), seconds: 15 },
                _ => return reply(400, "application/json", r#"{"error":"unknown command"}"#),
            };
            reply(200, "application/json", format!(r#"{{"sent":{}}}"#, api.send(cmd)))
        }
        _ => reply(404, "text/plain", "not found"),
    }
}

async fn serve_one(mut s: tokio::net::TcpStream, token: String, api: std::sync::Arc<dyn Api>) {
    let work = async {
        let mut buf = Vec::with_capacity(1024);
        let mut tmp = [0u8; 1024];
        let head_end = loop {
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break p + 4;
            }
            if buf.len() > 8192 {
                return None;
            }
            let n = s.read(&mut tmp).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&tmp[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = head.lines();
        let mut first = lines.next()?.split(' ');
        let (method, target) = (first.next()?.to_string(), first.next()?.to_string());
        let len: usize = lines
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.trim().parse().ok())
            .unwrap_or(0);
        if len > 2048 {
            return Some(reply(413, "text/plain", "too large"));
        }
        while buf.len() < head_end + len {
            let n = s.read(&mut tmp).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        let body = String::from_utf8_lossy(&buf[head_end..head_end + len]).into_owned();
        Some(handle(&method, &target, &body, &token, api.as_ref()))
    };
    // a slow or silent client must not hold a task forever
    let Ok(Some(r)) = tokio::time::timeout(std::time::Duration::from_secs(5), work).await else { return };
    let text = match r.status {
        200 => "OK",
        400 => "Bad Request",
        413 => "Payload Too Large",
        _ => "Not Found",
    };
    let out = format!(
        "HTTP/1.1 {} {text}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        r.status,
        r.content_type,
        r.body.len(),
        r.body
    );
    let _ = s.write_all(out.as_bytes()).await;
}

/// Listen on `addr`; the returned task handle stops the server when aborted.
pub async fn serve(addr: SocketAddr, token: String, api: std::sync::Arc<dyn Api>) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let l = TcpListener::bind(addr).await?;
    let local = l.local_addr()?;
    let task = tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { break };
            tokio::spawn(serve_one(s, token.clone(), api.clone()));
        }
    });
    Ok((local, task))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fake(Mutex<Vec<Command>>);
    impl Api for Fake {
        fn students(&self) -> Vec<(String, String)> {
            vec![("PC-01".into(), "an".into()), ("PC-02".into(), String::new())]
        }
        fn send(&self, c: Command) -> usize {
            self.0.lock().unwrap().push(c);
            2
        }
    }

    #[test]
    fn only_the_right_token_gets_anything() {
        let f = Fake::default();
        for bad in ["/", "/t/", "/t/wrong/", "/t/wrong/students", "/t/secre/students", "/t/secret2/students"] {
            assert_eq!(handle("GET", bad, "", "secret", &f).status, 404, "{bad}");
        }
        assert_eq!(handle("POST", "/t/wrong/cmd", r#"{"cmd":"lock"}"#, "secret", &f).status, 404);
        assert!(f.0.lock().unwrap().is_empty(), "nothing was sent");
        assert_eq!(handle("GET", "/t/secret/", "", "secret", &f).status, 200);
        assert_eq!(handle("GET", "/t/secret/students", "", "secret", &f).body, r#"["PC-01 · an","PC-02"]"#);
    }

    #[test]
    fn only_three_commands_exist_and_a_message_needs_text() {
        let f = Fake::default();
        let post = |b: &str| handle("POST", "/t/s/cmd", b, "s", &f);
        assert_eq!(post(r#"{"cmd":"lock"}"#).body, r#"{"sent":2}"#);
        assert_eq!(post(r#"{"cmd":"unlock"}"#).status, 200);
        assert_eq!(post(r#"{"cmd":"message","text":"hi"}"#).status, 200);
        for bad in [r#"{"cmd":"message","text":"  "}"#, r#"{"cmd":"shutdown"}"#, r#"{"cmd":"Shell"}"#, "nonsense", ""] {
            assert_eq!(post(bad).status, 400, "{bad}");
        }
        assert_eq!(f.0.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn over_a_real_socket() {
        let api = Arc::new(Fake::default());
        let (addr, task) = serve("127.0.0.1:0".parse().unwrap(), "tok".into(), api.clone()).await.unwrap();
        let ask = |req: String| async move {
            let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
            s.write_all(req.as_bytes()).await.unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            out
        };
        let r = ask("GET /t/tok/students HTTP/1.1\r\nHost: x\r\n\r\n".into()).await;
        assert!(r.starts_with("HTTP/1.1 200") && r.contains("PC-01"), "{r}");
        let body = r#"{"cmd":"lock"}"#;
        let r = ask(format!("POST /t/tok/cmd HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", body.len())).await;
        assert!(r.contains(r#"{"sent":2}"#), "{r}");
        assert!(ask("GET /t/nope/students HTTP/1.1\r\n\r\n".into()).await.starts_with("HTTP/1.1 404"));
        assert!(ask("POST /t/tok/cmd HTTP/1.1\r\nContent-Length: 99999\r\n\r\n".into()).await.starts_with("HTTP/1.1 413"));
        task.abort();
    }
}
