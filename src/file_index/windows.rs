use super::{ntfs_io as io, usn, *};
use anyhow::Context;
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::File,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    ptr,
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
};
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
fn handle_guid(file: &File) -> Result<String> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as _,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            VOLUME_NAME_GUID,
        )
    };
    ensure!(
        length > 0 && (length as usize) < buffer.len(),
        "handle volume GUID: {}",
        std::io::Error::last_os_error()
    );
    let path = String::from_utf16(&buffer[..length as usize])?;
    let end = path.find("}\\").context("handle has no volume GUID")? + 2;
    let guid = &path[..end];
    io::mount(guid)?;
    Ok(guid.into())
}
fn key(id: u64) -> String {
    format!("{id:016x}")
}
fn file_id(info: &BY_HANDLE_FILE_INFORMATION) -> u64 {
    ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64
}
fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut data = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut data) } == 0 {
        return Err(anyhow::Error::new(std::io::Error::last_os_error())
            .context("GetFileInformationByHandle"));
    }
    Ok(data)
}
fn open_path(path: &str) -> Result<File> {
    let raw = unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(anyhow::Error::new(std::io::Error::last_os_error())
            .context("CreateFileW metadata path"));
    }
    Ok(unsafe { File::from_raw_handle(raw as _) })
}
#[derive(Serialize, Deserialize)]
struct Position {
    identity: VolumeIdentity,
    journal: u64,
    next: i64,
}
#[derive(Default, Serialize)]
pub struct Metrics {
    pub enum_pages: usize,
    pub enumerated: usize,
    pub metadata_lookups: usize,
    pub metadata_micros: u128,
    pub unavailable: usize,
    pub deleted: usize,
    pub multi_name_files: usize,
    pub journal_records: usize,
    pub file_record_fallbacks: usize,
}
pub struct WindowsVolumeSource {
    volume: String,
    guid: String,
    serial: u32,
    root: String,
    file: File,
    known_files: HashSet<String>,
    deletion_hints: HashSet<String>,
    excluded: HashSet<String>,
    pub metrics: Metrics,
}
impl WindowsVolumeSource {
    pub fn id_for_path(&self, path: &std::path::Path) -> Result<String> {
        ensure!(path.is_absolute(), "fixture path must be absolute");
        let file = open_path(path.to_str().context("non-Unicode fixture path")?)?;
        let value = info(&file)?;
        ensure!(
            value.dwVolumeSerialNumber == self.serial
                && handle_guid(&file)?.eq_ignore_ascii_case(&self.guid),
            "fixture outside volume"
        );
        Ok(key(file_id(&value)))
    }
    pub fn open(volume: &str) -> Result<Self> {
        Self::open_access(volume, GENERIC_READ)
    }
    pub fn open_metadata(volume: &str) -> Result<Self> {
        Self::open_access(volume, 0)
    }
    pub fn volume_root(&self) -> &str {
        &self.guid
    }
    pub fn exclude_paths(&mut self, paths: &[std::path::PathBuf]) -> Result<()> {
        for path in paths {
            ensure!(path.is_absolute(), "storage path must be absolute");
            let file = match open_path(path.to_str().context("non-Unicode storage path")?) {
                Ok(file) => file,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => return Err(error.context("storage identity lookup")),
            };
            let value = info(&file)?;
            // A database on another volume cannot generate this source's USN records.
            if value.dwVolumeSerialNumber == self.serial
                && handle_guid(&file)?.eq_ignore_ascii_case(&self.guid)
            {
                self.excluded.insert(key(file_id(&value)));
            }
        }
        Ok(())
    }
    fn open_access(volume: &str, access: u32) -> Result<Self> {
        let guid = io::guid(volume)?;
        let serial = io::identity(&guid)?;
        let root_file = open_path(&guid)?;
        let root_info = info(&root_file)?;
        ensure!(
            root_info.dwVolumeSerialNumber == serial
                && handle_guid(&root_file)?.eq_ignore_ascii_case(&guid),
            "volume root identity changed"
        );
        let native = file_id(&root_info);
        let root = key(native);
        let file = io::open(&guid, access)?;
        // Reopen the root through the raw handle, binding metadata and raw reads
        // to the same GUID+serial rather than two drive-letter observations.
        let descriptor = FILE_ID_DESCRIPTOR {
            dwSize: size_of::<FILE_ID_DESCRIPTOR>() as u32,
            Type: FileIdType,
            Anonymous: FILE_ID_DESCRIPTOR_0 {
                FileId: native as i64,
            },
        };
        let raw = unsafe {
            OpenFileById(
                file.as_raw_handle() as _,
                &descriptor,
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                FILE_FLAG_BACKUP_SEMANTICS,
            )
        };
        ensure!(
            raw != INVALID_HANDLE_VALUE,
            "open bound volume root: {}",
            std::io::Error::last_os_error()
        );
        let bound = unsafe { File::from_raw_handle(raw as _) };
        let bound_info = info(&bound)?;
        ensure!(
            bound_info.dwVolumeSerialNumber == serial
                && file_id(&bound_info) == native
                && handle_guid(&bound)?.eq_ignore_ascii_case(&guid)
                && io::guid(volume)?.eq_ignore_ascii_case(&guid),
            "volume source identity changed"
        );
        Ok(Self {
            volume: volume.to_owned(),
            guid,
            serial,
            root,
            file,
            known_files: HashSet::new(),
            deletion_hints: HashSet::new(),
            excluded: HashSet::new(),
            metrics: Metrics::default(),
        })
    }
    fn position(&self, p: &Checkpoint) -> Result<Position> {
        Ok(serde_json::from_str(&p.0)?)
    }
    fn resolve_inner(&mut self, id: &str) -> Result<Resolved> {
        if id == self.root {
            return Ok(Resolved::Present(Node {
                id: id.into(),
                directory: true,
                names: Vec::new(),
            }));
        }
        let native = u64::from_str_radix(id, 16)?;
        let descriptor = FILE_ID_DESCRIPTOR {
            dwSize: size_of::<FILE_ID_DESCRIPTOR>() as u32,
            Type: FileIdType,
            Anonymous: FILE_ID_DESCRIPTOR_0 {
                FileId: native as i64,
            },
        };
        let raw = unsafe {
            OpenFileById(
                self.file.as_raw_handle() as _,
                &descriptor,
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            let e = std::io::Error::last_os_error();
            return match e.raw_os_error() {
                Some(2) | Some(3) => Ok(Resolved::Deleted),
                Some(5) => Ok(Resolved::Unavailable("access_denied")),
                Some(32) => Ok(Resolved::Unavailable("sharing_violation")),
                Some(87) => self.confirm_deleted(native),
                _ => Err(anyhow::Error::new(e).context("OpenFileById")),
            };
        }
        let file = unsafe { File::from_raw_handle(raw as _) };
        let before = info(&file).context("metadata: before")?;
        let mut standard = FILE_STANDARD_INFO::default();
        let standard_ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle() as _,
                FileStandardInfo,
                (&mut standard as *mut FILE_STANDARD_INFO).cast(),
                size_of::<FILE_STANDARD_INFO>() as u32,
            )
        } != 0;
        metadata_standard_result(
            standard_ok
                .then_some(())
                .ok_or_else(std::io::Error::last_os_error),
        )?;
        if standard.DeletePending || standard.NumberOfLinks == 0 {
            return Ok(Resolved::Unavailable("delete_pending"));
        }
        ensure!(
            file_id(&before) == native && before.dwVolumeSerialNumber == self.serial,
            "file identity mismatch"
        );
        let directory = before.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        let names = if !directory && before.nNumberOfLinks > 1 {
            self.metrics.multi_name_files += 1;
            self.links(&file, before.nNumberOfLinks)?
        } else {
            match io::file_record(&file) {
                Ok(record) => {
                    let mut bytes = vec![0; 8];
                    bytes.extend(record);
                    let (_, records) = usn::parse(&bytes).context("parse file USN")?;
                    ensure!(
                        records.len() == 1 && records[0].id == native,
                        "invalid file USN response"
                    );
                    vec![Name {
                        parent: key(records[0].parent),
                        name: records[0].name.clone(),
                    }]
                }
                Err(error)
                    if matches!(
                        error
                            .downcast_ref::<std::io::Error>()
                            .and_then(|e| e.raw_os_error()),
                        Some(1 | 50 | 87)
                    ) =>
                {
                    self.metrics.file_record_fallbacks += 1;
                    match self.name_from_handle(&file) {
                        Ok(name) => vec![name],
                        Err(error)
                            if matches!(
                                error
                                    .downcast_ref::<std::io::Error>()
                                    .and_then(|e| e.raw_os_error()),
                                Some(1 | 50 | 87)
                            ) =>
                        {
                            return Ok(Resolved::Unavailable("file_metadata_not_supported"));
                        }
                        Err(error) => {
                            return Err(error.context("file USN unsupported; final-path fallback"));
                        }
                    }
                }
                Err(error) => return Err(error.context("FSCTL_READ_FILE_USN_DATA")),
            }
        };
        let after = info(&file).context("metadata: after")?;
        ensure!(
            after.nNumberOfLinks == before.nNumberOfLinks,
            "retry_required: link count changed during reconciliation"
        );
        if !directory && before.dwFileAttributes & FILE_ATTRIBUTE_SYSTEM == 0 {
            self.known_files.insert(id.to_owned());
        }
        Ok(Resolved::Present(Node {
            id: id.into(),
            directory,
            names,
        }))
    }
    fn confirm_deleted(&self, native: u64) -> Result<Resolved> {
        // An invalid parameter from OpenFileById is not by itself proof of deletion.
        let id = key(native);
        if !self.known_files.contains(&id) && !self.deletion_hints.contains(&id) {
            return Ok(Resolved::Unavailable(
                "invalid_id_without_deletion_evidence",
            ));
        }
        let check = (|| -> Result<bool> {
            ensure!(
                io::guid(&self.volume)? == self.guid && io::identity(&self.volume)? == self.serial,
                "volume changed during deletion check"
            );
            let before = io::journal(&self.file)?;
            let slot = native & 0x0000_ffff_ffff_ffff;
            let bytes = match io::enumeration(&self.file, slot) {
                Ok(bytes) => Some(bytes),
                Err(e)
                    if e.downcast_ref::<std::io::Error>()
                        .and_then(|e| e.raw_os_error())
                        == Some(38) =>
                {
                    None
                }
                Err(e) => return Err(e.context("targeted MFT deletion check")),
            };
            let absent = if let Some(bytes) = bytes {
                let (next, records) = usn::parse(&bytes)?;
                ensure!(next > slot, "targeted MFT did not advance");
                ensure!(
                    records.iter().all(|r| r.id & 0x0000_ffff_ffff_ffff >= slot),
                    "targeted MFT returned earlier record"
                );
                !records.iter().any(|r| r.id == native)
            } else {
                true
            };
            let after = io::journal(&self.file)?;
            ensure!(
                before.UsnJournalID == after.UsnJournalID && io::guid(&self.volume)? == self.guid,
                "volume/journal changed during deletion check"
            );
            Ok(absent)
        })();
        match check {
            Ok(true) => Ok(Resolved::Deleted),
            Ok(false) => Ok(Resolved::Unavailable("invalid_id_still_in_mft")),
            Err(error) if error.downcast_ref::<std::io::Error>().is_some() => {
                Ok(Resolved::Unavailable("deletion_confirmation_unavailable"))
            }
            Err(error) => Err(error),
        }
    }
    fn name_from_handle(&self, file: &File) -> Result<Name> {
        let mut path = vec![0u16; 32768];
        let len = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as _,
                path.as_mut_ptr(),
                path.len() as u32,
                VOLUME_NAME_GUID,
            )
        };
        if len == 0 {
            return Err(name_race(
                std::io::Error::last_os_error().into(),
                "GetFinalPathNameByHandleW single name",
            ));
        }
        ensure!((len as usize) < path.len(), "final path exceeded bound");
        let path = String::from_utf16(&path[..len as usize])?;
        ensure!(
            path.get(..self.guid.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(&self.guid)),
            "final path outside volume"
        );
        let relative = &path[self.guid.len()..];
        let (parent, name) = relative.rsplit_once('\\').unwrap_or(("", relative));
        ensure!(!name.is_empty(), "empty final name");
        let parent_file = open_path(&format!("{}{parent}", self.guid))
            .map_err(|e| name_race(e, "open final-path parent"))?;
        let parent_info = info(&parent_file).context("final-path parent identity")?;
        ensure!(
            parent_info.dwVolumeSerialNumber == self.serial
                && parent_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
            "final parent outside volume"
        );
        // Re-open the returned path without following the final reparse point.
        let reopened = info(&open_path(&path).map_err(|e| name_race(e, "reopen final path"))?)?;
        let original = info(file)?;
        ensure!(
            file_id(&reopened) == file_id(&original)
                && reopened.dwVolumeSerialNumber == self.serial,
            "retry_required: final path identity changed"
        );
        Ok(Name {
            parent: key(file_id(&parent_info)),
            name: name.to_owned(),
        })
    }
    fn links(&self, file: &File, expected: u32) -> Result<Vec<Name>> {
        ensure!(expected <= 1024, "unsupported link count");
        let mut path = vec![0u16; 32768];
        let len = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as _,
                path.as_mut_ptr(),
                path.len() as u32,
                VOLUME_NAME_GUID,
            )
        };
        if len == 0 {
            return Err(name_race(
                std::io::Error::last_os_error().into(),
                "GetFinalPathNameByHandleW hard links",
            ));
        }
        ensure!(
            (len as usize) < path.len(),
            "hard-link handle path exceeded bound"
        );
        path[len as usize] = 0;
        let mut buffer = vec![0u16; 32768];
        let mut count = buffer.len() as u32;
        let handle =
            unsafe { FindFirstFileNameW(path.as_ptr(), 0, &mut count, buffer.as_mut_ptr()) };
        if handle == INVALID_HANDLE_VALUE {
            return Err(name_race(
                std::io::Error::last_os_error().into(),
                "FindFirstFileNameW",
            ));
        }
        struct Find(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Find {
            fn drop(&mut self) {
                unsafe {
                    FindClose(self.0);
                }
            }
        }
        let _guard = Find(handle);
        let mut names = Vec::new();
        loop {
            ensure!(names.len() < 1024, "link enumeration exceeded bound");
            let end = buffer
                .iter()
                .position(|v| *v == 0)
                .context("unterminated link name")?;
            let relative = String::from_utf16(&buffer[..end])?;
            ensure!(relative.starts_with('\\'), "invalid volume-relative link");
            let (parent, name) = relative.rsplit_once('\\').context("invalid link path")?;
            ensure!(!name.is_empty(), "empty link name");
            let parent_path = format!("{}{}", self.guid, parent.trim_start_matches('\\'));
            let parent_info =
                info(&open_path(&parent_path).map_err(|e| name_race(e, "open hard-link parent"))?)?;
            ensure!(
                parent_info.dwVolumeSerialNumber == self.serial
                    && parent_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
                "link parent outside volume"
            );
            names.push(Name {
                parent: key(file_id(&parent_info)),
                name: name.to_owned(),
            });
            buffer.fill(0);
            count = buffer.len() as u32;
            if unsafe { FindNextFileNameW(handle, &mut count, buffer.as_mut_ptr()) } == 0 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() == Some(38) {
                    break;
                }
                return Err(name_race(e.into(), "FindNextFileNameW"));
            }
        }
        ensure!(
            names.len() == expected as usize,
            "retry_required: incomplete link set"
        );
        Ok(names)
    }
}
// Only namespace lookups after an identity-bound handle exists use this classifier.
// Initial OpenFileById NOT_FOUND remains a distinct deletion result.
fn name_race(error: anyhow::Error, stage: &'static str) -> anyhow::Error {
    if error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| matches!(e.raw_os_error(), Some(2 | 3)))
    {
        error
            .context(stage)
            .context("retry_required: name path disappeared during reconciliation")
    } else {
        error.context(stage)
    }
}
impl VolumeSource for WindowsVolumeSource {
    fn identity(&self) -> VolumeIdentity {
        VolumeIdentity {
            backend: "windows-ntfs-v1".into(),
            stable_id: format!("{}:{:08x}", self.guid, self.serial),
        }
    }
    fn root(&self) -> String {
        self.root.clone()
    }
    fn checkpoint(&mut self) -> Result<Checkpoint> {
        let j = io::journal(&self.file)?;
        Ok(Checkpoint(serde_json::to_string(&Position {
            identity: self.identity(),
            journal: j.UsnJournalID,
            next: j.NextUsn,
        })?))
    }
    fn validate(&mut self, p: &Checkpoint) -> Result<()> {
        let p = self.position(p)?;
        let j = io::journal(&self.file)?;
        ensure!(
            p.identity == self.identity()
                && io::guid(&self.volume)? == self.guid
                && io::identity(&self.volume)? == self.serial,
            "rebuild_required: volume identity changed"
        );
        ensure!(
            p.journal == j.UsnJournalID
                && p.next >= j.FirstUsn.max(j.LowestValidUsn)
                && p.next <= j.NextUsn,
            "rebuild_required: journal changed or checkpoint expired"
        );
        Ok(())
    }
    fn enumerate(&mut self, cursor: Option<&str>) -> Result<Enumeration> {
        let start = cursor.map(str::parse).transpose()?.unwrap_or(0);
        let bytes = match io::enumeration(&self.file, start) {
            Ok(b) => b,
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .and_then(|e| e.raw_os_error())
                    == Some(38) =>
            {
                return Ok(Enumeration {
                    ids: Vec::new(),
                    next: None,
                });
            }
            Err(e) => return Err(e),
        };
        let (next, records) = usn::parse(&bytes)?;
        ensure!(next > start, "MFT did not advance");
        self.metrics.enum_pages += 1;
        self.metrics.enumerated += records.len();
        Ok(Enumeration {
            ids: records
                .into_iter()
                .map(|r| key(r.id))
                .filter(|id| !self.excluded.contains(id))
                .collect(),
            next: Some(next.to_string()),
        })
    }
    fn changes(&mut self, p: &Checkpoint) -> Result<Changes> {
        self.validate(p)?;
        let mut position = self.position(p)?;
        let target = io::journal(&self.file)?.NextUsn;
        if position.next == target {
            return Ok(Changes {
                ids: Vec::new(),
                next: p.clone(),
                caught_up: true,
            });
        }
        let (next, records) = usn::parse(&io::read(&self.file, position.journal, position.next)?)?;
        ensure!(
            next <= i64::MAX as u64 && next as i64 > position.next,
            "journal did not advance"
        );
        for r in &records {
            ensure!(
                r.usn >= position.next && r.usn < (next as i64),
                "record outside checkpoint range"
            );
            if r.reason & 0x200 != 0 {
                self.deletion_hints.insert(key(r.id));
            }
        }
        self.metrics.journal_records += records.len();
        position.next = next as i64;
        Ok(Changes {
            ids: records
                .into_iter()
                .map(|r| key(r.id))
                .filter(|id| !self.excluded.contains(id))
                .collect(),
            next: Checkpoint(serde_json::to_string(&position)?),
            caught_up: position.next >= target,
        })
    }
    fn resolve(&mut self, id: &str) -> Result<Resolved> {
        let now = Instant::now();
        self.metrics.metadata_lookups += 1;
        let result = classify_metadata_result(self.resolve_inner(id));
        self.metrics.metadata_micros += now.elapsed().as_micros();
        match &result {
            Ok(Resolved::Unavailable(_)) => self.metrics.unavailable += 1,
            Ok(Resolved::Deleted) => self.metrics.deleted += 1,
            _ => (),
        }
        result
    }
}

