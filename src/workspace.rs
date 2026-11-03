//! Visible desktop history only; runtime contexts and capabilities never enter this schema.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: String,
    pub text: String,
    pub error: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub draft: String,
    pub messages: Vec<Message>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub version: u32,
    pub sessions: Vec<Session>,
    pub tabs: Vec<String>,
    pub active: String,
    pub sidebar_width: u32,
    pub collapsed: bool,
    pub home_draft: String,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            version: 1,
            sessions: vec![],
            tabs: vec!["home".into()],
            active: "home".into(),
            sidebar_width: 205,
            collapsed: false,
            home_draft: String::new(),
        }
    }
}
impl Workspace {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "不支持的工作区版本，原文件保留");
        ensure!(
            self.sessions.len() <= 100 && self.tabs.len() <= 102,
            "会话数量超过上限 100"
        );
        ensure!(
            (170..=360).contains(&self.sidebar_width) && self.home_draft.len() <= 64000,
            "工作区布局或草稿过大"
        );
        let mut ids = HashSet::new();
        for s in &self.sessions {
            ensure!(
                uuid::Uuid::parse_str(&s.id).is_ok() && ids.insert(s.id.as_str()),
                "会话标识无效或重复"
            );
            ensure!(
                s.title.len() <= 512 && s.draft.len() <= 64000 && s.messages.len() <= 2000,
                "会话内容超过上限"
            );
            for m in &s.messages {
                ensure!(
                    ["user", "assistant", "receipt"].contains(&m.role.as_str())
                        && m.text.len() <= 256000,
                    "消息格式无效或过大"
                );
            }
        }
        let mut tabs = HashSet::new();
        ensure!(
            self.tabs.first().map(String::as_str) == Some("home"),
            "概览标签缺失"
        );
        for t in &self.tabs {
            ensure!(
                (t == "home" || t == "settings" || ids.contains(t.as_str()))
                    && tabs.insert(t.as_str()),
                "标签无效或重复"
            );
        }
        ensure!(tabs.contains(self.active.as_str()), "当前标签无效");
        Ok(())
    }
}
pub struct WorkspaceStore {
    path: PathBuf,
    loaded: bool,
    lock: Option<fs::File>,
    expected: Option<Vec<u8>>,
}
impl WorkspaceStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            loaded: false,
            lock: None,
            expected: None,
        }
    }
    pub fn load(&mut self) -> Result<Workspace> {
        if self.lock.is_none() {
            let parent = self
                .path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::create_dir_all(parent)?;
            let lock = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(self.path.with_extension("lock"))?;
            lock.try_lock().map_err(|_| anyhow::anyhow!("workspace_in_use: 其他窗口正在使用此工作区；请先关闭另一窗口再重启，当前内容可导出"))?;
            self.lock = Some(lock);
        }
        let bytes = self.read_bytes()?;
        let value = match &bytes {
            Some(bytes) => {
                serde_json::from_slice::<Workspace>(bytes).context("工作区损坏，原文件保留")?
            }
            None => Workspace::default(),
        };
        value.validate()?;
        self.expected = bytes;
        self.loaded = true;
        Ok(value)
    }
    fn read_bytes(&self) -> Result<Option<Vec<u8>>> {
        match fs::metadata(&self.path) {
            Ok(metadata) => {
                ensure!(metadata.len() <= MAX_BYTES, "工作区文件过大，原文件保留");
                let bytes = fs::read(&self.path).context("无法读取工作区")?;
                ensure!(bytes.len() as u64 <= MAX_BYTES, "工作区文件过大");
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&mut self, value: &Workspace) -> Result<()> {
        ensure!(
            self.loaded,
            "工作区未成功恢复，禁止覆盖原文件。请关闭应用，备份并移走配置目录中的 workspace.json 后重启"
        );
        value.validate()?;
        // Never advance the comparison baseline on a failed save. The lifetime lock
        // serializes cooperating processes; this comparison also detects outside edits.
        ensure!(
            self.read_bytes()? == self.expected,
            "workspace_conflict: 工作区已在外部改变，未覆盖；请导出当前内容并重启"
        );
        let bytes = serde_json::to_vec(value)?;
        ensure!(bytes.len() as u64 <= MAX_BYTES, "工作区超过 8 MiB，未保存");
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut pending = tempfile::NamedTempFile::new_in(parent)?;
        pending.write_all(&bytes)?;
        pending.as_file().sync_all()?;
        pending
            .persist(&self.path)
            .context("工作区保存失败，原记录保留")?;
        self.expected = Some(bytes);
        Ok(())
    }
}

/// Export is an explicit recovery action. Keep the caller's visible snapshot even
/// when it exceeds persistence quotas; never replace another export or workspace.
pub fn export(value: &Workspace, directory: &Path) -> Result<PathBuf> {
    let path = directory.join(format!("dao-shell-history-{}.json", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.sync_all()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_excludes_another_process_and_detects_external_edits_without_advancing_baseline() {
        const KEY: &str = "DAO_WORKSPACE_LOCK_FIXTURE";
        if let Some(path) = std::env::var_os(KEY) {
            let mut second = WorkspaceStore::new(path.into());
            assert!(
                second
                    .load()
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("workspace_in_use")
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut first = WorkspaceStore::new(path.clone());
        let original = first.load().unwrap();
        first.save(&original).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "workspace::tests::workspace_excludes_another_process_and_detects_external_edits_without_advancing_baseline"])
            .env(KEY, &path).output().unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stdout)
        );
        let mut external = original.clone();
        external.home_draft = "external edit".into();
        let bytes = serde_json::to_vec(&external).unwrap();
        fs::write(&path, &bytes).unwrap();
        for _ in 0..2 {
            assert!(
                first
                    .save(&original)
                    .unwrap_err()
                    .to_string()
                    .contains("workspace_conflict")
            );
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(first);
        assert_eq!(
            WorkspaceStore::new(path).load().unwrap().home_draft,
            "external edit"
        );
    }
    #[test]
    fn oversized_visible_history_can_be_exported_and_removed_without_losing_previous_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut store = WorkspaceStore::new(path.clone());
        let mut value = store.load().unwrap();
        store.save(&value).unwrap();
        let original = fs::read(&path).unwrap();
        value.sessions.push(Session {
            id: uuid::Uuid::new_v4().to_string(),
            title: "large fixture".into(),
            draft: "unsaved draft".into(),
            messages: vec![
                Message {
                    role: "assistant".into(),
                    text: "synthetic".repeat(1100),
                    error: false
                };
                2001
            ],
        });
        assert!(serde_json::to_vec(&value).unwrap().len() > MAX_BYTES as usize);
        assert!(store.save(&value).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let exported = export(&value, dir.path()).unwrap();
        let recovered: Workspace = serde_json::from_slice(&fs::read(exported).unwrap()).unwrap();
        assert_eq!(recovered.sessions[0].messages.len(), 2001);
        assert_eq!(recovered.sessions[0].draft, "unsaved draft");
        value.sessions.clear();
        store.save(&value).unwrap();
        assert!(store.load().unwrap().sessions.is_empty());
    }
    #[test]
    fn roundtrip_rejects_authority_and_unknown_version_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut store = WorkspaceStore::new(path.clone());
        let mut value = store.load().unwrap();
        value.sessions.push(Session {
            id: uuid::Uuid::new_v4().to_string(),
            title: "fixture".into(),
            draft: "draft".into(),
            messages: vec![Message {
                role: "user".into(),
                text: "synthetic".into(),
                error: false,
            }],
        });
        store.save(&value).unwrap();
        assert_eq!(store.load().unwrap().sessions[0].draft, "draft");
        let mut encoded = serde_json::to_value(&value).unwrap();
        encoded["token"] = "forbidden".into();
        assert!(serde_json::from_value::<Workspace>(encoded).is_err());
        fs::write(&path, b"{\"version\":99}").unwrap();
        assert!(store.save(&value).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"version\":99}");
    }
    #[test]
    fn interrupted_write_and_failure_preserve_previous_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut store = WorkspaceStore::new(path.clone());
        let value = store.load().unwrap();
        store.save(&value).unwrap();
        let original = fs::read(&path).unwrap();
        fs::write(dir.path().join("interrupted.tmp"), b"partial").unwrap();
        assert_eq!(store.load().unwrap().active, "home");
        let mut oversized = value.clone();
        oversized.home_draft = "x".repeat(64001);
        assert!(store.save(&oversized).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.save(&value).is_err());
        let unknown = Workspace {
            version: 2,
            ..Workspace::default()
        };
        assert!(unknown.validate().is_err());
        let mut unopened = WorkspaceStore::new(dir.path().join("other.json"));
        assert!(unopened.save(&value).is_err());
    }
}
