//! Local, identity-bound Startup file transitions. No shortcut is executed.
use crate::core::FileIdentity;
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub struct PinnedShortcut {
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub digest: [u8; 32],
}
impl PinnedShortcut {
    pub fn inspect(path: &Path) -> Result<Self> {
        ensure!(
            path.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("lnk")),
            "unsupported_startup_entry"
        );
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid_startup_path"))?;
        let _guard = crate::platform::pin_directory(parent)?;
        let identity = crate::platform::identity(path)?;
        ensure!(
            !identity.directory && identity.size >= 76 && identity.size <= 1024 * 1024,
            "unsupported_startup_entry"
        );
        let bytes = std::fs::read(path)?;
        ensure!(
            bytes.len() >= 76
                && bytes[..4] == [76, 0, 0, 0]
                && bytes[4..20] == [1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70],
            "invalid_shortcut_header"
        );
        crate::platform::require_identity(path, &identity)?;
        Ok(Self {
            path: path.to_owned(),
            identity,
            digest: Sha256::digest(bytes).into(),
        })
    }
    /// Both directories are selected by the fixed local provider, never by model arguments.
    pub fn relocate(&self, destination: &Path) -> Result<Self> {
        crate::platform::move_file_checked(&self.path, destination, &self.identity, &self.digest)?;
        let moved = Self::inspect(destination)?;
        ensure!(
            moved.identity.same_object(&self.identity) && moved.digest == self.digest,
            "post_move_unknown"
        );
        Ok(moved)
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn shortcut() -> Vec<u8> {
        let mut b = vec![0; 76];
        b[..4].copy_from_slice(&[76, 0, 0, 0]);
        b[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        b
    }
    #[test]
    fn isolated_disable_restore_preserves_identity_and_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let active = d.path().join("Startup");
        let retained = d.path().join("retained");
        std::fs::create_dir(&active).unwrap();
        std::fs::create_dir(&retained).unwrap();
        let source = active.join("fixture.lnk");
        std::fs::write(&source, shortcut()).unwrap();
        let p = PinnedShortcut::inspect(&source).unwrap();
        let target = retained.join("receipt.lnk");
        let disabled = p.relocate(&target).unwrap();
        assert!(!source.exists());
        assert!(disabled.identity.same_object(&p.identity));
        std::fs::write(&source, shortcut()).unwrap();
        assert!(disabled.relocate(&source).is_err());
        assert!(target.exists());
        std::fs::remove_file(&source).unwrap();
        disabled.relocate(&source).unwrap();
        assert!(!target.exists());
    }
    #[test]
    fn changed_content_and_replaced_file_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let s = d.path().join("x.lnk");
        std::fs::write(&s, shortcut()).unwrap();
        let p = PinnedShortcut::inspect(&s).unwrap();
        let mut b = shortcut();
        b[40] = 1;
        std::fs::write(&s, &b).unwrap();
        assert!(p.relocate(&d.path().join("y.lnk")).is_err());
        assert!(s.exists());
    }
    #[test]
    fn native_digest_check_is_independent_of_metadata() {
        let d = tempfile::tempdir().unwrap();
        let s = d.path().join("x.lnk");
        std::fs::write(&s, shortcut()).unwrap();
        let p = PinnedShortcut::inspect(&s).unwrap();
        let mut wrong = p.digest;
        wrong[0] ^= 1;
        assert!(
            crate::platform::move_file_checked(&s, &d.path().join("y.lnk"), &p.identity, &wrong)
                .is_err()
        );
        assert!(s.exists());
    }
}
