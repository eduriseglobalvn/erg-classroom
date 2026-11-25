//! H.264 encoding with the encoders that ship with Windows and its graphics drivers (Media Foundation), so we
//! bundle no codec. Output is Annex-B (start-code) byte streams, which WebCodecs can decode directly.
#![cfg(windows)]

use std::time::{Duration, Instant};
use windows::core::{Interface, GUID};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VARIANT;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("windows: {0}")]
    Win(#[from] windows::core::Error),
    #[error("width and height must be even and non-zero")]
    BadSize,
    #[error("input is {got} bytes, expected {want}")]
    BadInput { got: usize, want: usize },
}

pub struct Encoded {
    pub data: Vec<u8>,
    pub keyframe: bool,
}

pub struct H264Encoder {
    mft: IMFTransform,
    codec: Option<ICodecAPI>,
    /// Some = a graphics-card encoder. Those are asynchronous: they ask for input and announce output through events.
    events: Option<IMFMediaEventGenerator>,
    need_input: u32,
    /// SPS/PPS of an encoder that does not repeat them inside its keyframes.
    seq_header: Vec<u8>,
    w: usize,
    h: usize,
    fps: u32,
    bitrate: u32,
    gop: u32,
    frame_dur: i64,
    n: i64,
    out_size: u32,
    provides_samples: bool,
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn fail() -> Error {
    windows::core::Error::from(E_FAIL).into()
}

/// First hardware H.264 encoder Windows lists (Intel Quick Sync, NVIDIA, AMD), unlocked for use.
unsafe fn hardware_mft() -> Result<IMFTransform, Error> {
    let out = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
    let (mut list, mut n) = (std::ptr::null_mut::<Option<IMFActivate>>(), 0u32);
    MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER, None, Some(&out), &mut list, &mut n)?;
    let all: Vec<IMFActivate> = (0..n as usize).filter_map(|i| std::ptr::read(list.add(i))).collect();
    CoTaskMemFree(Some(list as *const _));
    for a in all {
        let Ok(mft) = a.ActivateObject::<IMFTransform>() else { continue };
        if mft.GetAttributes().and_then(|at| at.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)).is_ok() {
            return Ok(mft);
        }
    }
    Err(fail())
}

impl H264Encoder {
    /// `bitrate` in bits per second; `gop` = frames between forced keyframes.
    /// Uses the graphics card's encoder when there is one and falls back to the Windows software encoder.
    /// Environment variable ERG_ENCODER=cpu forces the software encoder.
    pub fn new(w: u32, h: u32, fps: u32, bitrate: u32, gop: u32) -> Result<Self, Error> {
        if w == 0 || h == 0 || !w.is_multiple_of(2) || !h.is_multiple_of(2) {
            return Err(Error::BadSize);
        }
        if std::env::var("ERG_ENCODER").is_ok_and(|v| v.eq_ignore_ascii_case("cpu")) {
            return Self::build(w, h, fps, bitrate, gop, false);
        }
        Self::build(w, h, fps, bitrate, gop, true).or_else(|_| Self::build(w, h, fps, bitrate, gop, false))
    }

