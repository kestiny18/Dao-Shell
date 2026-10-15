use dao_shell::workspace::{Workspace, WorkspaceStore};
use dao_shell::{
    config::Config,
    core::{FileObject, Operation},
    runtime::{Interaction, control::RequestControl},
    session::{Action, FileSession, Reply},
    settings::{self, ConnectionTest, SettingsUpdate, SettingsView},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};
use tauri::ipc::Channel;

pub struct Bridge {
    pub index_service: crate::index_service::Integration,
    pub inventory: std::sync::Arc<dao_shell::inventory::Inventory>,
    config_path: PathBuf,
    configuration_gate: Mutex<()>,
    sessions: Mutex<Sessions>,
    active: Mutex<Option<(String, String)>>,
    workspace: Mutex<WorkspaceStore>,
    pub control: RequestControl,
    overview: Mutex<Option<Value>>,
    applications: Mutex<crate::app_actions::Catalog>,
    processes: Mutex<crate::process_actions::Catalog>,
}
#[derive(Default)]
struct Sessions {
    contexts: HashMap<String, FileSession>,
    registered: HashSet<String>,
    configuration: String,
}
struct Release<'a>(&'a Bridge);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        if let Ok(mut owner) = self.0.active.lock() {
            *owner = None;
            self.0.control.release();
        }
    }
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Bridge {
    pub fn new() -> Self {
        Self::with_path(
            std::env::var_os("DAO_SHELL_DESKTOP_CONFIG")
                .map(PathBuf::from)
                .unwrap_or_else(Config::path),
        )
    }
    fn with_path(config_path: PathBuf) -> Self {
        let inventory =
            dao_shell::inventory::Inventory::new(config_path.with_file_name("file-inventory"));
        if let Ok(scope) = Config::load(&config_path).and_then(|c| c.scope(false)) {
            inventory.resume(&scope);
        }
        Self {
            index_service: crate::index_service::Integration::new(config_path.clone()),
            inventory,
            workspace: Mutex::new(WorkspaceStore::new(
                config_path.with_file_name("workspace.json"),
            )),
            config_path,
            configuration_gate: Mutex::new(()),
            sessions: Mutex::new(Sessions::default()),
            active: Mutex::new(None),
            control: RequestControl::default(),
            overview: Mutex::new(None),
            applications: Mutex::new(crate::app_actions::Catalog::default()),
            processes: Mutex::new(crate::process_actions::Catalog::default()),
        }
    }
    fn path(&self) -> PathBuf {
        self.config_path.clone()
    }
    pub fn load_workspace(&self) -> Result<Workspace, String> {
        let value = self
            .workspace
            .lock()
            .map_err(error)?
            .load()
            .map_err(error)?;
        let mut sessions = self.sessions.lock().map_err(error)?;
        sessions
            .registered
            .extend(value.sessions.iter().map(|s| s.id.clone()));
        Ok(value)
    }
    pub fn save_workspace(&self, value: Workspace) -> Result<(), String> {
        self.workspace
            .lock()
            .map_err(error)?
            .save(&value)
            .map_err(error)
    }
    pub fn register(&self, id: String) -> Result<(), String> {
        uuid::Uuid::parse_str(&id).map_err(error)?;
        let mut sessions = self.sessions.lock().map_err(error)?;
        if sessions.registered.len() >= 100 {
            return Err("会话数量已达 100，请继续使用已有会话".into());
        }
        sessions.registered.insert(id);
        Ok(())
    }
    pub fn inventory_command(
        &self,
        action: &str,
        id: Option<String>,
        path: Option<PathBuf>,
    ) -> Result<serde_json::Value, String> {
        let _gate = self.configuration_gate.lock().map_err(error)?;
        let scope = match Config::load(&self.path()).and_then(|c| c.scope(false)) {
            Ok(scope) => scope,
            Err(_) if matches!(action, "status" | "stop" | "remove") => {
                dao_shell::files::Scope::new(&[], &[]).map_err(error)?
            }
            Err(e) => return Err(error(e)),
        };
        self.inventory.restrict(&scope);
        match action {
            "status" => {}
            "add" => {
                self.inventory
                    .add(path.as_deref().ok_or("缺少目录")?, &scope)
                    .map_err(error)?;
            }
            "update" => self
                .inventory
                .update(id.as_deref().ok_or("缺少目录记录")?, &scope)
                .map_err(error)?,
            "stop" => self
                .inventory
                .stop(id.as_deref().ok_or("缺少目录记录")?)
                .map_err(error)?,
            "remove" => self
                .inventory
                .remove(id.as_deref().ok_or("缺少目录记录")?)
                .map_err(error)?,
            _ => return Err("不支持的目录操作".into()),
        }
        serde_json::to_value(self.inventory.statuses(&scope).map_err(error)?).map_err(error)
    }
    pub fn info(&self) -> Result<Value, String> {
        let config = Config::load(&self.path()).map_err(error)?;
        let session = FileSession::new_with_language(
            &config,
            self.control.cancellation(),
            dao_shell::localization::effective(config.ui_language),
        )
        .map_err(error)?;
        let mut value = serde_json::to_value(session.info()).map_err(error)?;
        value["effective_language"] = dao_shell::localization::effective(config.ui_language)
            .tag()
            .into();
        Ok(value)
    }
    pub fn language_info(&self) -> Value {
        let preference = Config::load(&self.path())
            .map(|c| c.ui_language)
            .unwrap_or_default();
        json!({"effective_language":dao_shell::localization::effective(preference).tag()})
    }
    pub fn settings(&self) -> Result<SettingsView, String> {
        settings::read(&self.path()).map_err(error)
    }
    pub fn save(&self, update: SettingsUpdate) -> Result<SettingsView, String> {
        self.reserve()?;
        let _release = Release(self);
        let mut sessions = self.sessions.lock().map_err(error)?;
        let _gate = self.configuration_gate.lock().map_err(error)?;
        let saved = settings::save(&self.path(), update).map_err(error)?;
        sessions.contexts.clear();
        if let Ok(scope) = Config::load(&self.path()).and_then(|c| c.scope(false)) {
            self.inventory.restrict(&scope);
        }
        Ok(saved)
    }
    pub fn overview(&self) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(self);
        // Invalidate earlier action references even when the next sampling fails.
        *self.applications.lock().map_err(error)? = crate::app_actions::Catalog::default();
        *self.processes.lock().map_err(error)? = crate::process_actions::Catalog::default();
        let mut snapshot =
            dao_shell::computer::overview(&self.control.cancellation()).map_err(error)?;
        self.applications
            .lock()
            .map_err(error)?
            .refresh(&mut snapshot);
        let targets = self.applications.lock().map_err(error)?.running_targets();
        self.processes
            .lock()
            .map_err(error)?
            .refresh(&mut snapshot, targets);
        *self.overview.lock().map_err(|_| "概览不可用".to_string())? = Some(snapshot.clone());
        Ok(snapshot)
    }
    pub fn profile(&self) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(self);
        serde_json::to_value(
            dao_shell::profile::collect(&self.control.cancellation()).map_err(error)?,
        )
        .map_err(error)
    }
    pub fn process_action(
        &self,
        action: &str,
        reference: &str,
        confirmed: bool,
    ) -> Result<Value, String> {
        match action {
            "prepare" if !confirmed => {
                self.reserve()?;
                let result = (|| {
                    let mut catalog = self.processes.lock().map_err(error)?;
                    let value = catalog.prepare(reference)?;
                    if self.control.cancellation().is_cancelled() {
                        catalog.cancel_pending();
                        return Err("已取消，未结束进程".into());
                    }
                    Ok(value)
                })();
                // Keep the existing global admission reserved while the human
                // decides; only this opaque confirmation may resolve it.
                if result.is_err() {
                    self.control.release();
                }
                result
            }
            "resolve" => {
                let mut catalog = self.processes.lock().map_err(error)?;
                if !catalog.owns_pending(reference) {
                    return Err("结束进程确认已失效".into());
                }
                let _release = Release(self);
                catalog.resolve(reference, confirmed, &self.control.cancellation())
            }
            _ => Err("不支持的进程操作".into()),
        }
    }
    pub fn application_action(&self, id: &str, action: &str) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(self);
        self.applications
            .lock()
            .map_err(error)?
            .execute(id, action, &self.control.cancellation())
    }
    pub fn test(&self, request: ConnectionTest) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(self);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(error)?;
        let report = runtime
            .block_on(settings::test_connection(
                request,
                &self.control.cancellation(),
            ))
            .map_err(error)?;
        serde_json::to_value(report).map_err(error)
    }
    pub fn reserve(&self) -> Result<(), String> {
        let _owner = self.active.lock().map_err(error)?;
        self.control.reserve().map_err(error)
    }
    pub fn reserve_for(&self, session_id: &str, request_id: &str) -> Result<(), String> {
        uuid::Uuid::parse_str(request_id).map_err(error)?;
        let mut owner = self.active.lock().map_err(error)?;
        if !self
            .sessions
            .lock()
            .map_err(error)?
            .registered
            .contains(session_id)
        {
            return Err("会话不存在".into());
        }
        self.control.reserve().map_err(error)?;
        *owner = Some((session_id.into(), request_id.into()));
        Ok(())
    }
    pub fn execute(
        &self,
        session_id: &str,
        request_id: &str,
        kind: &str,
        input: String,
        channel: Channel<Value>,
    ) -> Result<Reply, String> {
        let _release = Release(self);
        if input.len() > 64000 {
            return Err("输入过长".into());
        }
        let action = match kind {
            "say" => Action::Say(input),
            "search" => Action::Search(input),
            "open" => Action::Open(input),
            "open-selected" => Action::OpenSelected(input),
            "explain" => {
                let snapshot = self
                    .overview
                    .lock()
                    .map_err(|_| "概览不可用".to_string())?
                    .clone()
                    .ok_or("请先刷新电脑概览")?;
                Action::ExplainComputer(
                    json!({"system":snapshot["system"],"observed_at":snapshot["observed_at"],"facts_for_explanation":snapshot["facts_for_explanation"],"limits":snapshot["limits"]}),
                )
            }
            _ => return Err("不支持的请求".into()),
        };
        if self.active.lock().map_err(error)?.as_ref()
            != Some(&(session_id.into(), request_id.into()))
        {
            return Err("请求已失效".into());
        }
        let config = Config::load(&self.path()).map_err(error)?;
        self.inventory
            .restrict(&config.scope(false).map_err(error)?);
        let encoded = serde_json::to_string(&config).map_err(error)?;
        let (mut session, context_reset, configuration_changed) = {
            let mut sessions = self.sessions.lock().map_err(error)?;
            let changed = sessions.configuration != encoded;
            if changed {
                sessions.contexts.clear();
                sessions.configuration = encoded;
            }
            let existing = sessions.contexts.remove(session_id);
            let reset = existing.is_none();
            (
                match existing {
                    Some(s) => s,
                    None => FileSession::new_with_language(
                        &config,
                        self.control.cancellation(),
                        dao_shell::localization::effective(config.ui_language),
                    )
                    .map_err(error)?,
                },
                reset,
                changed,
            )
        };
        session.set_inventory(self.inventory.clone());
        session.set_index(self.index_service.client(config.access_mode));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(error)?;
        let mut ui = DesktopInteraction {
            bridge: self,
            channel,
            session_id,
            request_id,
        };
        if context_reset {
            ui.send(json!({"kind":"context_reset", "configuration_changed":configuration_changed}));
        }
        let reply = runtime.block_on(session.execute(action, &mut ui));
        self.sessions
            .lock()
            .map_err(error)?
            .contexts
            .insert(session_id.into(), session);
        Ok(reply)
    }
    pub fn cancel(&self) {
        self.control.cancel();
        if let Ok(mut catalog) = self.processes.try_lock()
            && catalog.cancel_pending()
        {
            self.control.release();
        }
    }
    pub fn cancel_for(&self, session_id: &str, request_id: &str) -> Result<(), String> {
        let owner = self.active.lock().map_err(error)?;
        if owner.as_ref() != Some(&(session_id.into(), request_id.into())) {
            return Err("请求已结束".into());
        }
        self.control.cancel();
        Ok(())
    }
    pub fn confirm(
        &self,
        session_id: &str,
        request_id: &str,
        id: &str,
        approved: bool,
    ) -> Result<(), String> {
        let owner = self.active.lock().map_err(error)?;
        if owner.as_ref() != Some(&(session_id.into(), request_id.into())) {
            return Err("请求已失效".into());
        }
        let current =
            serde_json::to_string(&Config::load(&self.path()).map_err(error)?).map_err(error)?;
        if self.sessions.lock().map_err(error)?.configuration != current {
            self.control.cancel();
            return Err("配置已变更，此确认已失效，请重新搜索".into());
        }
        self.control.answer(id, approved).map_err(error)
    }
}

