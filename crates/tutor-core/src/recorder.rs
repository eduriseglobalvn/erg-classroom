//! Replay: keep the screen pictures a tutor receives as numbered JPEGs per student, plus a
//! self-contained `replay.html` player (slider + play button) so a lesson can be reviewed later
//! in any browser.
//!
//! On Windows `finish` also writes one `<student>.mp4` per student (H.264, pictures played at `MP4_FPS`,
//! i.e. much faster than real time because pictures arrive about once per 3 s). The JPEG sequence stays
//! as the source and the browser player.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Recorder {
    root: PathBuf,
    counts: HashMap<String, u32>,
    /// Stop silently at this many pictures per student so a forgotten recording cannot fill the disk.
    max_per_student: u32,
}

impl Recorder {
    pub fn start(root: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(root)?;
        Ok(Self { root: root.to_path_buf(), counts: HashMap::new(), max_per_student: 20_000 })
    }

    /// `who` is a host name; anything but plain file-name characters is replaced.
    pub fn push(&mut self, who: &str, jpeg: &[u8]) -> std::io::Result<()> {
        if jpeg.len() < 4 || jpeg[..2] != [0xFF, 0xD8] || jpeg.len() > 4 << 20 {
            return Ok(()); // not a JPEG: ignore rather than store junk
        }
        let dir = self.root.join(clean(who));
        let n = self.counts.entry(clean(who)).or_insert(0);
        if *n >= self.max_per_student {
            return Ok(());
        }
        std::fs::create_dir_all(&dir)?;
        *n += 1;
        std::fs::write(dir.join(format!("frame-{:06}.jpg", n)), jpeg)
    }

    pub fn frames(&self, who: &str) -> u32 {
        self.counts.get(&clean(who)).copied().unwrap_or(0)
    }

    /// Write the player page and return its path.
    pub fn finish(self) -> std::io::Result<PathBuf> {
        let mut sets: Vec<(&String, u32)> = self.counts.iter().map(|(k, v)| (k, *v)).collect();
        sets.sort();
        let data: String = sets.iter().map(|(k, n)| format!("{{\"who\":\"{k}\",\"n\":{n}}}")).collect::<Vec<_>>().join(",");
        let html = format!(
            "<!doctype html><meta charset=utf-8><title>Replay</title><style>body{{font:14px 'Segoe UI',sans-serif;margin:1em}}img{{max-width:100%;border:1px solid #888}}</style>\
             <select id=w></select> <button id=p>▶</button> <input id=s type=range min=1 value=1 style=\"width:60%\"> <span id=l></span><br><img id=i>\
             <script>const D=[{data}];let c=D[0],f=1,t=null;const w=document.getElementById('w'),s=document.getElementById('s'),i=document.getElementById('i'),l=document.getElementById('l'),p=document.getElementById('p');\
             D.forEach((d,k)=>w.add(new Option(d.who,k)));function show(){{if(!c)return;s.max=c.n;s.value=f;i.src=c.who+'/frame-'+String(f).padStart(6,'0')+'.jpg';l.textContent=f+'/'+c.n}}\
             w.onchange=()=>{{c=D[w.value];f=1;show()}};s.oninput=()=>{{f=+s.value;show()}};\
             p.onclick=()=>{{if(t){{clearInterval(t);t=null;p.textContent='▶'}}else{{p.textContent='⏸';t=setInterval(()=>{{if(f<c.n){{f++;show()}}else{{clearInterval(t);t=null;p.textContent='▶'}}}},300)}}}};show();</script>"
        );
        #[cfg(windows)]
        for (who, n) in &sets {
            if let Err(e) = export_mp4(&self.root.join(who), *n, &self.root.join(format!("{who}.mp4"))) {
                tracing::warn!("mp4 for {who}: {e}");
            }
        }
        let path = self.root.join("replay.html");
        std::fs::write(&path, html)?;
        Ok(path)
    }
}

/// Playback rate of the exported video.
pub const MP4_FPS: u32 = 3;

