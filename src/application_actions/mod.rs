pub mod journal;
pub mod launch;
pub mod native;
use crate::{
    core::{Cancellation, now},
    process_control::{self, Identity},
    runtime::Interaction,
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone)]
enum Target {
    Launch(launch::Target),
    Process(Identity),
    Unsupported(String),
}
#[derive(Clone)]
struct Entry {
    reference: String,
    kind: String,
    name: String,
    target: Target,
    created: Instant,
}
#[derive(Default)]
pub struct Actions {
    entries: HashMap<String, Entry>,
    pages: HashMap<String, (String, Vec<String>, usize)>,
    pub directory: Option<PathBuf>,
    pub excluded_environment: Vec<String>,
    used: bool,
    last: Option<(String, String, Value)>,
    pub delivered: Vec<Value>,
    pub target_results: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    kind: String,
    query: Option<String>,
    observation_id: Option<String>,
    row_ref: Option<String>,
    limit: Option<usize>,
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    target_ref: String,
}
pub fn is_tool(name: &str) -> bool {
    matches!(
        name,
        "app_targets" | "app_launch" | "app_close" | "process_terminate"
    )
}
fn name(p: &Identity) -> String {
    crate::core::safe_text(p.image.rsplit(['\\', '/']).next().unwrap_or("Process"))
        .chars()
        .take(256)
        .collect()
}
impl Actions {
    pub fn begin_turn(&mut self) {
        self.used = false;
        self.last = None;
        self.delivered.clear();
        self.target_results.clear();
    }
    pub fn reset(&mut self) {
        self.entries.clear();
        self.pages.clear();
        self.begin_turn();
    }
    pub fn block_turn(&mut self) {
        self.used = true;
    }
    fn journal(&self) -> Result<journal::Journal> {
        journal::Journal::open(
            self.directory
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("journal_unavailable"))?,
        )
        .map_err(|_| anyhow::anyhow!("journal_unavailable"))
    }
    pub fn receipts(&self) -> Result<Value> {
        Ok(json!({"receipts":self.journal()?.list()?}))
    }
    pub fn clear_completed(&self) -> Result<Value> {
        Ok(json!({"cleared":self.journal()?.prune(true)?}))
    }
    pub fn reconcile(&self, id: &str) -> Result<Value> {
        let mut j = self.journal()?;
        let mut r = j
            .list()?
            .into_iter()
            .find(|r| r["operation_id"] == id)
            .ok_or_else(|| anyhow::anyhow!("receipt_expired"))?;
        if let Ok(identity) = serde_json::from_value::<Identity>(r["evidence"]["process"].clone()) {
            let check = native::reconcile(&identity);
            r["evidence"]["reconciliation"] = check;
        } else {
            r["evidence"]["reconciliation"] =
                json!({"process_state":"unknown","code":"original_instance_unverifiable"});
        }
        r["checked_at"] = json!(now());
        j.save(&r)?;
        Ok(r)
    }
    pub fn targets(&mut self, args: Value, cancel: &Cancellation) -> Result<Value> {
        let q: Query =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        ensure!(
            matches!(q.kind.as_str(), "launch" | "close" | "terminate"),
            "invalid_arguments"
        );
        ensure!(
            q.query.is_some() != (q.observation_id.is_some() && q.row_ref.is_some())
                && q.observation_id.is_some() == q.row_ref.is_some(),
            "invalid_arguments"
        );
        let limit = q.limit.unwrap_or(10);
        ensure!(
            (1..=50).contains(&limit) && q.query.as_ref().is_none_or(|s| s.chars().count() <= 128),
            "invalid_arguments"
        );
        if q.row_ref.is_some() {
            return Ok(
                json!({"status":"needs_reselection","items":[],"reason":"observation_is_not_action_identity"}),
            );
        }
        let query = q.query.as_deref().unwrap_or("").to_lowercase();
        let signature = format!("{}:{limit}:{query}", q.kind);
        self.entries
            .retain(|_, e| e.created.elapsed() < Duration::from_secs(120));
        self.pages
            .retain(|_, (_, ids, _)| ids.iter().all(|id| self.entries.contains_key(id)));
        cancel.check()?;
        if q.kind == "close" {
            return Ok(
                json!({"status":"unavailable","items":[],"reason":"close_identity_not_atomic","limits":"Normal close is unavailable; do not substitute force termination. Ask the user for a new explicit request."}),
            );
        }
        let (ids, offset) = if let Some(cursor) = q.cursor {
            let (s, ids, offset) = self
                .pages
                .get(&cursor)
                .ok_or_else(|| anyhow::anyhow!("target_expired"))?;
            ensure!(s == &signature, "invalid_arguments");
            (ids.clone(), *offset)
        } else {
            let mut found = Vec::new();
            if q.kind == "launch" {
                for (display, targets) in
                    launch::discover().map_err(|_| anyhow::anyhow!("launch_unavailable"))?
                {
                    if display.contains(&query) {
                        if targets.len() != 1 {
                            found.push((
                                display.clone(),
                                Target::Unsupported("ambiguous_launcher".into()),
                            ));
                        } else {
                            match targets.into_iter().next().flatten() {
                                Some(t) => found.push((display.clone(), Target::Launch(t))),
                                None => found.push((
                                    display.clone(),
                                    Target::Unsupported("launch_unavailable".into()),
                                )),
                            }
                        }
                    }
                }
            } else {
                for p in process_control::enumerate() {
                    let display = name(&p);
                    if display.to_lowercase().contains(&query) {
                        found.push((display, Target::Process(p)));
                    }
                }
            }
            found.sort_by(|a, b| a.0.cmp(&b.0));
            let mut ids = Vec::new();
            for (name, target) in found
                .into_iter()
                .take(256usize.saturating_sub(self.entries.len()))
            {
                cancel.check()?;
                let id = uuid::Uuid::new_v4().to_string();
                ids.push(id.clone());
                self.entries.insert(
                    id.clone(),
                    Entry {
                        reference: id,
                        kind: q.kind.clone(),
                        name,
                        target,
                        created: Instant::now(),
                    },
                );
            }
            (ids, 0)
        };
        let mut local = Vec::new();
        let mut model = Vec::new();
        for (i, id) in ids.iter().enumerate().skip(offset).take(limit) {
            let e = self
                .entries
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("target_expired"))?;
            let mut row = json!({"target_ref":e.reference,"name":e.name,"kind":e.kind,"index":i+1,"supported":true});
            match &e.target {
                Target::Unsupported(reason) => {
                    row["supported"] = json!(false);
                    row["reason"] = json!(reason);
                }
                Target::Launch(t) => {
                    row["path"] = json!(t.exe);
                    row["entry_path"] = json!(t.link);
                }
                Target::Process(p) => {
                    row["pid"] = json!(p.pid);
                    row["created"] = json!(p.created);
                    row["path"] = json!(p.image);
                }
            }
            let mut projected = row.clone();
            for key in ["pid", "created", "path", "entry_path"] {
                projected.as_object_mut().unwrap().remove(key);
            }
            model.push(projected);
            local.push(row);
        }
        let next = if offset + model.len() < ids.len() {
            ensure!(self.pages.len() < 512, "target_budget");
            let cursor = uuid::Uuid::new_v4().to_string();
            self.pages.insert(
                cursor.clone(),
                (signature, ids.clone(), offset + model.len()),
            );
            Some(cursor)
        } else {
            None
        };
        let value = json!({"status":"ok","items":model,"local_items":local,"page":{"returned":model.len(),"matched":ids.len(),"next_cursor":next},"query":{"kind":q.kind,"query":query,"limit":limit},"limits":"Current-user supported entry/process subset. Targets expire after two minutes. A reference is not approval."});
        self.target_results.push(value.clone());
        Ok(value)
    }
    pub fn action(
        &mut self,
        tool: &str,
        args: Value,
        cancel: &Cancellation,
        ui: &mut dyn Interaction,
    ) -> Result<Value> {
        let r: Reference =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        if let Some((name, id, value)) = &self.last {
            if name == tool && id == &r.target_ref {
                return Ok(value.clone());
            }
        }
        ensure!(!self.used, "turn_action_limit");
        self.used = true;
        if tool == "app_close" {
            let v = json!({"lifecycle":"failed_before_dispatch","evidence":{"code":"close_identity_not_atomic"}});
            self.last = Some((tool.into(), r.target_ref, v.clone()));
            return Ok(v);
        }
        ensure!(!native::elevated(), "elevated_host");
        let expected = if tool == "app_launch" {
            "launch"
        } else {
            "terminate"
        };
        let e = self
            .entries
            .get(&r.target_ref)
            .filter(|e| e.kind == expected && e.created.elapsed() < Duration::from_secs(120))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("target_expired"))?;
        verify(&e.target)?;
        cancel.check()?;
        let mut journal = self.journal()?;
        ensure!(
            !journal
                .list()?
                .iter()
                .any(|old| old["lifecycle"] == "unknown"
                    && old.get("checked_at").is_none()
                    && old["name"] == e.name),
            "needs_reconciliation"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let mut preview = json!({"operation_id":id,"kind":expected,"name":e.name,"created_at":now(),"expires_in_seconds":90,"ordinary_privilege":true,"risk":if expected=="terminate"{"unsaved_data_loss"}else{"creates_process_not_window_ready"}});
        match &e.target {
            Target::Unsupported(_) => anyhow::bail!("launch_unavailable"),
            Target::Launch(t) => {
                preview["path"] = json!(t.exe);
                preview["entry_path"] = json!(t.link);
                preview["working_directory"] = json!(t.exe.parent());
            }
            Target::Process(p) => {
                preview["pid"] = json!(p.pid);
                preview["created"] = json!(p.created);
                preview["path"] = json!(p.image);
            }
        }
        let start = Instant::now();
        let approved = ui.confirm_app_action(&preview)?;
        let mut receipt = json!({"operation_id":id,"kind":expected,"name":e.name,"created_at":preview["created_at"],"observed_at":now(),"lifecycle":"rejected","evidence":{"code":"not_dispatched"}});
        if approved && !cancel.is_cancelled() && start.elapsed() < Duration::from_secs(90) {
            verify(&e.target)?;
            cancel.check()?;
            receipt["lifecycle"] = json!("dispatching");
            journal.save(&receipt)?;
            let outcome = match &e.target {
                Target::Unsupported(_) => Err(anyhow::anyhow!("launch_unavailable")),
                Target::Launch(t) => native::launch(t, &self.excluded_environment, cancel),
                Target::Process(p) => {
                    let code = process_control::terminate(p, cancel);
                    Ok(
                        json!({"process":p,"code":code,"process_exited":matches!(code,"terminated"|"already_exited"),"process_state":if matches!(code,"terminated"|"already_exited"){"exited"}else if code=="pending"{"still_running"}else{"unknown"}}),
                    )
                }
            };
            match outcome {
                Ok(evidence) => {
                    let code = evidence["code"].as_str().unwrap_or("");
                    receipt["lifecycle"] = json!(if evidence["process_created"] == true
                        || matches!(code, "terminated" | "already_exited")
                    {
                        "completed"
                    } else if code == "pending" {
                        "unknown"
                    } else {
                        "failed_before_dispatch"
                    });
                    receipt["evidence"] = evidence;
                }
                Err(error) => {
                    receipt["lifecycle"] = json!("failed_before_dispatch");
                    receipt["evidence"] = json!({"code":public_error(&error)});
                }
            }
        } else if cancel.is_cancelled() {
            receipt["lifecycle"] = json!("rejected");
            receipt["evidence"]["code"] = json!("cancelled_before_dispatch");
        } else if start.elapsed() >= Duration::from_secs(90) {
            receipt["lifecycle"] = json!("expired");
        }
        receipt["observed_at"] = json!(now());
        if journal.save(&receipt).is_err() {
            receipt["persistence_warning"] = json!(true);
            receipt["lifecycle"] = json!("unknown");
        }
        self.delivered.push(receipt.clone());
        ui.result("app_action_receipt", &receipt);
        let projected = project_receipt(&receipt);
        self.last = Some((tool.into(), r.target_ref, projected.clone()));
        Ok(projected)
    }
}
fn verify(target: &Target) -> Result<()> {
    match target {
        Target::Unsupported(_) => anyhow::bail!("launch_unavailable"),
        Target::Launch(t) => {
            let _a = launch::pin(&t.link, &t.link_identity)
                .map_err(|_| anyhow::anyhow!("identity_changed"))?;
            let _b = launch::pin(&t.exe, &t.identity)
                .map_err(|_| anyhow::anyhow!("identity_changed"))?;
        }
        Target::Process(p) => {
            ensure!(
                process_control::capture(p.pid).as_ref() == Ok(p),
                "identity_changed"
            );
        }
    }
    Ok(())
}
pub fn public_error(error: &anyhow::Error) -> &str {
    match error.to_string().as_str() {
        "invalid_arguments" => "invalid_arguments",
        "target_expired" => "target_expired",
        "identity_changed" => "identity_changed",
        "needs_reconciliation" => "needs_reconciliation",
        "turn_action_limit" => "turn_action_limit",
        "elevated_host" => "elevated_host",
        "elevation_required" => "elevation_required",
        "launch_unavailable" => "launch_unavailable",
        "launch_failed" => "launch_failed",
        "environment_unavailable" => "environment_unavailable",
        "journal_unavailable" => "journal_unavailable",
        _ => "action_failed",
    }
}
pub fn project_receipt(local: &Value) -> Value {
    let mut out = json!({});
    for key in [
        "operation_id",
        "kind",
        "name",
        "created_at",
        "observed_at",
        "lifecycle",
        "checked_at",
        "persistence_warning",
    ] {
        if let Some(v) = local.get(key) {
            out[key] = v.clone();
        }
    }
    let e = &local["evidence"];
    for key in [
        "code",
        "process_created",
        "process_state",
        "process_exited",
        "exit_code",
        "observation_cancelled",
        "reconciliation",
    ] {
        if let Some(v) = e.get(key) {
            out["evidence"][key] = v.clone();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Ui {
        count: usize,
        approve: bool,
        change: Option<PathBuf>,
    }
    impl Interaction for Ui {
        fn confirm_app_action(&mut self, _: &Value) -> Result<bool> {
            self.count += 1;
            if let Some(p) = &self.change {
                std::fs::write(p, b"changed fixture")?;
            }
            Ok(self.approve)
        }
        fn confirm_move(&mut self, _: &crate::core::Operation) -> Result<bool> {
            panic!("no file operation")
        }
        fn confirm_open(&mut self, _: &crate::core::FileObject) -> Result<bool> {
            panic!("no file operation")
        }
        fn progress(&mut self, _: &str) {}
        fn result(&mut self, _: &str, _: &Value) {}
    }
    fn fixture() -> (tempfile::TempDir, Actions, String) {
        let d = tempfile::tempdir().unwrap();
        let exe = d.path().join("InertFixture.exe");
        let link = d.path().join("InertFixture.lnk");
        std::fs::write(&exe, b"not executable").unwrap();
        std::fs::write(&link, b"not an actual shortcut").unwrap();
        let id = "fixture-target".to_string();
        let target = launch::Target {
            identity: crate::platform::identity(&exe).unwrap(),
            link_identity: crate::platform::identity(&link).unwrap(),
            exe,
            link,
        };
        let mut a = Actions {
            directory: Some(d.path().join("journal")),
            ..Default::default()
        };
        a.entries.insert(
            id.clone(),
            Entry {
                reference: id.clone(),
                kind: "launch".into(),
                name: "InertFixture".into(),
                target: Target::Launch(target),
                created: Instant::now(),
            },
        );
        (d, a, id)
    }
    #[test]
    fn strict_arguments_and_old_observation_cannot_authorize_actions() {
        let mut a = Actions::default();
        let c = Cancellation::default();
        assert!(
            a.targets(json!({"kind":"launch","query":"x","admin":true}), &c)
                .is_err()
        );
        assert!(
            a.action(
                "app_launch",
                json!({"target_ref":"x","confirmed":true}),
                &c,
                &mut Ui {
                    count: 0,
                    approve: true,
                    change: None
                }
            )
            .is_err()
        );
        assert_eq!(
            a.targets(
                json!({"kind":"terminate","observation_id":"old","row_ref":"old:1"}),
                &c
            )
            .unwrap()["status"],
            "needs_reselection"
        );
    }
    #[test]
    fn close_is_explicitly_unavailable_and_never_falls_back() {
        let mut a = Actions::default();
        let c = Cancellation::default();
        assert_eq!(
            a.targets(json!({"kind":"close","query":"anything"}), &c)
                .unwrap()["reason"],
            "close_identity_not_atomic"
        );
        let mut ui = Ui {
            count: 0,
            approve: true,
            change: None,
        };
        a.action("app_close", json!({"target_ref":"anything"}), &c, &mut ui)
            .unwrap();
        assert!(
            a.action(
                "process_terminate",
                json!({"target_ref":"anything"}),
                &c,
                &mut ui
            )
            .is_err()
        );
        assert_eq!(ui.count, 0);
    }
    #[test]
    fn receipt_projection_has_no_native_identity_path_or_confirmation_token() {
        let v = project_receipt(
            &json!({"operation_id":"one","path":"secret","confirmation_id":"secret","evidence":{"process":{"pid":123,"image":"secret"},"process_created":true,"code":"process_created_not_window_ready"}}),
        );
        assert!(!v.to_string().contains("secret"));
        assert!(!v.to_string().contains("123"));
        assert_eq!(v["evidence"]["process_created"], true);
    }
    #[test]
    fn cancelled_and_expired_targets_never_prompt() {
        let (_d, mut a, id) = fixture();
        a.entries.get_mut(&id).unwrap().created = Instant::now() - Duration::from_secs(121);
        let mut ui = Ui {
            count: 0,
            approve: true,
            change: None,
        };
        assert!(
            a.action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui
            )
            .is_err()
        );
        assert_eq!(ui.count, 0);
    }
    #[cfg(windows)]
    #[test]
    fn local_rejection_is_one_shot_and_exact_duplicate_reuses_receipt() {
        if native::elevated() {
            return;
        }
        let (_d, mut a, id) = fixture();
        let mut ui = Ui {
            count: 0,
            approve: false,
            change: None,
        };
        let one = a
            .action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui,
            )
            .unwrap();
        let two = a
            .action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui,
            )
            .unwrap();
        assert_eq!(one, two);
        assert_eq!(ui.count, 1);
        assert_eq!(one["lifecycle"], "rejected");
        assert!(
            a.action(
                "app_launch",
                json!({"target_ref":"another"}),
                &Cancellation::default(),
                &mut ui
            )
            .is_err()
        );
        assert_eq!(ui.count, 1);
    }
    #[cfg(windows)]
    #[test]
    fn journal_failure_and_changed_frozen_target_never_dispatch() {
        if native::elevated() {
            return;
        }
        let (d, mut a, id) = fixture();
        let file = d.path().join("inert-path");
        std::fs::write(&file, b"file not a directory").unwrap();
        a.directory = Some(file);
        let mut ui = Ui {
            count: 0,
            approve: true,
            change: None,
        };
        assert!(
            a.action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui
            )
            .is_err()
        );
        assert_eq!(ui.count, 0);
        let (_d, mut a, id) = fixture();
        ui.change = Some(match &a.entries[&id].target {
            Target::Launch(t) => t.exe.clone(),
            _ => unreachable!(),
        });
        assert!(
            a.action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui
            )
            .is_err()
        );
        assert_eq!(ui.count, 1);
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires explicitly built dedicated phase2_owned_app fixture"]
    fn owned_native_launch_environment_receipt_and_force_termination() {
        let source = PathBuf::from(
            std::env::var_os("DAO_PHASE2_OWNED_APP").expect("dedicated fixture path"),
        );
        assert_eq!(source.file_stem().unwrap(), "phase2_owned_app");
        assert!(
            !native::elevated(),
            "Run the owned fixture test without elevation"
        );
        let (dir, mut a, id) = fixture();
        let target = match &mut a.entries.get_mut(&id).unwrap().target {
            Target::Launch(t) => t,
            _ => unreachable!(),
        };
        std::fs::copy(&source, &target.exe).unwrap();
        std::fs::write(dir.path().join("phase2-fixture.marker"), b"owned fixture").unwrap();
        target.identity = crate::platform::identity(&target.exe).unwrap();
        a.excluded_environment = vec!["PHASE2_API_TEST_SECRET".into()];
        let mut ui = Ui {
            count: 0,
            approve: true,
            change: None,
        };
        let launched = a
            .action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui,
            )
            .unwrap();
        assert_eq!(launched["evidence"]["process_created"], true);
        assert_eq!(ui.count, 1);
        let local = a.delivered[0].clone();
        let identity: Identity =
            serde_json::from_value(local["evidence"]["process"].clone()).unwrap();
        struct Cleanup(Identity);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = process_control::terminate(&self.0, &Cancellation::default());
            }
        }
        let _cleanup = Cleanup(identity.clone());
        let ready: Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("phase2-ready.json")).unwrap())
                .unwrap();
        for key in [
            "configured_key_present",
            "dao_injection_present",
            "webview_injection_present",
        ] {
            assert_eq!(ready[key], false, "{key}");
        }
        assert_eq!(ready["cwd_matches_exe_parent"], true);
        assert_eq!(
            a.reconcile(launched["operation_id"].as_str().unwrap())
                .unwrap()["evidence"]["reconciliation"]["process_state"],
            "still_running"
        );
        a.begin_turn();
        let reference = "owned-process".to_owned();
        a.entries.insert(
            reference.clone(),
            Entry {
                reference: reference.clone(),
                kind: "terminate".into(),
                name: "InertFixture".into(),
                target: Target::Process(identity),
                created: Instant::now(),
            },
        );
        let stopped = a
            .action(
                "process_terminate",
                json!({"target_ref":reference}),
                &Cancellation::default(),
                &mut ui,
            )
            .unwrap();
        assert_eq!(stopped["evidence"]["process_exited"], true);
        assert_eq!(ui.count, 2);
        assert_eq!(a.journal().unwrap().list().unwrap().len(), 2);
        std::fs::write(dir.path().join("exit-short.marker"), b"exit").unwrap();
        a.begin_turn();
        let short = a
            .action(
                "app_launch",
                json!({"target_ref":id}),
                &Cancellation::default(),
                &mut ui,
            )
            .unwrap();
        assert_eq!(short["evidence"]["process_created"], true);
        assert_eq!(short["evidence"]["process_state"], "exited");
    }
}
