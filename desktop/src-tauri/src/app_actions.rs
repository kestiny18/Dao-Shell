//! Explicit UI-only application actions. Inventory text never becomes a command.
#[cfg(test)]
use dao_shell::platform;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use dao_shell::application_actions::launch::{Target, discover, pin};
#[cfg(test)]
use dao_shell::application_actions::launch::{local_exe, parse_link};
#[derive(Default)]
pub struct Catalog {
    targets: HashMap<String, Option<Target>>,
}
const MISSING: &str = "未找到唯一同名、无参数的本地开始菜单程序入口，请从 Windows 开始菜单操作";
impl Catalog {
    pub fn running_targets(&self) -> HashMap<String, PathBuf> {
        let mut counts = HashMap::new();
        for target in self.targets.values().flatten() {
            *counts
                .entry(target.exe.to_string_lossy().to_lowercase())
                .or_insert(0usize) += 1;
        }
        self.targets
            .iter()
            .filter_map(|(id, target)| {
                let target = target.as_ref()?;
                if counts.get(&target.exe.to_string_lossy().to_lowercase()) != Some(&1) {
                    return None;
                }
                let _link = pin(&target.link, &target.link_identity).ok()?;
                let _exe = pin(&target.exe, &target.identity).ok()?;
                Some((id.clone(), target.exe.clone()))
            })
            .collect()
    }
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
        excluded_environment: &[String],
    ) -> Result<Value, String> {
        self.execute_with(id, action, |action, path| {
            cancel.check().map_err(|e| e.to_string())?;
            dao_shell::shell_handoff::application(action, path, excluded_environment, cancel)
                .map_err(|e| e.to_string())
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
    fn running_association_requires_unique_pinned_exact_shortcut_target() {
        let (_dir, target) = fixture();
        let unique = catalog(Some(target.clone()));
        assert_eq!(unique.running_targets().get("fixture"), Some(&target.exe));
        let shared = Catalog {
            targets: HashMap::from([
                ("a".into(), Some(target.clone())),
                ("b".into(), Some(target.clone())),
            ]),
        };
        assert!(shared.running_targets().is_empty());
        std::fs::write(&target.link, "replaced synthetic shortcut").unwrap();
        assert!(unique.running_targets().is_empty());
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
        // Shell resolution may expand an 8.3 TEMP path on Windows runners.
        assert_eq!(
            parsed.exe.canonicalize().unwrap(),
            t.exe.canonicalize().unwrap()
        );
        assert!(parsed.identity.same_object(&t.identity));
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
        assert!(c.execute("fixture", "uninstall", &cancel, &[]).is_err());
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
