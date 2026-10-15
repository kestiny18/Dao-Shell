//! Experiment-only storage. Never adopt or change permissions on an existing directory.
use super::security::{self, Descriptor, owned, raw, wide};
use anyhow::{Result, ensure};
use std::{
    os::windows::io::OwnedHandle,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Com::CoTaskMemFree, Threading::*},
    UI::Shell::*,
};

pub fn location() -> Result<PathBuf> {
    let mut value = ptr::null_mut();
    ensure!(
        unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramData, 0, ptr::null_mut(), &mut value) } >= 0,
        "ProgramData unavailable"
    );
    let result = unsafe {
        let mut len = 0;
        while *value.add(len) != 0 {
            len += 1;
        }
        let result = String::from_utf16(std::slice::from_raw_parts(value, len));
        CoTaskMemFree(value.cast());
        result?
    };
    Ok(PathBuf::from(result).join("DaoShellIndexExperiment"))
}
fn sid_text(sid: PSID) -> Result<String> {
    let mut value = ptr::null_mut();
    ensure!(
        unsafe { ConvertSidToStringSidW(sid, &mut value) } != 0,
        "storage SID invalid"
    );
    unsafe {
        let mut len = 0;
        while *value.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16(std::slice::from_raw_parts(value, len));
        LocalFree(value.cast());
        Ok(text?)
    }
}
fn validate_acl(path: &Path, directory: bool) -> Result<()> {
    validate_acl_reader(path, directory, None)
}
pub(super) fn validate_acl_reader(
    path: &Path,
    directory: bool,
    reader: Option<&str>,
) -> Result<()> {
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    ensure!(
        unsafe {
            GetNamedSecurityInfoW(
                wide(&path.to_string_lossy()).as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut acl,
                ptr::null_mut(),
                &mut descriptor,
            )
        } == 0,
        "storage ACL unreadable"
    );
    let _descriptor = Descriptor(descriptor);
    ensure!(
        matches!(sid_text(owner)?.as_str(), "S-1-5-18" | "S-1-5-32-544"),
        "storage owner is not trusted"
    );
    ensure!(!acl.is_null(), "storage has null DACL");
    let mut control = 0;
    let mut revision = 0;
    ensure!(
        unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } != 0,
        "storage descriptor invalid"
    );
    if directory {
        ensure!(
            control & SE_DACL_PROTECTED != 0,
            "storage DACL is not protected"
        );
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    ensure!(
        unsafe {
            GetAclInformation(
                acl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } != 0
            && info.AceCount == 2 + u32::from(reader.is_some()),
        "unexpected storage ACL"
    );
    let mut identities = Vec::new();
    for index in 0..info.AceCount {
        let mut ace = ptr::null_mut();
        ensure!(
            unsafe { GetAce(acl, index, &mut ace) } != 0,
            "storage ACE invalid"
        );
        let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        let identity = sid_text((&ace.SidStart as *const u32).cast_mut().cast())?;
        let expected = if reader == Some(identity.as_str()) {
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
        } else {
            FILE_ALL_ACCESS
        };
        ensure!(
            ace.Header.AceType == 0
                && ace.Mask == expected
                && ace.Header.AceFlags & INHERIT_ONLY_ACE as u8 == 0,
            "unexpected storage rights"
        );
        identities.push(identity);
    }
    identities.sort();
    let mut expected = vec!["S-1-5-18", "S-1-5-32-544"];
    if let Some(reader) = reader {
        expected.push(reader);
    }
    expected.sort();
    ensure!(identities == expected, "unexpected storage principal");
    Ok(())
}
pub(super) fn hold(path: &Path, directory: bool) -> Result<OwnedHandle> {
    let handle = owned(unsafe {
        CreateFileW(
            wide(&path.to_string_lossy()).as_ptr(),
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    })?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    ensure!(
        unsafe { GetFileInformationByHandle(raw(&handle), &mut info) } != 0,
        "storage identity unavailable"
    );
    ensure!(
        info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
            && (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) == directory,
        "unsafe storage object"
    );
    if !directory {
        ensure!(
            info.nNumberOfLinks == 1,
            "storage file has additional names"
        );
    }
    Ok(handle)
}
fn create_directory(path: &Path) -> Result<()> {
    create_directory_reader(path, None)
}
pub(super) fn create_directory_reader(path: &Path, reader: Option<&str>) -> Result<()> {
    let mut sddl = "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)".to_owned();
    if let Some(reader) = reader {
        ensure!(
            reader.starts_with("S-1-")
                && reader
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S'),
            "invalid reader SID"
        );
        sddl.push_str(&format!("(A;OICI;0x1200a9;;;{reader})"));
    }
    let mut descriptor = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(&sddl).as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } != 0,
        "storage ACL creation failed"
    );
    let _descriptor = Descriptor(descriptor);
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let created = unsafe { CreateDirectoryW(wide(&path.to_string_lossy()).as_ptr(), &attrs) } != 0;
    ensure!(
        created || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS,
        "protected storage creation failed"
    );
    Ok(())
}
pub(super) fn prepare(database: &Path) -> Result<Vec<OwnedHandle>> {
    ensure!(
        security::current()?.elevated,
        "volume worker requires elevation"
    );
    let root = location()?;
    let parent = database
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid storage path"))?;
    ensure!(
        database.file_name().is_some_and(|v| v == "index.db")
            && parent.parent() == Some(root.as_path()),
        "volume database must be <known ProgramData>/DaoShellIndexExperiment/<experiment>/index.db"
    );
    let slug = parent.file_name().and_then(|v| v.to_str()).unwrap_or("");
    ensure!(
        !slug.is_empty()
            && slug.len() <= 64
            && slug.bytes().all(|v| v.is_ascii_alphanumeric() || v == b'-'),
        "invalid experiment name"
    );
    let mut held = Vec::new();
    let mut ancestors = root.parent().unwrap().ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        held.push(hold(ancestor, true)?);
    }
    for directory in [&root, &parent.to_path_buf()] {
        create_directory(directory)?;
        held.push(hold(directory, true)?);
        validate_acl(directory, true)?;
    }
    finish_storage(parent, held)
}
pub(super) fn finish_storage(
    parent: &Path,
    mut held: Vec<OwnedHandle>,
) -> Result<Vec<OwnedHandle>> {
    // Make SQLite-created files owned by Administrators, not the split-token user.
    // Otherwise the ordinary owner could rewrite their DACL despite directory protection.
    let mut token = ptr::null_mut();
    ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_DEFAULT, &mut token) } != 0,
        "storage token unavailable"
    );
    let token = owned(token)?;
    let mut owner = ptr::null_mut();
    ensure!(
        unsafe { ConvertStringSidToSidW(wide("S-1-5-32-544").as_ptr(), &mut owner) } != 0,
        "storage owner unavailable"
    );
    let owner_info = TOKEN_OWNER { Owner: owner };
    let changed = unsafe {
        SetTokenInformation(
            raw(&token),
            TokenOwner,
            (&owner_info as *const TOKEN_OWNER).cast(),
            size_of::<TOKEN_OWNER>() as u32,
        )
    };
    unsafe {
        LocalFree(owner);
    }
    ensure!(changed != 0, "storage token owner could not be secured");
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        ensure!(
            [
                "index.db",
                "index.db-wal",
                "index.db-shm",
                "index.db-journal",
                "worker-diagnostics.jsonl"
            ]
            .iter()
            .any(|name| entry.file_name() == *name),
            "unknown protected storage entry"
        );
        let guard = hold(&entry.path(), false)?;
        validate_acl(&entry.path(), false)?;
        // SQLite may replace WAL/SHM files. Keep only the main DB bound for the worker lifetime.
        if entry.file_name() == "index.db" {
            held.push(guard);
        }
    }
    Ok(held)
}
pub(super) fn validate_database(database: &Path) -> Result<OwnedHandle> {
    let guard = hold(database, false)?;
    validate_acl(database, false)?;
    Ok(guard)
}

