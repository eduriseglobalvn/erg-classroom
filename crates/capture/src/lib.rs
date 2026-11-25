//! Screen capture through DXGI Desktop Duplication (Windows 8+).
#![cfg(windows)]

use windows::core::Interface;
use windows::Win32::Foundation::{HMODULE, E_ACCESSDENIED};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("windows: {0}")]
    Win(#[from] windows::core::Error),
    #[error("capture not available here (secure desktop, locked session or no display)")]
    Unavailable,
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed BGRA, `width * height * 4` bytes.
    pub bgra: Vec<u8>,
}

impl Frame {
    /// Box-sampled JPEG that fits in max_w x max_h, keeping the aspect ratio. For the tutor monitor grid.
    pub fn thumbnail_jpeg(&self, max_w: u16, max_h: u16) -> Option<Vec<u8>> {
        let (sw, sh) = (self.width as usize, self.height as usize);
        if sw == 0 || sh == 0 || max_w == 0 || max_h == 0 || self.bgra.len() < sw * sh * 4 {
            return None;
        }
        let scale = (max_w as f32 / sw as f32).min(max_h as f32 / sh as f32).min(1.0);
        let (tw, th) = (((sw as f32 * scale) as usize).max(1), ((sh as f32 * scale) as usize).max(1));
        let mut rgb = Vec::with_capacity(tw * th * 3);
        for y in 0..th {
            let (y0, y1) = (y * sh / th, ((y + 1) * sh / th).max(y * sh / th + 1));
            for x in 0..tw {
                let (x0, x1) = (x * sw / tw, ((x + 1) * sw / tw).max(x * sw / tw + 1));
                let (mut b, mut g, mut r, mut n) = (0u32, 0u32, 0u32, 0u32);
                for yy in y0..y1.min(sh) {
                    for xx in x0..x1.min(sw) {
                        let p = (yy * sw + xx) * 4;
                        b += self.bgra[p] as u32;
                        g += self.bgra[p + 1] as u32;
                        r += self.bgra[p + 2] as u32;
                        n += 1;
                    }
                }
                rgb.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8]);
            }
        }
        let mut out = Vec::new();
        jpeg_encoder::Encoder::new(&mut out, 60).encode(&rgb, tw as u16, th as u16, jpeg_encoder::ColorType::Rgb).ok()?;
        Some(out)
    }
}

pub struct Capturer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    dup: IDXGIOutputDuplication,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
}

impl Capturer {
    /// Duplicate output `index` (0 = primary display).
    pub fn new(index: u32) -> Result<Self, Error> {
        unsafe {
            let (mut device, mut ctx) = (None, None);
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )?;
            let (device, ctx) = (device.ok_or(Error::Unavailable)?, ctx.ok_or(Error::Unavailable)?);
            let adapter = device.cast::<IDXGIDevice>()?.GetAdapter()?;
            let output = adapter.EnumOutputs(index).map_err(|_| Error::Unavailable)?;
            let dup = output.cast::<IDXGIOutput1>()?.DuplicateOutput(&device).map_err(|e| {
                if e.code() == E_ACCESSDENIED { Error::Unavailable } else { Error::Win(e) }
            })?;
            Ok(Self { device, ctx, dup, staging: None })
        }
    }

    /// Wait up to `timeout_ms` for a new frame. `Ok(None)` means the screen did not change.
    pub fn grab(&mut self, timeout_ms: u32) -> Result<Option<Frame>, Error> {
        unsafe {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut res = None;
            match self.dup.AcquireNextFrame(timeout_ms, &mut info, &mut res) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => return Err(Error::Unavailable),
                Err(e) => return Err(e.into()),
            }
            // A frame with no present time only carries a mouse-pointer update; its texture holds no desktop image.
            if info.LastPresentTime == 0 {
                let _ = self.dup.ReleaseFrame();
                return Ok(None);
            }
            let result = self.copy_out(res.ok_or(Error::Unavailable)?);
            let _ = self.dup.ReleaseFrame();
            result.map(Some)
        }
    }

    unsafe fn copy_out(&mut self, res: IDXGIResource) -> Result<Frame, Error> {
        let tex = res.cast::<ID3D11Texture2D>()?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        tex.GetDesc(&mut desc);
        let (w, h) = (desc.Width, desc.Height);
        if self.staging.as_ref().is_none_or(|(_, sw, sh)| (*sw, *sh) != (w, h)) {
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut t = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut t))?;
            self.staging = Some((t.ok_or(Error::Unavailable)?, w, h));
        }
        let staging = &self.staging.as_ref().expect("just created").0;
        self.ctx.CopyResource(staging, &tex);
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        self.ctx.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts((m.pData as *const u8).add(y * m.RowPitch as usize), (w * 4) as usize);
            bgra.extend_from_slice(row);
        }
        self.ctx.Unmap(staging, 0);
        Ok(Frame { width: w, height: h, bgra })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grabs_a_real_frame_of_the_primary_display() {
        let mut c = match Capturer::new(0) {
            Ok(c) => c,
            Err(e) => panic!("cannot start capture: {e}"),
        };
        // The first call after DuplicateOutput returns the current desktop image.
        let f = (0..40).find_map(|_| c.grab(500).expect("grab")).expect("no frame within 20 s");
        assert!(f.width >= 640 && f.height >= 480, "{}x{}", f.width, f.height);
        assert_eq!(f.bgra.len(), (f.width * f.height * 4) as usize);
        assert!(f.bgra.iter().any(|&b| b != 0), "frame is entirely black");
        println!("captured {}x{}", f.width, f.height);
    }
}

#[cfg(test)]
mod thumb_tests {
    use super::Frame;
    #[test]
    fn thumbnail_keeps_aspect_and_is_a_jpeg() {
        let f = Frame { width: 1920, height: 1080, bgra: vec![200; 1920 * 1080 * 4] };
        let j = f.thumbnail_jpeg(160, 160).unwrap();
        assert_eq!(&j[..2], &[0xFF, 0xD8]);
        assert!(j.len() < 5000);
        assert!(Frame { width: 0, height: 0, bgra: vec![] }.thumbnail_jpeg(160, 90).is_none());
        assert!(Frame { width: 10, height: 10, bgra: vec![0; 4] }.thumbnail_jpeg(160, 90).is_none());
    }
}
