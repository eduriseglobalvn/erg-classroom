//! Global typed-character hook (WH_KEYBOARD_LL). It only turns key presses into characters and
//! hands them to the callback; the callback decides what is kept (see KeywordMonitor: only
//! whole words from the teacher's list ever leave the machine).
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK};
use windows::Win32::UI::WindowsAndMessaging::*;

type Sink = Box<dyn FnMut(char) + Send>;
static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();

unsafe extern "system" fn proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && (w.0 as u32 == WM_KEYDOWN || w.0 as u32 == WM_SYSKEYDOWN) {
        let k = &*(l.0 as *const KBDLLHOOKSTRUCT);
        // injected keys (our own tests, remote control) are not the student typing
        if k.flags.0 & LLKHF_INJECTED.0 == 0 {
            if let Some(c) = to_char(k.vkCode, k.scanCode) {
                if let Some(s) = SINK.get() {
                    if let Ok(mut f) = s.lock() {
                        f(c);
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, w, l)
}

fn to_char(vk: u32, scan: u32) -> Option<char> {
    if vk == VK_BACK.0 as u32 {
        return Some('\u{8}');
    }
    unsafe {
        let mut state = [0u8; 256];
        GetKeyboardState(&mut state).ok()?;
        let mut buf = [0u16; 4];
        // flag bit 2 = do not change the keyboard's dead-key state
        let n = ToUnicodeEx(vk, scan, &state, &mut buf, 4, Some(GetKeyboardLayout(0)));
        (n == 1).then(|| char::from_u32(buf[0] as u32)).flatten()
    }
}

/// Start the hook on its own thread. Only one hook per process; later calls are ignored.
/// The thread (and hook) lives until the process exits.
pub fn start(sink: impl FnMut(char) + Send + 'static) {
    if SINK.set(Mutex::new(Box::new(sink))).is_err() {
        return;
    }
    std::thread::spawn(|| unsafe {
        let Ok(_hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc), None, 0) else { return };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}