fn sanitized_diagnostic(value: &serde_json::Value) -> serde_json::Value {
    let error = value.get("error").and_then(|v| v.as_str()).unwrap_or("");
    let stages = [
        "retry_required: link count changed during reconciliation",
        "retry_required: final path identity changed",
        "retry_required: incomplete link set",
        "retry_required: name path disappeared during reconciliation",
        "FindFirstFileNameW",
        "FindNextFileNameW",
        "open hard-link parent",
        "OpenFileById",
        "metadata: before",
        "metadata: after",
        "metadata: delete-pending state unavailable",
        "FSCTL_READ_FILE_USN_DATA",
        "parse file USN",
        "targeted MFT deletion check",
        "GetFinalPathNameByHandleW single name",
        "open final-path parent",
        "reopen final path",
        "CreateFileW metadata path",
        "GetFileInformationByHandle",
        "query_journal",
        "journal",
        "USN",
        "budget_exhausted",
        "cancelled",
        "database",
        "source identity mismatch",
        "file identity mismatch",
        "volume",
        "invalid name",
        "invalid link",
        "unsupported link count",
        "final path",
        "link enumeration",
    ];
    let stage = if error.is_empty() {
        None
    } else {
        Some(
            stages
                .iter()
                .copied()
                .find(|stage| error.contains(stage))
                .unwrap_or("unclassified_error"),
        )
    };
    let win32 = error
        .split("os error ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse::<u32>().ok());
    let mut output = serde_json::Map::new();
    for key in ["rebuild", "elapsed_ms", "generation", "complete"] {
        if let Some(v) = value.get(key)
            && (v.is_number() || v.is_boolean() || v.is_null())
        {
            output.insert(key.into(), v.clone());
        }
    }
    let mut metrics = serde_json::Map::new();
    for key in [
        "enum_pages",
        "enumerated",
        "metadata_lookups",
        "metadata_micros",
        "unavailable",
        "deleted",
        "multi_name_files",
        "journal_records",
        "file_record_fallbacks",
    ] {
        if let Some(v) = value.get("metrics").and_then(|m| m.get(key))
            && v.is_number()
        {
            metrics.insert(key.into(), v.clone());
        }
    }
    output.insert("metrics".into(), metrics.into());
    output.insert("error_stage".into(), serde_json::json!(stage));
    output.insert("win32_code".into(), serde_json::json!(win32));
    output.into()
}
pub fn diagnostics(experiment: &str) -> Result<serde_json::Value> {
    use std::io::Read;
    ensure!(
        security::current()?.elevated,
        "diagnostics requires elevation"
    );
    ensure!(
        !experiment.is_empty()
            && experiment.len() <= 64
            && experiment
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-'),
        "invalid experiment name"
    );
    let root = location()?;
    let directory = root.join(experiment);
    let mut held = Vec::new();
    let mut ancestors = directory.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        held.push(hold(ancestor, true)?);
    }
    validate_acl(&root, true)?;
    validate_acl(&directory, true)?;
    let path = directory.join("worker-diagnostics.jsonl");
    held.push(hold(&path, false)?);
    validate_acl(&path, false)?;
    let file = std::fs::File::open(path)?;
    let total = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut bytes)?;
    let mut records = Vec::new();
    let mut incomplete = false;
    for line in bytes
        .split(|c| *c == b'\n')
        .filter(|line| !line.is_empty())
        .take(1024)
    {
        match serde_json::from_slice::<serde_json::Value>(line) {
            Ok(value) => records.push(sanitized_diagnostic(&value)),
            Err(_) => incomplete = true,
        }
    }
    Ok(
        serde_json::json!({"records":records,"truncated":total>1024*1024,"incomplete_record":incomplete}),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagnostic_export_never_reflects_names_or_unrecognized_fields() {
        let output = super::sanitized_diagnostic(
            &serde_json::json!({"error":"open final-path parent: secret-path (os error 2)","generation":2,"metrics":{"metadata_lookups":3,"filename":"secret-path"},"elapsed_ms":"secret-path","name":"secret-path"}),
        );
        assert!(!output.to_string().contains("secret-path"));
        assert_eq!(output["win32_code"], 2);
        assert_eq!(output["error_stage"], "open final-path parent");
        assert_eq!(output["metrics"]["metadata_lookups"], 3);
    }
}
