//! Fixed, protected per-volume collection preference. No index schema migration.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    version: u32,
    include_system_files: bool,
}
pub(super) fn load(database: &Path) -> Result<bool> {
    let path = database.with_file_name("policy.json");
    if !path.try_exists()? {
        return Ok(false);
    }
    let _guard = storage::hold(&path, false)?;
    storage::validate_acl_reader(&path, false, None)?;
    ensure!(
        std::fs::metadata(&path)?.len() <= 128,
        "policy preference exceeds bound"
    );
    read(&std::fs::read(path)?)
}
fn read(bytes: &[u8]) -> Result<bool> {
    ensure!(bytes.len() <= 128, "policy preference exceeds bound");
    let value: Preference = serde_json::from_slice(bytes)?;
    ensure!(value.version == 1, "unsupported policy preference");
    Ok(value.include_system_files)
}
pub(super) fn save(database: &Path, include: bool) -> Result<()> {
    let root = database.parent().context("policy parent missing")?;
    let _root = storage::hold(root, true)?;
    storage::validate_acl_reader(root, true, None)?;
    save_checked(database, include, |path| {
        let _guard = storage::hold(path, false)?;
        storage::validate_acl_reader(path, false, None)
    })
}
pub(super) fn save_checked(
    database: &Path,
    include: bool,
    validate: impl Fn(&Path) -> Result<()>,
) -> Result<()> {
    use std::io::Write;
    let destination = database.with_file_name("policy.json");
    if destination.try_exists()? {
        validate(&destination)?;
    }
    let pending = database.with_file_name("policy.pending");
    if pending.try_exists()? {
        validate(&pending)?;
        std::fs::remove_file(&pending)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    let outcome = (|| -> Result<()> {
        file.write_all(&serde_json::to_vec(&Preference {
            version: 1,
            include_system_files: include,
        })?)?;
        file.sync_all()?;
        drop(file);
        validate(&pending)?;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        ensure!(
            unsafe {
                MoveFileExW(
                    security::wide(&pending.to_string_lossy()).as_ptr(),
                    security::wide(&destination.to_string_lossy()).as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } != 0,
            "policy preference atomic save failed"
        );
        Ok(())
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    outcome
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_preference_roundtrip_restart_and_failed_save_retain_previous_policy() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("index.db");
        save_checked(&db, true, |_| Ok(())).unwrap();
        let path = dir.path().join("policy.json");
        assert!(read(&std::fs::read(&path).unwrap()).unwrap());
        assert!(
            save_checked(&db, false, |p| {
                ensure!(
                    p.file_name().unwrap() != "policy.pending",
                    "synthetic ACL rejection"
                );
                Ok(())
            })
            .is_err()
        );
        assert!(read(&std::fs::read(&path).unwrap()).unwrap());
        assert!(!dir.path().join("policy.pending").exists());
        std::fs::write(
            dir.path().join("policy.pending"),
            "interrupted private preference",
        )
        .unwrap();
        save_checked(&db, false, |_| Ok(())).unwrap();
        assert!(!read(&std::fs::read(path).unwrap()).unwrap());
    }
    #[test]
    fn bounded_preference_is_explicit_and_rejects_private_or_arbitrary_settings() {
        assert!(read(br#"{"version":1,"include_system_files":true}"#).unwrap());
        assert!(!read(br#"{"version":1,"include_system_files":false}"#).unwrap());
        for bytes in [
            br#"{"version":2,"include_system_files":true}"#.as_slice(),
            br#"{"version":1,"include_system_files":true,"path":"private"}"#.as_slice(),
            &[b' '; 129],
        ] {
            assert!(read(bytes).is_err());
        }
    }
}
