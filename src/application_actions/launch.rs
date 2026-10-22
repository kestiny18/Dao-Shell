//! Shared conservative Start-menu discovery; no executable dispatch surface.
use crate::{core::FileIdentity, platform};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Clone)]
pub struct Target {
    pub link: PathBuf,
    pub link_identity: FileIdentity,
    pub exe: PathBuf,
    pub identity: FileIdentity,
}
pub fn pin(
    path: &Path,
    expected: &FileIdentity,
) -> Result<(platform::DirectoryGuard, std::fs::File), String> {
    let parent =
        platform::pin_directory(path.parent().ok_or("缺少父目录")?).map_err(|e| e.to_string())?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options
        .open(path)
        .map_err(|_| "入口不可读取或正在变化，请刷新概览")?;
    platform::require_identity(path, expected).map_err(|_| "入口已变化，请刷新概览")?;
    Ok((parent, file))
}
#[cfg(any(windows, test))]
pub fn local_exe(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let b = text.as_bytes();
    if b.len() < 4
        || !b[0].is_ascii_alphabetic()
        || b[1] != b':'
        || b[2] != b'\\'
        || text[3..].contains(':')
    {
        return false;
    }
    if path
        .extension()
        .and_then(|p| p.to_str())
        .is_none_or(|p| !p.eq_ignore_ascii_case("exe"))
    {
        return false;
    }
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    ![
        "cmd",
        "powershell",
        "pwsh",
        "wscript",
        "cscript",
        "mshta",
        "rundll32",
        "regsvr32",
        "msiexec",
        "explorer",
        "reg",
        "schtasks",
    ]
    .contains(&name.as_str())
        && ![
            "unins",
            "uninstall",
            "setup",
            "install",
            "update",
            "repair",
            "mainten",
        ]
        .iter()
        .any(|s| name.contains(s))
}

#[cfg(not(windows))]
pub fn discover() -> Result<HashMap<String, Vec<Option<Target>>>, String> {
    Ok(HashMap::new())
}
#[cfg(windows)]
pub fn discover() -> Result<HashMap<String, Vec<Option<Target>>>, String> {
    use windows::Win32::{
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        UI::Shell::{
            FOLDERID_CommonPrograms, FOLDERID_Programs, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    let _apartment = Apartment;
    let mut found = HashMap::<String, Vec<Option<Target>>>::new();
    let mut dirs = Vec::new();
    #[cfg(debug_assertions)]
    let fixture = match std::env::var_os("DAO_SHELL_ACTION_FIXTURE_ROOT") {
        Some(root) => Some(fixture_root(
            Path::new(&root),
            std::env::var_os("DAO_SHELL_DESKTOP_CONFIG")
                .as_deref()
                .map(Path::new),
        )?),
        None => None,
    };
    #[cfg(debug_assertions)]
    if let Some(root) = &fixture {
        let text = root.to_string_lossy();
        dirs.push((
            PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)).join("menu"),
            0,
        ));
    }
    #[cfg(debug_assertions)]
    let ordinary = fixture.is_none();
    #[cfg(not(debug_assertions))]
    let ordinary = true;
    for folder in if ordinary {
        vec![&FOLDERID_Programs, &FOLDERID_CommonPrograms]
    } else {
        vec![]
    } {
        if let Ok(raw) = unsafe { SHGetKnownFolderPath(folder, KF_FLAG_DEFAULT, None) } {
            let path = unsafe { raw.to_string() };
            unsafe { CoTaskMemFree(Some(raw.0.cast())) };
            if let Ok(path) = path
                && path.as_bytes().get(1) == Some(&b':')
            {
                dirs.push((PathBuf::from(path), 0));
            }
        }
    }
    let mut count = 0;
    while let Some((dir, depth)) = dirs.pop() {
        if depth > 8 {
            continue;
        }
        if !fixed_drive(&dir) {
            continue;
        }
        let Ok(_parents) = platform::pin_directory(&dir) else {
            continue;
        };
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            continue;
        };
        if platform::is_link(&meta) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            count += 1;
            if count > 4096 {
                return Ok(HashMap::new());
            }
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if platform::is_link(&meta) {
                continue;
            }
            if meta.is_dir() {
                dirs.push((path, depth + 1));
                continue;
            }
            if path
                .extension()
                .is_none_or(|s| !s.eq_ignore_ascii_case("lnk"))
            {
                continue;
            }
            let key = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            // Count unsupported duplicates too: ambiguity must not select the other entry.
            let target = parse_link(&path);
            #[cfg(debug_assertions)]
            let target = target.filter(|t| {
                fixture.as_ref().is_none_or(|root| {
                    t.exe
                        .canonicalize()
                        .ok()
                        .is_some_and(|exe| exe.starts_with(root))
                })
            });
            found.entry(key).or_default().push(target);
        }
    }
    Ok(found)
}

