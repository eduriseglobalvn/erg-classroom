//! "Protect": per-user Explorer policies (the same values Group Policy writes), applied under HKCU
//! so the signed-in student gets them without administrator rights. Everything we set is removed
//! again by applying the default (empty) profile.
//!
//! shortcut: a student can undo per-user policies with regedit unless regedit is blocked too; a
//! machine-wide version needs the SYSTEM service.
use erg_proto::Protect;
use winreg::{enums::*, RegKey};

const EXPLORER: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer";
const SYSTEM: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";

/// Bit mask for NoDrives: bit 0 = A ... bit 25 = Z. Letters outside A-Z are ignored. C is never
/// hidden, so a typo cannot make the system drive vanish and confuse the student.
pub fn drive_mask(letters: &[char]) -> u32 {
    letters.iter().filter_map(|c| {
        let c = c.to_ascii_uppercase();
        (c.is_ascii_uppercase() && c != 'C').then(|| 1u32 << (c as u8 - b'A'))
    }).fold(0, |m, b| m | b)
}

fn set_or_clear(key: &RegKey, name: &str, on: bool, value: u32) -> std::io::Result<()> {
    if on {
        key.set_value(name, &value)
    } else {
        match key.delete_value(name) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

pub fn apply_at(hive: &RegKey, explorer: &str, system: &str, p: &Protect) -> std::io::Result<()> {
    let (ex, _) = hive.create_subkey(explorer)?;
    let mask = drive_mask(&p.hide_drives);
    set_or_clear(&ex, "NoDrives", mask != 0, mask)?;
    set_or_clear(&ex, "NoViewOnDrive", mask != 0, mask)?;
    set_or_clear(&ex, "NoControlPanel", p.no_control_panel, 1)?;
    set_or_clear(&ex, "NoRun", p.no_run_dialog, 1)?;
    let (sy, _) = hive.create_subkey(system)?;
    set_or_clear(&sy, "DisableTaskMgr", p.no_task_manager, 1)
}

pub fn apply(p: &Protect) -> std::io::Result<()> {
    apply_at(&RegKey::predef(HKEY_CURRENT_USER), EXPLORER, SYSTEM, p)
}

/// For the SYSTEM service, whose own HKCU is the wrong hive: write the profile into the loaded hive of the
/// signed-in user `sid` (HKEY_USERS\<sid>). Only a plain SID is accepted so it cannot escape into other keys.
pub fn apply_for_sid(sid: &str, p: &Protect) -> Result<(), String> {
    if !sid_ok(sid) {
        return Err(format!("bad SID {sid:?}"));
    }
    apply_at(&RegKey::predef(HKEY_USERS), &format!("{sid}\\{EXPLORER}"), &format!("{sid}\\{SYSTEM}"), p).map_err(|e| format!("{sid}: {e}"))
}

pub fn sid_ok(s: &str) -> bool {
    s.len() > 4 && s.len() <= 100 && s.starts_with("S-1-") && s[4..].chars().all(|c| c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    const E: &str = "Software\\ErgProtectTest\\Explorer";
    const S: &str = "Software\\ErgProtectTest\\System";

    #[test]
    fn mask_hides_letters_but_never_c() {
        assert_eq!(drive_mask(&['d']), 1 << 3);
        assert_eq!(drive_mask(&['D', 'E', 'c', '1', 'é']), (1 << 3) | (1 << 4));
        assert_eq!(drive_mask(&[]), 0);
        assert_eq!(drive_mask(&['Z']), 1 << 25);
    }

    #[test]
    fn profile_is_written_and_fully_removed_by_the_default_profile() {
        let h = RegKey::predef(HKEY_CURRENT_USER);
        let _ = h.delete_subkey_all("Software\\ErgProtectTest");
        let p = Protect { no_control_panel: true, hide_drives: vec!['D'], no_run_dialog: true, no_task_manager: true };
        apply_at(&h, E, S, &p).unwrap();
        let ex = h.open_subkey(E).unwrap();
        assert_eq!(ex.get_value::<u32, _>("NoDrives").unwrap(), 8);
        assert_eq!(ex.get_value::<u32, _>("NoControlPanel").unwrap(), 1);
        assert_eq!(ex.get_value::<u32, _>("NoRun").unwrap(), 1);
        assert_eq!(h.open_subkey(S).unwrap().get_value::<u32, _>("DisableTaskMgr").unwrap(), 1);

        apply_at(&h, E, S, &Protect { no_control_panel: true, ..Default::default() }).unwrap();
        let ex = h.open_subkey(E).unwrap();
        assert!(ex.get_value::<u32, _>("NoDrives").is_err(), "drives must be visible again");
        assert!(ex.get_value::<u32, _>("NoRun").is_err());

        apply_at(&h, E, S, &Protect::default()).unwrap();
        assert!(h.open_subkey(E).unwrap().get_value::<u32, _>("NoControlPanel").is_err());
        apply_at(&h, E, S, &Protect::default()).unwrap(); // removing twice is fine
        let _ = h.delete_subkey_all("Software\\ErgProtectTest");
    }
}

#[cfg(test)]
mod sid_tests {
    use super::*;

    #[test]
    fn only_plain_sids_become_registry_paths() {
        assert!(sid_ok("S-1-5-21-1-2-3-1001"));
        for bad in ["", "S-1-", "S-1-5-21\\..\\x", ".DEFAULT", "S-1-5-21-1/2"] {
            assert!(!sid_ok(bad), "{bad:?}");
            assert!(apply_for_sid(bad, &Protect::default()).is_err());
        }
    }

    /// HKEY_USERS\<own SID> is the same hive as HKCU, so the service's route can be exercised without admin.
    #[test]
    fn the_hkey_users_route_reaches_the_users_hive() {
        let out = std::process::Command::new("whoami").args(["/user", "/fo", "csv", "/nh"]).output().unwrap();
        let sid = String::from_utf8_lossy(&out.stdout).split(',').nth(1).unwrap_or("").trim().trim_matches('"').to_string();
        assert!(sid_ok(&sid), "{sid:?}");
        let users = RegKey::predef(HKEY_USERS);
        let (e, s) = (format!("{sid}\\Software\\ErgProtectSidTest\\Explorer"), format!("{sid}\\Software\\ErgProtectSidTest\\System"));
        apply_at(&users, &e, &s, &Protect { hide_drives: vec!['E'], no_run_dialog: true, ..Default::default() }).unwrap();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\ErgProtectSidTest\\Explorer").unwrap();
        assert_eq!(hkcu.get_value::<u32, _>("NoDrives").unwrap(), 1 << 4);
        assert_eq!(hkcu.get_value::<u32, _>("NoRun").unwrap(), 1);
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all("Software\\ErgProtectSidTest");
    }
}