    /// "GPU" or "CPU": which encoder this one ended up with.
    pub fn kind(&self) -> &'static str {
        if self.events.is_some() { "GPU" } else { "CPU" }
    }

    fn build(w: u32, h: u32, fps: u32, bitrate: u32, gop: u32, hardware: bool) -> Result<Self, Error> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // already-initialised is fine
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
            let made = if hardware { hardware_mft() } else { CoCreateInstance(&CLSID_MSH264EncoderMFT, None, CLSCTX_INPROC_SERVER).map_err(Error::from) };
            // from here on the encoder owns one MFStartup; Drop gives it back
            let r = made.and_then(|mft| Self::configure(mft, w, h, fps, bitrate, gop, hardware));
            if r.is_err() {
                let _ = MFShutdown();
            }
            r
        }
    }

    unsafe fn configure(mft: IMFTransform, w: u32, h: u32, fps: u32, bitrate: u32, gop: u32, hardware: bool) -> Result<Self, Error> {
        // Must be set before the media types or the encoder keeps its default look-ahead buffering.
        // Best-effort: encoders may reject individual properties.
        let codec = mft.cast::<ICodecAPI>().ok();
        if let Some(c) = &codec {
            let set = |g: &GUID, v: VARIANT| {
                let _ = c.SetValue(g, &v);
            };
            set(&CODECAPI_AVLowLatencyMode, VARIANT::from(true));
            set(&CODECAPI_AVEncCommonRateControlMode, VARIANT::from(eAVEncCommonRateControlMode_CBR.0 as u32));
            set(&CODECAPI_AVEncCommonMeanBitRate, VARIANT::from(bitrate));
            set(&CODECAPI_AVEncMPVGOPSize, VARIANT::from(gop));
            set(&CODECAPI_AVEncMPVDefaultBPictureCount, VARIANT::from(0u32));
        }

        let out = MFCreateMediaType()?;
        out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        out.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
        out.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
        out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
        mft.SetOutputType(0, &out, 0)?;

        let inp = MFCreateMediaType()?;
        inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        inp.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        inp.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
        inp.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
        inp.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        inp.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        mft.SetInputType(0, &inp, 0)?;

        let events = if hardware { Some(mft.cast::<IMFMediaEventGenerator>()?) } else { None };
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        let info = mft.GetOutputStreamInfo(0)?;
        let provides = (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32;
        let mut me = Self {
            mft,
            codec,
            events,
            need_input: 0,
            seq_header: Vec::new(),
            w: w as usize,
            h: h as usize,
            fps,
            bitrate,
            gop,
            frame_dur: 10_000_000 / fps as i64,
            n: 0,
            out_size: info.cbSize.max(1 << 20),
            provides_samples: info.dwFlags & provides != 0,
        };
        me.read_seq_header();
        Ok(me)
    }

    unsafe fn read_seq_header(&mut self) {
        if let Ok(t) = self.mft.GetOutputCurrentType(0) {
            let len = t.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER).unwrap_or(0) as usize;
            let mut blob = vec![0u8; len];
            if len > 0 && t.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut blob, None).is_ok() {
                self.seq_header = blob;
            }
        }
    }

    /// Encode one BGRA frame (`w * h * 4` bytes). Returns zero or more encoded access units.
    /// A graphics-card encoder that stops working is replaced by the software one, so a Show never ends over it.
    pub fn encode(&mut self, bgra: &[u8], force_keyframe: bool) -> Result<Vec<Encoded>, Error> {
        let want = self.w * self.h * 4;
        if bgra.len() != want {
            return Err(Error::BadInput { got: bgra.len(), want });
        }
        match self.encode_once(bgra, force_keyframe) {
            Err(_) if self.events.is_some() => {
                *self = Self::build(self.w as u32, self.h as u32, self.fps, self.bitrate, self.gop, false)?;
                self.encode_once(bgra, true)
            }
            r => r,
        }
    }

    fn encode_once(&mut self, bgra: &[u8], force_keyframe: bool) -> Result<Vec<Encoded>, Error> {
        let nv12 = bgra_to_nv12(bgra, self.w, self.h);
        unsafe {
            if force_keyframe {
                if let Some(c) = &self.codec {
                    let _ = c.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &VARIANT::from(1u32));
                }
            }
            let buf = MFCreateMemoryBuffer(nv12.len() as u32)?;
            let mut p = std::ptr::null_mut();
            buf.Lock(&mut p, None, None)?;
            std::ptr::copy_nonoverlapping(nv12.as_ptr(), p, nv12.len());
            buf.Unlock()?;
            buf.SetCurrentLength(nv12.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buf)?;
            sample.SetSampleTime(self.n * self.frame_dur)?;
            sample.SetSampleDuration(self.frame_dur)?;
            self.n += 1;
            if self.events.is_none() {
                self.mft.ProcessInput(0, &sample, 0)?;
                return self.drain();
            }
            // Asynchronous encoder: wait (briefly) until it wants a frame, hand it over, then wait for the answer so
            // the picture leaves in this call and not one frame later.
            let mut out = Vec::new();
            let limit = Instant::now() + Duration::from_millis(500);
            while self.need_input == 0 {
                if !self.pump(&mut out)? {
                    if Instant::now() > limit {
                        return Err(fail());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            self.need_input -= 1;
            self.mft.ProcessInput(0, &sample, 0)?;
            let limit = Instant::now() + Duration::from_millis(40);
            while out.is_empty() && Instant::now() < limit {
                if !self.pump(&mut out)? {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Ok(out)
        }
    }

    /// Handle one pending event of the asynchronous encoder. false = nothing was waiting.
    unsafe fn pump(&mut self, out: &mut Vec<Encoded>) -> Result<bool, Error> {
        let Some(events) = &self.events else { return Ok(false) };
        let ev = match events.GetEvent(MF_EVENT_FLAG_NO_WAIT) {
            Ok(e) => e,
            Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        let kind = ev.GetType()?;
        if kind == METransformNeedInput.0 as u32 {
            self.need_input += 1;
        } else if kind == METransformHaveOutput.0 as u32 {
            if let Some(u) = self.take_output()? {
                out.push(u);
            }
        }
        Ok(true)
    }

    /// One ProcessOutput call. None = no unit this time (need more input, or the stream format was renegotiated).
    unsafe fn take_output(&mut self) -> Result<Option<Encoded>, Error> {
        let sample = if self.provides_samples {
            None
        } else {
            let buf = MFCreateMemoryBuffer(self.out_size)?;
            let s = MFCreateSample()?;
            s.AddBuffer(&buf)?;
            Some(s)
        };
        let mut data = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: std::mem::ManuallyDrop::new(sample),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        }];
        let mut status = 0u32;
        let r = self.mft.ProcessOutput(0, &mut data, &mut status);
        let sample = std::mem::ManuallyDrop::take(&mut data[0].pSample);
        std::mem::ManuallyDrop::drop(&mut data[0].pEvents);
        match r {
            Ok(()) => {
                let s = sample.ok_or_else(fail)?;
                let b = s.ConvertToContiguousBuffer()?;
                let (mut p, mut len) = (std::ptr::null_mut(), 0u32);
                b.Lock(&mut p, None, Some(&mut len))?;
                let mut bytes = std::slice::from_raw_parts(p, len as usize).to_vec();
                b.Unlock()?;
                let keyframe = s.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
                // decoders joining at a keyframe need SPS/PPS in front of it
                if keyframe && !self.seq_header.is_empty() && !has_sps(&bytes) {
                    bytes.splice(0..0, self.seq_header.iter().copied());
                }
                Ok(Some(Encoded { data: bytes, keyframe }))
            }
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(None),
            Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                let t = self.mft.GetOutputAvailableType(0, 0)?;
                self.mft.SetOutputType(0, &t, 0)?;
                self.read_seq_header();
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    unsafe fn drain(&mut self) -> Result<Vec<Encoded>, Error> {
        let mut out = Vec::new();
        while let Some(u) = self.take_output()? {
            out.push(u);
        }
        Ok(out)
    }
}

/// True when an Annex-B buffer contains a sequence parameter set (NAL type 7).
fn has_sps(annexb: &[u8]) -> bool {
    annexb.windows(4).any(|w| w[..3] == [0, 0, 1] && w[3] & 0x1f == 7)
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = MFShutdown();
        }
    }
}

/// BT.601 limited-range conversion. Plain loops, split over a few threads in bands of rows: at 60 frames a
/// second one thread takes too large a share of the 16 ms a frame has.
pub fn bgra_to_nv12(bgra: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 3 / 2];
    let (y_plane, uv) = out.split_at_mut(w * h);
    let band = (h / 2).div_ceil(4).max(1) * 2; // rows per thread, even so each band owns whole chroma rows
    std::thread::scope(|s| {
        for (n, (yb, uvb)) in y_plane.chunks_mut(band * w).zip(uv.chunks_mut(band / 2 * w)).enumerate() {
            s.spawn(move || {
                let y0 = n * band;
                for (row, line) in yb.chunks_mut(w).enumerate() {
                    let src = &bgra[(y0 + row) * w * 4..];
                    for (x, o) in line.iter_mut().enumerate() {
                        let (b, g, r) = (src[x * 4] as i32, src[x * 4 + 1] as i32, src[x * 4 + 2] as i32);
                        *o = (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16) as u8;
                    }
                }
                for (row, line) in uvb.chunks_mut(w).enumerate() {
                    let y = y0 + row * 2;
                    for x in (0..w).step_by(2) {
                        let (mut rs, mut gs, mut bs) = (0, 0, 0);
                        for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let i = ((y + dy) * w + x + dx) * 4;
                            bs += bgra[i] as i32;
                            gs += bgra[i + 1] as i32;
                            rs += bgra[i + 2] as i32;
                        }
                        let (r, g, b) = (rs / 4, gs / 4, bs / 4);
                        line[x] = (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128) as u8;
                        line[x + 1] = (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128) as u8;
                    }
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv12_conversion_of_known_colours() {
        // 2x2 pure red, green, blue, white pixels in BGRA.
        let px = |b, g, r| [b, g, r, 255u8];
        let red: Vec<u8> = (0..4).flat_map(|_| px(0, 0, 255)).collect();
        let nv = bgra_to_nv12(&red, 2, 2);
        assert_eq!(&nv[..4], &[82, 82, 82, 82]); // Y of red
        assert_eq!(&nv[4..], &[90, 240]); // U, V of red
        let white: Vec<u8> = (0..4).flat_map(|_| px(255, 255, 255)).collect();
        assert_eq!(&bgra_to_nv12(&white, 2, 2)[..4], &[235; 4]);
        // several bands (threads): every row and chroma row is written, wherever the band edges fall
        for h in [2usize, 6, 10, 18] {
            let red: Vec<u8> = (0..4 * h).flat_map(|_| px(0, 0, 255)).collect();
            let nv = bgra_to_nv12(&red, 4, h);
            assert!(nv[..4 * h].iter().all(|&v| v == 82), "luma, h={h}");
            assert!(nv[4 * h..].chunks(2).all(|c| c == [90, 240]), "chroma, h={h}");
        }
    }

    #[test]
    fn rejects_odd_sizes_and_wrong_input() {
        assert!(matches!(H264Encoder::new(1281, 720, 30, 1_000_000, 30), Err(Error::BadSize)));
    }

    /// Runs once with whatever `new` picks (the graphics card when there is one) and once with the software encoder.
    #[test]
    fn encodes_a_moving_picture_to_annexb_h264() {
        let (w, h) = (1280usize, 720usize);
        let auto = H264Encoder::new(w as u32, h as u32, 30, 3_000_000, 30).expect("encoder");
        let cpu = H264Encoder::build(w as u32, h as u32, 30, 3_000_000, 30, false).expect("software encoder");
        assert_eq!(cpu.kind(), "CPU");
        for mut enc in [auto, cpu] {
            let mut units = Vec::new();
            for f in 0..45usize {
                let mut img = vec![255u8; w * h * 4];
                for y in 0..h {
                    for x in 0..w {
                        let i = (y * w + x) * 4;
                        img[i] = ((x + f * 8) % 256) as u8;
                        img[i + 1] = (y % 256) as u8;
                        img[i + 2] = (f * 5 % 256) as u8;
                    }
                }
                units.extend(enc.encode(&img, f == 0).expect("encode"));
            }
            assert!(units.len() >= 30, "only {} access units", units.len());
            let first = &units[0].data;
            assert!(first.starts_with(&[0, 0, 0, 1]) || first.starts_with(&[0, 0, 1]), "not Annex-B");
            let nal_types: Vec<u8> = first
                .windows(4)
                .enumerate()
                .filter(|(_, w)| w[..3] == [0, 0, 1])
                .map(|(i, _)| first[i + 3] & 0x1f)
                .collect();
            assert!(nal_types.contains(&7) && nal_types.contains(&8), "no SPS/PPS: {nal_types:?}");
            assert!(nal_types.contains(&5), "first unit is not an IDR: {nal_types:?}");
            let total: usize = units.iter().map(|u| u.data.len()).sum();
            println!("{}: {} units, {} bytes ({} kbit/s at 30 fps)", enc.kind(), units.len(), total, total * 8 * 30 / units.len() / 1000);
        }
    }
}

#[cfg(test)]
mod timing {
    use super::*;
    /// Not a pass/fail speed gate; prints the cost of one 1080p frame so regressions are visible.
    #[test]
    fn report_1080p_cost_per_frame() {
        let (w, h) = (1920usize, 1080usize);
        let img: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
        for hardware in [true, false] {
            let Ok(mut enc) = H264Encoder::build(w as u32, h as u32, 60, 8_000_000, 120, hardware) else {
                println!("1080p: no graphics-card encoder on this machine");
                continue;
            };
            let t = Instant::now();
            let n = 60;
            let mut out = 0;
            for f in 0..n {
                out += enc.encode(&img, f == 0).expect("encode").len();
            }
            let per = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
            println!("1080p {}: {per:.1} ms/frame (convert + encode), {out} units", enc.kind());
            assert!(out + 2 >= n, "{out} units for {n} frames");
        }
    }
}
