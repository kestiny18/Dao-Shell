//! Bounded local resource extraction for application thumbnails; never executes a file.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{os::windows::fs::MetadataExt, path::PathBuf, ptr};
use windows_sys::Win32::{
    Graphics::Gdi::*,
    Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, GetDriveTypeW},
    System::WindowsProgramming::DRIVE_FIXED,
    UI::{Shell::ExtractIconExW, WindowsAndMessaging::*},
};

fn source(value: &str) -> Option<(PathBuf, i32)> {
    let value = value.trim();
    let (path, index) = match value.rsplit_once(',') {
        Some((path, suffix)) if suffix.trim().parse::<i32>().is_ok() => {
            (path, suffix.trim().parse().ok()?)
        }
        _ => (value, 0),
    };
    let path = path.trim().trim_matches('"');
    let bytes = path.as_bytes();
    // No network locations, shell strings, device paths, relative paths or ADS.
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || path[2..].contains(':')
        || path.contains('\0')
    {
        return None;
    }
    let root: Vec<u16> = format!("{}:\\", bytes[0] as char)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
        return None;
    }
    let path = PathBuf::from(path);
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !["exe", "dll", "ico"].contains(&extension.as_str()) {
        return None;
    }
    Some((path, index))
}

struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        unsafe {
            DestroyIcon(self.0);
        }
    }
}
struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

pub(super) fn load(value: &str) -> Option<String> {
    let (path, index) = source(value)?;
    // Do not follow junctions/symlinks that could redirect a local-looking path
    // to a network location. A fallback tile is preferable to network I/O here.
    for component in path.ancestors() {
        if component.symlink_metadata().ok()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return None;
        }
    }
    let canonical = path.canonicalize().ok()?;
    let canonical = canonical.to_string_lossy();
    let local = canonical.strip_prefix("\\\\?\\").unwrap_or(&canonical);
    // Recheck after following links, including mapped/network destinations.
    let (path, _) = source(local)?;
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 * 1024 {
        return None;
    }
    let wide: Vec<_> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut raw = ptr::null_mut();
    unsafe {
        ExtractIconExW(wide.as_ptr(), index, &mut raw, ptr::null_mut(), 1);
    }
    if raw.is_null() {
        return None;
    }
    let icon = Icon(raw);
    // Composite onto a small opaque tile; this also handles legacy mask-only icons.
    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = 32;
    info.bmiHeader.biHeight = -32;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    let mut pixels = ptr::null_mut();
    let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
    if dc.is_null() {
        return None;
    }
    let bitmap =
        unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, ptr::null_mut(), 0) };
    if bitmap.is_null() {
        unsafe {
            DeleteDC(dc);
        }
        return None;
    }
    let previous = unsafe { SelectObject(dc, bitmap) };
    let _canvas = Canvas {
        dc,
        bitmap,
        previous,
    };
    if pixels.is_null() {
        return None;
    }
    unsafe {
        ptr::write_bytes(pixels.cast::<u8>(), 255, 32 * 32 * 4);
    }
    if unsafe { DrawIconEx(dc, 0, 0, icon.0, 32, 32, 0, ptr::null_mut(), DI_NORMAL) } == 0 {
        return None;
    }
    unsafe {
        GdiFlush();
    }
    let data = unsafe { std::slice::from_raw_parts_mut(pixels.cast::<u8>(), 32 * 32 * 4) };
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 32, 32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(data).ok()?;
    }
    Some(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nonlocal_and_non_resource_sources() {
        for value in [
            r"\\server\share\icon.ico",
            r"\\?\C:\icon.ico",
            "https://example.com/icon.ico",
            "relative.exe",
            r"C:\tool.exe:secret",
            r"C:\script.ps1",
            "C:\\bad\0.ico",
        ] {
            assert!(source(value).is_none(), "{value:?}");
        }
    }
    #[test]
    fn extracts_packaged_icon_and_handles_missing_file() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("desktop/src-tauri/icons/icon.ico");
        let image = load(&format!("\"{}\",0", path.display())).expect("fixture icon");
        let data = STANDARD
            .decode(image.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
        assert!(load(r"C:\dao-shell-nonexistent-icon.ico,0").is_none());
    }
}
