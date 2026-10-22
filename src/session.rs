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
    pub search_receipt: Option<serde_json::Value>,
    pub observations: Vec<serde_json::Value>,
    pub action_receipts: Vec<serde_json::Value>,
    pub action_targets: Vec<serde_json::Value>,
}

pub enum Action {
    Say(String),
    Search(String),
    Open(String),
    OpenSelected(String),
    Reset,
    ExplainComputer(serde_json::Value),
    Application {
        tool: String,
        arguments: serde_json::Value,
    },
    ActionReceipts,
    ReconcileAction(String),
    ClearCompletedActions,
    QueryComputer {
        tool: String,
        arguments: serde_json::Value,
    },
}

pub struct FileSession {
    runtime: Runtime,
    dialogue: Option<Dialogue>,
    info: SessionInfo,
    language: crate::localization::Language,
}

impl FileSession {
    pub fn new(config: &Config, cancel: Cancellation) -> Result<Self> {
        Self::new_with_language(config, cancel, crate::localization::Language::Chinese)
    }
    pub fn new_with_language(
        config: &Config,
        cancel: Cancellation,
        language: crate::localization::Language,
    ) -> Result<Self> {
        let scope = config.scope(false)?;
        let mut runtime = Runtime {
            scope,
            objects: Objects::default(),
            journal: None,
            cancel,
            last_results: Vec::new(),
            last_search: None,
            side_effects_blocked: false,
            computer: Default::default(),
            application_actions: Default::default(),
        };
        runtime.computer.desktop = true;
        runtime.computer.sharing = config.computer_query_sharing;
        let (dialogue, model_error) = match config
            .model
            .as_ref()
            .map(|c| Dialogue::new_file_entry_language(c, &runtime, language))
        {
            Some(Ok(dialogue)) => (Some(dialogue), None),
            Some(Err(error)) => (None, Some(format!("{error:#}"))),
            None => (
                None,
                Some(language.choose("自然语言需要连接模型；仍可使用文件名搜索或本地电脑状态入口", "Natural language requires a model. File-name search and local computer queries remain available.").into()),
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
            language,
        })
    }

    pub fn set_action_context(&mut self, session_id: &str, execution_id: &str, revision: &str) {
        self.runtime.application_actions.context = json!({"session_id":session_id,"execution_id":execution_id,"configuration_revision":revision});
    }
    pub fn set_action_directory(&mut self, directory: std::path::PathBuf, config: &Config) {
        self.runtime.application_actions.directory = Some(directory);
        self.runtime.application_actions.excluded_environment = config
            .models
            .connections
            .iter()
            .map(|c| c.api_key_env.clone())
            .chain(config.model.iter().map(|c| c.api_key_env.clone()))
            .filter(|s| !s.is_empty())
            .collect();
    }
    pub fn set_inventory(&mut self, inventory: std::sync::Arc<crate::inventory::Inventory>) {
        self.runtime.objects.set_inventory(inventory);
    }
    pub fn set_index(
        &mut self,
        index: Option<std::sync::Arc<dyn crate::search_index::IndexClient>>,
    ) {
        if let Some(dialogue) = &mut self.dialogue {
            dialogue.set_global_name_discovery(index.is_some());
        }
        self.runtime.objects.set_index(index);
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    pub async fn execute(&mut self, action: Action, ui: &mut dyn Interaction) -> Reply {
        self.runtime.last_search = None;
        self.runtime.computer.delivered.clear();
        self.runtime.application_actions.begin_turn();
        self.runtime.side_effects_blocked = false;
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
            search_receipt: self.runtime.last_search.clone(),
            observations: self.runtime.computer.delivered.clone(),
            action_receipts: self.runtime.application_actions.delivered.clone(),
            action_targets: self.runtime.application_actions.target_results.clone(),
            message,
            error,
            items,
        }
    }