fn classify_metadata_result(result: Result<Resolved>) -> Result<Resolved> {
    match result {
        Err(error)
            if matches!(
                error
                    .downcast_ref::<std::io::Error>()
                    .and_then(|e| e.raw_os_error()),
                Some(5 | 32)
            ) =>
        {
            Ok(Resolved::Unavailable("metadata_access_or_sharing_denied"))
        }
        other => other,
    }
}

fn metadata_standard_result(result: std::io::Result<()>) -> Result<()> {
    result.context("metadata: delete-pending state unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_metadata_preserves_typed_native_error() {
        for code in [5, 32, 87, 303] {
            let error =
                metadata_standard_result(Err(std::io::Error::from_raw_os_error(code))).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<std::io::Error>()
                    .unwrap()
                    .raw_os_error(),
                Some(code)
            );
            let classified = classify_metadata_result(Err(error));
            if matches!(code, 5 | 32) {
                assert!(matches!(
                    classified,
                    Ok(Resolved::Unavailable("metadata_access_or_sharing_denied"))
                ));
            } else {
                assert!(classified.is_err());
            }
        }
        assert!(metadata_standard_result(Ok(())).is_ok());
        assert!(classify_metadata_result(Err(anyhow::anyhow!("file identity mismatch"))).is_err());
        assert!(
            classify_metadata_result(Err(anyhow::anyhow!("invalid file USN response"))).is_err()
        );
    }
    #[test]
    fn only_missing_namespace_lookups_are_retryable() {
        for code in [2, 3] {
            assert!(super::super::metadata_retryable(&name_race(
                std::io::Error::from_raw_os_error(code).into(),
                "open hard-link parent"
            )));
        }
        for code in [5, 32, 87] {
            assert!(!super::super::metadata_retryable(&name_race(
                std::io::Error::from_raw_os_error(code).into(),
                "open hard-link parent"
            )));
        }
    }
    #[test]
    fn storage_exclusion_uses_volume_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("storage.db");
        std::fs::write(&path, "fictional").unwrap();
        let drive = &path.to_str().unwrap()[..2];
        let mut local = WindowsVolumeSource::open_metadata(drive).unwrap();
        local.exclude_paths(std::slice::from_ref(&path)).unwrap();
        assert!(local.excluded.contains(&local.id_for_path(&path).unwrap()));
        let mut cloned = WindowsVolumeSource::open_metadata(drive).unwrap();
        cloned.guid = format!(r"\\?\Volume{{{}}}\", uuid::Uuid::new_v4());
        assert_eq!(cloned.serial, local.serial);
        cloned.exclude_paths(std::slice::from_ref(&path)).unwrap();
        assert!(cloned.excluded.is_empty());
        assert!(cloned.id_for_path(&path).is_err());
        for candidate in ['C', 'D', 'E', 'F'] {
            let volume = format!("{candidate}:");
            if let Ok(mut other) = WindowsVolumeSource::open_metadata(&volume)
                && other.serial != local.serial
            {
                other.exclude_paths(std::slice::from_ref(&path)).unwrap();
                assert!(other.excluded.is_empty());
                return;
            }
        }
        eprintln!("different-volume exclusion branch: NOT_VERIFIED (no second readable volume)");
    }
    #[test]
    fn owned_fixture_hardlinks_resolve_without_volume_access() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("原目录");
        std::fs::create_dir(&folder).unwrap();
        let one = folder.join("原始.txt");
        let two = folder.join("链接.txt");
        std::fs::write(&one, "fixture").unwrap();
        std::fs::hard_link(&one, &two).unwrap();
        let drive = one.to_str().unwrap()[..2].to_owned();
        let serial = io::identity(&drive).unwrap();
        let guid = io::guid(&drive).unwrap();
        let file = open_path(&guid).unwrap();
        let root = key(file_id(&info(&file).unwrap()));
        let file = io::open(&drive, 0).unwrap();
        let mut source = WindowsVolumeSource {
            volume: drive,
            guid,
            serial,
            root,
            file,
            known_files: HashSet::new(),
            deletion_hints: HashSet::new(),
            excluded: HashSet::new(),
            metrics: Metrics::default(),
        };
        let id = source.id_for_path(&one).unwrap();
        let result = source.resolve(&id).unwrap();
        match result {
            Resolved::Present(node) => {
                assert_eq!(node.names.len(), 2);
                assert!(node.names.iter().any(|n| n.name == "链接.txt"));
            }
            other => panic!("{other:?}"),
        }
        let renamed = folder.join("改名.txt");
        std::fs::rename(&two, &renamed).unwrap();
        let Resolved::Present(node) = source.resolve(&id).unwrap() else {
            panic!("missing renamed file")
        };
        assert_eq!(node.names.len(), 2);
        assert!(node.names.iter().any(|n| n.name == "改名.txt"));
        assert!(!node.names.iter().any(|n| n.name == "链接.txt"));
        std::fs::remove_file(&one).unwrap();
        let Resolved::Present(node) = source.resolve(&id).unwrap() else {
            panic!("missing remaining link")
        };
        assert_eq!(node.names.len(), 1);
        assert_eq!(node.names[0].name, "改名.txt");
        let folder_id = source.id_for_path(&folder).unwrap();
        let new_folder = dir.path().join("新中文目录");
        std::fs::rename(&folder, &new_folder).unwrap();
        let Resolved::Present(parent) = source.resolve(&folder_id).unwrap() else {
            panic!("missing renamed parent")
        };
        assert_eq!(parent.names[0].name, "新中文目录");
        let final_path = new_folder.join("改名.txt");
        // Exercise the same handle/path reconciliation used when file-USN is unsupported.
        let handle = open_path(final_path.to_str().unwrap()).unwrap();
        let fallback = source.name_from_handle(&handle).unwrap();
        assert_eq!(fallback.parent, folder_id);
        assert_eq!(fallback.name, "改名.txt");
        drop(handle);
        std::fs::remove_file(&final_path).unwrap();
        assert!(!matches!(
            source.resolve(&id).unwrap(),
            Resolved::Present(_)
        ));
    }
}