/// Explicit debug fixture only. Invalid overrides fail closed, never fall back to real menus.
#[cfg(all(windows, debug_assertions))]
fn fixture_root(root: &Path, config: Option<&Path>) -> Result<PathBuf, String> {
    let config = config.ok_or("invalid_fixture")?;
    let _root = platform::pin_directory(root).map_err(|_| "invalid_fixture")?;
    let root = root.canonicalize().map_err(|_| "invalid_fixture")?;
    let config_meta = std::fs::symlink_metadata(config).map_err(|_| "invalid_fixture")?;
    if !config_meta.is_file()
        || platform::is_link(&config_meta)
        || config
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .as_ref()
            != Some(&root)
    {
        return Err("invalid_fixture".into());
    }
    let marker = root.join("phase2-owned-fixture.marker");
    let meta = std::fs::symlink_metadata(&marker).map_err(|_| "invalid_fixture")?;
    if !meta.is_file()
        || platform::is_link(&meta)
        || meta.len() != b"DaoShellPhase2OwnedFixture".len() as u64
        || std::fs::read(&marker).ok().as_deref() != Some(b"DaoShellPhase2OwnedFixture")
    {
        return Err("invalid_fixture".into());
    }
    let _menu = platform::pin_directory(&root.join("menu")).map_err(|_| "invalid_fixture")?;
    Ok(root)
}

#[cfg(all(test, windows, debug_assertions))]
mod fixture_tests {
    use super::*;
    #[test]
    fn explicit_config_marker_and_non_reparse_menu_are_required() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.json");
        std::fs::write(&config, b"{}").unwrap();
        assert!(fixture_root(root.path(), None).is_err());
        assert!(fixture_root(root.path(), Some(&config)).is_err());
        std::fs::write(
            root.path().join("phase2-owned-fixture.marker"),
            b"DaoShellPhase2OwnedFixture",
        )
        .unwrap();
        assert!(fixture_root(root.path(), Some(&config)).is_err());
        std::fs::create_dir(root.path().join("menu")).unwrap();
        assert!(fixture_root(root.path(), Some(&config)).is_ok());
        let other = tempfile::tempdir().unwrap();
        assert!(fixture_root(root.path(), Some(&other.path().join("config.json"))).is_err());
    }
}
#[cfg(windows)]
fn fixed_drive(path: &Path) -> bool {
    use windows::{Win32::Storage::FileSystem::GetDriveTypeW, core::PCWSTR};
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
        return false;
    }
    let root: Vec<u16> = s[..3].encode_utf16().chain(Some(0)).collect();
    unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) == 3 }
}
#[cfg(windows)]
pub fn parse_link(path: &Path) -> Option<Target> {
    use windows::{
        Win32::{
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ},
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{Interface, PCWSTR},
    };
    unsafe {
        if !fixed_drive(path) {
            return None;
        }
        let _parents = platform::pin_directory(path.parent()?).ok()?;
        let link_identity = platform::identity(path).ok()?;
        let _locked = pin(path, &link_identity).ok()?;
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = shell.cast().ok()?;
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).ok()?;
        let mut args = [0u16; 32768];
        shell.GetArguments(&mut args).ok()?;
        if args[0] != 0 {
            return None;
        }
        let mut exe = [0u16; 32768];
        shell.GetPath(&mut exe, std::ptr::null_mut(), 4).ok()?;
        let exe =
            PathBuf::from(String::from_utf16(&exe[..exe.iter().position(|c| *c == 0)?]).ok()?);
        if !local_exe(&exe) {
            return None;
        }
        let mut cwd = [0u16; 32768];
        shell.GetWorkingDirectory(&mut cwd).ok()?;
        let cwd = String::from_utf16(&cwd[..cwd.iter().position(|c| *c == 0)?]).ok()?;
        if !cwd.is_empty() && !cwd.eq_ignore_ascii_case(&exe.parent()?.to_string_lossy()) {
            return None;
        }
        // Reject reparse parents before touching the target (including remote junctions).
        if !fixed_drive(&exe) {
            return None;
        }
        let _parents = platform::pin_directory(exe.parent()?).ok()?;
        let identity = platform::identity(&exe).ok()?;
        if identity.directory {
            return None;
        }
        Some(Target {
            link: path.to_path_buf(),
            link_identity,
            exe,
            identity,
        })
    }
}
