//! Lesson profile (Class Wizard), seating layout and groups, attendance, Wake-on-LAN and the
//! lesson planner timeline. All pure data + small helpers, so everything here is unit-tested.
use erg_proto::Command;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

fn save_json<T: Serialize>(v: &T, path: &Path) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    // Write beside the target then rename, so a crash never leaves a half-written file.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).map_err(std::io::Error::other)?)?;
    std::fs::rename(tmp, path)
}

fn load_json<T: for<'a> Deserialize<'a>>(path: &Path) -> std::io::Result<T> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(std::io::Error::other)
}

// ---------------------------------------------------------------- lesson profile

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassProfile {
    pub teacher: String,
    pub title: String,
    pub objectives: Vec<String>,
    /// 0 = no timer.
    pub minutes: u32,
    pub end_class_when_timer_stops: bool,
}

impl ClassProfile {
    pub fn save(&self, p: &Path) -> std::io::Result<()> { save_json(self, p) }
    pub fn load(p: &Path) -> std::io::Result<Self> { load_json(p) }

    pub fn seconds_left(&self, elapsed_s: u64) -> Option<u32> {
        (self.minutes > 0).then(|| (self.minutes as u64 * 60).saturating_sub(elapsed_s) as u32)
    }

    /// Toolbar command for the students.
    pub fn lesson_command(&self, elapsed_s: u64) -> Command {
        Command::SetLesson { title: self.title.clone(), objectives: self.objectives.clone(), seconds_left: self.seconds_left(elapsed_s).unwrap_or(0) }
    }
}

// ---------------------------------------------------------------- layout and groups

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// host name -> (column, row)
    pub seats: BTreeMap<String, (u16, u16)>,
    /// group name -> host names
    pub groups: BTreeMap<String, Vec<String>>,
}

impl Layout {
    pub fn save(&self, p: &Path) -> std::io::Result<()> { save_json(self, p) }
    pub fn load(p: &Path) -> std::io::Result<Self> { load_json(p) }

    /// Place hosts in reading order on a grid `cols` wide; keeps seats already assigned.
    pub fn arrange(&mut self, hosts: &[String], cols: u16) {
        let cols = cols.max(1);
        let taken: std::collections::HashSet<(u16, u16)> = self.seats.values().copied().collect();
        let mut free = (0u16..).flat_map(|r| (0..cols).map(move |c| (c, r))).filter(|s| !taken.contains(s));
        for h in hosts {
            if !self.seats.contains_key(h) {
                self.seats.insert(h.clone(), free.next().expect("infinite iterator"));
            }
        }
    }

    /// Split into `n` balanced groups named "Nhóm 1".. in host order; replaces existing groups.
    pub fn auto_group(&mut self, hosts: &[String], n: usize) {
        self.groups.clear();
        let n = n.clamp(1, hosts.len().max(1));
        for (i, h) in hosts.iter().enumerate() {
            self.groups.entry(format!("Nhóm {}", i % n + 1)).or_default().push(h.clone());
        }
    }

    pub fn group_of(&self, host: &str) -> Option<&str> {
        self.groups.iter().find(|(_, m)| m.iter().any(|x| x == host)).map(|(g, _)| g.as_str())
    }

    /// A host belongs to one group at a time.
    pub fn assign(&mut self, host: &str, group: &str) {
        for m in self.groups.values_mut() {
            m.retain(|x| x != host);
        }
        self.groups.retain(|_, m| !m.is_empty());
        self.groups.entry(group.to_string()).or_default().push(host.to_string());
    }
}

// ---------------------------------------------------------------- attendance

#[derive(Debug, Default)]
pub struct Attendance(BTreeMap<String, (String, String, u64)>);

impl Attendance {
    /// Last entry per host wins. Name and class are cut to 80 chars.
    pub fn record(&mut self, host: &str, name: &str, class: &str, ts_ms: u64) {
        let cut = |s: &str| s.trim().chars().take(80).collect::<String>();
        self.0.insert(host.to_string(), (cut(name), cut(class), ts_ms));
    }
    pub fn present(&self) -> usize { self.0.len() }
    /// Hosts that are online but did not register.
    pub fn missing<'a>(&self, online: &'a [String]) -> Vec<&'a String> {
        online.iter().filter(|h| !self.0.contains_key(*h)).collect()
    }
    pub fn csv(&self) -> String {
        let cell = |s: &str| {
            let s = if s.starts_with(['=', '+', '-', '@']) { format!("'{s}") } else { s.to_string() };
            format!("\"{}\"", s.replace('"', "\"\""))
        };
        let mut o = String::from("Host,Name,Class,Time(ms)\n");
        for (h, (n, c, t)) in &self.0 {
            o.push_str(&format!("{},{},{},{t}\n", cell(h), cell(n), cell(c)));
        }
        o
    }
}

