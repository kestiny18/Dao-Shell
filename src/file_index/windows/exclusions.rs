//! Fixed Windows discovery rules. Paths are never supplied by an IPC request.
use super::*;
use std::collections::BTreeSet;
pub const DEFAULT: &str = "windows_default_v1";
pub const ALL: &str = "windows_all_v1";
pub fn id(include: bool) -> &'static str {
    if include { ALL } else { DEFAULT }
}
struct Rule {
    path: String,
    category: &'static str,
    bound_id: Option<u64>,
    handle: Option<File>,
}
pub(super) struct Exclusions {
    include: bool,
    rules: Vec<Rule>,
    unavailable: usize,
    location_unavailable: usize,
    pub(super) skipped: usize,
    changed: bool,
}
fn within(path: &str, root: &str) -> bool {
    // Native canonical names preserve case, including case-sensitive directories.
    let path = path.trim_end_matches('\\');
    let root = root.trim_end_matches('\\');
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('\\'))
}
fn canonical(file: &File) -> Result<String> {
    let mut value = vec![0u16; 32768];
    let n = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as _,
            value.as_mut_ptr(),
            value.len() as u32,
            VOLUME_NAME_GUID,
        )
    };
    ensure!(
        n > 0 && (n as usize) < value.len(),
        "policy handle path unavailable"
    );
    Ok(String::from_utf16(&value[..n as usize])?)
}
fn bound_directory(path: &str, guid: &str, serial: u32) -> Result<Option<File>> {
    let relative = path
        .strip_prefix(guid)
        .context("policy root outside volume")?;
    let mut current = guid.trim_end_matches('\\').to_owned();
    for part in relative.split('\\').filter(|part| !part.is_empty()) {
        current.push('\\');
        current.push_str(part);
        let file = match open_path(&current) {
            Ok(file) => file,
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| matches!(e.raw_os_error(), Some(2 | 3))) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let value = info(&file)?;
        ensure!(
            value.dwVolumeSerialNumber == serial
                && value.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
                && value.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
                && handle_guid(&file)?.eq_ignore_ascii_case(guid),
            "policy directory identity unavailable"
        );
    }
    let file = open_path(path)?;
    ensure!(
        canonical(&file)?.eq_ignore_ascii_case(path),
        "policy canonical path changed"
    );
    Ok(Some(file))
}
fn component_rules(path: &str) -> Vec<Rule> {
    [
        ("WinSxS", "windows_components"),
        ("servicing", "windows_components"),
        ("Installer", "installer_cache"),
        ("System32\\DriverStore\\FileRepository", "driver_cache"),
        ("Temp", "system_temp"),
        ("SoftwareDistribution\\Download", "update_cache"),
    ]
    .into_iter()
    .map(|(tail, category)| Rule {
        path: format!("{path}\\{tail}"),
        category,
        bound_id: None,
        handle: None,
    })
    .collect()
}
impl Exclusions {
    pub(super) fn new(include: bool, guid: &str, serial: u32) -> Result<Self> {
        let mut rules = vec![
            Rule {
                path: format!("{guid}System Volume Information"),
                category: "volume_internal",
                bound_id: None,
                handle: None,
            },
            Rule {
                path: format!("{guid}$Recycle.Bin"),
                category: "recycle_bin",
                bound_id: None,
                handle: None,
            },
        ];
        let windows_path = (|| -> Result<Option<String>> {
            let mut directory = vec![0u16; 32768];
            let length = unsafe {
                windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW(
                    directory.as_mut_ptr(),
                    directory.len() as u32,
                )
            };
            ensure!(
                length > 0 && (length as usize) < directory.len(),
                "Windows directory unavailable"
            );
            let windows = String::from_utf16(&directory[..length as usize])?;
            let root = open_path(&windows)?;
            if info(&root)?.dwVolumeSerialNumber == serial
                && handle_guid(&root)?.eq_ignore_ascii_case(guid)
            {
                return Ok(Some(canonical(&root)?));
            }
            Ok(None)
        })();
        let location_unavailable = usize::from(windows_path.is_err());
        if let Ok(Some(path)) = windows_path {
            rules.extend(component_rules(&path));
        }
        let mut value = Self {
            include,
            rules,
            unavailable: 0,
            location_unavailable,
            skipped: 0,
            changed: false,
        };
        value.refresh(guid, serial);
        value.changed = false;
        Ok(value)
    }
    pub(super) fn refresh(&mut self, guid: &str, serial: u32) {
        self.unavailable = self.location_unavailable;
        for rule in &mut self.rules {
            let bound = match bound_directory(&rule.path, guid, serial) {
                Ok(value) => value,
                Err(_) => {
                    self.unavailable += 1;
                    None
                }
            };
            let id = bound
                .as_ref()
                .and_then(|file| info(file).ok())
                .map(|value| file_id(&value));
            self.changed |= rule.bound_id != id;
            rule.bound_id = id;
            if let Some(file) = &bound
                && let Ok(path) = canonical(file)
            {
                rule.path = path;
            }
            rule.handle = bound;
        }
    }
    pub(super) fn descriptor(&self) -> CollectionPolicy {
        CollectionPolicy {
            id: id(self.include).into(),
            categories: if self.include {
                vec![]
            } else {
                self.rules
                    .iter()
                    .filter(|r| r.bound_id.is_some())
                    .map(|r| r.category.to_owned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            },
            excluded_roots: if self.include {
                0
            } else {
                self.rules.iter().filter(|r| r.bound_id.is_some()).count()
            },
            unavailable_roots: if self.include { 0 } else { self.unavailable },
            skipped_resolutions: self.skipped,
        }
    }
    fn stable(rule: &Rule) -> bool {
        rule.handle.as_ref().is_some_and(|file| {
            info(file).is_ok_and(|value| {
                Some(file_id(&value)) == rule.bound_id
                    && value.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
            }) && canonical(file).is_ok_and(|path| path.eq_ignore_ascii_case(&rule.path))
        })
    }
    fn declared(&self, path: &str) -> bool {
        !self.include
            && self
                .rules
                .iter()
                .any(|r| r.bound_id.is_some() && within(path, &r.path))
    }
    pub(super) fn excludes(&self, path: &str) -> bool {
        !self.include
            && self
                .rules
                .iter()
                .any(|r| r.bound_id.is_some() && within(path, &r.path) && Self::stable(r))
    }
    fn root_or_ancestor(&self, path: &str) -> bool {
        self.rules.iter().any(|r| within(&r.path, path))
    }
    pub(super) fn checkpoint(&mut self) {
        self.changed = false;
    }
    fn move_affects_rules(&self, old: &str, new: &str) -> bool {
        !self.include
            && (self.root_or_ancestor(old)
                || self.root_or_ancestor(new)
                || self.declared(old) != self.declared(new))
    }
    pub(super) fn moved(&self, old: &str, new: &str) -> bool {
        !self.include
            && (self
                .rules
                .iter()
                .any(|r| (within(old, &r.path) || within(new, &r.path)) && !Self::stable(r))
                || self.move_affects_rules(old, new))
    }
    pub(super) fn changed(&self) -> bool {
        !self.include && self.changed
    }
}
pub(super) fn path(file: &File) -> Result<String> {
    canonical(file)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_windows_location_is_used_without_assuming_c_or_excluding_profiles() {
        let rules = component_rules(r"\\?\Volume{non-c-system}\AlternateOS");
        assert_eq!(rules.len(), 6);
        assert!(rules.iter().all(|rule| {
            rule.path
                .starts_with(r"\\?\Volume{non-c-system}\AlternateOS\")
                && !rule.path.contains("C:")
        }));
        for preserved in ["Users", "AppData", "ProgramData", "Program Files"] {
            assert!(!rules.iter().any(|rule| rule.path.ends_with(preserved)));
        }
    }
    fn fixture(root: &std::path::Path) -> (WindowsVolumeSource, Exclusions) {
        let drive = &root.to_str().unwrap()[..2];
        let source = WindowsVolumeSource::open_metadata(drive).unwrap();
        let file = open_path(root.to_str().unwrap()).unwrap();
        let path = canonical(&file).unwrap();
        let mut policy = Exclusions {
            include: false,
            rules: vec![Rule {
                path,
                category: "system_temp",
                bound_id: None,
                handle: None,
            }],
            unavailable: 0,
            location_unavailable: 0,
            skipped: 0,
            changed: false,
        };
        policy.refresh(&source.guid, source.serial);
        policy.changed = false;
        (source, policy)
    }
    #[test]
    fn native_single_link_skips_expensive_metadata_and_outside_hardlink_remains() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let inside = cache.join("fixture.txt");
        let outside = temp.path().join("outside.txt");
        std::fs::write(&inside, "synthetic").unwrap();
        let (mut source, policy) = fixture(&cache);
        source.policy = Some(policy);
        let id = source.id_for_path(&inside).unwrap();
        assert!(matches!(source.resolve(&id).unwrap(), Resolved::Excluded));
        assert_eq!(source.policy().unwrap().skipped_resolutions, 1);
        std::fs::hard_link(&inside, &outside).unwrap();
        let Resolved::Present(node) = source.resolve(&id).unwrap() else {
            panic!("outside hardlink lost")
        };
        assert_eq!(node.names.len(), 1);
        assert_eq!(node.names[0].name, "outside.txt");
        source.policy.as_mut().unwrap().include = true;
        let Resolved::Present(node) = source.resolve(&id).unwrap() else {
            panic!("inclusion lost")
        };
        assert_eq!(node.names.len(), 2);
        source.policy.as_mut().unwrap().include = false;
        std::fs::remove_file(inside).unwrap();
        assert!(matches!(source.resolve(&id).unwrap(), Resolved::Present(_)));
    }
    #[test]
    fn native_missing_rule_root_and_replacement_fail_open_without_touching_user_content() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let (source, mut policy) = fixture(&cache);
        let declared = policy.rules[0].path.clone();
        std::fs::rename(&cache, temp.path().join("moved-cache")).unwrap();
        std::fs::create_dir(&cache).unwrap();
        assert!(!policy.excludes(&format!("{declared}\\user.txt")));
        policy.refresh(&source.guid, source.serial);
        assert!(policy.changed());
        assert!(policy.excludes(&format!("{declared}\\user.txt")));
        std::fs::remove_dir(&cache).unwrap();
        policy.refresh(&source.guid, source.serial);
        assert_eq!(policy.descriptor().unavailable_roots, 0);
        assert_eq!(policy.descriptor().excluded_roots, 0);
        std::fs::create_dir(&cache).unwrap();
        policy.refresh(&source.guid, source.serial);
        assert_eq!(policy.descriptor().excluded_roots, 1);
    }
    #[test]
    fn native_junction_rule_never_excludes_its_user_data_target() {
        use windows_sys::Win32::{
            Foundation::GENERIC_WRITE,
            System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_REPARSE_POINT},
        };
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("user-data");
        let cache = temp.path().join("cache");
        std::fs::create_dir(&target).unwrap();
        std::fs::create_dir(&cache).unwrap();
        let user = target.join("keep.txt");
        std::fs::write(&user, "synthetic preserved").unwrap();
        let substitute = wide(&format!(r"\??\{}", target.display()));
        let print = wide(target.to_str().unwrap());
        let mut names = Vec::new();
        for value in substitute.iter().chain(&print) {
            names.extend(value.to_le_bytes());
        }
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(&0xa0000003u32.to_le_bytes());
        data[4..6].copy_from_slice(&((names.len() + 8) as u16).to_le_bytes());
        data[10..12].copy_from_slice(&(((substitute.len() - 1) * 2) as u16).to_le_bytes());
        data[12..14].copy_from_slice(&((substitute.len() * 2) as u16).to_le_bytes());
        data[14..16].copy_from_slice(&(((print.len() - 1) * 2) as u16).to_le_bytes());
        data.extend(names);
        let raw = unsafe {
            CreateFileW(
                wide(cache.to_str().unwrap()).as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        let handle = unsafe { File::from_raw_handle(raw as _) };
        let mut returned = 0;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    handle.as_raw_handle() as _,
                    FSCTL_SET_REPARSE_POINT,
                    data.as_ptr().cast(),
                    data.len() as u32,
                    ptr::null_mut(),
                    0,
                    &mut returned,
                    ptr::null_mut(),
                )
            },
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        drop(handle);
        let (mut source, policy) = fixture(&cache);
        assert_eq!(policy.descriptor().unavailable_roots, 1);
        assert_eq!(policy.descriptor().excluded_roots, 0);
        source.policy = Some(policy);
        let id = source.id_for_path(&user).unwrap();
        assert!(matches!(source.resolve(&id).unwrap(), Resolved::Present(_)));
        assert_eq!(
            std::fs::read_to_string(user).unwrap(),
            "synthetic preserved"
        );
        std::fs::remove_dir(cache).unwrap();
    }
    #[test]
    fn native_directory_rename_pairs_preserve_normal_incremental_and_detect_crossings() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let projects = temp.path().join("Projects");
        std::fs::create_dir(&projects).unwrap();
        let (mut source, policy) = fixture(&cache);
        source.policy = Some(policy);
        let cache_id = u64::from_str_radix(&source.id_for_path(&cache).unwrap(), 16).unwrap();
        let projects_id = u64::from_str_radix(&source.id_for_path(&projects).unwrap(), 16).unwrap();
        let record = |parent, reason, name: &str| usn::Record {
            id: 12,
            parent,
            reason,
            name: name.into(),
            attributes: FILE_ATTRIBUTE_DIRECTORY,
            usn: 1,
        };
        assert!(!source.boundary_changed(&[
            record(projects_id, 0x1000, "old"),
            record(projects_id, 0x2000, "new")
        ]));
        assert!(source.boundary_changed(&[
            record(cache_id, 0x1000, "old"),
            record(projects_id, 0x2000, "new")
        ]));
        assert!(source.boundary_changed(&[record(projects_id, 0x2000, "new")]));
        // An unrelated unbound rule must not turn every project rename into a rebuild.
        source.policy.as_mut().unwrap().unavailable = 1;
        assert!(!source.boundary_changed(&[
            record(projects_id, 0x1000, "old"),
            record(projects_id, 0x2000, "new")
        ]));
        source.policy.as_mut().unwrap().include = true;
        assert!(!source.boundary_changed(&[record(projects_id, 0x2000, "new")]));
    }
    fn rules() -> Exclusions {
        Exclusions {
            include: false,
            rules: vec![Rule {
                path: r"\\?\Volume{fixture}\Windows\Temp".into(),
                category: "system_temp",
                bound_id: Some(1),
                handle: None,
            }],
            unavailable: 0,
            location_unavailable: 0,
            skipped: 0,
            changed: false,
        }
    }
    #[test]
    fn rules_preserve_prefix_siblings_user_data_and_normal_project_renames() {
        let mut p = rules();
        assert!(p.declared(r"\\?\Volume{fixture}\Windows\Temp\a"));
        for tail in [
            "Windows\\Temp-old\\a",
            "Users\\a\\AppData\\Local\\app",
            "ProgramData\\app",
            "Program Files\\app",
            "Windows\\System32\\app.exe",
        ] {
            assert!(!p.excludes(&format!(r"\\?\Volume{{fixture}}\{tail}")));
        }
        assert!(!p.move_affects_rules(
            r"\\?\Volume{fixture}\Projects\old",
            r"\\?\Volume{fixture}\Projects\new"
        ));
        assert!(p.move_affects_rules(
            r"\\?\Volume{fixture}\Windows\Temp\old",
            r"\\?\Volume{fixture}\Projects\new"
        ));
        assert!(p.move_affects_rules(
            r"\\?\Volume{fixture}\Windows",
            r"\\?\Volume{fixture}\Windows-old"
        ));
        p.include = true;
        assert!(!p.excludes(r"\\?\Volume{fixture}\Windows\Temp\a"));
        assert!(!p.moved("unknown", "unknown"));
    }
}