    async fn perform(&mut self, action: Action, ui: &mut dyn Interaction) -> Result<String> {
        let selected = matches!(&action, Action::OpenSelected(_));
        match action {
            Action::ExplainComputer(snapshot) => {
                ensure!(
                    self.runtime.computer.sharing == crate::config::ComputerQuerySharing::Enabled,
                    "{}",
                    if self.runtime.computer.sharing == crate::config::ComputerQuerySharing::Ask {
                        "permission_required"
                    } else {
                        "permission_denied"
                    }
                );
                let snapshot = crate::observation::model_projection(
                    &json!({"kind":"resource_snapshot","observed_at":snapshot["observed_at"],"limits":snapshot["limits"],"data":snapshot}),
                );
                let dialogue = self.dialogue.as_mut().ok_or_else(|| {
                    anyhow::anyhow!(
                        self.language
                            .choose("请先在设置中连接模型", "Connect a model in Settings first")
                    )
                })?;
                dialogue.observe_local("resource_snapshot", &snapshot);
                dialogue.turn(self.language.choose("请根据刚才电脑概览的本地采样，简洁解释磁盘、CPU 和内存情况。标明采样时间与局限，只分析已有数据，不调用文件工具。", "Briefly explain the local disk, CPU and memory snapshot. State its observation time and limits. Analyze only the supplied facts and do not call file tools."), &mut self.runtime, ui).await
            }
            Action::Application { tool, arguments } => {
                ensure!(crate::application_actions::is_tool(&tool), "unsupported");
                let value = if tool == "app_targets" {
                    let value = self
                        .runtime
                        .application_actions
                        .targets(arguments, &self.runtime.cancel)?;
                    ui.result("app_targets", &value);
                    value
                } else {
                    self.runtime.application_actions.action(
                        &tool,
                        arguments,
                        &self.runtime.cancel,
                        ui,
                    )?
                };
                Ok(
                    if tool == "app_targets" && value["reason"] == "close_identity_not_atomic" {
                        self.language.choose("正常关闭暂不可用；未发送消息，也不会改为强制结束。","Normal close is unavailable. No message was sent and force termination is not substituted.").into()
                    } else if tool == "app_targets" {
                        self.language
                            .choose("请选择已核验的具体目标。", "Select a verified target.")
                            .into()
                    } else {
                        self.language
                        .choose(
                            "请查看本地动作回执；派发和退出证据分别列出。",
                            "See the local action receipt for separate dispatch and exit evidence.",
                        )
                        .into()
                    },
                )
            }
            Action::ActionReceipts => {
                let value = self.runtime.application_actions.receipts()?;
                ui.result("app_action_history", &value);
                Ok(self
                    .language
                    .choose("本地操作回执已载入。", "Local action receipts loaded.")
                    .into())
            }
            Action::ReconcileAction(id) => {
                let value = self.runtime.application_actions.reconcile(&id)?;
                self.runtime
                    .application_actions
                    .delivered
                    .push(value.clone());
                ui.result("app_action_receipt", &value);
                Ok(self
                    .language
                    .choose(
                        "仅核对原目标，没有重放操作。",
                        "Checked the original target without replaying the action.",
                    )
                    .into())
            }
            Action::ClearCompletedActions => {
                let value = self.runtime.application_actions.clear_completed()?;
                ui.result("app_action_history", &value);
                Ok(self
                    .language
                    .choose(
                        "已清理完成回执，未知结果保留。",
                        "Completed receipts cleared; unknown outcomes retained.",
                    )
                    .into())
            }
            Action::Say(input) => {
                ensure!(!input.trim().is_empty(), "请输入查询 / Enter a query");
                ensure!(input.len() <= 16 * 1024, "单次输入不超过 16KB");
                let dialogue = self.dialogue.as_mut().ok_or_else(|| anyhow::anyhow!(self.language.choose("自然语言需要连接模型；请在设置连接模型，或使用文件名搜索/查看电脑状态。", "Natural language requires a model. Connect one in Settings, or use file-name search / local computer status.")))?;
                dialogue.turn(&input, &mut self.runtime, ui).await
            }
            Action::QueryComputer { tool, arguments } => {
                ui.progress(
                    self.language
                        .choose("正在查询电脑状态……", "Reading computer status…"),
                );
                let value = self
                    .runtime
                    .computer
                    .query(&tool, arguments, &self.runtime.cancel)
                    .map_err(|e| {
                        anyhow::anyhow!(crate::observation::error_code(&e, &self.runtime.cancel))
                    })?;
                ui.result(&tool, &value);
                // Local viewing is independent of permission to send data to a model.
                Ok(self
                    .language
                    .choose(
                        "本地观测已完成；请查看观测卡。",
                        "Local observation complete. See the observation card.",
                    )
                    .into())
            }
            Action::Search(query) => self.search(&query, ui),
            Action::Open(id) | Action::OpenSelected(id) => {
                ensure!(
                    self.runtime.last_results.contains(&id),
                    "此候选已失效，请重新查找"
                );
                self.runtime.side_effects_blocked = false;
                let result = if selected {
                    self.runtime.open_selected(&id, ui)?
                } else {
                    self.runtime
                        .call("file_open", json!({"object_id":id}), ui)?
                };
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.observe_local("file_open", &result);
                }
                Ok(result["message"]
                    .as_str()
                    .unwrap_or("打开请求已处理")
                    .into())
            }
            Action::Reset => {
                self.runtime.objects.reset();
                self.runtime.computer.reset();
                self.runtime.application_actions.reset();
                self.runtime.last_results.clear();
                self.runtime.side_effects_blocked = false;
                if let Some(dialogue) = &mut self.dialogue {
                    dialogue.reset();
                }
                Ok(self
                    .language
                    .choose("已开始新会话。", "Started a new session.")
                    .into())
            }
        }
    }

    fn search(&mut self, query: &str, ui: &mut dyn Interaction) -> Result<String> {
        ensure!(!query.trim().is_empty(), "请输入文件名关键词");
        let result = self
            .runtime
            .call("file_search", json!({"query":query}), ui)?;
        if let Some(dialogue) = &mut self.dialogue {
            dialogue.observe_local("file_search", &result);
        }
        Ok(
            if result["source"] == "windows_service" && result["truncated"] == true {
                format!(
                    "{}{}{}",
                    self.language.choose("找到 ", "Matches: "),
                    self.runtime.last_results.len(),
                    self.language.choose(
                        " 个候选；部分卷或记录尚不可用，详见索引覆盖回执。",
                        "; some volumes or records are unavailable. See the index coverage receipt."
                    )
                )
            } else if result["truncated"] == true {
                self.language.choose("本次扫描达到预算，结果并不完整；可缩小目录范围后重试。", "This scan reached its budget and results are incomplete. Narrow the folder scope and retry.").into()
            } else {
                format!(
                    "{}{}{}",
                    self.language.choose("找到 ", "Matches: "),
                    self.runtime.last_results.len(),
                    self.language.choose(" 个候选。", ".")
                )
            },
        )
    }
}
