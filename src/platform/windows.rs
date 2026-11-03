use crate::core::FileIdentity;
use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsStr,
    fs::File,
    mem::{offset_of, size_of},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
};

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn handle(file: &File) -> HANDLE {
    file.as_raw_handle() as HANDLE
}

fn file_handle(path: &Path, mutation: bool, directory: bool) -> Result<File> {
    file_handle_access(path, mutation, directory, false)
}
fn file_handle_access(path: &Path, mutation: bool, directory: bool, content: bool) -> Result<File> {
    let path_wide = wide(path.as_os_str());
    // Attribute-only handles do not participate in all Windows share checks.
    // READ_DATA is FILE_LIST_DIRECTORY for directories and makes the delete denial effective.
    let access = FILE_READ_ATTRIBUTES
        | if directory || content {
            FILE_READ_DATA
        } else {
            0
        }
        | if mutation { DELETE } else { 0 };
    let share = if mutation {
        FILE_SHARE_READ
    } else if directory {
        FILE_SHARE_READ | FILE_SHARE_WRITE
    } else {
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
    };
    // OPEN_REPARSE_POINT lets us inspect the link itself instead of following it.
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            access,
            share,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    ensure!(
        raw != INVALID_HANDLE_VALUE,
        "无法打开对象 {}：{}",
        path.display(),
        std::io::Error::last_os_error()
    );
    // SAFETY: CreateFileW returned a unique, valid owned handle.
    let file = unsafe { File::from_raw_handle(raw as _) };
    let info = info(&file)?;
    ensure!(
        info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "不跟随重解析点：{}",
        path.display()
    );
    ensure!(
        info.dwFileAttributes & FILE_ATTRIBUTE_DEVICE == 0,
        "不支持设备路径"
    );
    Ok(file)
}
fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    ensure!(
        unsafe { GetFileInformationByHandle(handle(file), &mut info) } != 0,
        "无法读取文件身份：{}",
        std::io::Error::last_os_error()
    );
    Ok(info)
}
fn identity_of(file: &File) -> Result<FileIdentity> {
    let i = info(file)?;
    Ok(FileIdentity {
        volume: i.dwVolumeSerialNumber as u64,
        index: ((i.nFileIndexHigh as u64) << 32) | i.nFileIndexLow as u64,
        size: ((i.nFileSizeHigh as u64) << 32) | i.nFileSizeLow as u64,
        modified: ((i.ftLastWriteTime.dwHighDateTime as u64) << 32)
            | i.ftLastWriteTime.dwLowDateTime as u64,
        directory: i.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
    })
}
pub fn identity(path: &Path) -> Result<FileIdentity> {
    identity_of(&file_handle(path, false, false)?)
}

/// Holding every ancestor without FILE_SHARE_DELETE prevents directory replacement during a write.
pub struct DirectoryGuard {
    _handles: Vec<File>,
}
pub fn pin_directory(path: &Path) -> Result<DirectoryGuard> {
    let mut current = PathBuf::new();
    let mut handles = Vec::new();
    for part in path.components() {
        current.push(part);
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        let file = file_handle(&current, false, true)?;
        ensure!(identity_of(&file)?.directory, "路径组件不是目录");
        handles.push(file);
    }
    ensure!(!handles.is_empty(), "需要绝对目录");
    Ok(DirectoryGuard { _handles: handles })
}