// ---------------------------------------------------------------- Wake-on-LAN

pub fn magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut p = [0xFFu8; 102];
    for i in 0..16 {
        p[6 + i * 6..12 + i * 6].copy_from_slice(&mac);
    }
    p
}

/// Broadcast a magic packet on UDP port 9. `broadcast` is normally 255.255.255.255.
pub fn wake(mac: [u8; 6], broadcast: std::net::Ipv4Addr) -> std::io::Result<()> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0")?;
    s.set_broadcast(true)?;
    s.send_to(&magic_packet(mac), (broadcast, 9))?;
    Ok(())
}

// ---------------------------------------------------------------- lesson planner

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Seconds after the lesson started.
    pub at_s: u32,
    pub label: String,
    pub command: Command,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub steps: Vec<Step>,
}

impl Plan {
    pub fn save(&self, p: &Path) -> std::io::Result<()> { save_json(self, p) }
    pub fn load(p: &Path) -> std::io::Result<Self> { load_json(p) }

    /// Steps whose time fell in (`from_s`, `to_s`], in time order. Call with the previous and the
    /// current elapsed time every tick; each step fires exactly once as long as ticks do not overlap.
    pub fn due(&self, from_s: u32, to_s: u32) -> Vec<&Step> {
        let mut v: Vec<&Step> = self.steps.iter().filter(|s| s.at_s > from_s && s.at_s <= to_s).collect();
        v.sort_by_key(|s| s.at_s);
        v
    }

    /// Steps at second 0 would never be "after" anything, so the first tick uses this.
    pub fn at_start(&self) -> Vec<&Step> { self.steps.iter().filter(|s| s.at_s == 0).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(n: usize) -> Vec<String> { (1..=n).map(|i| format!("PC-{i:02}")).collect() }

    #[test]
    fn profile_roundtrips_through_a_file_and_counts_down() {
        let p = ClassProfile { teacher: "Lan".into(), title: "Excel".into(), objectives: vec!["SUM".into()], minutes: 45, end_class_when_timer_stops: true };
        let f = std::env::temp_dir().join("erg-prof-test").join("p.json");
        p.save(&f).unwrap();
        assert_eq!(ClassProfile::load(&f).unwrap(), p);
        assert_eq!(p.seconds_left(60), Some(2640));
        assert_eq!(p.seconds_left(99_999), Some(0));
        assert_eq!(ClassProfile { minutes: 0, ..p.clone() }.seconds_left(5), None);
        assert!(matches!(p.lesson_command(0), Command::SetLesson { seconds_left: 2700, .. }));
        assert!(ClassProfile::load(&f.with_file_name("nope.json")).is_err());
    }

    #[test]
    fn arrange_fills_a_grid_without_moving_existing_seats() {
        let mut l = Layout::default();
        l.seats.insert("PC-02".into(), (1, 0));
        l.arrange(&hosts(5), 3);
        assert_eq!(l.seats["PC-02"], (1, 0));
        let mut all: Vec<_> = l.seats.values().copied().collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 5, "no two students share a seat");
        assert_eq!(l.seats["PC-01"], (0, 0));
        assert_eq!(l.seats["PC-03"], (2, 0));
        l.arrange(&hosts(1), 0); // zero columns must not hang or panic
    }

    #[test]
    fn groups_are_balanced_and_exclusive() {
        let mut l = Layout::default();
        l.auto_group(&hosts(10), 3);
        let sizes: Vec<usize> = l.groups.values().map(|g| g.len()).collect();
        assert_eq!(sizes, vec![4, 3, 3]);
        l.assign("PC-01", "Đội đỏ");
        assert_eq!(l.group_of("PC-01"), Some("Đội đỏ"));
        assert_eq!(l.groups.values().flatten().filter(|h| *h == "PC-01").count(), 1);
        l.auto_group(&[], 5); // empty class
        assert!(l.groups.is_empty());
        let f = std::env::temp_dir().join("erg-layout-test.json");
        l.auto_group(&hosts(4), 2);
        l.save(&f).unwrap();
        assert_eq!(Layout::load(&f).unwrap(), l);
    }