struct DesktopInteraction<'a> {
    bridge: &'a Bridge,
    channel: Channel<Value>,
    session_id: &'a str,
    request_id: &'a str,
}
impl DesktopInteraction<'_> {
    fn send(&self, mut value: Value) {
        value["session_id"] = self.session_id.into();
        value["execution_id"] = self.request_id.into();
        if self.channel.send(value).is_err() {
            self.bridge.cancel();
        }
    }
}
impl Interaction for DesktopInteraction<'_> {
    fn confirm_move(&mut self, _: &Operation) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn confirm_open(&mut self, object: &FileObject) -> anyhow::Result<bool> {
        let confirmation = self.bridge.control.confirmation(Duration::from_secs(90))?;
        self.send(json!({"kind":"confirm_open","request_id":confirmation.id,"object":object,"expires_in_seconds":90}));
        let approved = self.bridge.control.wait(&confirmation);
        self.send(json!({"kind":"confirmation_closed","request_id":confirmation.id}));
        Ok(approved)
    }
    fn progress(&mut self, text: &str) {
        self.send(json!({"kind":"progress","text":text}));
    }
    fn result(&mut self, capability: &str, value: &Value) {
        self.send(json!({"kind":"result","capability":capability,"value":value}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_confirmation_holds_admission_invalid_tokens_cannot_release_and_cancel_revokes_it() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = Bridge::with_path(dir.path().join("config.json"));
        bridge.reserve().unwrap();
        bridge
            .processes
            .lock()
            .unwrap()
            .fixture_pending("opaque-confirmation");
        assert!(bridge.application_action("unknown", "uninstall").is_err());
        assert!(bridge.process_action("resolve", "wrong", true).is_err());
        assert!(bridge.reserve().is_err());
        let result = bridge
            .process_action("resolve", "opaque-confirmation", false)
            .unwrap();
        assert_eq!(result["state"], "cancelled");
        bridge.reserve().unwrap();
        bridge.processes.lock().unwrap().fixture_pending("later");
        bridge.cancel();
        assert!(bridge.process_action("resolve", "later", true).is_err());
        bridge.reserve().unwrap();
        drop(Release(&bridge));
        assert!(bridge.process_action("prepare", "unknown", false).is_err());
        bridge.reserve().unwrap();
        drop(Release(&bridge));
    }
    #[test]
    fn inventory_control_is_independent_and_inaccessible_roots_can_be_removed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("synthetic.txt"), "fixture").unwrap();
        let config_path = temp.path().join("config.json");
        Config {
            read_roots: vec![root.clone()],
            ..Default::default()
        }
        .save(&config_path)
        .unwrap();
        let bridge = Bridge::with_path(config_path);
        bridge.reserve().unwrap();
        let rows = bridge
            .inventory_command("add", None, Some(root.clone()))
            .unwrap();
        let id = rows[0]["id"].as_str().unwrap().to_owned();
        assert!(
            bridge
                .inventory_command("stop", Some(id.clone()), None)
                .is_ok()
        );
        assert!(
            bridge
                .inventory_command("update", Some(id.clone()), None)
                .is_ok()
        );
        std::fs::rename(&root, temp.path().join("relocated")).unwrap();
        let rows = bridge.inventory_command("status", None, None).unwrap();
        assert_eq!(rows[0]["state"], "unavailable");
        assert_eq!(rows[0]["root"], "（当前无访问权限）");
        assert!(
            bridge
                .inventory_command("remove", Some(id), None)
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("relocated/synthetic.txt")).unwrap(),
            "fixture"
        );
        bridge.inventory.shutdown();
        drop(Release(&bridge));
    }
    #[test]
    fn application_actions_share_request_reservation_and_release_after_error() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = Bridge::with_path(dir.path().join("config.json"));
        bridge.reserve().unwrap();
        assert!(bridge.profile().unwrap_err().contains("请求"));
        assert!(
            bridge
                .application_action("unknown", "uninstall")
                .unwrap_err()
                .contains("请求")
        );
        drop(Release(&bridge));
        assert!(
            bridge
                .application_action("unknown", "uninstall")
                .unwrap_err()
                .contains("失效")
        );
        bridge.reserve().unwrap();
        drop(Release(&bridge));
    }
    #[test]
    fn saving_restricted_roots_invalidates_old_candidates_and_future_search_scope() {
        let dir = std::env::temp_dir().join(format!("dao-p2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        Config {
            read_roots: vec![dir.clone()],
            ..Default::default()
        }
        .save(&path)
        .unwrap();
        std::fs::write(dir.join("synthetic.txt"), "fixture").unwrap();
        let bridge = Bridge::with_path(path.clone());
        let sid = uuid::Uuid::new_v4().to_string();
        let rid = uuid::Uuid::new_v4().to_string();
        bridge.register(sid.clone()).unwrap();
        bridge.reserve_for(&sid, &rid).unwrap();
        let found = bridge
            .execute(
                &sid,
                &rid,
                "search",
                "synthetic.txt".into(),
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert_eq!(found.items.len(), 1);
        let view = bridge.settings().unwrap();
        let mut config = view.config;
        config.read_roots.clear();
        bridge
            .save(SettingsUpdate {
                expected_revision: view.revision,
                config,
                keys: vec![],
            })
            .unwrap();
        assert!(bridge.sessions.lock().unwrap().contexts.is_empty());
        assert!(
            Config::load(&path)
                .unwrap()
                .scope(false)
                .unwrap()
                .check(&dir, false)
                .is_err()
        );
        bridge.reserve_for(&sid, &rid).unwrap();
        let stale = bridge
            .execute(
                &sid,
                &rid,
                "open",
                found.items[0].id.clone(),
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert!(stale.error);
        std::fs::remove_file(dir.join("synthetic.txt")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn configuration_save_cannot_race_a_turn_and_external_edits_drop_old_candidates() {
        let directory = std::env::temp_dir().join(format!("dao-bridge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("config.json");
        let config = Config {
            read_roots: vec![directory.clone()],
            ..Default::default()
        };
        config.save(&path).unwrap();
        std::fs::write(directory.join("fixture.txt"), "synthetic").unwrap();
        let bridge = Bridge::with_path(path.clone());
        let sid = uuid::Uuid::new_v4().to_string();
        let rid = uuid::Uuid::new_v4().to_string();
        bridge.register(sid.clone()).unwrap();
        bridge.reserve_for(&sid, &rid).unwrap();
        let found = bridge
            .execute(
                &sid,
                &rid,
                "search",
                "fixture.txt".into(),
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert_eq!(found.items.len(), 1);
        let id = found.items[0].id.clone();
        let view = bridge.settings().unwrap();
        bridge.reserve().unwrap();
        assert!(
            bridge
                .save(SettingsUpdate {
                    expected_revision: view.revision,
                    config: view.config,
                    keys: vec![]
                })
                .is_err()
        );
        bridge.control.release();
        Config::default().save(&path).unwrap();
        bridge.reserve_for(&sid, &rid).unwrap();
        let stale = bridge
            .execute(&sid, &rid, "open", id, Channel::new(|_| Ok(())))
            .unwrap();
        assert!(stale.error);
        assert!(stale.message.contains("已失效"));
        std::fs::remove_file(directory.join("fixture.txt")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn sessions_own_candidates_and_cancel_confirmation_cannot_cross_requests() {
        let dir = std::env::temp_dir().join(format!("dao-p1-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let config = Config {
            read_roots: vec![dir.clone()],
            ..Default::default()
        };
        let path = dir.join("config.json");
        config.save(&path).unwrap();
        std::fs::write(dir.join("sample.txt"), "synthetic").unwrap();
        let bridge = Bridge::with_path(path.clone());
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        let r = uuid::Uuid::new_v4().to_string();
        let later = uuid::Uuid::new_v4().to_string();
        bridge.register(a.clone()).unwrap();
        bridge.register(b.clone()).unwrap();
        bridge.reserve_for(&a, &r).unwrap();
        let found = bridge
            .execute(
                &a,
                &r,
                "search",
                "sample.txt".into(),
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert_eq!(found.items.len(), 1);
        bridge.reserve_for(&b, &r).unwrap();
        let foreign = bridge
            .execute(
                &b,
                &r,
                "open",
                found.items[0].id.clone(),
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert!(foreign.error);
        bridge.reserve_for(&a, &r).unwrap();
        assert!(bridge.reserve_for(&b, &later).is_err());
        let c = bridge.control.confirmation(Duration::from_secs(2)).unwrap();
        assert!(bridge.confirm(&b, &r, &c.id, true).is_err());
        assert!(bridge.cancel_for(&b, &r).is_err());
        bridge.cancel_for(&a, &r).unwrap();
        assert!(!bridge.control.wait(&c));
        assert!(bridge.confirm(&a, &r, &c.id, true).is_err());
        drop(Release(&bridge));
        bridge.reserve_for(&b, &later).unwrap();
        assert!(bridge.cancel_for(&a, &r).is_err());
        assert!(!bridge.control.cancellation().is_cancelled());
        drop(Release(&bridge));
        let unknown = uuid::Uuid::new_v4().to_string();
        assert!(bridge.reserve_for(&unknown, &r).is_err());
        bridge.reserve_for(&b, &later).unwrap();
        let c = bridge.control.confirmation(Duration::from_secs(2)).unwrap();
        Config::default().save(&path).unwrap();
        assert!(bridge.confirm(&b, &later, &c.id, true).is_err());
        assert!(!bridge.control.wait(&c));
        drop(Release(&bridge));
        std::fs::remove_file(dir.join("sample.txt")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
