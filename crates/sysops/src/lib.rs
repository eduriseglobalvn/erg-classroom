//! Small Windows operations a student machine needs: clipboard, speaker mute, attention beep,
//! battery / Wi-Fi status and a typed-character hook for the teacher's keyword list.
#![cfg(windows)]
pub mod keys;

use windows::core::Result as WinResult;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;

const CF_UNICODETEXT: u32 = 13;

/// What the address bar of the foreground browser window shows, read through UI Automation (the first edit box of
/// the window). It walks the window's controls, so call it when the tab changed, not on every tick.
pub fn foreground_address() -> Option<String> {
    use windows::Win32::System::Variant::{VARIANT, VT_I4};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;
        let win = uia.ElementFromHandle(GetForegroundWindow()).ok()?;
        // a plain integer variant owns nothing, so it needs no VariantClear
        let mut kind = VARIANT::default();
        (*kind.Anonymous.Anonymous).vt = VT_I4;
        (*kind.Anonymous.Anonymous).Anonymous.lVal = UIA_EditControlTypeId.0;
        let cond = uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &kind).ok()?;
        let edit = win.FindFirst(TreeScope_Descendants, &cond).ok()?;
        let s = edit.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok()?.CurrentValue().ok()?.to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// (program file name, window title) of the window the user is working in.
pub fn foreground() -> Option<(String, String)> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    unsafe {
        let h = GetForegroundWindow();
        if h.0.is_null() {
            return None;
        }
        let mut t = [0u16; 512];
        let len = GetWindowTextW(h, &mut t).max(0) as usize;
        let title = String::from_utf16_lossy(&t[..len]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        let mut exe = String::new();
        // an elevated program (Task Manager) may refuse even this; the title still identifies it
        if let Ok(p) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let (mut buf, mut n) = ([0u16; 1024], 1024u32);
            if QueryFullProcessImageNameW(p, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut n).is_ok() {
                let path = String::from_utf16_lossy(&buf[..n as usize]);
                exe = path.rsplit('\\').next().unwrap_or("").to_string();
            }
            let _ = CloseHandle(p);
        }
        Some((exe, title))
    }
}

/// Closes the clipboard on every exit path.
struct Open;
impl Open {
    fn new() -> WinResult<Self> {
        // Another program may hold the clipboard for a moment.
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        unsafe { OpenClipboard(None).map(|_| Open) }
    }
}
impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

pub fn set_clipboard(text: &str) -> WinResult<()> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _c = Open::new()?;
    unsafe {
        EmptyClipboard()?;
        let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2)?;
        let p = GlobalLock(mem) as *mut u16;
        if p.is_null() {
            return Err(windows::core::Error::from_win32());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
        let _ = GlobalUnlock(mem);
        // On success the system owns the memory.
        SetClipboardData(CF_UNICODETEXT, Some(HANDLE(mem.0)))?;
    }
    Ok(())
}

pub fn clipboard() -> Option<String> {
    let _c = Open::new().ok()?;
    unsafe {
        let h = GetClipboardData(CF_UNICODETEXT).ok()?;
        let mem = HGLOBAL(h.0);
        let p = GlobalLock(mem) as *const u16;
        if p.is_null() {
            return None;
        }
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
        let _ = GlobalUnlock(mem);
        Some(s)
    }
}

fn volume() -> WinResult<IAudioEndpointVolume> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // already-initialised is fine
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        en.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
    }
}

pub fn set_mute(on: bool) -> WinResult<()> {
    unsafe { volume()?.SetMute(on, std::ptr::null()) }
}

pub fn is_muted() -> WinResult<bool> {
    unsafe { Ok(volume()?.GetMute()?.as_bool()) }
}

pub fn beep() {
    let _ = unsafe { MessageBeep(MB_ICONASTERISK) };
}

/// (battery percent if there is a battery, on AC power)
pub fn battery() -> (Option<u8>, bool) {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
        return (None, true);
    }
    // 255 = unknown, 128 flag = no battery
    let pct = (s.BatteryFlag != 128 && s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent);
    (pct, s.ACLineStatus != 0)
}

