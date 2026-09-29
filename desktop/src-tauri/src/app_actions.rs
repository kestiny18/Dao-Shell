//! Explicit UI-only application actions. Inventory text never becomes a command.
use dao_shell::{core::FileIdentity, platform};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
struct Target {
    link: PathBuf,
    link_identity: FileIdentity,
    exe: PathBuf,
    identity: FileIdentity,
}
#[derive(Default)]
pub struct Catalog {
    targets: HashMap<String, Option<Target>>,
}
const MISSING: &str = "未找到唯一同名、无参数的本地开始菜单程序入口，请从 Windows 开始菜单操作";
impl Catalog {
    pub fn refresh(&mut self, snapshot: &mut Value) {
        self.bind(snapshot, discover().unwrap_or_default());
    }
    fn bind(&mut self, snapshot: &mut Value, links: HashMap<String, Vec<Option<Target>>>) {
        self.targets.clear();
        if let Some(items) = snapshot["applications"]["items"].as_array_mut() {
            let mut names = HashMap::new();
            for item in items.iter() {
                *names
                    .entry(item["name"].as_str().unwrap_or("").to_lowercase())
                    .or_insert(0usize) += 1;
            }
            for item in items {
                let name = item["name"].as_str().unwrap_or("").to_lowercase();
                let target = links
                    .get(&name)
                    .filter(|v| v.len() == 1 && names.get(&name) == Some(&1))
                    .and_then(|v| v[0].clone());
                let id = uuid::Uuid::new_v4().to_string();
                item["action_id"] = json!(id);
                item["action_reason"] = json!(if target.is_some() { "" } else { MISSING });
                self.targets.insert(id, target);
            }
        }
    }
    pub fn execute(
        &self,
        id: &str,
        action: &str,
        cancel: &dao_shell::core::Cancellation,
    ) -> Result<Value, String> {
        self.execute_with(id, action, |action, path| {
            cancel.check().map_err(|e| e.to_string())?;
            dispatch(action, path)
        })
    }
    fn execute_with(
        &self,
        id: &str,
        action: &str,
        executor: impl FnOnce(&str, Option<&Path>) -> Result<bool, String>,
    ) -> Result<Value, String> {
        if !["run", "admin", "location", "uninstall"].contains(&action) {
            return Err("未知应用操作".into());
        }
        let target = self.targets.get(id).ok_or("应用引用已失效，请刷新概览")?;
        if action == "uninstall" {
            return receipt(executor(action, None)?);
        }
        let t = target.as_ref().ok_or(MISSING)?;
        // Hold both names and file bytes against replacement until OS handoff.
        let _link = pin(&t.link, &t.link_identity)?;
        let _exe = pin(&t.exe, &t.identity)?;
        receipt(executor(action, Some(&t.exe))?)
    }
}
fn receipt(accepted: bool) -> Result<Value, String> {
    Ok(
        json!({"status":if accepted {"handed_off"} else {"cancelled"},"message":if accepted {"请求已交给 Windows；不代表应用已启动或卸载已完成。"} else {"操作已取消，未报告成功。"}}),
    )
}
fn pin(
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
fn local_exe(path: &Path) -> bool {
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Target) {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("Fixture.lnk");
        let exe = dir.path().join("Fixture.exe");
        std::fs::write(&link, "synthetic shortcut").unwrap();
        std::fs::write(&exe, "not executable").unwrap();
        let t = Target {
            link_identity: platform::identity(&link).unwrap(),
            identity: platform::identity(&exe).unwrap(),
            link,
            exe,
        };
        (dir, t)
    }
    fn catalog(t: Option<Target>) -> Catalog {
        Catalog {
            targets: HashMap::from([("fixture".into(), t)]),
        }
    }
    #[test]
    fn rejects_unsafe_executable_names_and_paths() {
        for s in [
            r"\\server\share\App.exe",
            r"\\?\C:\App.exe",
            r"C:\App.exe:stream",
            r"C:\setup.exe",
            r"C:\unins000.exe",
            r"C:\pwsh.exe",
            r"C:\App.cmd",
        ] {
            assert!(!local_exe(Path::new(s)), "{s}");
        }
        assert!(local_exe(Path::new(r"C:\Fixture\App.exe")));
    }
    #[test]
    fn parses_only_local_argument_free_shortcut_and_rejects_changed_contents() {
        use windows::{
            Win32::{
                System::Com::{
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance,
                    CoInitializeEx, CoUninitialize, IPersistFile,
                },
                UI::Shell::{IShellLinkW, ShellLink},
            },
            core::{Interface, PCWSTR},
        };
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe { CoUninitialize() };
            }
        }
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
        }
        let _apartment = Apartment;
        let (_dir, t) = fixture();
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let save = |target: &str, args: &str| unsafe {
            let shell: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            shell.SetPath(PCWSTR(wide(target).as_ptr())).unwrap();
            shell.SetArguments(PCWSTR(wide(args).as_ptr())).unwrap();
            let persist: IPersistFile = shell.cast().unwrap();
            persist
                .Save(PCWSTR(wide(&t.link.to_string_lossy()).as_ptr()), true)
                .unwrap();
        };
        save(&t.exe.to_string_lossy(), "");
        let parsed = parse_link(&t.link).expect("safe synthetic shortcut");
        assert_eq!(parsed.exe, t.exe);
        save(&t.exe.to_string_lossy(), "--uninstall");
        assert!(parse_link(&t.link).is_none());
        assert!(
            catalog(Some(parsed))
                .execute_with("fixture", "run", |_, _| panic!("changed link"))
                .is_err()
        );
    }
    #[test]
    fn action_allowlist_missing_and_stale_ids_do_not_dispatch() {
        let c = catalog(None);
        for (id, action) in [
            ("fixture", "run"),
            ("fixture", "admin"),
            ("fixture", "location"),
            ("fixture", "cmd"),
            ("old", "uninstall"),
        ] {
            assert!(
                c.execute_with(id, action, |_, _| panic!("must not execute"))
                    .is_err()
            );
        }
        let result = c
            .execute_with("fixture", "uninstall", |a, p| {
                assert_eq!(a, "uninstall");
                assert!(p.is_none());
                Ok(true)
            })
            .unwrap();
        assert_eq!(result["status"], "handed_off");
        let cancel = dao_shell::core::Cancellation::default();
        cancel.cancel();
        assert!(c.execute("fixture", "uninstall", &cancel).is_err());
    }
    #[test]
    fn changed_link_or_exe_rejects_without_dispatch() {
        for replace_link in [false, true] {
            let (_dir, t) = fixture();
            let changed = if replace_link { &t.link } else { &t.exe };
            std::fs::remove_file(changed).unwrap();
            std::fs::write(changed, "replacement bytes").unwrap();
            assert!(
                catalog(Some(t))
                    .execute_with("fixture", "run", |_, _| panic!("must not execute"))
                    .is_err()
            );
        }
    }
    #[test]
    fn handoff_holds_both_files_and_preserves_cancel_and_failure() {
        let (_dir, t) = fixture();
        let c = catalog(Some(t.clone()));
        let result = c
            .execute_with("fixture", "admin", |a, p| {
                assert_eq!(a, "admin");
                assert_eq!(p, Some(t.exe.as_path()));
                assert!(std::fs::write(&t.exe, "replace").is_err());
                assert!(std::fs::remove_file(&t.link).is_err());
                Ok(false)
            })
            .unwrap();
        assert_eq!(result["status"], "cancelled");
        assert!(
            c.execute_with("fixture", "run", |_, _| Err("mock failure".into()))
                .unwrap_err()
                .contains("mock failure")
        );
        assert_eq!(
            c.execute_with("fixture", "location", |_, p| {
                assert_eq!(p, Some(t.exe.as_path()));
                Ok(true)
            })
            .unwrap()["status"],
            "handed_off"
        );
    }
    #[test]
    fn refresh_invalidates_tokens_and_rejects_ambiguous_shortcuts_and_records() {
        let (_dir, t) = fixture();
        let mut c = Catalog::default();
        let mut snapshot = json!({"applications":{"items":[{"name":"Fixture"}]}});
        c.bind(
            &mut snapshot,
            HashMap::from([("fixture".into(), vec![Some(t.clone())])]),
        );
        let old = snapshot["applications"]["items"][0]["action_id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(snapshot["applications"]["items"][0]["action_reason"], "");
        c.bind(
            &mut snapshot,
            HashMap::from([("fixture".into(), vec![Some(t.clone()), None])]),
        );
        assert!(c.execute_with(&old, "uninstall", |_, _| panic!()).is_err());
        assert_ne!(snapshot["applications"]["items"][0]["action_reason"], "");
        snapshot["applications"]["items"] = json!([{"name":"Fixture"},{"name":"Fixture"}]);
        c.bind(
            &mut snapshot,
            HashMap::from([("fixture".into(), vec![Some(t)])]),
        );
        assert!(c.targets.values().all(Option::is_none));
    }
}
#[cfg(not(windows))]
fn discover() -> Result<HashMap<String, Vec<Option<Target>>>, String> {
    Ok(HashMap::new())
}
#[cfg(windows)]
fn discover() -> Result<HashMap<String, Vec<Option<Target>>>, String> {
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
    for folder in [&FOLDERID_Programs, &FOLDERID_CommonPrograms] {
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
            found.entry(key).or_default().push(target);
        }
    }
    Ok(found)
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
#[cfg(not(windows))]
fn dispatch(_: &str, _: Option<&Path>) -> Result<bool, String> {
    Err("此操作仅支持 Windows".into())
}
#[cfg(windows)]
fn dispatch(action: &str, path: Option<&Path>) -> Result<bool, String> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::{
        Win32::{
            Foundation::ERROR_CANCELLED,
            System::Com::CoTaskMemFree,
            UI::{
                Shell::{
                    SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, SHOpenFolderAndSelectItems,
                    SHParseDisplayName, ShellExecuteExW,
                },
                WindowsAndMessaging::SW_SHOWNORMAL,
            },
        },
        core::PCWSTR,
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
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let target = wide(
        &path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ms-settings:appsfeatures".into()),
    );
    if action == "location" {
        let mut pidl = std::ptr::null_mut();
        unsafe {
            SHParseDisplayName(PCWSTR(target.as_ptr()), None, &mut pidl, 0, None)
                .map_err(|e| e.to_string())?;
        }
        let result = unsafe { SHOpenFolderAndSelectItems(pidl, None, 0) };
        unsafe {
            CoTaskMemFree(Some(pidl.cast()));
        }
        return result.map(|_| true).map_err(|e| e.to_string());
    }
    let verb = wide(if action == "admin" { "runas" } else { "open" });
    let directory = path
        .and_then(Path::parent)
        .map(|p| wide(&p.to_string_lossy()));
    let info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(target.as_ptr()),
        lpDirectory: directory
            .as_ref()
            .map_or(PCWSTR::null(), |s| PCWSTR(s.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    let mut info = info;
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => Ok(true),
        Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(false),
        Err(e) => Err(format!("Windows 未接受操作：{e}")),
    }
}

#[cfg(windows)]
fn parse_link(path: &Path) -> Option<Target> {
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