    #[test]
    fn attendance_tracks_missing_and_defuses_csv_formulas() {
        let mut a = Attendance::default();
        a.record("PC-01", "  Nguyễn \"A\" ", "10A1", 5);
        a.record("PC-02", "=cmd()", "10A1", 6);
        a.record("PC-01", "Nguyễn A", "10A1", 9);
        assert_eq!(a.present(), 2);
        assert_eq!(a.missing(&hosts(3)), vec!["PC-03"]);
        let c = a.csv();
        assert!(c.contains("\"PC-01\",\"Nguyễn A\",\"10A1\",9"));
        assert!(c.contains("\"'=cmd()\""));
    }

    #[test]
    fn magic_packet_layout() {
        let m = [1, 2, 3, 4, 5, 6];
        let p = magic_packet(m);
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert!(p[6..].chunks(6).all(|c| c == m));
        assert_eq!(p.len(), 102);
        wake(m, std::net::Ipv4Addr::LOCALHOST).unwrap();
    }

    #[test]
    fn planner_fires_each_step_exactly_once() {
        let plan = Plan { steps: vec![
            Step { at_s: 0, label: "khoá".into(), command: Command::Lock { blank: false, text: None } },
            Step { at_s: 30, label: "mở".into(), command: Command::Unlock },
            Step { at_s: 10, label: "tin".into(), command: Command::Beep },
        ]};
        assert_eq!(plan.at_start().len(), 1);
        let mut fired = vec![];
        let mut prev = 0;
        for now in [5, 12, 12, 29, 31, 100] {
            fired.extend(plan.due(prev, now).iter().map(|s| s.label.clone()));
            prev = now;
        }
        assert_eq!(fired, ["tin", "mở"]);
        let f = std::env::temp_dir().join("erg-plan-test.json");
        plan.save(&f).unwrap();
        assert_eq!(Plan::load(&f).unwrap(), plan);
    }
}

// ---------------------------------------------------------------- remembered MAC addresses

/// host name -> MAC, remembered from beacons so machines can be woken while they are off.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MacBook(pub BTreeMap<String, [u8; 6]>);

impl MacBook {
    pub fn load(p: &Path) -> Self { load_json(p).unwrap_or_default() }
    pub fn save(&self, p: &Path) -> std::io::Result<()> { save_json(self, p) }
    /// True when this changed the book (caller should save).
    pub fn remember(&mut self, host: &str, mac: Option<[u8; 6]>) -> bool {
        match mac {
            Some(m) if m != [0; 6] && self.0.get(host) != Some(&m) => { self.0.insert(host.to_string(), m); true }
            _ => false,
        }
    }
    /// Wake the given hosts (all when empty). Returns how many packets were sent.
    pub fn wake(&self, hosts: &[String], broadcast: std::net::Ipv4Addr) -> usize {
        self.0.iter().filter(|(h, _)| hosts.is_empty() || hosts.contains(h)).filter(|(_, m)| wake(**m, broadcast).is_ok()).count()
    }
}

#[cfg(test)]
mod macbook_tests {
    use super::*;
    #[test]
    fn macs_are_remembered_filtered_and_persisted() {
        let mut b = MacBook::default();
        assert!(b.remember("PC-1", Some([1, 2, 3, 4, 5, 6])));
        assert!(!b.remember("PC-1", Some([1, 2, 3, 4, 5, 6])));
        assert!(!b.remember("PC-2", Some([0; 6])));
        assert!(!b.remember("PC-3", None));
        b.remember("PC-4", Some([9; 6]));
        assert_eq!(b.wake(&["PC-4".into()], std::net::Ipv4Addr::LOCALHOST), 1);
        assert_eq!(b.wake(&[], std::net::Ipv4Addr::LOCALHOST), 2);
        let f = std::env::temp_dir().join("erg-macbook-test.json");
        b.save(&f).unwrap();
        assert_eq!(MacBook::load(&f), b);
        assert_eq!(MacBook::load(&f.with_file_name("none.json")), MacBook::default());
    }
}
