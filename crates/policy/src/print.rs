//! Print control: pause every local printer queue, or pause and delete everything queued.
//!
//! shortcut: acts on queues that exist when the command arrives; a printer added later is not
//! covered until the next command. Upgrade by watching `FindFirstPrinterChangeNotification`.
use erg_proto::PrintPolicy;
use windows::core::PWSTR;
use windows::Win32::Graphics::Printing::*;

/// Names of the printers on this machine.
pub fn printers() -> Vec<String> {
    let (mut need, mut n) = (0u32, 0u32);
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    unsafe {
        let _ = EnumPrintersW(flags, None, 4, None, &mut need, &mut n);
        if need == 0 {
            return vec![];
        }
        let mut buf = vec![0u8; need as usize];
        if EnumPrintersW(flags, None, 4, Some(&mut buf), &mut need, &mut n).is_err() {
            return vec![];
        }
        let info = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, n as usize);
        info.iter().filter_map(|i| i.pPrinterName.to_string().ok()).collect()
    }
}

fn set_state(name: &str, command: u32, purge: bool) -> Result<(), String> {
    let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let defaults = PRINTER_DEFAULTSW { pDatatype: PWSTR::null(), pDevMode: std::ptr::null_mut(), DesiredAccess: PRINTER_ACCESS_ADMINISTER };
    let mut h = PRINTER_HANDLE::default();
    unsafe {
        OpenPrinterW(PWSTR(wide.as_mut_ptr()), &mut h, Some(&defaults)).map_err(|e| format!("{name}: {e}"))?;
        let r = SetPrinterW(h, 0, None, command).map_err(|e| e.to_string());
        if purge && command != PRINTER_CONTROL_RESUME {
            let _ = SetPrinterW(h, 0, None, PRINTER_CONTROL_PURGE);
        }
        let _ = ClosePrinter(h);
        r.map_err(|e| format!("{name}: {e}"))
    }
}

/// Returns the printers that could not be changed (empty = all done).
pub fn apply(p: PrintPolicy) -> Vec<String> {
    let (cmd, purge) = match p {
        PrintPolicy::Allow => (PRINTER_CONTROL_RESUME, false),
        PrintPolicy::Pause => (PRINTER_CONTROL_PAUSE, false),
        PrintPolicy::BlockAll => (PRINTER_CONTROL_PAUSE, true),
    };
    printers().iter().filter_map(|n| set_state(n, cmd, purge).err()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_and_applying_never_panic_even_without_printers() {
        let names = printers();
        println!("{} printers: {names:?}", names.len());
        // Allow is harmless on a real machine: it only resumes queues.
        let failed = apply(PrintPolicy::Allow);
        println!("not changed: {failed:?}");
    }
}
