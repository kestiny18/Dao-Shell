//! Bounded native volume discovery. No caller-supplied volume or database path.
use super::super::security::{current, wide};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::ptr;
use windows_sys::Win32::{
    Foundation::*, Storage::FileSystem::*, System::WindowsProgramming::DRIVE_FIXED,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volume {
    pub guid: String,
    pub serial: Option<u32>,
    pub mounts: Vec<String>,
    pub supported: bool,
    pub reason: Option<String>,
    pub pipe: Option<String>,
    pub worker_started: bool,
    pub worker_error: Option<String>,
}
pub(super) fn key(guid: &str, serial: u32) -> Result<String> {
    let id = guid
        .strip_prefix(r"\\?\Volume{")
        .and_then(|v| v.strip_suffix("}\\"))
        .ok_or_else(|| anyhow::anyhow!("invalid discovered volume"))?;
    Ok(format!(
        "{}-{serial:08x}",
        uuid::Uuid::parse_str(id)?.simple()
    ))
}
pub(super) fn valid_key(key: &str) -> bool {
    key.is_ascii()
        && key.len() == 41
        && key.as_bytes()[32] == b'-'
        && key[..32]
            .bytes()
            .chain(key[33..].bytes())
            .all(|c| c.is_ascii_hexdigit())
}
fn text(value: &[u16]) -> Result<String> {
    Ok(String::from_utf16(
        &value[..value.iter().position(|c| *c == 0).unwrap_or(value.len())],
    )?)
}
fn describe(guid: String) -> Result<Volume> {
    // A GUID returned by the OS must still satisfy the same strict input grammar.
    super::super::super::ntfs_io::mount(&guid)?;
    let mut volume = Volume {
        guid,
        serial: None,
        mounts: Vec::new(),
        supported: false,
        reason: None,
        pipe: None,
        worker_started: false,
        worker_error: None,
    };
    let root = wide(&volume.guid);
    if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
        volume.reason = Some("not_fixed_volume".into());
        return Ok(volume);
    }
    let mut serial = 0;
    let mut fs = [0u16; 32];
    if unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            ptr::null_mut(),
            0,
            &mut serial,
            ptr::null_mut(),
            ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    } == 0
    {
        volume.reason = Some("volume_unavailable".into());
        return Ok(volume);
    }
    volume.serial = Some(serial);
    if text(&fs)? != "NTFS" {
        volume.reason = Some("unsupported_filesystem".into());
        return Ok(volume);
    }
    let mut paths = vec![0u16; 32768];
    let mut needed = 0;
    if unsafe {
        GetVolumePathNamesForVolumeNameW(
            root.as_ptr(),
            paths.as_mut_ptr(),
            paths.len() as u32,
            &mut needed,
        )
    } != 0
    {
        for part in paths
            .split(|c| *c == 0)
            .take_while(|part| !part.is_empty())
            .take(32)
        {
            let path = String::from_utf16(part)?;
            // Existing desktop actions support local drive paths only.
            if path.len() == 3 && path.as_bytes()[0].is_ascii_alphabetic() && path.ends_with(":\\")
            {
                volume.mounts.push(path);
            }
        }
    }
    volume.mounts.sort();
    volume.supported = true;
    volume.pipe = Some(format!("volume-{}", key(&volume.guid, serial)?));
    Ok(volume)
}
pub(super) fn discover() -> Result<Vec<Volume>> {
    let mut buffer = [0u16; 64];
    let handle = unsafe { FindFirstVolumeW(buffer.as_mut_ptr(), buffer.len() as u32) };
    ensure!(handle != INVALID_HANDLE_VALUE, "volume discovery failed");
    struct Find(HANDLE);
    impl Drop for Find {
        fn drop(&mut self) {
            unsafe {
                FindVolumeClose(self.0);
            }
        }
    }
    let _find = Find(handle);
    let mut volumes = Vec::new();
    loop {
        ensure!(volumes.len() < 128, "volume discovery limit exceeded");
        volumes.push(describe(text(&buffer)?)?);
        buffer.fill(0);
        if unsafe { FindNextVolumeW(handle, buffer.as_mut_ptr(), buffer.len() as u32) } == 0 {
            ensure!(
                unsafe { GetLastError() } == ERROR_NO_MORE_FILES,
                "volume discovery interrupted"
            );
            break;
        }
    }
    volumes.sort_by(|a, b| a.guid.cmp(&b.guid));
    let mut enabled = 0;
    for volume in &mut volumes {
        if volume.supported {
            enabled += 1;
            if enabled > 16 {
                volume.supported = false;
                volume.pipe = None;
                volume.reason = Some("volume_worker_limit".into());
            }
        }
    }
    Ok(volumes)
}
pub fn local_discovery() -> Result<Vec<Volume>> {
    // Discovery itself needs no elevation and does not open raw volume handles.
    let _ = current()?;
    discover()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_keys_bind_guid_and_serial() {
        let guid = r"\\?\Volume{46ebe9ac-18bb-43fb-ae5a-511a8ad2f41a}\";
        let first = key(guid, 1).unwrap();
        assert!(valid_key(&first));
        assert_ne!(first, key(guid, 2).unwrap());
        assert!(!valid_key("../../escape"));
        assert!(key("E:", 1).is_err());
    }
}
