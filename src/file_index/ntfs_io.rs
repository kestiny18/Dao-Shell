use anyhow::{Context, Result, ensure};

use std::{
    ffi::OsStr,
    fs::File,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    ptr,
};
use windows_sys::Win32::{
    Foundation::INVALID_HANDLE_VALUE,
    Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, GetVolumeInformationW,
        GetVolumeNameForVolumeMountPointW, OPEN_EXISTING,
    },
    System::{
        IO::DeviceIoControl,
        Ioctl::{
            FSCTL_ENUM_USN_DATA, FSCTL_QUERY_USN_JOURNAL, FSCTL_READ_USN_JOURNAL, MFT_ENUM_DATA_V0,
            READ_USN_JOURNAL_DATA_V0, USN_JOURNAL_DATA_V0,
        },
    },
};

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
pub fn mount(volume: &str) -> Result<String> {
    if let Some(id) = volume
        .strip_prefix(r"\\?\Volume{")
        .and_then(|v| v.strip_suffix("}\\"))
    {
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "invalid volume GUID");
        return Ok(volume.to_owned());
    }
    let b = volume.as_bytes();
    ensure!(
        b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':',
        "volume must be a drive letter, e.g. E:"
    );
    Ok(format!("{}\\", volume.to_ascii_uppercase()))
}
pub fn identity(volume: &str) -> Result<u32> {
    let path = wide(&mount(volume)?);
    let mut serial = 0;
    let mut fs = [0u16; 32];
    let ok = unsafe {
        GetVolumeInformationW(
            path.as_ptr(),
            ptr::null_mut(),
            0,
            &mut serial,
            ptr::null_mut(),
            ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    ensure!(
        ok != 0,
        "volume_information: {}",
        std::io::Error::last_os_error()
    );
    let fs = String::from_utf16_lossy(&fs[..fs.iter().position(|v| *v == 0).unwrap_or(fs.len())]);
    ensure!(fs == "NTFS", "unsupported filesystem {fs}");
    Ok(serial)
}
pub fn guid(volume: &str) -> Result<String> {
    if volume.starts_with(r"\\?\Volume{") {
        return mount(volume);
    }
    let path = wide(&mount(volume)?);
    let mut buffer = [0u16; 64];
    let ok = unsafe {
        GetVolumeNameForVolumeMountPointW(path.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32)
    };
    ensure!(ok != 0, "volume_guid: {}", std::io::Error::last_os_error());
    Ok(String::from_utf16(
        &buffer[..buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len())],
    )?)
}
pub fn open(volume: &str, access: u32) -> Result<File> {
    mount(volume)?;
    // Bind this handle to the resolved volume, not a drive letter that could be remounted.
    let path = wide(guid(volume)?.trim_end_matches('\\'));
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    ensure!(
        raw != INVALID_HANDLE_VALUE,
        "open_volume desired_access={access}: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { File::from_raw_handle(raw as _) })
}
pub fn ioctl<T>(file: &File, code: u32, input: Option<&T>) -> Result<Vec<u8>> {
    ioctl_buffer(file, code, input, 1024 * 1024)
}
pub fn file_record(file: &File) -> Result<Vec<u8>> {
    // A single NTFS component, not a full path; avoid allocating a volume page per file.
    ioctl_buffer::<u8>(
        file,
        windows_sys::Win32::System::Ioctl::FSCTL_READ_FILE_USN_DATA,
        None,
        4096,
    )
}
fn ioctl_buffer<T>(file: &File, code: u32, input: Option<&T>, size: usize) -> Result<Vec<u8>> {
    let mut output = vec![0u8; size];
    let mut returned = 0;
    let (p, n) = input
        .map(|v| ((v as *const T).cast(), size_of::<T>() as u32))
        .unwrap_or((ptr::null(), 0));
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as _,
            code,
            p,
            n,
            output.as_mut_ptr().cast(),
            output.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    output.truncate(returned as usize);
    Ok(output)
}
pub fn journal(file: &File) -> Result<USN_JOURNAL_DATA_V0> {
    let bytes = ioctl::<u8>(file, FSCTL_QUERY_USN_JOURNAL, None).context("query_journal")?;
    ensure!(
        bytes.len() >= size_of::<USN_JOURNAL_DATA_V0>(),
        "short journal structure"
    );
    Ok(unsafe { ptr::read_unaligned(bytes.as_ptr().cast()) })
}
pub fn enumeration(file: &File, start: u64) -> Result<Vec<u8>> {
    ioctl(
        file,
        FSCTL_ENUM_USN_DATA,
        Some(&MFT_ENUM_DATA_V0 {
            StartFileReferenceNumber: start,
            LowUsn: 0,
            HighUsn: i64::MAX,
        }),
    )
}
pub fn read(file: &File, id: u64, start: i64) -> Result<Vec<u8>> {
    ioctl(
        file,
        FSCTL_READ_USN_JOURNAL,
        Some(&READ_USN_JOURNAL_DATA_V0 {
            StartUsn: start,
            ReasonMask: u32::MAX,
            ReturnOnlyOnClose: 0,
            Timeout: 0,
            BytesToWaitFor: 0,
            UsnJournalID: id,
        }),
    )
}
