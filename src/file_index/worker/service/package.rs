use super::super::{
    security::{self, Descriptor, owned, wide},
    storage,
};
use super::PRODUCT_DIRECTORY;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::io::OwnedHandle,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Security::{Authorization::*, Cryptography::*, *},
    Storage::FileSystem::*,
    System::Com::CoTaskMemFree,
    UI::Shell::*,
};
pub(super) const IMAGE: &str = "dao-index-worker.exe";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub schema: u32,
    pub owner_sid: String,
    pub current: String,
    pub previous: Option<String>,
    pub pending: bool,
    pub recover_to: Option<String>,
    #[serde(default)]
    pub uninstalled: bool,
}
pub(super) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == 1
                && valid_digest(&self.current)
                && self.previous.as_deref().is_none_or(valid_digest),
            "invalid installation manifest version"
        );
        ensure!(self.recover_to.as_ref().is_none_or(|value|value==&self.current || self.previous.as_ref()==Some(value)),"invalid recovery version");
        ensure!(
            self.owner_sid.starts_with("S-1-")
                && self.owner_sid.len() <= 184
                && self
                    .owner_sid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
                && !matches!(self.owner_sid.as_str(), "S-1-5-18" | "S-1-5-32-544"),
            "invalid installation owner"
        );
        Ok(())
    }
}
pub(super) struct Layout {
    pub programs: PathBuf,
    pub data: PathBuf,
}
fn folder(id: &windows_sys::core::GUID) -> Result<PathBuf> {
    let mut value = ptr::null_mut();
    ensure!(
        unsafe { SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut value) } >= 0,
        "Known Folder unavailable"
    );
    let text = unsafe {
        let mut len = 0;
        while *value.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16(std::slice::from_raw_parts(value, len));
        CoTaskMemFree(value.cast());
        text?
    };
    Ok(PathBuf::from(text))
}
impl Layout {
    pub fn fixed() -> Result<Self> {
        Ok(Self {
            programs: folder(&FOLDERID_ProgramFiles)?.join(PRODUCT_DIRECTORY),
            data: folder(&FOLDERID_ProgramData)?.join(PRODUCT_DIRECTORY),
        })
    }
    pub fn image(&self, digest: &str) -> Result<PathBuf> {
        ensure!(valid_digest(digest), "invalid build digest");
        Ok(self.programs.join(digest).join(IMAGE))
    }
    pub fn command(&self, digest: &str) -> Result<String> {
        Ok(format!("\"{}\" service-run", self.image(digest)?.display()))
    }
    pub fn hold_ancestors(&self) -> Result<Vec<OwnedHandle>> {
        let mut held = Vec::new();
        for target in [&self.programs, &self.data] {
            let mut ancestors = target.parent().unwrap().ancestors().collect::<Vec<_>>();
            ancestors.reverse();
            for path in ancestors {
                held.push(storage::hold(path, true)?);
            }
        }
        Ok(held)
    }
    pub fn create_roots(&self, sid: &str) -> Result<Vec<OwnedHandle>> {
        let mut held = self.hold_ancestors()?;
        for (path, reader) in [(&self.programs, Some(sid)), (&self.data, None)] {
            storage::create_directory_reader(path, reader)?;
            held.push(storage::hold(path, true)?);
            storage::validate_acl_reader(path, true, reader)?;
        }
        Ok(held)
    }
    pub fn read_manifest(&self) -> Result<Manifest> {
        let _ancestors = self.hold_ancestors()?;
        let _root = storage::hold(&self.programs, true)?;
        let path = self.programs.join("manifest.json");
        let _file = storage::hold(&path, false)?;
        let mut bytes = Vec::new();
        File::open(&path)?.take(8193).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 8192, "manifest exceeds bound");
        let manifest: Manifest = serde_json::from_slice(&bytes)?;
        manifest.validate()?;
        storage::validate_acl_reader(&self.programs, true, Some(&manifest.owner_sid))?;
        storage::validate_acl_reader(&path, false, Some(&manifest.owner_sid))?;
        Ok(manifest)
    }
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<()> {
        manifest.validate()?;
        let path = self.programs.join("manifest.json");
        let temporary = self
            .programs
            .join(format!("manifest-{}.tmp", uuid::Uuid::new_v4()));
        let mut file = create_file(&temporary, Some(&manifest.owner_sid))?;
        file.write_all(&serde_json::to_vec(manifest)?)?;
        file.sync_all()?;
        drop(file);
        ensure!(
            unsafe {
                MoveFileExW(
                    wide(&temporary.to_string_lossy()).as_ptr(),
                    wide(&path.to_string_lossy()).as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } != 0,
            "manifest atomic replacement failed"
        );
        Ok(())
    }
    pub fn stage(&self, manifest: &Manifest, bytes: &[u8]) -> Result<()> {
        ensure!(
            digest(bytes)? == manifest.current,
            "payload digest mismatch"
        );
        let directory = self.programs.join(&manifest.current);
        storage::create_directory_reader(&directory, Some(&manifest.owner_sid))?;
        let _held = storage::hold(&directory, true)?;
        storage::validate_acl_reader(&directory, true, Some(&manifest.owner_sid))?;
        let image = directory.join(IMAGE);
        if !image.try_exists()? {
            let mut file = create_file(&image, Some(&manifest.owner_sid))?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        self.verify_image(manifest, &manifest.current)?;
        Ok(())
    }
    pub fn verify_image(&self, manifest: &Manifest, build: &str) -> Result<PathBuf> {
        let image = self.image(build)?;
        let _directory = storage::hold(image.parent().unwrap(), true)?;
        let _image = storage::hold(&image, false)?;
        storage::validate_acl_reader(image.parent().unwrap(), true, Some(&manifest.owner_sid))?;
        storage::validate_acl_reader(&image, false, Some(&manifest.owner_sid))?;
        verify_image_digest(&read_locked(&image)?, build)?;
        Ok(image)
    }
    pub fn volume_files(&self) -> Result<Vec<PathBuf>> {
        let root = self.data.join("volumes");
        if !root.try_exists()? {
            return Ok(Vec::new());
        }
        let _root = storage::hold(&root, true)?;
        storage::validate_acl_reader(&root, true, None)?;
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            ensure!(
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(super::volumes::valid_key),
                "unknown volume data directory"
            );
            let path = entry.path();
            let _guard = storage::hold(&path, true)?;
            storage::validate_acl_reader(&path, true, None)?;
            for file in std::fs::read_dir(path)? {
                let file = file?;
                ensure!(
                    [
                        "index.db",
                        "index.db-wal",
                        "index.db-shm",
                        "index.db-journal",
                        "worker-diagnostics.jsonl"
                    ]
                    .iter()
                    .any(|name| file.file_name() == *name),
                    "unknown index data file"
                );
                let path = file.path();
                let _guard = storage::hold(&path, false)?;
                storage::validate_acl_reader(&path, false, None)?;
                files.push(path);
            }
        }
        Ok(files)
    }
    pub fn compatible_databases(&self) -> Result<()> {
        for path in self
            .volume_files()?
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|v| v == "index.db"))
        {
            validate_header(&path)?;
        }
        Ok(())
    }
    pub fn remove_payloads(&self, manifest: &Manifest) -> Result<()> {
        // Validate every entry before deleting anything; no recursive deletion or adoption.
        let mut files = Vec::new();
        let mut directories = Vec::new();
        for entry in std::fs::read_dir(&self.programs)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            if name == "manifest.json" {
                continue;
            }
            if name.to_str().is_some_and(valid_digest) {
                let _guard = storage::hold(&path, true)?;
                storage::validate_acl_reader(&path, true, Some(&manifest.owner_sid))?;
                for file in std::fs::read_dir(&path)? {
                    let file = file?;
                    ensure!(file.file_name() == IMAGE, "unknown installed payload entry");
                    self.verify_image(manifest, name.to_str().unwrap())?;
                    files.push(file.path());
                }
                directories.push(path);
            } else {
                let temporary = name
                    .to_str()
                    .and_then(|n| n.strip_prefix("manifest-"))
                    .and_then(|n| n.strip_suffix(".tmp"));
                ensure!(
                    temporary.is_some_and(|v| uuid::Uuid::parse_str(v).is_ok()),
                    "unknown installation entry"
                );
                let _guard = storage::hold(&path, false)?;
                storage::validate_acl_reader(&path, false, Some(&manifest.owner_sid))?;
                files.push(path);
            }
        }
        for file in files {
            std::fs::remove_file(file)?;
        }
        for directory in directories {
            std::fs::remove_dir(directory)?;
        }
        Ok(())
    }
    pub fn remove_index(&self) -> Result<()> {
        let files = self.volume_files()?;
        let volumes = self.data.join("volumes");
        if !volumes.try_exists()? {
            return Ok(());
        }
        let directories = std::fs::read_dir(&volumes)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        ensure!(
            files
                .iter()
                .chain(directories.iter())
                .all(|p| p.is_absolute() && p.starts_with(&volumes)),
            "index cleanup outside fixed directory"
        );
        for file in files {
            std::fs::remove_file(file)?;
        }
        for directory in directories {
            std::fs::remove_dir(directory)?;
        }
        std::fs::remove_dir(volumes)?;
        Ok(())
    }
}
fn validate_header(path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 100];
    file.read_exact(&mut header)?;
    ensure!(
        &header[..16] == b"SQLite format 3\0"
            && u32::from_be_bytes(header[60..64].try_into()?) == 2
            && u32::from_be_bytes(header[68..72].try_into()?) == 0x44414931,
        "unsupported index schema; upgrade has not changed service or data"
    );
    Ok(())
}
fn create_file(path: &Path, reader: Option<&str>) -> Result<File> {
    let mut sddl = "O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)".to_owned();
    if let Some(sid) = reader {
        sddl.push_str(&format!("(A;;0x1200a9;;;{sid})"));
    }
    let mut sd = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(&sddl).as_ptr(),
                1,
                &mut sd,
                ptr::null_mut(),
            )
        } != 0,
        "package descriptor invalid"
    );
    let _sd = Descriptor(sd);
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd,
        bInheritHandle: 0,
    };
    let handle = owned(unsafe {
        CreateFileW(
            wide(&path.to_string_lossy()).as_ptr(),
            FILE_GENERIC_WRITE,
            FILE_SHARE_READ,
            &attrs,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    })?;
    Ok(File::from(handle))
}
pub(super) fn read_locked(path: &Path) -> Result<Vec<u8>> {
    let handle = owned(unsafe {
        CreateFileW(
            wide(&path.to_string_lossy()).as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    })?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    ensure!(
        unsafe { GetFileInformationByHandle(security::raw(&handle), &mut info) } != 0
            && info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
                == 0
            && info.nNumberOfLinks == 1,
        "unsafe package image"
    );
    let mut bytes = Vec::new();
    File::from(handle)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 64 * 1024 * 1024, "package exceeds bound");
    Ok(bytes)
}
pub(super) fn payload(expected: &str) -> Result<(String, Vec<u8>)> {
    ensure!(valid_digest(expected), "expected SHA256 required");
    let bytes = read_locked(&std::env::current_exe()?)?;
    ensure!(
        digest(&bytes)? == expected,
        "running installer source differs from frozen expected SHA256"
    );
    Ok((expected.to_owned(), bytes))
}
fn verify_image_digest(bytes: &[u8], build: &str) -> Result<()> {
    ensure!(digest(bytes)? == build, "installed image digest mismatch");
    Ok(())
}
pub(super) fn digest(bytes: &[u8]) -> Result<String> {
    let mut algorithm = ptr::null_mut();
    ensure!(
        unsafe {
            BCryptOpenAlgorithmProvider(&mut algorithm, BCRYPT_SHA256_ALGORITHM, ptr::null(), 0)
        } >= 0,
        "SHA256 provider unavailable"
    );
    struct Algorithm(BCRYPT_ALG_HANDLE);
    impl Drop for Algorithm {
        fn drop(&mut self) {
            unsafe {
                BCryptCloseAlgorithmProvider(self.0, 0);
            }
        }
    }
    let algorithm = Algorithm(algorithm);
    let mut hash = [0u8; 32];
    ensure!(
        unsafe {
            BCryptHash(
                algorithm.0,
                ptr::null(),
                0,
                bytes.as_ptr(),
                u32::try_from(bytes.len())?,
                hash.as_mut_ptr(),
                32,
            )
        } >= 0,
        "SHA256 failed"
    );
    Ok(hash.iter().map(|byte| format!("{byte:02X}")).collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_image_replacement_fails_frozen_digest() {
        let original = b"fixed audited image";
        let expected = digest(original).unwrap();
        assert!(verify_image_digest(original, &expected).is_ok());
        assert!(verify_image_digest(b"replaced payload", &expected).is_err());
    }
    #[test]
    fn digest_and_manifest_reject_untrusted_package_identifiers() {
        assert_eq!(
            digest(b"abc").unwrap(),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
        assert!(!valid_digest("../../escape"));
        assert!(!valid_digest(&"a".repeat(64)));
        let manifest = Manifest {
            schema: 1,
            owner_sid: "S-1-5-21-1-2-3-1001".into(),
            current: "A".repeat(64),
            previous: None,
            pending: false,
            recover_to: None,
            uninstalled: false,
        };
        assert!(manifest.validate().is_ok());
        let mut bad = manifest;
        bad.current = "C:\\evil.exe".into();
        assert!(bad.validate().is_err());
        assert!(payload(&"0".repeat(64)).is_err());
    }
    #[test]
    fn schema_header_preflight_does_not_adopt_or_migrate_foreign_data() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("index.db");
        drop(crate::file_index::IndexStore::open(&path).unwrap());
        validate_header(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut foreign = before.clone();
        foreign[60..64].copy_from_slice(&999u32.to_be_bytes());
        std::fs::write(&path, &foreign).unwrap();
        assert!(validate_header(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), foreign);
    }
    #[test]
    fn source_snapshot_rejects_hardlinks_and_directories_without_writing() {
        let fixture = tempfile::tempdir().unwrap();
        let source = fixture.path().join("payload.exe");
        let alias = fixture.path().join("alias.exe");
        std::fs::write(&source, b"fixture payload only").unwrap();
        assert_eq!(read_locked(&source).unwrap(), b"fixture payload only");
        std::fs::hard_link(&source, &alias).unwrap();
        assert!(read_locked(&source).is_err());
        assert!(read_locked(&alias).is_err());
        assert!(read_locked(fixture.path()).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"fixture payload only");
    }
    #[test]
    fn fixed_image_derivation_rejects_path_injection_and_unknown_recovery() {
        let layout = Layout {
            programs: std::path::PathBuf::from(r"C:\fixture\programs"),
            data: std::path::PathBuf::from(r"C:\fixture\data"),
        };
        for bad in [
            "..",
            r"..\escape",
            r"C:\payload.exe",
            "A\" service-run --evil",
            "a",
        ] {
            assert!(layout.image(bad).is_err());
        }
        let mut manifest = Manifest {
            schema: 1,
            owner_sid: "S-1-5-21-1-2-3-1001".into(),
            current: "A".repeat(64),
            previous: Some("B".repeat(64)),
            pending: true,
            recover_to: Some("C".repeat(64)),
            uninstalled: false,
        };
        assert!(manifest.validate().is_err());
        manifest.recover_to = Some("A".repeat(64));
        assert!(manifest.validate().is_ok());
        manifest.owner_sid = "S-1-5-21-1)(A;;GA;;;WD)".into();
        assert!(manifest.validate().is_err());
    }
}