pub fn move_file(source: &Path, destination: &Path, expected: &FileIdentity) -> Result<()> {
    move_file_impl(source, destination, expected, None)
}
/// Content and identity are checked using the same handle that performs the rename.
pub fn move_file_checked(
    source: &Path,
    destination: &Path,
    expected: &FileIdentity,
    digest: &[u8; 32],
) -> Result<()> {
    move_file_impl(source, destination, expected, Some(digest))
}
fn move_file_impl(
    source: &Path,
    destination: &Path,
    expected: &FileIdentity,
    digest: Option<&[u8; 32]>,
) -> Result<()> {
    let _source_parent = pin_directory(source.parent().context("源文件没有父目录")?)?;
    let _target_parent = pin_directory(destination.parent().context("目标文件没有父目录")?)?;
    let source_handle = file_handle_access(source, true, false, digest.is_some())?;
    ensure!(
        &identity_of(&source_handle)? == expected,
        "源文件已变化，原方案失效"
    );
    ensure!(!expected.directory, "首版只移动普通文件");
    if let Some(expected_digest) = digest {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        ensure!(expected.size <= 1024 * 1024, "content_budget");
        let mut bytes = Vec::new();
        source_handle
            .try_clone()?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        ensure!(&actual == expected_digest, "content_changed");
    }
    let parent_id = identity(destination.parent().unwrap())?;
    ensure!(parent_id.volume == expected.volume, "不支持跨卷移动");
    let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
    let bytes = offset_of!(FILE_RENAME_INFO, FileName) + name.len() * 2;
    // usize storage provides sufficient alignment for FILE_RENAME_INFO and its flexible array.
    let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let rename = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*rename).Anonymous.ReplaceIfExists = false;
        (*rename).RootDirectory = ptr::null_mut();
        (*rename).FileNameLength = (name.len() * 2) as u32;
        ptr::copy_nonoverlapping(
            name.as_ptr(),
            ptr::addr_of_mut!((*rename).FileName).cast::<u16>(),
            name.len(),
        );
    }
    ensure!(
        unsafe {
            SetFileInformationByHandle(
                handle(&source_handle),
                FileRenameInfo,
                rename.cast(),
                bytes as u32,
            )
        } != 0,
        "移动失败（不会覆盖目标）：{}",
        std::io::Error::last_os_error()
    );
    ensure!(
        identity(destination)?.same_object(expected),
        "移动后文件身份无法核实"
    );
    Ok(())
}

pub fn open_document(path: &Path, expected: &FileIdentity) -> Result<()> {
    open_document_with_environment(path, expected, &[], &crate::core::Cancellation::default())
}

pub fn open_document_with_environment(
    path: &Path,
    expected: &FileIdentity,
    excluded: &[String],
    cancel: &crate::core::Cancellation,
) -> Result<()> {
    let _guard = pin_directory(path.parent().context("无法打开卷根目录")?)?;
    let file = file_handle(path, false, true)?;
    ensure!(&identity_of(&file)? == expected, "对象已变化，请重新查找");
    ensure!(
        crate::shell_handoff::document(path, expected, excluded, cancel)?,
        "打开已取消，未报告成功"
    );
    Ok(())
}

/// Only the internal process with a rebuilt environment invokes the OS shell.
pub(crate) fn open_document_direct(path: &Path, expected: &FileIdentity) -> Result<()> {
    use windows::{
        Win32::UI::{
            Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
        core::PCWSTR,
    };
    let _guard = pin_directory(path.parent().context("无法打开卷根目录")?)?;
    let file = file_handle(path, false, true)?;
    ensure!(&identity_of(&file)? == expected, "对象已变化，请重新查找");
    // Permit document writes but hold the identity against replacement through handoff.
    // ShellExecute is not proof of the application's final state.
    let normal = path
        .to_string_lossy()
        .strip_prefix(r"\\?\")
        .unwrap_or(&path.to_string_lossy())
        .to_owned();
    let target = wide(OsStr::new(&normal));
    let verb = wide(OsStr::new("open"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        // Complete the shell handoff before the short-lived helper exits.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(target.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    let result = unsafe { ShellExecuteExW(&mut info) };
    if info.hInstApp.0 as isize == 31 {
        anyhow::bail!("此类型尚未配置 Windows 默认打开应用，请先在系统设置中选择关联应用");
    }
    ensure!(
        result.is_ok(),
        "Windows 未接受打开请求（代码 {}）；未报告文件已显示或程序已运行",
        info.hInstApp.0 as isize
    );
    Ok(())
}
