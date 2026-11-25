//! Write BGRA frames to an H.264 `.mp4` through the Media Foundation sink writer (Windows only).
#![cfg(windows)]
use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("windows: {0}")]
    Win(#[from] windows::core::Error),
    #[error("width and height must be even and non-zero")]
    BadSize,
    #[error("frame must be width*height*4 bytes (got {got}, want {want})")]
    BadFrame { got: usize, want: usize },
}

pub struct Mp4Writer {
    writer: IMFSinkWriter,
    w: usize,
    h: usize,
    dur: i64,
    n: i64,
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

impl Mp4Writer {
    /// `fps` is the playback rate of the pushed frames (a slow capture can be played back faster).
    pub fn create(path: &std::path::Path, w: u32, h: u32, fps: u32, bitrate: u32) -> Result<Self, Error> {
        if w == 0 || h == 0 || !w.is_multiple_of(2) || !h.is_multiple_of(2) || fps == 0 {
            return Err(Error::BadSize);
        }
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, None)?;
            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
            out.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
            out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
            let stream = writer.AddStream(&out)?;
            let inp = MFCreateMediaType()?;
            inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            inp.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            inp.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            inp.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
            inp.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
            inp.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
            writer.SetInputMediaType(stream, &inp, None)?;
            writer.BeginWriting()?;
            Ok(Self { writer, w: w as usize, h: h as usize, dur: 10_000_000 / fps as i64, n: 0 })
        }
    }

    pub fn push(&mut self, bgra: &[u8]) -> Result<(), Error> {
        let want = self.w * self.h * 4;
        if bgra.len() != want {
            return Err(Error::BadFrame { got: bgra.len(), want });
        }
        let nv12 = erg_encode::bgra_to_nv12(bgra, self.w, self.h);
        unsafe {
            let buf = MFCreateMemoryBuffer(nv12.len() as u32)?;
            let mut p = std::ptr::null_mut();
            buf.Lock(&mut p, None, None)?;
            std::ptr::copy_nonoverlapping(nv12.as_ptr(), p, nv12.len());
            buf.Unlock()?;
            buf.SetCurrentLength(nv12.len() as u32)?;
            let s = MFCreateSample()?;
            s.AddBuffer(&buf)?;
            s.SetSampleTime(self.n * self.dur)?;
            s.SetSampleDuration(self.dur)?;
            self.n += 1;
            self.writer.WriteSample(0, &s)?;
        }
        Ok(())
    }

    /// Flush and close the file. Must be called, or the mp4 is unreadable.
    pub fn finish(self) -> Result<(), Error> {
        unsafe {
            self.writer.Finalize()?;
            let _ = MFShutdown();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_readable_mp4() {
        let (w, h) = (320u32, 240u32);
        let path = std::env::temp_dir().join("erg-mp4-test.mp4");
        let _ = std::fs::remove_file(&path);
        let mut m = Mp4Writer::create(&path, w, h, 10, 1_000_000).unwrap();
        for i in 0..20u8 {
            let mut f = vec![0u8; (w * h * 4) as usize];
            for px in f.chunks_exact_mut(4) {
                px.copy_from_slice(&[i.wrapping_mul(10), 80, 200, 255]);
            }
            m.push(&f).unwrap();
        }
        assert!(matches!(m.push(&[0; 4]), Err(Error::BadFrame { .. })));
        m.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 2000, "tiny file: {}", bytes.len());
        assert_eq!(&bytes[4..8], b"ftyp");
        assert!(bytes.windows(4).any(|x| x == b"moov"), "no moov box: file not finalized");
        // Media Foundation can open it again and sees the right size
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
            let r = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None).unwrap();
            let t = r.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32).unwrap();
            assert_eq!(t.GetUINT64(&MF_MT_FRAME_SIZE).unwrap(), pack(w, h));
        }
    }

    #[test]
    fn odd_sizes_are_refused() {
        assert!(matches!(Mp4Writer::create(std::path::Path::new("x.mp4"), 321, 240, 10, 1), Err(Error::BadSize)));
    }
}
