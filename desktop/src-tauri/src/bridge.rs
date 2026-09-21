use dao_shell::{
    config::Config,
    core::{FileObject, Operation},
    runtime::{Interaction, control::RequestControl},
    session::{Action, FileSession, Reply},
    settings::{self, ConnectionTest, SettingsUpdate, SettingsView},
};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::ipc::Channel;

pub struct Bridge {
    config_path: PathBuf,
    session: Mutex<Option<FileSession>>,
    pub control: RequestControl,
    overview: Mutex<Option<Value>>,
    configuration: Mutex<String>,
}
struct Release<'a>(&'a RequestControl);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.release();
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
        Self {
            config_path,
            session: Mutex::new(None),
            control: RequestControl::default(),
            overview: Mutex::new(None),
            configuration: Mutex::new(String::new()),
        }
    }
    fn path(&self) -> PathBuf {
        self.config_path.clone()
    }
    fn ensure_session(&self, session: &mut Option<FileSession>) -> Result<(), String> {
        let config = Config::load(&self.path()).map_err(error)?;
        let encoded = serde_json::to_string(&config).map_err(error)?;
        let mut previous = self
            .configuration
            .lock()
            .map_err(|_| "配置状态不可用".to_string())?;
        if session.is_none() || *previous != encoded {
            // Also honor CLI edits before the next request; old IDs cannot cross policies.
            *session = Some(FileSession::new(&config, self.control.cancellation()).map_err(error)?);
            *previous = encoded;
        }
        Ok(())
    }
    pub fn info(&self) -> Result<Value, String> {
        let mut session = self
            .session
            .try_lock()
            .map_err(|_| "正在处理请求".to_string())?;
        self.ensure_session(&mut session)?;
        serde_json::to_value(session.as_ref().unwrap().info()).map_err(error)
    }
    pub fn settings(&self) -> Result<SettingsView, String> {
        settings::read(&self.path()).map_err(error)
    }
    pub fn save(&self, update: SettingsUpdate) -> Result<SettingsView, String> {
        self.reserve()?;
        let _release = Release(&self.control);
        let mut session = self.session.lock().map_err(|_| "会话不可用".to_string())?;
        let saved = settings::save(&self.path(), update).map_err(error)?;
        *session = None;
        Ok(saved)
    }
    pub fn overview(&self) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(&self.control);
        let snapshot =
            dao_shell::computer::overview(&self.control.cancellation()).map_err(error)?;
        *self.overview.lock().map_err(|_| "概览不可用".to_string())? = Some(snapshot.clone());
        Ok(snapshot)
    }
    pub fn test(&self, request: ConnectionTest) -> Result<Value, String> {
        self.reserve()?;
        let _release = Release(&self.control);
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
        self.control.reserve().map_err(error)
    }
    pub fn execute(
        &self,
        kind: &str,
        input: String,
        channel: Channel<Value>,
    ) -> Result<Reply, String> {
        let _release = Release(&self.control);
        let action = match kind {
            "say" => Action::Say(input),
            "search" => Action::Search(input),
            "open" => Action::Open(input),
            "reset" => Action::Reset,
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
        let mut session = self
            .session
            .lock()
            .map_err(|_| "会话不可用，请重启".to_string())?;
        self.ensure_session(&mut session)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(error)?;
        let mut ui = DesktopInteraction {
            bridge: self,
            channel,
        };
        Ok(runtime.block_on(session.as_mut().unwrap().execute(action, &mut ui)))
    }
    pub fn cancel(&self) {
        self.control.cancel();
    }
    pub fn confirm(&self, id: &str, approved: bool) -> Result<(), String> {
        self.control.answer(id, approved).map_err(error)
    }
}

struct DesktopInteraction<'a> {
    bridge: &'a Bridge,
    channel: Channel<Value>,
}
impl DesktopInteraction<'_> {
    fn send(&self, value: Value) {
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
        bridge.reserve().unwrap();
        let found = bridge
            .execute("search", "fixture.txt".into(), Channel::new(|_| Ok(())))
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
        bridge.reserve().unwrap();
        let stale = bridge
            .execute("open", id, Channel::new(|_| Ok(())))
            .unwrap();
        assert!(stale.error);
        assert!(stale.message.contains("已失效"));
        std::fs::remove_file(directory.join("fixture.txt")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
