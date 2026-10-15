//! Process executable icon -> base64 PNG (Windows). Non-Windows returns None.

#[cfg(not(windows))]
pub fn exe_icon_png_base64(_path: &str) -> Option<String> {
    None
}

#[cfg(windows)]
pub fn exe_icon_png_base64(path: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let (w, h, rgba) = win::icon_rgba(path)?;
    Some(STANDARD.encode(png_encode(w, h, &rgba)))
}

#[cfg(windows)]
mod win {
    use windows::core::HSTRING;
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    /// Returns (width, height, RGBA pixels) of the large icon for `path`.
    pub fn icon_rgba(path: &str) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            let mut info: SHFILEINFOW = std::mem::zeroed();
            let h = HSTRING::from(path);
            let ok = SHGetFileInfoW(&h, Default::default(), Some(&mut info as *mut _), std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
            if ok == 0 || info.hIcon.is_invalid() {
                return None;
            }
            let hicon = info.hIcon;
            let result = read_icon(hicon);
            let _ = DestroyIcon(hicon);
            result
        }
    }

    unsafe fn read_icon(hicon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<(u32, u32, Vec<u8>)> {
        let mut ii: ICONINFO = std::mem::zeroed();
        GetIconInfo(hicon, &mut ii).ok()?;
        let color = ii.hbmColor;
        let mask = ii.hbmMask;
        let result = color_pixels(color);
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        result
    }

    unsafe fn color_pixels(hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(HGDIOBJ(hbm.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _)) == 0 {
            return None;
        }
        let (w, h) = (bm.bmWidth as u32, bm.bmHeight as u32);
        if w == 0 || h == 0 {
            return None;
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        };
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(None);
        let lines = GetDIBits(hdc, hbm, 0, h, Some(bgra.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);
        ReleaseDC(None, hdc);
        if lines == 0 {
            return None;
        }
        let mut rgba = Vec::with_capacity(bgra.len());
        for px in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        // ponytail: icons without an alpha channel (alpha all 0) are forced opaque;
        // upgrade: derive alpha from the hbmMask bitmap for exact transparency.
        if rgba.chunks_exact(4).all(|p| p[3] == 0) {
            rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        }
        Some((w, h, rgba))
    }
}

/// Minimal PNG encoder (RGBA8, stored-deflate zlib). No external crate.
#[cfg(windows)]
fn png_encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    // Scanlines with filter byte 0.
    let stride = (w * 4) as usize;
    let mut raw = Vec::with_capacity((stride + 1) * h as usize);
    for row in rgba.chunks_exact(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }

    // zlib stream: header + stored blocks + adler32.
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(if i + 1 == blocks.len() { 1 } else { 0 });
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn png_encode_has_signature_and_iend() {
        let png = super::png_encode(1, 1, &[1, 2, 3, 255]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']);
        // IEND chunk: length 0, type "IEND", CRC AE 42 60 82.
        assert_eq!(&png[png.len() - 8..], &[b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn extracts_notepad_icon_as_png() {
        let b64 = super::exe_icon_png_base64(r"C:\Windows\System32\notepad.exe").expect("icon");
        // base64 of the PNG signature starts with "iVBORw0KGgo".
        assert!(b64.starts_with("iVBORw0KGgo"));
    }
}
