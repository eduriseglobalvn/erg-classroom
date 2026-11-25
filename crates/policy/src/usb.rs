//! Removable storage control through the Windows policy keys (same as Group Policy
//! "Removable Disks: Deny read/write/execute access"). Needs administrator rights under HKLM.
//!
//! shortcut: only covers USB disks and CD/DVD classes, not phones (MTP) or other portable devices.
use erg_proto::UsbPolicy;
use winreg::{enums::*, RegKey};

const BASE: &str = "SOFTWARE\\Policies\\Microsoft\\Windows\\RemovableStorageDevices";
const REMOVABLE_DISK: &str = "{53f5630d-b6bf-11d0-94f2-00a0c91efb8b}";
const CD_DVD: &str = "{53f56308-b6bf-11d0-94f2-00a0c91efb8b}";

/// (read denied, write denied, execute denied) for each policy.
pub fn flags(p: UsbPolicy) -> (bool, bool, bool) {
    match p {
        UsbPolicy::Allow => (false, false, false),
        UsbPolicy::ReadOnly => (false, true, false),
        UsbPolicy::BlockAll => (true, true, true),
        UsbPolicy::NoExecute => (false, false, true),
    }
}

pub fn apply_at(hive: &RegKey, base: &str, p: UsbPolicy) -> std::io::Result<()> {
    let _ = hive.delete_subkey_all(base); // never leave stale values from a previous mode
    if p == UsbPolicy::Allow {
        return Ok(());
    }
    let (r, w, x) = flags(p);
    for class in [REMOVABLE_DISK, CD_DVD] {
        let (k, _) = hive.create_subkey(format!("{base}\\{class}"))?;
        k.set_value("Deny_Read", &(r as u32))?;
        k.set_value("Deny_Write", &(w as u32))?;
        k.set_value("Deny_Execute", &(x as u32))?;
    }
    Ok(())
}

pub fn apply(p: UsbPolicy) -> std::io::Result<()> {
    apply_at(&RegKey::predef(HKEY_LOCAL_MACHINE), BASE, p)
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: &str = "Software\\ErgUsbTest";

    fn read(h: &RegKey, name: &str) -> Option<u32> {
        h.open_subkey(format!("{T}\\{REMOVABLE_DISK}")).ok()?.get_value(name).ok()
    }

    #[test]
    fn each_mode_writes_its_flags_and_allow_removes_everything() {
        let h = RegKey::predef(HKEY_CURRENT_USER);
        let _ = h.delete_subkey_all(T);
        apply_at(&h, T, UsbPolicy::ReadOnly).unwrap();
        assert_eq!((read(&h, "Deny_Read"), read(&h, "Deny_Write"), read(&h, "Deny_Execute")), (Some(0), Some(1), Some(0)));
        apply_at(&h, T, UsbPolicy::NoExecute).unwrap();
        assert_eq!((read(&h, "Deny_Write"), read(&h, "Deny_Execute")), (Some(0), Some(1)));
        apply_at(&h, T, UsbPolicy::BlockAll).unwrap();
        assert_eq!(read(&h, "Deny_Read"), Some(1));
        assert!(h.open_subkey(format!("{T}\\{CD_DVD}")).is_ok(), "CD/DVD class is covered too");
        apply_at(&h, T, UsbPolicy::Allow).unwrap();
        assert!(h.open_subkey(T).is_err());
        apply_at(&h, T, UsbPolicy::Allow).unwrap(); // removing twice is fine
    }
}
