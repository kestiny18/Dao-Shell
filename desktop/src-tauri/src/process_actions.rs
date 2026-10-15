//! Ephemeral UI-only process references and one-shot frozen confirmations.
use dao_shell::{
    core::Cancellation,
    process_control::{self, Identity},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
const UNAVAILABLE: &str = "缺少可靠的同用户进程身份或唯一程序关联；无法安全结束，请刷新概览";
#[derive(Clone)]
struct Target {
    items: Vec<Identity>,
    created: Instant,
}

struct Pending {
    id: String,
    items: Vec<Identity>,
    created: Instant,
}
#[derive(Default)]
pub struct Catalog {
    targets: HashMap<String, Target>,
    pending: Option<Pending>,
}
fn name(item: &Identity) -> &str {
    item.image.rsplit(['\\', '/']).next().unwrap_or("未知程序")
}
fn code_text(code: &str) -> &str {
    match code {
        "terminated" => "已退出",
        "already_exited" => "原进程已退出",
        "pending" => "已请求结束，仍在等待退出",
        "identity_changed" => "身份已变化，未操作",
        "cancelled" => "已取消，未操作",
        "access_denied" => "Windows 拒绝访问，未报告退出",
        "protected_process" => "受保护进程，未操作",
        "other_user" | "other_session" => "非当前用户会话，未操作",
        _ => "身份核验不可用，未操作",
    }
}
impl Catalog {
    #[cfg(test)]
    pub fn fixture_pending(&mut self, id: &str) {
        self.pending = Some(Pending {
            id: id.into(),
            items: vec![Identity {
                pid: 10,
                created: "133700000000000001".into(),
                image: r"C:\fixture\App.exe".into(),
                owner: "fixture-owner".into(),
                session: 1,
            }],
            created: Instant::now(),
        });
    }
    pub fn owns_pending(&self, id: &str) -> bool {
        self.pending.as_ref().is_some_and(|p| p.id == id)
    }
    pub fn cancel_pending(&mut self) -> bool {
        self.pending.take().is_some()
    }
    fn insert(&mut self, items: Vec<Identity>) -> Option<String> {
        if items.is_empty() || items.len() > 32 || self.targets.len() >= 512 {
            return None;
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.targets.insert(
            id.clone(),
            Target {
                items,
                created: Instant::now(),
            },
        );
        Some(id)
    }
    pub fn refresh(&mut self, snapshot: &mut Value, apps: HashMap<String, PathBuf>) {
        *self = Self::default();
        let processes = process_control::enumerate();
        self.bind(snapshot, apps, &processes);
        let original = snapshot["applications"]["limits"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        snapshot["applications"]["limits"] = json!(format!(
            "{original} 桌面也按唯一已核验的开始菜单程序完整路径关联当前用户可核验实例；结束只处理确认中列出的集合，未知关联不可操作。"
        ));
    }
    fn bind(
        &mut self,
        snapshot: &mut Value,
        apps: HashMap<String, PathBuf>,
        processes: &[Identity],
    ) {
        let by_pid: HashMap<_, _> = processes.iter().map(|p| (p.pid, p)).collect();
        let mut refs = HashMap::new();
        for ranking in ["cpu", "memory"] {
            if let Some(rows) = snapshot["process_rankings"][ranking].as_array_mut() {
                for row in rows {
                    if let Some(fields) = row.as_object_mut() {
                        fields.remove("termination_ref");
                    }
                    row["termination_reason"] = json!(UNAVAILABLE);
                    let Some(pid) = row["pid"].as_u64().and_then(|p| u32::try_from(p).ok()) else {
                        continue;
                    };
                    let Some(item) = by_pid
                        .get(&pid)
                        .filter(|p| row["created_filetime"].as_str() == Some(p.created.as_str()))
                    else {
                        continue;
                    };
                    let id = refs
                        .entry(pid)
                        .or_insert_with(|| self.insert(vec![(*item).clone()]));
                    if let Some(id) = id {
                        row["termination_ref"] = json!(id);
                        row["termination_reason"] = json!("");
                    }
                }
            }
        }
        if let Some(items) = snapshot["applications"]["items"].as_array_mut() {
            for app in items {
                if let Some(fields) = app.as_object_mut() {
                    fields.remove("termination_ref");
                    fields.remove("running_instances");
                }
                app["termination_reason"] = json!(UNAVAILABLE);
                let Some(exe) = app["action_id"].as_str().and_then(|id| apps.get(id)) else {
                    continue;
                };
                let instances: Vec<_> = processes
                    .iter()
                    .filter(|p| p.image.eq_ignore_ascii_case(&exe.to_string_lossy()))
                    .cloned()
                    .collect();
                if !instances.is_empty() {
                    app["running"] = json!(true);
                    app["running_instances"] = json!(instances.len());
                }
                if let Some(id) = self.insert(instances) {
                    app["termination_ref"] = json!(id);
                    app["termination_reason"] = json!("");
                }
            }
        }
    }
    pub fn prepare(&mut self, id: &str) -> Result<Value, String> {
        self.prepare_with(id, process_control::capture)
    }
    fn prepare_with(
        &mut self,
        id: &str,
        probe: impl Fn(u32) -> Result<Identity, &'static str>,
    ) -> Result<Value, String> {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.created.elapsed() >= Duration::from_secs(90))
        {
            self.pending = None;
        }
        if self.pending.is_some() {
            return Err("已有结束进程确认待处理，请先确认或取消".into());
        }
        let target = self
            .targets
            .get(id)
            .filter(|t| t.created.elapsed() < Duration::from_secs(300))
            .ok_or("进程引用已过期，请刷新概览")?;
        for item in &target.items {
            match probe(item.pid) {
                Ok(actual) if actual == *item => {}
                Err("already_exited") => {}
                _ => return Err("进程身份或权限已变化，请刷新概览；尚未结束任何进程".into()),
            }
        }
        let token = uuid::Uuid::new_v4().to_string();
        let processes: Vec<_> = target
            .items
            .iter()
            .map(|p| json!({"pid":p.pid,"name":name(p)}))
            .collect();
        self.pending = Some(Pending {
            id: token.clone(),
            items: target.items.clone(),
            created: Instant::now(),
        });
        Ok(
            json!({"state":"confirmation_required","confirmation_id":token,"processes":processes,"count":processes.len(),"warning":"强制结束可能丢失未保存的内容；只结束此处列出的进程，不自动结束其他实例或子进程。"}),
        )
    }
    pub fn resolve(
        &mut self,
        id: &str,
        confirmed: bool,
        cancel: &Cancellation,
    ) -> Result<Value, String> {
        self.resolve_with(id, confirmed, cancel, process_control::terminate)
    }
    fn resolve_with(
        &mut self,
        id: &str,
        confirmed: bool,
        cancel: &Cancellation,
        execute: impl Fn(&Identity, &Cancellation) -> &'static str,
    ) -> Result<Value, String> {
        if self.pending.as_ref().is_none_or(|p| p.id != id) {
            return Err("结束进程确认已失效，请重新选择".into());
        }
        let pending = self.pending.take().unwrap();
        if !confirmed {
            return Ok(
                json!({"state":"cancelled","message":"已取消，未结束进程。","processes":[]}),
            );
        }
        if pending.created.elapsed() >= Duration::from_secs(90) {
            return Err("结束进程确认已过期，未执行".into());
        }
        let results: Vec<_> = pending
            .items
            .iter()
            .map(|p| {
                let code = if cancel.is_cancelled() {
                    "cancelled"
                } else {
                    execute(p, cancel)
                };
                json!({"pid":p.pid,"name":name(p),"state":code,"message":code_text(code)})
            })
            .collect();
        let ended = results
            .iter()
            .filter(|r| matches!(r["state"].as_str(), Some("terminated" | "already_exited")))
            .count();
        let waiting = results.iter().filter(|r| r["state"] == "pending").count();
        let state = if results.iter().all(|r| r["state"] == "cancelled") {
            "cancelled"
        } else if ended == results.len() {
            "completed"
        } else if ended + waiting == results.len() {
            "pending"
        } else if ended + waiting > 0 {
            "partial"
        } else {
            "failed"
        };
        let detail = results
            .iter()
            .map(|r| {
                format!(
                    "{} / PID {}：{}",
                    r["name"].as_str().unwrap_or(""),
                    r["pid"],
                    r["message"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("；");
        Ok(
            json!({"state":state,"message":format!("已退出 {ended}/{}，仍在结束 {waiting}。{detail}",results.len()),"processes":results}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    fn item(pid: u32) -> Identity {
        Identity {
            pid,
            created: format!("1337000000000000{pid}"),
            image: r"C:\fixture\App.exe".into(),
            owner: "fixture-owner".into(),
            session: 1,
        }
    }
    #[test]
    fn cancel_expiry_replay_changed_identity_and_unknown_refs_never_execute() {
        let mut catalog = Catalog::default();
        let identity = item(10);
        let reference = catalog.insert(vec![identity.clone()]).unwrap();
        assert!(
            catalog
                .prepare_with("10", |_| panic!("unknown PID isn't authority"))
                .is_err()
        );
        assert!(
            catalog
                .prepare_with(&reference, |_| {
                    let mut changed = identity.clone();
                    changed.created.push('1');
                    Ok(changed)
                })
                .is_err()
        );
        for code in [
            "access_denied",
            "protected_process",
            "other_user",
            "identity_unavailable",
        ] {
            assert!(catalog.prepare_with(&reference, |_| Err(code)).is_err());
        }
        let prepared = catalog
            .prepare_with(&reference, |_| Ok(identity.clone()))
            .unwrap();
        let token = prepared["confirmation_id"].as_str().unwrap();
        assert!(
            catalog
                .prepare_with(&reference, |_| panic!("pending"))
                .is_err()
        );
        assert!(
            catalog
                .resolve_with("wrong", true, &Cancellation::default(), |_, _| panic!(
                    "wrong token"
                ))
                .is_err()
        );
        assert_eq!(
            catalog
                .resolve_with(token, false, &Cancellation::default(), |_, _| panic!(
                    "cancelled"
                ))
                .unwrap()["state"],
            "cancelled"
        );
        assert!(
            catalog
                .resolve_with(token, true, &Cancellation::default(), |_, _| panic!(
                    "replay"
                ))
                .is_err()
        );
        catalog
            .prepare_with(&reference, |_| Ok(identity.clone()))
            .unwrap();
        let token = catalog.pending.as_ref().unwrap().id.clone();
        catalog.pending.as_mut().unwrap().created = Instant::now() - Duration::from_secs(91);
        assert!(
            catalog
                .resolve_with(&token, true, &Cancellation::default(), |_, _| panic!(
                    "expired"
                ))
                .is_err()
        );
        catalog.targets.get_mut(&reference).unwrap().created =
            Instant::now() - Duration::from_secs(301);
        assert!(
            catalog
                .prepare_with(&reference, |_| panic!("old reference"))
                .is_err()
        );
    }
    #[test]
    fn frozen_multi_instance_set_reports_pending_partial_and_cancellation_without_expansion() {
        let mut catalog = Catalog::default();
        let identities = vec![item(10), item(11), item(12)];
        let reference = catalog.insert(identities.clone()).unwrap();
        let prepared = catalog
            .prepare_with(&reference, |pid| Ok(item(pid)))
            .unwrap();
        assert_eq!(prepared["count"], 3);
        catalog
            .targets
            .get_mut(&reference)
            .unwrap()
            .items
            .push(item(13));
        let calls = RefCell::new(Vec::new());
        let result = catalog
            .resolve_with(
                prepared["confirmation_id"].as_str().unwrap(),
                true,
                &Cancellation::default(),
                |p, _| {
                    calls.borrow_mut().push(p.pid);
                    match p.pid {
                        10 => "terminated",
                        11 => "pending",
                        _ => "access_denied",
                    }
                },
            )
            .unwrap();
        assert_eq!(*calls.borrow(), vec![10, 11, 12]);
        assert_eq!(result["state"], "partial");
        assert!(result["message"].as_str().unwrap().contains("仍在结束 1"));
        let reference = catalog.insert(vec![item(20)]).unwrap();
        let p = catalog
            .prepare_with(&reference, |pid| Ok(item(pid)))
            .unwrap();
        assert_eq!(
            catalog
                .resolve_with(
                    p["confirmation_id"].as_str().unwrap(),
                    true,
                    &Cancellation::default(),
                    |_, _| "pending"
                )
                .unwrap()["state"],
            "pending"
        );
        let p = catalog
            .prepare_with(&reference, |pid| Ok(item(pid)))
            .unwrap();
        let cancelled = Cancellation::default();
        cancelled.cancel();
        let result = catalog
            .resolve_with(
                p["confirmation_id"].as_str().unwrap(),
                true,
                &cancelled,
                |_, _| panic!("cancel"),
            )
            .unwrap();
        assert_eq!(result["processes"][0]["state"], "cancelled");
        catalog
            .prepare_with(&reference, |pid| Ok(item(pid)))
            .unwrap();
        assert!(catalog.cancel_pending());
        assert!(!catalog.cancel_pending());
    }
    #[test]
    fn bindings_require_precise_creation_and_exact_unique_app_target_never_process_name() {
        let mut catalog = Catalog::default();
        let mut snapshot = json!({"process_rankings":{"cpu":[{"pid":10,"created_filetime":item(10).created},{"pid":11,"created_filetime":"wrong"}],"memory":[{"pid":10,"created_filetime":item(10).created}]},"applications":{"items":[{"name":"A","action_id":"a"},{"name":"same basename","action_id":"b"}]},"facts_for_explanation":{"safe":true}});
        catalog.bind(
            &mut snapshot,
            HashMap::from([
                ("a".into(), PathBuf::from(r"C:\fixture\App.exe")),
                ("b".into(), PathBuf::from(r"C:\other\App.exe")),
            ]),
            &[item(10), item(11)],
        );
        assert_eq!(
            snapshot["process_rankings"]["cpu"][0]["termination_ref"],
            snapshot["process_rankings"]["memory"][0]["termination_ref"]
        );
        assert!(snapshot["process_rankings"]["cpu"][1]["termination_ref"].is_null());
        assert_eq!(snapshot["applications"]["items"][0]["running_instances"], 2);
        assert!(snapshot["applications"]["items"][1]["termination_ref"].is_null());
        assert_eq!(snapshot["facts_for_explanation"], json!({"safe":true}));
        assert!(catalog.insert(vec![item(10); 33]).is_none());
        let old = snapshot["process_rankings"]["cpu"][0]["termination_ref"]
            .as_str()
            .unwrap()
            .to_string();
        catalog = Catalog::default();
        assert!(
            catalog
                .prepare_with(&old, |_| panic!("invalidated"))
                .is_err()
        );
    }
}
