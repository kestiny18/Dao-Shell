pub mod control;
use crate::{
    core::{Cancellation, FileObject, Operation, safe_text},
    files::{self, Objects, Scope, Search},
    operations, resources,
    storage::Journal,
};
use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

pub trait Interaction {
    fn confirm_move(&mut self, operation: &Operation) -> Result<bool>;
    fn confirm_open(&mut self, object: &FileObject) -> Result<bool>;
    fn progress(&mut self, text: &str);
    fn result(&mut self, capability: &str, value: &Value);
}

pub struct Runtime {
    pub scope: Scope,
    pub objects: Objects,
    pub journal: Option<Journal>,
    pub cancel: Cancellation,
    pub last_results: Vec<String>,
    pub side_effects_blocked: bool,
}
impl Runtime {
    pub fn call(
        &mut self,
        name: &str,
        arguments: Value,
        ui: &mut dyn Interaction,
    ) -> Result<Value> {
        if name == "file_search" {
            // Clear before cancellation, decoding, and scanning can fail.
            self.last_results.clear();
        }
        self.cancel.check()?;
        if self.side_effects_blocked && matches!(name, "file_open" | "file_move_batch") {
            bail!("本轮的打开/移动已取消或有未知结果，需用户下一条指令；不能重复请求确认");
        }
        let value = match name {
            "file_search" => {
                let request: Search = serde_json::from_value(arguments)?;
                ui.progress(&format!(
                    "正在查找文件（关键词：{}）……",
                    if !request.terms.is_empty() {
                        safe_text(&request.terms.join(" + "))
                    } else if request.query.is_empty() {
                        "全部".into()
                    } else {
                        safe_text(&request.query)
                    }
                ));
                let result = files::search(&request, &self.scope, &mut self.objects, &self.cancel)?;
                self.last_results = result.items.iter().map(|o| o.id.clone()).collect();
                let mut value = serde_json::to_value(result)?;
                value["active_selection"] = json!(
                    self.last_results
                        .iter()
                        .enumerate()
                        .map(|(i, id)| json!({"index": i + 1, "object_id": id}))
                        .collect::<Vec<_>>()
                );
                value
            }
            "file_inspect" => {
                let request: Reference = serde_json::from_value(arguments)?;
                serde_json::to_value(self.objects.checked(
                    &request.object_id,
                    &self.scope,
                    false,
                )?)?
            }
            "file_open" => {
                let request: Reference = serde_json::from_value(arguments)?;
                let object = self
                    .objects
                    .checked(&request.object_id, &self.scope, false)?;
                if ui.confirm_open(&object)? && !self.cancel.is_cancelled() {
                    files::open(&object)?
                } else {
                    self.side_effects_blocked = true;
                    json!({"status":"Cancelled", "message":"用户未选择打开；没有提交请求"})
                }
            }
            "file_move_batch" => {
                let request: Move = serde_json::from_value(arguments)?;
                let journal = self
                    .journal
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("此会话未启用持久写操作记录，不能移动文件"))?;
                let plan = operations::prepare(
                    &request.object_ids,
                    &request.destination,
                    &self.scope,
                    &self.objects,
                    journal,
                )?;
                let receipt = operations::run(plan, &self.scope, journal, &self.cancel, |op| {
                    ui.confirm_move(op)
                })?;
                if matches!(
                    receipt.status,
                    crate::core::Status::Cancelled | crate::core::Status::Unknown
                ) {
                    self.side_effects_blocked = true;
                }
                serde_json::to_value(receipt)?
            }
            "resource_snapshot" | "process_list" => {
                let request: resources::Request = serde_json::from_value(arguments)?;
                ui.progress(&format!(
                    "正在采样资源与进程（按{}排序）……",
                    match request.sort {
                        resources::ProcessSort::Cpu => "CPU",
                        resources::ProcessSort::Memory => "内存",
                    }
                ));
                resources::snapshot(&request, &self.cancel, name == "process_list")?
            }
            _ => bail!("不支持的能力；不能执行任意命令"),
        };
        ui.result(name, &value);
        Ok(value)
    }

    pub fn current_selection(&self) -> Result<Value> {
        let items = self
            .last_results
            .iter()
            .map(|id| self.objects.get(id).cloned())
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"items":items}))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    object_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Move {
    object_ids: Vec<String>,
    destination: PathBuf,
}
