pub mod control;
use crate::{
    core::{Cancellation, FileObject, Operation, safe_text},
    files::{self, Objects, Scope, Search},
    operations, resources,
    storage::Journal,
};
use anyhow::{Result, bail, ensure};
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
    pub last_search: Option<Value>,
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
            self.last_search = None;
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
                // Correlate event/final delivery without conflating separate tool searches.
                value["receipt_id"] = json!(uuid::Uuid::new_v4().to_string());
                value["active_selection"] = json!(
                    self.last_results
                        .iter()
                        .enumerate()
                        .map(|(i, id)| json!({"index": i + 1, "object_id": id}))
                        .collect::<Vec<_>>()
                );
                self.last_search = Some(value.clone());
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
                self.open_using(&request.object_id, false, ui, files::open)?
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
    /// This explicit local selection is not exposed in the model tool schema.
    pub fn open_selected(&mut self, id: &str, ui: &mut dyn Interaction) -> Result<Value> {
        ensure!(
            self.last_results.iter().any(|value| value == id),
            "此候选已失效，请重新查找"
        );
        let value = self.open_using(id, true, ui, files::open)?;
        ui.result("file_open", &value);
        Ok(value)
    }
    fn open_using(
        &mut self,
        id: &str,
        selected: bool,
        ui: &mut dyn Interaction,
        opener: impl FnOnce(&FileObject) -> Result<Value>,
    ) -> Result<Value> {
        self.cancel.check()?;
        ensure!(
            !self.side_effects_blocked,
            "本轮打开已取消或结果未知，请重新选择"
        );
        if selected {
            ensure!(
                self.last_results.iter().any(|value| value == id),
                "此候选已失效，请重新查找"
            );
        }
        let object = self.objects.checked(id, &self.scope, false)?;
        if !selected && !ui.confirm_open(&object)? {
            self.side_effects_blocked = true;
            return Ok(json!({"status":"Cancelled","message":"用户未选择打开；没有提交请求"}));
        }
        self.cancel.check()?;
        opener(&object)
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

#[cfg(test)]
mod open_tests {
    use super::*;
    #[derive(Default)]
    struct Ui {
        confirmations: usize,
        approve: bool,
        cancel: Option<Cancellation>,
    }
    impl Interaction for Ui {
        fn confirm_move(&mut self, _: &Operation) -> Result<bool> {
            unreachable!()
        }
        fn confirm_open(&mut self, _: &FileObject) -> Result<bool> {
            self.confirmations += 1;
            if let Some(cancel) = &self.cancel {
                cancel.cancel();
            }
            Ok(self.approve)
        }
        fn progress(&mut self, _: &str) {}
        fn result(&mut self, _: &str, _: &Value) {}
    }
    #[test]
    fn explicit_selection_skips_only_the_second_prompt_and_preserves_boundaries() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("allowed");
        std::fs::create_dir(&root).unwrap();
        let path = root.join("synthetic.exe");
        std::fs::write(&path, "inert bytes, mock handoff").unwrap();
        let mut runtime = Runtime {
            scope: Scope::new(std::slice::from_ref(&root), &[]).unwrap(),
            objects: Objects::default(),
            journal: None,
            cancel: Cancellation::default(),
            last_results: vec![],
            last_search: None,
            side_effects_blocked: false,
        };
        let id = runtime.objects.insert(&path).unwrap().id;
        let mut ui = Ui::default();
        assert!(
            runtime
                .open_using(&id, true, &mut ui, |_| panic!("stale candidate"))
                .is_err()
        );
        runtime.last_results.push(id.clone());
        assert_eq!(
            runtime
                .open_using(&id, true, &mut ui, |_| Ok(
                    json!({"verification":"handoff"})
                ))
                .unwrap()["verification"],
            "handoff"
        );
        assert_eq!(ui.confirmations, 0);
        let declined = runtime
            .open_using(&id, false, &mut ui, |_| panic!("declined"))
            .unwrap();
        assert_eq!(declined["status"], "Cancelled");
        assert_eq!(ui.confirmations, 1);
        assert!(
            runtime
                .open_using(&id, true, &mut ui, |_| panic!("blocked"))
                .is_err()
        );
        runtime.side_effects_blocked = false;
        ui.approve = true;
        ui.cancel = Some(runtime.cancel.clone());
        assert!(
            runtime
                .open_using(&id, false, &mut ui, |_| panic!("cancelled after prompt"))
                .is_err()
        );
        runtime.cancel = Cancellation::default();
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "changed size and identity").unwrap();
        assert!(
            runtime
                .open_using(&id, true, &mut ui, |_| panic!("replaced"))
                .is_err()
        );
        let outside = fixture.path().join("outside.html");
        std::fs::write(&outside, "fixture").unwrap();
        let id = runtime.objects.insert(&outside).unwrap().id;
        runtime.last_results.push(id.clone());
        assert!(
            runtime
                .open_using(&id, true, &mut ui, |_| panic!("out of scope"))
                .is_err()
        );
    }
}