/// Signal quality 0-100 of the connected Wi-Fi network, None when not on Wi-Fi.
pub fn wifi_signal() -> Option<u8> {
    use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE as H};
    use windows::Win32::NetworkManagement::WiFi::*;
    unsafe {
        let (mut ver, mut h) = (0u32, H::default());
        if WlanOpenHandle(2, None, &mut ver, &mut h) != ERROR_SUCCESS.0 {
            return None;
        }
        let mut list = std::ptr::null_mut();
        let mut out = None;
        if WlanEnumInterfaces(h, None, &mut list) == ERROR_SUCCESS.0 && !list.is_null() {
            let l = &*list;
            for i in 0..l.dwNumberOfItems as usize {
                let guid = l.InterfaceInfo[i].InterfaceGuid;
                let (mut size, mut data) = (0u32, std::ptr::null_mut());
                let r = WlanQueryInterface(h, &guid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None);
                if r == ERROR_SUCCESS.0 && !data.is_null() {
                    let a = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    if a.isState == wlan_interface_state_connected {
                        out = Some(a.wlanAssociationAttributes.wlanSignalQuality.min(100) as u8);
                    }
                    WlanFreeMemory(data);
                }
                if out.is_some() {
                    break;
                }
            }
            WlanFreeMemory(list as _);
        }
        WlanCloseHandle(h, None);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_roundtrips_unicode_and_is_restored() {
        let before = clipboard();
        let text = "Xin chào ERG – 你好 😀";
        set_clipboard(text).unwrap();
        assert_eq!(clipboard().as_deref(), Some(text));
        set_clipboard("").unwrap();
        assert_eq!(clipboard().as_deref(), Some(""));
        if let Some(b) = before {
            let _ = set_clipboard(&b);
        }
    }

    #[test]
    fn mute_toggles_and_the_original_state_is_restored() {
        let Ok(orig) = is_muted() else { return }; // no audio device (e.g. CI): nothing to test
        set_mute(!orig).unwrap();
        assert_eq!(is_muted().unwrap(), !orig);
        set_mute(orig).unwrap();
        assert_eq!(is_muted().unwrap(), orig);
    }

    #[test]
    fn status_queries_return_sane_values() {
        if let (Some(p), _) = battery() {
            assert!(p <= 100);
        }
        println!("battery {:?} wifi {:?}", battery(), wifi_signal());
        if let Some(s) = wifi_signal() {
            assert!(s <= 100);
        }
    }
}

// ---------------------------------------------------------------- remote control input

/// Inject one teacher input event. Mouse coordinates are 0..=1 of the primary screen.
pub fn input(e: erg_proto::InputEvent) {
    use erg_proto::InputEvent::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let mut i = INPUT::default();
    match e {
        MouseMove { x, y } => {
            i.r#type = INPUT_MOUSE;
            i.Anonymous.mi = MOUSEINPUT {
                dx: (x.clamp(0.0, 1.0) * 65535.0) as i32,
                dy: (y.clamp(0.0, 1.0) * 65535.0) as i32,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                ..Default::default()
            };
        }
        MouseButton { button, down } => {
            let flag = match (button, down) {
                (0, true) => MOUSEEVENTF_LEFTDOWN,
                (0, false) => MOUSEEVENTF_LEFTUP,
                (1, true) => MOUSEEVENTF_RIGHTDOWN,
                (1, false) => MOUSEEVENTF_RIGHTUP,
                (_, true) => MOUSEEVENTF_MIDDLEDOWN,
                (_, false) => MOUSEEVENTF_MIDDLEUP,
            };
            i.r#type = INPUT_MOUSE;
            i.Anonymous.mi = MOUSEINPUT { dwFlags: flag, ..Default::default() };
        }
        Wheel(d) => {
            i.r#type = INPUT_MOUSE;
            i.Anonymous.mi = MOUSEINPUT { mouseData: d as i32 as u32, dwFlags: MOUSEEVENTF_WHEEL, ..Default::default() };
        }
        Key { vk, down } => {
            i.r#type = INPUT_KEYBOARD;
            i.Anonymous.ki = KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP },
                ..Default::default()
            };
        }
    }
    unsafe { SendInput(&[i], std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN};

    #[test]
    fn mouse_move_lands_on_the_requested_spot_and_is_restored() {
        let mut before = POINT::default();
        unsafe { GetCursorPos(&mut before).unwrap() };
        let (w, h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        // A person moving the real mouse during the test can win a race; retry a few times.
        let mut now = POINT::default();
        for _ in 0..10 {
            input(erg_proto::InputEvent::MouseMove { x: 0.5, y: 0.25 });
            std::thread::sleep(std::time::Duration::from_millis(100));
            unsafe { GetCursorPos(&mut now).unwrap() };
            if (now.x - w / 2).abs() <= 2 && (now.y - h / 4).abs() <= 2 {
                break;
            }
        }
        unsafe { SetCursorPos(before.x, before.y).unwrap() };
        assert!((now.x - w / 2).abs() <= 2 && (now.y - h / 4).abs() <= 2, "cursor at {now:?} on {w}x{h}");
    }
}
