//! Session-owned opaque references and fixed maintenance entry points.
use super::{contract::*, engine::Engine, startup::PinnedShortcut};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, time::Instant};
#[derive(Default)]
pub struct Store {
    pub directory: Option<PathBuf>,
    pub used: bool,
    source: Option<String>,
    packages: HashMap<String, (Package, Instant)>,
    shortcuts: HashMap<String, (PinnedShortcut, String, Instant)>,
    pub context: Value,
    pub delivered: Vec<Value>,
}
pub fn is_tool(s: &str) -> bool {
    matches!(
        s,
        "software_search"
            | "software_inspect"
            | "software_install"
            | "software_upgrade"
            | "software_uninstall"
            | "startup_change"
            | "network_maintenance"
    )
}
pub fn is_write(s: &str) -> bool {
    is_tool(s) && !matches!(s, "software_search" | "software_inspect")
}
pub fn error_code(error: &anyhow::Error) -> &'static str {
    match error.to_string().as_str() {
        "maintenance_reconciliation_required" => "maintenance_reconciliation_required",
        "side_effect_already_attempted" => "side_effect_already_attempted",
        "invalid_arguments" => "invalid_arguments",
        "software_source_confirmation_required" => "software_source_confirmation_required",
        _ => "maintenance_unavailable_or_stale",
    }
}
fn provider() -> Result<Box<dyn Provider>> {
    #[cfg(windows)]
    {
        Ok(Box::new(super::typed::NativeProvider::open().map_err(
            |_| anyhow::anyhow!("software_provider_unavailable"),
        )?))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("unsupported_platform")
    }
}
impl Store {
    fn engine(&self) -> Result<Engine> {
        Engine::open(
            self.directory
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("maintenance_journal_unavailable"))?,
        )
    }
    pub fn unresolved(&self) -> bool {
        if self.directory.is_none() {
            return false;
        }
        self.engine().and_then(|e| e.unresolved()).unwrap_or(true)
    }
    pub fn begin_turn(&mut self) {
        self.used = false;
        self.delivered.clear();
    }
    pub fn reset(&mut self) {
        self.source = None;
        self.packages.clear();
        self.shortcuts.clear();
        self.delivered.clear();
    }
    pub fn local(&mut self, args: Value) -> Result<Value> {
        match args["kind"].as_str() {
            Some("source_authorize") => {
                let token = uuid::Uuid::new_v4().to_string();
                self.source = Some(token.clone());
                Ok(
                    json!({"source_ref":token,"source":"winget","network_access":"official_community_catalog_only"}),
                )
            }
            Some("history") => Ok(json!({"receipts":self.engine()?.history()?})),
            Some("startup_targets") => self.startup_targets(),
            _ => anyhow::bail!("unsupported_maintenance_entry"),
        }
    }
    fn remember(&mut self, p: Package) -> Value {
        let key = uuid::Uuid::new_v4().to_string();
        let row = json!({"package_ref":key,"version_ref":key,"installed_ref":if matches!(p.installation,Installation::Present{..}){Some(key.clone())}else{None},"name":p.name,"publisher":p.publisher,"package_id":p.identity.package_id,"version":p.identity.version,"scope":p.identity.scope,"source":"winget","installation":p.installation,"installation_state":match &p.installation{Installation::AbsentVerified{..}=>"absent_verified",Installation::Present{..}=>"present",_=>"unknown"},"installed_version":match &p.installation{Installation::Present{identity,..}=>Some(identity.version.clone()),_=>None},"supported_actions":match &p.installation{Installation::AbsentVerified{..}=>vec!["install"],Installation::Present{identity,..}=>if identity.version==p.identity.version{vec!["uninstall"]}else{vec!["upgrade","uninstall"]},_=>vec![]}});
        self.packages.insert(key, (p, Instant::now()));
        row
    }
    pub fn call(
        &mut self,
        name: &str,
        args: Value,
        cancel: &crate::core::Cancellation,
        ui: &mut dyn crate::runtime::Interaction,
    ) -> Result<Value> {
        validate_arguments(name, &args)?;
        if is_write(name) {
            ensure!(!self.used, "side_effect_already_attempted");
            self.used = true;
            ensure!(!self.unresolved(), "maintenance_reconciliation_required");
        }
        match name {
            "software_search" => {
                ensure!(
                    self.source.as_deref() == args["source_ref"].as_str() && self.source.is_some(),
                    "software_source_confirmation_required"
                );
                let query = args["query"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid_arguments"))?;
                let mut provider = provider()?;
                let packages = provider.search(query)?;
                self.packages.clear();
                Ok(
                    json!({"items":packages.into_iter().map(|p|self.remember(p)).collect::<Vec<_>>(),"coverage":"provider_scoped","source":"winget","missing_means":"unknown"}),
                )
            }
            "software_inspect" => {
                let key = args["package_ref"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid_arguments"))?;
                let (p, t) = self
                    .packages
                    .get(key)
                    .ok_or_else(|| anyhow::anyhow!("reference_expired"))?;
                ensure!(t.elapsed().as_secs() < 300, "reference_expired");
                let p = provider()?.inspect(&p.identity)?;
                Ok(self.remember(p))
            }
            "software_install" | "software_upgrade" | "software_uninstall" => {
                let key = args[if name == "software_install" {
                    "package_ref"
                } else {
                    "installed_ref"
                }]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("invalid_arguments"))?;
                let (p, t) = self
                    .packages
                    .get(key)
                    .ok_or_else(|| anyhow::anyhow!("reference_expired"))?;
                ensure!(t.elapsed().as_secs() < 300, "reference_expired");
                if name != "software_uninstall" {
                    ensure!(
                        args["version_ref"].as_str() == Some(key),
                        "version_reference_mismatch"
                    );
                }
                if name == "software_install" {
                    ensure!(
                        args["scope"] == serde_json::to_value(p.identity.scope)?,
                        "scope_changed"
                    );
                }
                let mut provider = provider()?;
                let target = if name == "software_uninstall" {
                    match &p.installation {
                        Installation::Present { identity, .. } => identity,
                        _ => anyhow::bail!("installation_state_not_authoritative"),
                    }
                } else {
                    &p.identity
                };
                let package = provider.inspect(target)?;
                ensure!(
                    package.installation == p.installation,
                    "package_state_changed"
                );
                let mut plan = Plan::prepare(
                    match name {
                        "software_install" => Operation::Install,
                        "software_upgrade" => Operation::Upgrade,
                        _ => Operation::Uninstall,
                    },
                    package,
                )?;
                plan.context = self.context.clone();
                if plan.operation == Operation::Upgrade
                    && let Installation::Present { identity, .. } = &plan.package.installation
                {
                    ensure!(
                        provider.compare_versions(&plan.package.identity, &identity.version)?
                            == std::cmp::Ordering::Greater,
                        "target_not_newer"
                    );
                }
                let ui = std::cell::RefCell::new(ui);
                let context = self.context.clone();
                self.engine()?.execute(
                    &plan,
                    provider.as_mut(),
                    cancel,
                    &mut |p| {
                        let mut preview = serde_json::to_value(p)?;
                        preview["context"] = context.clone();
                        ui.borrow_mut().confirm_maintenance(&preview)
                    },
                    &mut |stage| {
                        ui.borrow_mut()
                            .result("maintenance_progress", &json!({"stage":stage}))
                    },
                )
            }
            "startup_change" => self.startup_change(args, cancel, ui),
            "network_maintenance" => {
                ensure!(
                    args["kind"] == "flush_dns_cache" && args.get("endpoint_ref").is_none(),
                    "unsupported_network_maintenance"
                );
                let receipt = json!({"operation_id":uuid::Uuid::new_v4().to_string(),"operation":"flush_dns_cache","context":self.context,"risk":"Clears this computer resolver cache only; application caches and connectivity are not verified. No automatic retry or network retest."});
                self.engine()?.local_transition(
                    receipt,
                    cancel,
                    &mut |p| Ok(ui.confirm_maintenance(p)?.approved),
                    &mut super::dns::flush,
                )
            }
            _ => anyhow::bail!("unsupported"),
        }
    }
    fn startup_root() -> Result<PathBuf> {
        #[cfg(windows)]
        {
            use windows::Win32::UI::Shell::*;
            let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None) }?;
            let s = unsafe { raw.to_string() }?;
            unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast())) };
            Ok(PathBuf::from(s))
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("unsupported_platform")
        }
    }
    fn startup_targets(&mut self) -> Result<Value> {
        let root = Self::startup_root()?;
        let mut items = Vec::new();
        self.shortcuts.clear();
        if root.exists() {
            for entry in std::fs::read_dir(&root)?.take(256) {
                let entry = entry?;
                if let Ok(p) = PinnedShortcut::inspect(&entry.path()) {
                    let key = uuid::Uuid::new_v4().to_string();
                    items.push(json!({"startup_ref":key,"name":entry.file_name().to_string_lossy(),"path":entry.path(),"mode":"disable","coverage":"current_user_startup_lnk"}));
                    self.shortcuts
                        .insert(key, (p, "disable".into(), Instant::now()));
                }
            }
        }
        for r in self.engine()?.history()? {
            if r["operation"] == "startup_disable"
                && r["lifecycle"] == "verified"
                && let Some(path) = r["evidence"]["retained"].as_str()
                && let Ok(p) = PinnedShortcut::inspect(std::path::Path::new(path))
            {
                if !retained_matches(&p, &r["evidence"]) {
                    continue;
                }
                let key = uuid::Uuid::new_v4().to_string();
                items.push(json!({"startup_ref":key,"name":r["original_name"],"path":r["original"],"mode":"restore"}));
                self.shortcuts.insert(
                    key,
                    (
                        p,
                        r["original"].as_str().unwrap_or("").into(),
                        Instant::now(),
                    ),
                );
            }
        }
        Ok(
            json!({"items":items,"coverage":"current_user_startup_lnk_only","limits":"Registration is not proof of enabled state. Registry, RunOnce, services and tasks cannot be changed."}),
        )
    }
    fn startup_change(
        &mut self,
        args: Value,
        cancel: &crate::core::Cancellation,
        ui: &mut dyn crate::runtime::Interaction,
    ) -> Result<Value> {
        let key = args["startup_ref"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid_arguments"))?;
        let (p, target, t) = self
            .shortcuts
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("reference_expired"))?;
        ensure!(t.elapsed().as_secs() < 300, "reference_expired");
        let root = Self::startup_root()?;
        let operation_id = uuid::Uuid::new_v4().to_string();
        let disable = target == "disable";
        ensure!(
            args["mode"] == if disable { "disable" } else { "restore" },
            "startup_mode_mismatch"
        );
        let retained = root
            .parent()
            .ok_or_else(|| anyhow::anyhow!("unsupported_startup_path"))?
            .join("DaoShellDisabledStartup");
        let original = if disable {
            p.path.clone()
        } else {
            PathBuf::from(target)
        };
        ensure!(
            original.parent() == Some(root.as_path()),
            "startup_scope_changed"
        );
        let destination = if disable {
            retained.join(format!("{operation_id}.lnk"))
        } else {
            original.clone()
        };
        let p = p.clone();
        let receipt = json!({"operation_id":operation_id,"operation":if disable{"startup_disable"}else{"startup_restore"},"original":original,"original_name":original.file_name().map(|s|s.to_string_lossy()),"target":destination,"context":self.context,"risk":"Changes next-login Startup registration only. Does not stop a running app or prove enabled state."});
        self.engine()?.local_transition(receipt,cancel,&mut |v|Ok(ui.confirm_maintenance(v)?.approved),&mut ||{
            let _guard=crate::platform::pin_directory(root.parent().unwrap())?;
            with_retention(&retained,disable,||{
                let moved=p.relocate(&destination)?;
                Ok(json!({"identity":moved.identity,"digest":moved.digest.to_vec(),"lifecycle":"verified","retained":if disable{Some(moved.path)}else{None},"same_identity":true,"mode":if disable{"disabled_registration"}else{"restored_registration"}}))
            })
        })
    }
}
fn with_retention<T>(
    path: &std::path::Path,
    create: bool,
    dispatch: impl FnOnce() -> Result<T>,
) -> Result<T> {
    use std::io::{Read, Write};
    const MARKER: &[u8] = b"dao-shell-startup-v1";
    let created = if create && !path.exists() {
        std::fs::create_dir(path)?;
        true
    } else {
        false
    };
    let _guard = crate::platform::pin_directory(path)?;
    let marker = path.join(".dao-owner");
    if created {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)?
            .write_all(MARKER)?;
    }
    let metadata = std::fs::symlink_metadata(&marker)?;
    ensure!(
        metadata.is_file()
            && !crate::platform::is_link(&metadata)
            && metadata.len() == MARKER.len() as u64,
        "retention_directory_conflict"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(marker)?
        .take((MARKER.len() + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes == MARKER, "retention_directory_conflict");
    dispatch()
}

pub fn projection(value: &Value) -> Value {
    let mut result = json!({});
    if let Some(rows) = value["items"].as_array() {
        result["items"] = json!(rows.iter().map(projection).collect::<Vec<_>>());
    }
    for key in [
        "source_ref",
        "package_ref",
        "version_ref",
        "installed_ref",
        "startup_ref",
        "name",
        "package_id",
        "version",
        "scope",
        "source",
        "supported_actions",
        "installation_state",
        "installed_version",
        "coverage",
        "missing_means",
        "status",
        "operation_id",
        "operation",
        "lifecycle",
        "mode",
        "error",
    ] {
        if let Some(v) = value.get(key) {
            result[key] = v.clone();
        }
    }
    for key in [
        "cache_clear_reported",
        "connectivity_verified",
        "dns_server_changed",
        "restart_requested",
        "mode",
        "reboot_required",
        "provider_code",
    ] {
        if let Some(v) = value["evidence"].get(key) {
            result[key] = v.clone();
        }
    }
    result
}

fn retained_matches(p: &PinnedShortcut, evidence: &Value) -> bool {
    serde_json::from_value::<crate::core::FileIdentity>(evidence["identity"].clone())
        .is_ok_and(|identity| identity.same_object(&p.identity))
        && evidence["digest"] == json!(p.digest.to_vec())
}
fn validate_arguments(name: &str, args: &Value) -> Result<()> {
    let definition = crate::capabilities::desktop_definitions()
        .into_iter()
        .find(|d| d["function"]["name"] == name)
        .ok_or_else(|| anyhow::anyhow!("unsupported"))?;
    let schema = &definition["function"]["parameters"];
    let object = args
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid_arguments"))?;
    let props = schema["properties"].as_object().unwrap();
    ensure!(
        object.keys().all(|k| props.contains_key(k)),
        "invalid_arguments"
    );
    for key in schema["required"].as_array().unwrap() {
        ensure!(
            object.contains_key(key.as_str().unwrap()),
            "invalid_arguments"
        );
    }
    for (key, value) in object {
        let prop = &props[key];
        ensure!(value.is_string(), "invalid_arguments");
        let text = value.as_str().unwrap();
        ensure!(
            text.len() <= 1024 && !text.chars().any(char::is_control),
            "invalid_arguments"
        );
        if let Some(max) = prop["maxLength"].as_u64() {
            ensure!(text.chars().count() <= max as usize, "invalid_arguments");
        }
        if let Some(values) = prop["enum"].as_array() {
            ensure!(values.contains(value), "invalid_arguments");
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_error_details_are_not_public_history_or_model_text() {
        assert_eq!(
            error_code(&anyhow::anyhow!("PRIVATE_NATIVE_PATH: native failure")),
            "maintenance_unavailable_or_stale"
        );
        assert_eq!(
            error_code(&anyhow::anyhow!("maintenance_reconciliation_required")),
            "maintenance_reconciliation_required"
        );
    }
    #[cfg(windows)]
    #[test]
    fn retained_directory_is_owned_bounded_and_never_overwrites_a_foreign_marker() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retained");
        assert_eq!(with_retention(&path, true, || Ok(1)).unwrap(), 1);
        assert_eq!(with_retention(&path, false, || Ok(2)).unwrap(), 2);
        let marker = path.join(".dao-owner");
        std::fs::write(&marker, b"foreign").unwrap();
        let mut called = false;
        assert!(
            with_retention(&path, true, || {
                called = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!called);
        assert_eq!(std::fs::read(&marker).unwrap(), b"foreign");
    }
    #[cfg(windows)]
    #[test]
    fn restore_proof_requires_original_identity_and_digest() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.lnk");
        let mut bytes = vec![0; 76];
        bytes[..4].copy_from_slice(&[76, 0, 0, 0]);
        bytes[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        std::fs::write(&path, &bytes).unwrap();
        let original = PinnedShortcut::inspect(&path).unwrap();
        let evidence = json!({"identity":original.identity,"digest":original.digest.to_vec()});
        assert!(retained_matches(&original, &evidence));
        assert!(!retained_matches(&original, &json!({})));
        bytes[40] = 1;
        std::fs::write(&path, bytes).unwrap();
        let changed = PinnedShortcut::inspect(&path).unwrap();
        assert!(!retained_matches(&changed, &evidence));
    }
    #[test]
    fn projection_excludes_native_paths_terms_and_tokens_but_preserves_dns_limits() {
        let v = projection(
            &json!({"operation":"flush_dns_cache","lifecycle":"verified","target":"PRIVATE_PATH","context":{"session_id":"PRIVATE_TOKEN"},"agreements":["PRIVATE_TERMS"],"evidence":{"cache_clear_reported":true,"connectivity_verified":false,"native_detail":"PRIVATE_NATIVE"}}),
        );
        assert_eq!(v["cache_clear_reported"], true);
        assert_eq!(v["connectivity_verified"], false);
        assert!(!v.to_string().contains("PRIVATE"));
    }
    #[test]
    fn rejects_forged_authorization_and_command_parameters() {
        for args in [
            json!({"kind":"flush_dns_cache","confirmed":true}),
            json!({"kind":"flush_dns_cache","command":"x"}),
            json!({"kind":"reset_network"}),
            json!({}),
            json!({"kind":7}),
        ] {
            assert!(validate_arguments("network_maintenance", &args).is_err());
        }
        assert!(
            validate_arguments("network_maintenance", &json!({"kind":"flush_dns_cache"})).is_ok()
        );
        assert!(
            validate_arguments(
                "software_search",
                &json!({"source_ref":"fixture","query":"x".repeat(257)})
            )
            .is_err()
        );
    }
    #[test]
    fn reset_revokes_source_and_references() {
        let mut s = Store::default();
        s.local(json!({"kind":"source_authorize"})).unwrap();
        assert!(s.source.is_some());
        s.reset();
        assert!(s.source.is_none());
    }
}