/// JPEG -> BGRA at (w, h); nearest-neighbour when the picture has another size (a student changed resolution).
fn decode_bgra(jpeg: &[u8], w: usize, h: usize) -> Result<Vec<u8>, String> {
    let mut d = jpeg_decoder::Decoder::new(jpeg);
    let px = d.decode().map_err(|e| e.to_string())?;
    let info = d.info().ok_or("no jpeg info")?;
    let (sw, sh) = (info.width as usize, info.height as usize);
    let bpp = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => 3,
        jpeg_decoder::PixelFormat::L8 => 1,
        _ => return Err("unsupported jpeg pixel format".into()),
    };
    let mut out = vec![255u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let s = ((y * sh / h) * sw + x * sw / w) * bpp;
            let o = (y * w + x) * 4;
            let (r, g, b) = if bpp == 3 { (px[s], px[s + 1], px[s + 2]) } else { (px[s], px[s], px[s]) };
            out[o..o + 3].copy_from_slice(&[b, g, r]);
        }
    }
    Ok(out)
}

#[cfg(windows)]
fn export_mp4(dir: &Path, n: u32, out: &Path) -> Result<(), String> {
    let first = std::fs::read(dir.join("frame-000001.jpg")).map_err(|e| e.to_string())?;
    let mut d = jpeg_decoder::Decoder::new(first.as_slice());
    d.read_info().map_err(|e| e.to_string())?;
    let info = d.info().ok_or("no jpeg info")?;
    let (w, h) = ((info.width as usize) & !1, (info.height as usize) & !1);
    let mut m = erg_mp4::Mp4Writer::create(out, w as u32, h as u32, MP4_FPS, 1_500_000).map_err(|e| e.to_string())?;
    for i in 1..=n {
        // a damaged picture is skipped rather than ending the whole video
        if let Ok(bytes) = std::fs::read(dir.join(format!("frame-{i:06}.jpg"))) {
            if let Ok(f) = decode_bgra(&bytes, w, h) {
                m.push(&f).map_err(|e| e.to_string())?;
            }
        }
    }
    m.finish().map_err(|e| e.to_string())
}

fn clean(who: &str) -> String {
    let s: String = who.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(60).collect();
    if s.is_empty() { "student".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn finish_also_writes_an_mp4_per_student() {
        let root = std::env::temp_dir().join("erg-recorder-mp4-test");
        let _ = std::fs::remove_dir_all(&root);
        let (w, h) = (64u16, 48u16);
        let rgb: Vec<u8> = (0..w as usize * h as usize).flat_map(|i| [(i % 251) as u8, 90, 160]).collect();
        let mut jpg = vec![];
        jpeg_encoder::Encoder::new(&mut jpg, 80).encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb).unwrap();
        let mut r = Recorder::start(&root).unwrap();
        for _ in 0..6 {
            r.push("PC-01", &jpg).unwrap();
        }
        r.finish().unwrap();
        let mp4 = std::fs::read(root.join("PC-01.mp4")).expect("mp4 written");
        assert_eq!(&mp4[4..8], b"ftyp");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn frames_are_numbered_per_student_and_the_player_lists_them() {
        let root = std::env::temp_dir().join("erg-recorder-test");
        let _ = std::fs::remove_dir_all(&root);
        let mut r = Recorder::start(&root).unwrap();
        let jpg = [0xFF, 0xD8, 1, 2, 3];
        r.push("PC-01", &jpg).unwrap();
        r.push("PC-01", &jpg).unwrap();
        r.push("PC 02/../x", &jpg).unwrap(); // hostile name cannot leave the folder
        r.push("PC-01", b"not a jpeg").unwrap();
        assert_eq!((r.frames("PC-01"), r.frames("PC 02/../x")), (2, 1));
        assert!(root.join("PC-01").join("frame-000002.jpg").exists());
        assert!(root.join("PC_02____x").join("frame-000001.jpg").exists());
        let page = r.finish().unwrap();
        let html = std::fs::read_to_string(page).unwrap();
        assert!(html.contains("{\"who\":\"PC-01\",\"n\":2}") && html.contains("PC_02____x"));
        assert!(!root.join("x").exists() && !root.join("PC 02").exists(), "the ../ in a host name must not be followed");
    }

    #[test]
    fn a_forgotten_recording_stops_at_the_cap() {
        let root = std::env::temp_dir().join("erg-recorder-cap");
        let _ = std::fs::remove_dir_all(&root);
        let mut r = Recorder::start(&root).unwrap();
        r.max_per_student = 3;
        for _ in 0..10 {
            r.push("a", &[0xFF, 0xD8, 0, 0]).unwrap();
        }
        assert_eq!(r.frames("a"), 3);
    }
}
