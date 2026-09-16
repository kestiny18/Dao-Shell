use crate::core::FileIdentity;
use anyhow::Result;
use std::{fs, path::Path};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{DirectoryGuard, identity, move_file, open_document, pin_directory};

pub fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(not(windows))]
pub struct DirectoryGuard;
#[cfg(not(windows))]
pub fn pin_directory(_: &Path) -> Result<DirectoryGuard> {
    anyhow::bail!("文件修改暂只支持 Windows")
}
#[cfg(not(windows))]
pub fn move_file(_: &Path, _: &Path, _: &FileIdentity) -> Result<()> {
    anyhow::bail!("文件修改暂只支持 Windows")
}
#[cfg(not(windows))]
pub fn open_document(_: &Path, _: &FileIdentity) -> Result<()> {
    anyhow::bail!("打开文件暂只支持 Windows")
}
#[cfg(unix)]
pub fn identity(path: &Path) -> Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path)?;
    anyhow::ensure!(!is_link(&m), "不跟随符号链接");
    Ok(FileIdentity {
        volume: m.dev(),
        index: m.ino(),
        size: m.len(),
        modified: m.mtime_nsec() as u64 ^ (m.mtime() as u64).rotate_left(32),
        directory: m.is_dir(),
    })
}

/// Strong identity plus metadata: a stale candidate cannot silently authorize a new file.
pub fn require_identity(path: &Path, expected: &FileIdentity) -> Result<()> {
    anyhow::ensure!(
        &identity(path)? == expected,
        "对象已变化，请重新查找并确认：{}",
        path.display()
    );
    Ok(())
}
