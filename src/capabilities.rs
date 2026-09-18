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
    pub journal: Journal,
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
                let plan = operations::prepare(
                    &request.object_ids,
                    &request.destination,
                    &self.scope,
                    &self.objects,
                    &mut self.journal,
                )?;
                let receipt =
                    operations::run(plan, &self.scope, &mut self.journal, &self.cancel, |op| {
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

fn function(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function", "function":{"name":name,"description":description,
        "parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}
pub fn definitions() -> Vec<Value> {
    let reference =
        json!({"object_id":{"type":"string","description":"本次会话真实查询结果的 id，不能捏造"}});
    let resources = json!({"sample_ms":{"type":"integer","minimum":500,"maximum":5000},
        "limit":{"type":"integer","minimum":1,"maximum":50},"filter":{"type":"string"},"sort":{"type":"string","enum":["cpu","memory"]}});
    vec![
        function(
            "file_search",
            "在授权范围内查文件名、相对路径和元数据，不读正文。优先使用 terms + extension + kind 一次表达需求（如 terms=[客户端,初始化], extension=sql, kind=file）。默认按相关度排序，文件名命中优先于父目录及更远路径。返回当前候选编号；每次搜索替换候选，零结果清空。",
            json!({
            "query":{"type":"string","description":"文件名包含的关键词；空字符串匹配全部"},
            "terms":{"type":"array","maxItems":12,"items":{"type":"string","minLength":1,"maxLength":128},"description":"相对路径中的关键词；与 query 二选一。不要把已用扩展名表达的文件类型重复作为必需词"},
            "match_mode":{"type":"string","enum":["all","any"],"description":"terms 默认 all（都命中）；any 表示任一命中"},
            "kind":{"type":"string","enum":["any","file","directory"]},
            "directory":{"type":"string","description":"已授权范围内的绝对目录；省略则查询已配置根目录"},
            "extension":{"type":"string","description":"如 pdf；不带通配符"},
            "modified_after":{"type":"string","description":"RFC3339，含时区，闭区间下界"},
            "modified_before":{"type":"string","description":"RFC3339，含时区，开区间上界"},
            "min_bytes":{"type":"integer","minimum":0},"max_bytes":{"type":"integer","minimum":0},
            "sort":{"type":"string","enum":["relevance","modified_desc","name","size_desc"]},
            "page":{"type":"integer","minimum":0,"maximum":100},"limit":{"type":"integer","minimum":1,"maximum":50}}),
            &[],
        ),
        function(
            "file_inspect",
            "复核一个已观察对象的元数据；对象变化会报错，需要重新查找。",
            reference.clone(),
            &["object_id"],
        ),
        function(
            "file_open",
            "请求本地用户选择打开具体文件或目录。仅返回系统是否接受请求，不保证窗口可见。",
            reference,
            &["object_id"],
        ),
        function(
            "file_move_batch",
            "准备并展示不可变移动方案，由本地用户确认后执行并逐项核验；1–20 个普通文件，同卷，不覆盖。未知结果禁止重试。",
            json!({
            "object_ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":20,"uniqueItems":true},
            "destination":{"type":"string","description":"写入范围内的绝对目的目录，允许创建方案中明确列出的子目录"}}),
            &["object_ids", "destination"],
        ),
        function(
            "resource_snapshot",
            "采集短时间的整机 CPU、内存、磁盘可用空间和进程；只读，不是卡顿根因诊断。",
            resources.clone(),
            &[],
        ),
        function(
            "process_list",
            "短时采样并列出进程；只读，不结束进程，不查询命令行或环境变量。",
            resources,
            &[],
        ),
    ]
}
