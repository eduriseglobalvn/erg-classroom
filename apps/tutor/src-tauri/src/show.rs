//! Teacher-side Show: the capture -> H.264 -> multicast pipeline lives in `erg-broadcast`.
pub use erg_broadcast::{foreground_window, Handle as ShowHandle, Source};

pub const GROUP: [u8; 4] = [239, 255, 77, 20];
pub const PORT: u16 = 47900;

pub fn start_tap(key: [u8; 16], stream: u32, source: Source, tap: erg_broadcast::Tap) -> Result<ShowHandle, String> {
    erg_broadcast::start_tap(source, erg_broadcast::Target { group: GROUP, port: PORT, key, stream }, Some(tap))
}

pub fn start(key: [u8; 16], stream: u32, source: Source) -> Result<ShowHandle, String> {
    erg_broadcast::start(source, erg_broadcast::Target { group: GROUP, port: PORT, key, stream })
}
