//! Decides, from the foreground window alone, whether the student opened something forbidden.
//! Nothing is closed: the agent only locks with the warning screen, so the teacher finds the evidence after unlocking.
use std::collections::HashSet;
use std::sync::OnceLock;

// "browser.exe" is Coc Coc
const BROWSERS: &[&str] = &["chrome.exe", "msedge.exe", "firefox.exe", "browser.exe", "opera.exe", "brave.exe", "iexplore.exe"];
const ADULT_WORDS: &[&str] = &["porn", "porno", "pornhub", "xvideos", "xnxx", "xhamster", "sex", "xxx", "hentai", "jav", "nsfw", "onlyfans"];
const ADULT_PHRASES: &[&str] = &["phim người lớn", "phim nguoi lon"];
const TOOL_EXES: &[&str] = &["taskmgr.exe", "regedit.exe", "control.exe"];
const TOOL_TITLES: &[&str] = &["task manager", "registry editor", "control panel", "all control panel items", "trình quản lý tác vụ"];

/// The teacher's own additions, lowercase. A site is "example.com" (sub-sites included); a word is one word or a short phrase.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Lists {
    pub sites: Vec<String>,
    pub words: Vec<String>,
}

/// Adult-site names from StevenBlack/hosts (MIT, see data/NOTICE-adult-domains.txt), one per line.
fn adult_sites() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| include_str!("../data/adult-domains.txt").lines().collect())
}

pub fn is_browser(exe: &str) -> bool {
    BROWSERS.contains(&exe.to_lowercase().as_str())
}

/// Host name out of what the address bar shows ("https://www.a.com/x?y", "a.com/x", "a.com").
pub fn host_of(address: &str) -> Option<String> {
    let a = address.trim().to_lowercase();
    let a = a.split_once("://").map_or(a.as_str(), |(_, rest)| rest);
    let host = a.split(['/', '?', '#', ':', ' ']).next()?.trim_start_matches("www.");
    (host.contains('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).then(|| host.to_string())
}

/// True when `host` or one of its parent domains ("a.b.com" -> "b.com") satisfies `listed`.
fn host_listed(host: &str, listed: impl Fn(&str) -> bool) -> bool {
    let mut h = host;
    loop {
        if listed(h) { return true; }
        match h.split_once('.') {
            Some((_, rest)) if rest.contains('.') => h = rest,
            _ => return false,
        }
    }
}

/// Whole words only ("Essex" must not match "sex"); a phrase with a space is looked for as it is.
fn has_word(text: &str, word: &str) -> bool {
    if word.contains(' ') { text.contains(word) } else { text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word) }
}

/// `exe` is the program's file name, `title` its window title, `address` the browser's address bar when it could be read.
/// Returns "adult" (built-in list), "site" / "word" (the teacher's lists) or "tool" (system program).
pub fn check(exe: &str, title: &str, address: Option<&str>, lists: &Lists) -> Option<&'static str> {
    let (exe, title) = (exe.to_lowercase(), title.to_lowercase());
    if BROWSERS.contains(&exe.as_str()) {
        let host = address.and_then(host_of);
        if let Some(h) = &host {
            if host_listed(h, |d| adult_sites().contains(d)) { return Some("adult"); }
            if host_listed(h, |d| lists.sites.iter().any(|s| s.trim_start_matches("*.") == d)) { return Some("site"); }
        }
        if ADULT_WORDS.iter().chain(ADULT_PHRASES).any(|w| has_word(&title, w)) { return Some("adult"); }
        let text = format!("{title} {}", address.unwrap_or("").to_lowercase());
        return lists.words.iter().any(|w| has_word(&text, w)).then_some("word");
    }
    // Control Panel is an Explorer window, so the title has to decide; a sub-page is "Control Panel\System..."
    let tool = TOOL_EXES.contains(&exe.as_str()) || TOOL_TITLES.iter().any(|t| title == *t || title.starts_with(&format!("{t}\\")));
    tool.then_some("tool")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_violations_are_reported() {
        let none = Lists::default();
        let c = |exe, title| check(exe, title, None, &none);
        assert_eq!(c("chrome.exe", "Pornhub - Google Chrome"), Some("adult"));
        assert_eq!(c("browser.exe", "Xem phim SEX hay - Cốc Cốc"), Some("adult"));
        assert_eq!(c("msedge.exe", "Phim người lớn - Microsoft Edge"), Some("adult"));
        assert_eq!(c("chrome.exe", "Essex County - Wikipedia - Google Chrome"), None);
        assert_eq!(c("chrome.exe", "Task Manager - Wikipedia - Google Chrome"), None);
        assert_eq!(c("WINWORD.EXE", "sex education.docx - Word"), None);
        assert_eq!(c("Taskmgr.exe", "Task Manager"), Some("tool"));
        assert_eq!(c("explorer.exe", "Control Panel\\System and Security"), Some("tool"));
        assert_eq!(c("explorer.exe", "Documents"), None);
    }

    #[test]
    fn address_is_matched_against_the_built_in_and_the_teachers_lists() {
        assert!(adult_sites().len() > 50_000, "the bundled list is missing or cut short");
        let site = *adult_sites().iter().next().unwrap();
        let none = Lists::default();
        // an innocent title does not help once the address is on the list, sub-sites included
        assert_eq!(check("chrome.exe", "Home - Google Chrome", Some(&format!("https://cdn.{site}/a?b=1")), &none), Some("adult"));
        assert_eq!(check("chrome.exe", "Home - Google Chrome", Some("vi.wikipedia.org/wiki/Essex"), &none), None);
        let mine = Lists { sites: vec!["*.garena.vn".into(), "facebook.com".into()], words: vec!["casino".into(), "cá độ".into()] };
        assert_eq!(check("msedge.exe", "x", Some("https://www.facebook.com/"), &mine), Some("site"));
        assert_eq!(check("msedge.exe", "x", Some("lienquan.garena.vn/tin"), &mine), Some("site"));
        assert_eq!(check("msedge.exe", "x", Some("notfacebook.com"), &mine), None);
        assert_eq!(check("msedge.exe", "Kèo cá độ hôm nay", None, &mine), Some("word"));
        assert_eq!(check("msedge.exe", "x", Some("best-casino.example/play"), &mine), Some("word"));
        assert_eq!(host_of("Search Google or type a URL"), None);
    }
}
