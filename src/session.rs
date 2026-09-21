//! In-process file session shared by the desktop adapter and contract tests.
//! Each instance owns its candidates, model history and cancellation; no global selection.
use crate::{
    capabilities::{Interaction, Runtime},
    config::Config,
    core::{Cancellation, FileObject},
    dialogue::Dialogue,
    files::Objects,
};
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
pub struct SessionInfo {
    pub access_mode: crate::config::AccessMode,
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
    ExplainComputer(serde_json::Value),
}

pub struct FileSession {
    runtime: Runtime,
    dialogue: Option<Dialogue>,
    info: SessionInfo,
}

impl FileSession {
    pub fn new(config: &Config, cancel: Cancellation) -> Result<Self> {
        let scope = config.scope(false)?;
        let runtime = Runtime {
            scope,
            objects: Objects::default(),
            journal: None,
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
                Some("尚未配置模型，请在设置 → 模型与连接中添加；也可以使用文件名搜索".into()),
            ),
        };
        let info = SessionInfo {
            access_mode: config.access_mode,
            model: config.model.as_ref().map(|m| m.model.clone()),
            model_error,
            roots: runtime.scope.read_roots().to_vec(),
        };
        Ok(Self {
            runtime,
            dialogue,
            info,
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
            Action::ExplainComputer(snapshot) => {
                let dialogue = self
                    .dialogue
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("请先在设置中连接模型"))?;
                dialogue.observe_local("resource_snapshot", &snapshot);
                dialogue.turn("请根据刚才电脑概览的本地采样，简洁解释磁盘、CPU 和内存情况。标明采样时间与局限，只分析已有数据，不调用文件工具。", &mut self.runtime, ui).await
            }
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
