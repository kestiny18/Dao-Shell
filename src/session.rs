//! In-process file session shared by the desktop adapter and contract tests.
//! Each instance owns its candidates, model history and cancellation; no global selection.
use crate::{
    capabilities::{Interaction, Runtime},
    config::Config,
    core::{Cancellation, FileObject},
    dialogue::Dialogue,
    files::{Objects, Scope},
    storage::Journal,
};
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
pub struct SessionInfo {
    pub model: Option<String>,
    pub model_error: Option<String>,
    pub roots: Vec<std::path::PathBuf>,
}

#[derive(Serialize)]
pub struct Reply {
    pub message: String,
    pub error: bool,
    pub items: Vec<FileObject>,
}

pub enum Action {
    Say(String),
    Search(String),
    Open(String),
    Reset,
}

pub struct FileSession {
    runtime: Runtime,
    dialogue: Option<Dialogue>,
    info: SessionInfo,
    // Runtime currently requires a Journal. This file-only profile cannot create operations;
    // keep its empty journal temporary instead of locking the CLI's durable operation history.
    _temporary: tempfile::TempDir,
}

impl FileSession {
    pub fn new(config: &Config, cancel: Cancellation) -> Result<Self> {
        let roots: Vec<_> = config
            .read_roots
            .iter()
            .chain(&config.write_roots)
            .cloned()
            .collect();
        let scope = Scope::new(&roots, &[])?;
        let temporary = tempfile::tempdir()?;
        let runtime = Runtime {
            scope,
            objects: Objects::default(),
            journal: Journal::open(temporary.path())?,
            cancel,
            last_results: Vec::new(),
            side_effects_blocked: false,
        };
        let (dialogue, model_error) = match config
            .model
            .as_ref()
            .map(|c| Dialogue::new_file_entry(c, &runtime))
        {
            Some(Ok(dialogue)) => (Some(dialogue), None),
            Some(Err(error)) => (None, Some(format!("{error:#}"))),
            None => (
                None,
                Some("尚未连接模型，请先运行 daosh setup；也可以使用文件名搜索".into()),
            ),
        };
        let info = SessionInfo {
            model: config.model.as_ref().map(|m| m.model.clone()),
            model_error,
            roots: runtime.scope.read_roots().to_vec(),
        };
        Ok(Self {
            runtime,
            dialogue,
            info,
            _temporary: temporary,
        })
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    pub async fn execute(&mut self, action: Action, ui: &mut dyn Interaction) -> Reply {
        let result = match self.runtime.cancel.check() {
            Ok(()) => self.perform(action, ui).await,
            Err(error) => Err(error),
        };
        let (message, error) = match result {
            Ok(message) => (message, false),
            Err(error) => {
                // A transport failure can leave an assistant/tool pair unfinished.
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.reset();
                }
                (format!("{error:#}"), true)
            }
        };
        let items = self
            .runtime
            .last_results
            .iter()
            .filter_map(|id| self.runtime.objects.get(id).ok().cloned())
            .collect();
        Reply {
            message,
            error,
            items,
        }
    }

    async fn perform(&mut self, action: Action, ui: &mut dyn Interaction) -> Result<String> {
        match action {
            Action::Say(input) => {
                ensure!(!input.trim().is_empty(), "请输入你想找的文件");
                let dialogue = self.dialogue.as_mut().ok_or_else(|| {
                    anyhow::anyhow!(self.info.model_error.clone().unwrap_or_default())
                })?;
                dialogue.turn(&input, &mut self.runtime, ui).await
            }
            Action::Search(query) => {
                ensure!(!query.trim().is_empty(), "请输入文件名关键词");
                let result = self
                    .runtime
                    .call("file_search", json!({"query":query}), ui)?;
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.observe_local("file_search", &result);
                }
                Ok(if result["truncated"] == true {
                    "本次扫描达到预算，结果并不完整；可缩小目录范围后重试。".into()
                } else {
                    format!("找到 {} 个候选。", self.runtime.last_results.len())
                })
            }
            Action::Open(id) => {
                ensure!(
                    self.runtime.last_results.contains(&id),
                    "此候选已失效，请重新查找"
                );
                self.runtime.side_effects_blocked = false;
                let result = self
                    .runtime
                    .call("file_open", json!({"object_id":id}), ui)?;
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.observe_local("file_open", &result);
                }
                Ok(result["message"]
                    .as_str()
                    .unwrap_or("打开请求已处理")
                    .into())
            }
            Action::Reset => {
                self.runtime.objects = Objects::default();
                self.runtime.last_results.clear();
                self.runtime.side_effects_blocked = false;
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.reset();
                }
                Ok("已开始新会话。".into())
            }
        }
    }
}
