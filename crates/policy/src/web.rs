//! Web control through browser enterprise policies (Chrome, Edge, Firefox).
//!
//! shortcut: only browsers that honour these policies are covered (not Tor, portable or unknown
//! browsers, not non-browser traffic). Upgrade with a WFP filter driver when that matters.
//! Writing under HKLM needs administrator rights, so the SYSTEM service applies it, not the agent.
use erg_proto::WebPolicy;
use winreg::{enums::*, RegKey};

const BROWSERS: &[&str] = &["Google\\Chrome", "Microsoft\\Edge"];
const FIREFOX: &str = "Mozilla\\Firefox\\WebsiteFilter";

fn valid(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.chars().all(|c| c.is_ascii_alphanumeric() || "-._*/:".contains(c))
}

/// Entries become Chrome URL patterns and Firefox match patterns; reject anything odd.
pub fn clean(list: &[String]) -> Result<Vec<String>, String> {
    list.iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| if valid(s) { Ok(s.to_string()) } else { Err(format!("invalid site: {s:?}")) })
        .collect()
}

fn fill(root: &RegKey, path: &str, items: &[String]) -> std::io::Result<()> {
    let _ = root.delete_subkey_all(path);
    if items.is_empty() {
        return Ok(());
    }
    let (k, _) = root.create_subkey(path)?;
    for (i, v) in items.iter().enumerate() {
        k.set_value((i + 1).to_string(), v)?;
    }
    Ok(())
}

/// Firefox needs match patterns, so a bare host becomes `*://host/*`.
fn firefox_pattern(h: &str) -> String {
    if h == "*" { "<all_urls>".into() } else if h.contains("://") { h.into() } else { format!("*://{h}/*") }
}

/// Applies `mode` below `policies` (normally `HKLM\SOFTWARE\Policies`). Unrestricted removes our keys.
pub fn apply_at(hive: &RegKey, policies: &str, mode: WebPolicy, list: &[String]) -> Result<(), String> {
    let list = clean(list)?;
    let (block, allow): (Vec<String>, Vec<String>) = match mode {
        WebPolicy::Unrestricted => (vec![], vec![]),
        WebPolicy::BlockAll => (vec!["*".into()], vec![]),
        WebPolicy::BlockList => (list, vec![]),
        WebPolicy::AllowListOnly => {
            if list.is_empty() {
                return Err("allow-list is empty".into());
            }
            (vec!["*".into()], list)
        }
    };
    let io = |e: std::io::Error| e.to_string();
    for b in BROWSERS {
        fill(hive, &format!("{policies}\\{b}\\URLBlocklist"), &block).map_err(io)?;
        fill(hive, &format!("{policies}\\{b}\\URLAllowlist"), &allow).map_err(io)?;
    }
    let ff = |v: &[String]| v.iter().map(|h| firefox_pattern(h)).collect::<Vec<_>>();
    fill(hive, &format!("{policies}\\{FIREFOX}\\Block"), &ff(&block)).map_err(io)?;
    fill(hive, &format!("{policies}\\{FIREFOX}\\Exceptions"), &ff(&allow)).map_err(io)?;
    Ok(())
}

pub fn apply(mode: WebPolicy, list: &[String]) -> Result<(), String> {
    apply_at(&RegKey::predef(HKEY_LOCAL_MACHINE), "SOFTWARE\\Policies", mode, list)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "Software\\ErgPolicyTest";

    fn read(hive: &RegKey, path: &str) -> Vec<String> {
        hive.open_subkey(format!("{BASE}\\{path}"))
            .map(|k| (1..20).filter_map(|i| k.get_value::<String, _>(i.to_string()).ok()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn modes_write_the_expected_policy_values_and_unrestricted_clears_them() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(BASE);
        let sites = vec!["school.edu.vn".to_string(), " wiki.org ".to_string()];

        apply_at(&hkcu, BASE, WebPolicy::AllowListOnly, &sites).unwrap();
        assert_eq!(read(&hkcu, "Google\\Chrome\\URLBlocklist"), ["*"]);
        assert_eq!(read(&hkcu, "Microsoft\\Edge\\URLAllowlist"), ["school.edu.vn", "wiki.org"]);
        assert_eq!(read(&hkcu, "Mozilla\\Firefox\\WebsiteFilter\\Block"), ["<all_urls>"]);
        assert_eq!(read(&hkcu, "Mozilla\\Firefox\\WebsiteFilter\\Exceptions")[0], "*://school.edu.vn/*");

        apply_at(&hkcu, BASE, WebPolicy::BlockList, &["game.com".into()]).unwrap();
        assert_eq!(read(&hkcu, "Google\\Chrome\\URLBlocklist"), ["game.com"]);
        assert!(read(&hkcu, "Google\\Chrome\\URLAllowlist").is_empty(), "stale allow entries must go");

        apply_at(&hkcu, BASE, WebPolicy::BlockAll, &[]).unwrap();
        assert_eq!(read(&hkcu, "Microsoft\\Edge\\URLBlocklist"), ["*"]);

        apply_at(&hkcu, BASE, WebPolicy::Unrestricted, &[]).unwrap();
        assert!(read(&hkcu, "Google\\Chrome\\URLBlocklist").is_empty());
        let _ = hkcu.delete_subkey_all(BASE);
    }

    #[test]
    fn bad_input_is_rejected_before_touching_the_registry() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        assert!(apply_at(&hkcu, BASE, WebPolicy::AllowListOnly, &[]).is_err());
        assert!(apply_at(&hkcu, BASE, WebPolicy::BlockList, &["a b".into()]).is_err());
        assert!(apply_at(&hkcu, BASE, WebPolicy::BlockList, &["x\u{0}y".into()]).is_err());
    }
}
