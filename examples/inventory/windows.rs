use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    fs::File,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, GetVolumeInformationW,
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW, OPEN_EXISTING,
    },
    System::{
        IO::DeviceIoControl,
        Ioctl::{FSCTL_QUERY_USN_JOURNAL, USN_JOURNAL_DATA_V0},
    },
};

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn string(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}

/// Read-only volume metadata probe. Never requests elevation or creates a journal.
pub fn probe(root: &Path) -> Result<Value> {
    let root = dao_shell::files::normalize_existing(root)?;
    ensure!(root.is_dir(), "root 必须是目录");
    let path = wide(root.as_os_str());
    let mut mount = vec![0u16; 32768];
    ensure!(
        unsafe { GetVolumePathNameW(path.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } != 0,
        "无法查询卷：{}",
        std::io::Error::last_os_error()
    );
    let mut filesystem = [0u16; 64];
    let mut serial = 0;
    ensure!(
        unsafe {
            GetVolumeInformationW(
                mount.as_ptr(),
                ptr::null_mut(),
                0,
                &mut serial,
                ptr::null_mut(),
                ptr::null_mut(),
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        } != 0,
        "无法查询文件系统：{}",
        std::io::Error::last_os_error()
    );
    let mut result = json!({ "root": root, "mount": string(&mount), "filesystem": string(&filesystem), "volume_serial": serial, "mode": "read_only_no_elevation", "incremental_index": "not_implemented" });
    if string(&filesystem) != "NTFS" {
        result["usn"] = json!({"status": "unsupported_by_this_spike"});
        return Ok(result);
    }
    let mut volume = [0u16; 64];
    ensure!(
        unsafe {
            GetVolumeNameForVolumeMountPointW(
                mount.as_ptr(),
                volume.as_mut_ptr(),
                volume.len() as u32,
            )
        } != 0,
        "无法查询卷名称：{}",
        std::io::Error::last_os_error()
    );
    let device = wide(OsStr::new(string(&volume).trim_end_matches('\\')));
    // SAFETY: NUL-terminated paths, no output pointers, synchronous owned handle.
    let raw = unsafe {
        CreateFileW(
            device.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        result["usn"] = error("open_volume");
        return Ok(result);
    }
    // SAFETY: CreateFileW returned a unique valid owned handle; File closes it on every exit.
    let file = unsafe { File::from_raw_handle(raw as _) };
    let mut journal = USN_JOURNAL_DATA_V0::default();
    let mut returned = 0;
    // SAFETY: output points to a correctly sized/aligned initialized structure; synchronous call.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as _,
            FSCTL_QUERY_USN_JOURNAL,
            ptr::null(),
            0,
            (&mut journal as *mut USN_JOURNAL_DATA_V0).cast(),
            size_of::<USN_JOURNAL_DATA_V0>() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    result["usn"] = if ok == 0 {
        error("query_journal")
    } else {
        ensure!(
            returned as usize >= size_of::<USN_JOURNAL_DATA_V0>(),
            "USN 日志返回结构不完整"
        );
        json!({"status": "available", "journal_id": journal.UsnJournalID.to_string(), "first_usn": journal.FirstUsn, "next_usn": journal.NextUsn, "lowest_valid_usn": journal.LowestValidUsn,
            "note": "metadata probe only; event reading and replay are not verified"})
    };
    Ok(result)
}

fn error(stage: &str) -> Value {
    let error = std::io::Error::last_os_error();
    let status = match error.raw_os_error() {
        Some(5) => "access_denied",
        Some(1179) => "journal_not_active",
        Some(1178) => "journal_delete_in_progress",
        _ => "unavailable",
    };
    json!({"status": status, "stage": stage, "win32_error": error.raw_os_error(), "message": error.to_string()})
}
