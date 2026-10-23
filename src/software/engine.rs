//! Persistent single-dispatch maintenance transaction. Recovery never repeats a native action.
use super::contract::*;
use crate::{application_actions::journal::Journal, core::Cancellation};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
pub struct Engine {
    journal: Journal,
    used: bool,
}
pub struct Approval {
    pub approved: bool,
    pub agreement_digest: Option<String>,
}
impl Engine {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            journal: Journal::open(path)?,
            used: false,
        })
    }
    pub fn begin_turn(&mut self) {
        self.used = false;
    }
    pub fn block_turn(&mut self) {
        self.used = true;
    }
    pub fn used(&self) -> bool {
        self.used
    }
    pub fn history(&self) -> Result<Vec<Value>> {
        self.journal.list()
    }
    /// Startup has no surviving external provider after the journal lock is acquired.
    /// Record present file placement, while retaining unknown historical attribution.
    pub fn check_startup(
        &mut self,
        root: &std::path::Path,
        retained: &std::path::Path,
    ) -> Result<Vec<Value>> {
        ensure!(
            root.parent()
                .is_some_and(|p| p.join("DaoShellDisabledStartup") == retained),
            "startup_scope_changed"
        );
        let _parent = crate::platform::pin_directory(root.parent().unwrap())?;
        for mut r in self.history()? {
            if r["lifecycle"] == "startup_checked_original" {
                continue; // Settled observations must not later claim a newly changed file.
            }
            if r["operation"] == "startup_disable"
                && r["lifecycle"] == "verified"
                && r["startup_proof"].is_null()
            {
                r["startup_proof"] = json!({"original":r["original"],"retained":r["evidence"]["retained"],"identity":r["evidence"]["identity"],"digest":r["evidence"]["digest"]});
            }
            if !matches!(
                r["operation"].as_str(),
                Some("startup_disable" | "startup_restore")
            ) || r["startup_proof"].is_null()
            {
                continue;
            }
            let checked = match std::fs::symlink_metadata(retained) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    super::startup::observe(&r["startup_proof"], root, retained)
                }
                Ok(_) => super::store::with_retention(retained, false, || {
                    super::startup::observe(&r["startup_proof"], root, retained)
                }),
                Err(e) => Err(e.into()),
            };
            let observed = checked.unwrap_or("conflict_or_unknown");
            r["startup_check"] = json!({"at":crate::core::now(),"placement":observed,"attribution":"unknown","automatic_replay":false});
            if matches!(
                r["lifecycle"].as_str(),
                Some(
                    "unknown"
                        | "verified"
                        | "startup_checked_retained"
                        | "startup_checked_original"
                )
            ) {
                r["lifecycle"] = json!(match observed {
                    "retained" => "startup_checked_retained",
                    "original" => "startup_checked_original",
                    _ => "unknown",
                });
            }
            self.journal.save(&r)?;
        }
        self.history()
    }
    pub fn local_transition(
        &mut self,
        mut receipt: Value,
        cancel: &Cancellation,
        confirm: &mut dyn FnMut(&Value) -> Result<bool>,
        dispatch: &mut dyn FnMut() -> Result<Value>,
    ) -> Result<Value> {
        ensure!(!self.used, "side_effect_already_attempted");
        self.used = true;
        ensure!(!self.unresolved()?, "maintenance_reconciliation_required");
        let confirmation_started = std::time::Instant::now();
        if cancel.is_cancelled()
            || !confirm(&receipt)?
            || cancel.is_cancelled()
            || confirmation_started.elapsed() >= std::time::Duration::from_secs(90)
        {
            return Ok(json!({"status":"cancelled_before_dispatch"}));
        }
        receipt["maintenance_version"] = json!(1);
        receipt["approval_origin"] = json!("local_confirmation");
        receipt["created_at"] = json!(crate::core::now());
        receipt["lifecycle"] = json!("dispatching");
        self.journal.save(&receipt)?;
        if cancel.is_cancelled()
            || confirmation_started.elapsed() >= std::time::Duration::from_secs(90)
        {
            receipt["lifecycle"] = json!("cancelled_before_dispatch");
            self.journal.save(&receipt)?;
            return Ok(receipt);
        }
        match dispatch() {
            Ok(evidence) => {
                receipt["lifecycle"] = evidence["lifecycle"].clone();
                receipt["evidence"] = evidence;
            }
            Err(_) => {
                receipt["lifecycle"] = json!("unknown");
                receipt["evidence"] = json!({"code":"provider_outcome_unknown"});
            }
        }
        self.finish(receipt)
    }
    pub fn unresolved(&self) -> Result<bool> {
        Ok(self.history()?.iter().any(|r| {
            matches!(
                r["lifecycle"].as_str(),
                Some(
                    "unknown" | "dispatching" | "dispatched" | "awaiting_user" | "awaiting_reboot"
                )
            )
        }))
    }
    pub fn execute(
        &mut self,
        plan: &Plan,
        provider: &mut dyn Provider,
        cancel: &Cancellation,
        confirm: &mut dyn FnMut(&Plan) -> Result<Approval>,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Value> {
        ensure!(!self.used, "side_effect_already_attempted");
        self.used = true;
        ensure!(!self.unresolved()?, "maintenance_reconciliation_required");
        let current = provider.inspect(&plan.package.identity)?;
        // Confirm only a current, exact plan. Agreement approval is validated separately below.
        plan.revalidate(&current, plan.package.agreement_digest.as_deref())?;
        if cancel.is_cancelled() {
            return Ok(json!({"status":"cancelled_before_dispatch"}));
        }
        let confirmation_started = std::time::Instant::now();
        let approval = confirm(plan)?;
        if !approval.approved
            || cancel.is_cancelled()
            || confirmation_started.elapsed() >= std::time::Duration::from_secs(90)
        {
            return Ok(json!({"status":"cancelled_before_dispatch"}));
        }
        let current = provider.inspect(&plan.package.identity)?;
        plan.revalidate(&current, approval.agreement_digest.as_deref())?;
        let mut receipt = json!({"maintenance_version":1,"operation_id":plan.operation_id,"created_at":crate::core::now(),"operation":plan.operation,"package":plan.package.identity,"prestate":plan.package.installation,"lifecycle":"dispatching","evidence":{"code":"outcome_not_observed"}});
        receipt["context"] = plan.context.clone();
        receipt["agreement_digest"] = json!(approval.agreement_digest);
        receipt["approval_origin"] = json!("local_confirmation");
        self.journal.save(&receipt)?; // Must succeed before any native dispatch.
        if cancel.is_cancelled()
            || confirmation_started.elapsed() >= std::time::Duration::from_secs(90)
        {
            receipt["lifecycle"] = json!("cancelled_before_dispatch");
            self.journal.save(&receipt)?;
            return Ok(receipt);
        }
        let result = provider.dispatch(plan, cancel, progress);
        match result {
            Ok(result) => {
                receipt["evidence"] = serde_json::to_value(&result)?;
                receipt["lifecycle"] = serde_json::to_value(&result.outcome)?;
                if matches!(result.outcome, Outcome::Verified) {
                    let checked = provider.inspect(&plan.package.identity);
                    let verified =
                        checked
                            .as_ref()
                            .is_ok_and(|p| match (&plan.operation, &p.installation) {
                                (Operation::Uninstall, Installation::AbsentVerified { scope }) => {
                                    *scope == plan.package.identity.scope
                                }
                                (
                                    Operation::Install | Operation::Upgrade,
                                    Installation::Present { identity, .. },
                                ) => identity == &plan.package.identity,
                                _ => false,
                            });
                    if !verified {
                        receipt["lifecycle"] = json!("unknown");
                        receipt["evidence"]["postcheck"] = json!("state_not_verified");
                    }
                }
            }
            Err(_) => {
                receipt["lifecycle"] = json!("unknown");
                receipt["evidence"] = json!({"code":"provider_outcome_unknown"});
            }
        }
        self.finish(receipt)
    }
    fn finish(&mut self, mut receipt: Value) -> Result<Value> {
        if self.journal.save(&receipt).is_err() {
            receipt["persistence_warning"] = json!(true);
            receipt["lifecycle"] = json!("unknown");
        }
        Ok(receipt)
    }
    /// A read-only check can resolve the gate only when the provider explicitly proves idle.
    /// Seeing the desired package state alone does not attribute an interrupted action.
    pub fn reconcile(
        &mut self,
        id: &str,
        provider_idle: bool,
        current: &Installation,
    ) -> Result<Value> {
        let mut r = self
            .history()?
            .into_iter()
            .find(|r| r["operation_id"] == id)
            .ok_or_else(|| anyhow::anyhow!("receipt_not_found"))?;
        r["last_check"] = json!({"at":crate::core::now(),"observed_state":current,"attribution":"unknown","provider_idle":provider_idle});
        if provider_idle && r["lifecycle"] != "awaiting_reboot" {
            r["lifecycle"] = json!("checked_unknown");
        }
        self.journal.save(&r)?;
        Ok(r)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;
    #[cfg(windows)]
    fn startup_fixture() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        super::super::startup::PinnedShortcut,
        Value,
    ) {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Startup");
        let retained = d.path().join("DaoShellDisabledStartup");
        std::fs::create_dir(&root).unwrap();
        super::super::store::with_retention(&retained, true, || Ok(())).unwrap();
        let original = root.join("fixture.lnk");
        let mut bytes = vec![0; 76];
        bytes[..4].copy_from_slice(&[76, 0, 0, 0]);
        bytes[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        std::fs::write(&original, bytes).unwrap();
        let p = super::super::startup::PinnedShortcut::inspect(&original).unwrap();
        let proof = json!({"original":original,"retained":retained.join("one.lnk"),"identity":p.identity,"digest":p.digest.to_vec()});
        (d, root, retained, p, proof)
    }
    #[cfg(windows)]
    #[test]
    fn startup_final_save_failure_reopens_checks_and_restore_requires_new_confirmation() {
        let (d, root, retained, p, proof) = startup_fixture();
        let journal = d.path().join("journal");
        let saved = std::path::PathBuf::from(proof["retained"].as_str().unwrap());
        let mut engine = Engine::open(&journal).unwrap();
        let result = engine.local_transition(json!({"operation_id":"disable","operation":"startup_disable","startup_proof":proof}), &Cancellation::default(), &mut |_| Ok(true), &mut || {
            p.relocate(&saved)?;
            Ok(json!({"lifecycle":"verified","oversize":"x".repeat(40000)}))
        }).unwrap();
        assert_eq!(result["persistence_warning"], true);
        drop(engine);
        let mut engine = Engine::open(&journal).unwrap();
        assert!(engine.unresolved().unwrap());
        assert_eq!(engine.history().unwrap()[0]["startup_proof"], proof);
        let rows = engine.check_startup(&root, &retained).unwrap();
        assert_eq!(rows[0]["lifecycle"], "startup_checked_retained");
        assert_eq!(rows[0]["startup_check"]["attribution"], "unknown");
        assert!(!engine.unresolved().unwrap());
        let restore =
            json!({"operation_id":"restore","operation":"startup_restore","startup_proof":proof});
        let mut calls = 0;
        engine
            .local_transition(
                restore.clone(),
                &Cancellation::default(),
                &mut |_| Ok(false),
                &mut || {
                    calls += 1;
                    Ok(json!({"lifecycle":"verified"}))
                },
            )
            .unwrap();
        assert_eq!(calls, 0);
        assert!(saved.exists());
        engine.begin_turn();
        let original = p.path.clone();
        engine
            .local_transition(
                restore,
                &Cancellation::default(),
                &mut |_| Ok(true),
                &mut || {
                    super::super::startup::PinnedShortcut::inspect(&saved)?.relocate(&original)?;
                    Ok(json!({"lifecycle":"verified","oversize":"x".repeat(40000)}))
                },
            )
            .unwrap();
        drop(engine);
        let mut engine = Engine::open(&journal).unwrap();
        assert!(engine.unresolved().unwrap());
        let rows = engine.check_startup(&root, &retained).unwrap();
        assert!(
            rows.iter()
                .all(|r| r["lifecycle"] == "startup_checked_original")
        );
        assert!(!engine.unresolved().unwrap());
        assert!(original.exists());
        assert!(!saved.exists());
        // Settled records may be cleaned, including old disable proofs after restoration.
        assert_eq!(engine.journal.prune(true).unwrap(), 2);
    }
    #[cfg(windows)]
    #[test]
    fn startup_crash_process_helper() {
        let Some(directory) = std::env::var_os("DAOSHELL_STARTUP_CRASH_FIXTURE") else {
            return;
        };
        let d = std::path::PathBuf::from(directory);
        let proof: Value =
            serde_json::from_slice(&std::fs::read(d.join("proof.json")).unwrap()).unwrap();
        let p = super::super::startup::PinnedShortcut::inspect(std::path::Path::new(
            proof["original"].as_str().unwrap(),
        ))
        .unwrap();
        let mut engine = Engine::open(&d.join("journal")).unwrap();
        engine.local_transition(json!({"operation_id":"disable","operation":"startup_disable","startup_proof":proof}), &Cancellation::default(), &mut |_| Ok(true), &mut || {
            p.relocate(std::path::Path::new(proof["retained"].as_str().unwrap()))?;
            std::process::exit(91); // Abrupt process exit: no final receipt and no Rust cleanup.
        }).unwrap();
        panic!("fixture did not exit after move");
    }
    #[cfg(windows)]
    #[test]
    fn startup_crash_after_move_preserves_proof_and_software_unknown_gate() {
        let (d, root, retained, _p, proof) = startup_fixture();
        let journal = d.path().join("journal");
        let saved = std::path::PathBuf::from(proof["retained"].as_str().unwrap());
        std::fs::write(
            d.path().join("proof.json"),
            serde_json::to_vec(&proof).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "software::engine::tests::startup_crash_process_helper",
                "--nocapture",
            ])
            .env("DAOSHELL_STARTUP_CRASH_FIXTURE", d.path())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(91));
        let mut engine = Engine::open(&journal).unwrap();
        engine.journal.save(&json!({"operation_id":"software","operation":"install","lifecycle":"unknown","created_at":crate::core::now()})).unwrap();
        engine.check_startup(&root, &retained).unwrap();
        assert!(engine.unresolved().unwrap());
        assert_eq!(
            engine
                .history()
                .unwrap()
                .iter()
                .find(|r| r["operation_id"] == "software")
                .unwrap()["lifecycle"],
            "unknown"
        );
        assert_eq!(engine.journal.prune(true).unwrap(), 0);
        assert!(saved.exists());
        let mut moved = false;
        assert!(engine.local_transition(json!({"operation_id":"blocked","operation":"startup_restore","startup_proof":proof}), &Cancellation::default(), &mut |_| Ok(true), &mut || { moved=true; Ok(json!({"lifecycle":"verified"})) }).is_err());
        assert!(!moved);
    }
    #[cfg(windows)]
    #[test]
    fn startup_checks_unmoved_conflict_replacement_and_content_drift() {
        for scenario in [
            "unmoved",
            "conflict",
            "replacement",
            "drift",
            "bad_root",
            "missing_both",
        ] {
            let (d, root, retained, p, mut proof) = startup_fixture();
            let saved = std::path::PathBuf::from(proof["retained"].as_str().unwrap());
            match scenario {
                "unmoved" => {}
                "bad_root" => {
                    proof["original"] = json!(d.path().join("foreign.lnk"));
                }
                "missing_both" => {
                    std::fs::remove_file(&p.path).unwrap();
                }
                other => {
                    p.relocate(&saved).unwrap();
                    let bytes = std::fs::read(&saved).unwrap();
                    match other {
                        "conflict" => std::fs::write(&p.path, bytes).unwrap(),
                        "replacement" => {
                            std::fs::rename(&saved, retained.join("parked.lnk")).unwrap();
                            std::fs::write(&saved, bytes).unwrap();
                        }
                        "drift" => {
                            let mut changed = bytes;
                            changed[40] = 1;
                            std::fs::write(&saved, changed).unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            let mut engine = Engine::open(&d.path().join("journal")).unwrap();
            engine.journal.save(&json!({"operation_id":"one","created_at":crate::core::now(),"operation":"startup_disable","lifecycle":"unknown","startup_proof":proof})).unwrap();
            let rows = engine.check_startup(&root, &retained).unwrap();
            if scenario == "unmoved" {
                assert_eq!(rows[0]["lifecycle"], "startup_checked_original");
                assert!(!engine.unresolved().unwrap());
            } else {
                assert_eq!(rows[0]["lifecycle"], "unknown", "{scenario}");
                assert!(engine.unresolved().unwrap(), "{scenario}");
            }
        }
    }
    #[cfg(windows)]
    #[test]
    fn startup_initial_persistence_failure_never_moves() {
        let (d, _root, _retained, p, proof) = startup_fixture();
        let mut engine = Engine::open(&d.path().join("journal")).unwrap();
        let saved = std::path::PathBuf::from(proof["retained"].as_str().unwrap());
        let mut called = false;
        assert!(engine.local_transition(json!({"operation_id":"oversize","operation":"startup_disable","startup_proof":proof,"oversize":"x".repeat(40000)}), &Cancellation::default(), &mut |_| Ok(true), &mut || { called=true; p.relocate(&saved)?; Ok(json!({"lifecycle":"verified"})) }).is_err());
        assert!(!called);
        assert!(p.path.exists());
        assert!(!saved.exists());
    }
    #[test]
    fn local_refusal_and_cancel_never_dispatch_and_failed_final_save_recovers_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let mut engine = Engine::open(directory.path()).unwrap();
        let cancel = Cancellation::default();
        let mut calls = 0;
        engine
            .local_transition(
                json!({"operation_id":"cancelled","operation":"fixture"}),
                &cancel,
                &mut |_| {
                    cancel.cancel();
                    Ok(true)
                },
                &mut || {
                    calls += 1;
                    Ok(json!({"lifecycle":"verified"}))
                },
            )
            .unwrap();
        assert_eq!(calls, 0);
        assert!(engine.used());
        assert!(engine.history().unwrap().is_empty());
        cancel.reset();
        engine.begin_turn();
        let receipt = engine
            .local_transition(
                json!({"operation_id":"oversize","operation":"fixture"}),
                &cancel,
                &mut |_| Ok(true),
                &mut || {
                    calls += 1;
                    Ok(json!({"lifecycle":"verified","large":"x".repeat(40000)}))
                },
            )
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(receipt["lifecycle"], "unknown");
        assert_eq!(receipt["persistence_warning"], true);
        drop(engine);
        let engine = Engine::open(directory.path()).unwrap();
        assert!(engine.unresolved().unwrap());
        assert_eq!(engine.history().unwrap()[0]["lifecycle"], "unknown");
    }
    struct Fake {
        package: Package,
        calls: usize,
        fail: bool,
        checks: usize,
        cancel_after_check: Option<Cancellation>,
    }
    impl Provider for Fake {
        fn search(&mut self, _: &str) -> Result<Vec<Package>> {
            Ok(vec![self.package.clone()])
        }
        fn inspect(&mut self, _: &Identity) -> Result<Package> {
            self.checks += 1;
            if self.checks == 2
                && let Some(cancel) = &self.cancel_after_check
            {
                cancel.cancel();
            }
            Ok(self.package.clone())
        }
        fn compare_versions(&self, _: &Identity, _: &str) -> Result<Ordering> {
            Ok(Ordering::Equal)
        }
        fn dispatch(
            &mut self,
            p: &Plan,
            _: &Cancellation,
            _: &mut dyn FnMut(&str),
        ) -> Result<DispatchResult> {
            self.calls += 1;
            if self.fail {
                anyhow::bail!("native detail must not escape")
            };
            self.package.installation = Installation::Present {
                identity: p.package.identity.clone(),
                instance: "fixture-instance".into(),
            };
            Ok(DispatchResult {
                outcome: Outcome::Verified,
                provider_code: "ok".into(),
                reboot_required: Some(false),
            })
        }
    }
    fn setup() -> (tempfile::TempDir, Engine, Fake, Plan) {
        let d = tempfile::tempdir().unwrap();
        let e = Engine::open(d.path()).unwrap();
        let p = Package {
            identity: Identity {
                source_id: "Microsoft.Winget.Source_8wekyb3d8bbwe".into(),
                package_id: "Fixture.Safe".into(),
                version: "1".into(),
                scope: Scope::User,
                architecture: "x64".into(),
            },
            name: "Fixture".into(),
            publisher: "Fixture".into(),
            installation: Installation::AbsentVerified { scope: Scope::User },
            agreement_digest: None,
            agreements: vec![],
        };
        let plan = Plan::prepare(Operation::Install, p.clone()).unwrap();
        (
            d,
            e,
            Fake {
                package: p,
                calls: 0,
                fail: false,
                checks: 0,
                cancel_after_check: None,
            },
            plan,
        )
    }
    fn approve(_: &Plan) -> Result<Approval> {
        Ok(Approval {
            approved: true,
            agreement_digest: None,
        })
    }
    #[test]
    fn cancellation_during_final_identity_check_still_prevents_dispatch() {
        let (_directory, mut engine, mut provider, plan) = setup();
        let cancel = Cancellation::default();
        provider.cancel_after_check = Some(cancel.clone());
        let result = engine
            .execute(&plan, &mut provider, &cancel, &mut approve, &mut |_| {})
            .unwrap();
        assert_eq!(provider.calls, 0);
        assert_eq!(result["lifecycle"], "cancelled_before_dispatch");
        assert!(!engine.unresolved().unwrap());
    }
    #[test]
    fn changed_call_id_cannot_dispatch_twice() {
        let (_d, mut e, mut p, plan) = setup();
        let c = Cancellation::default();
        assert_eq!(
            e.execute(&plan, &mut p, &c, &mut approve, &mut |_| {})
                .unwrap()["lifecycle"],
            "verified"
        );
        assert!(
            e.execute(&plan, &mut p, &c, &mut approve, &mut |_| {})
                .is_err()
        );
        assert_eq!(p.calls, 1);
    }
    #[test]
    fn refusal_consumes_round_and_dispatches_nothing() {
        let (_d, mut e, mut p, plan) = setup();
        let c = Cancellation::default();
        e.execute(
            &plan,
            &mut p,
            &c,
            &mut |_| {
                Ok(Approval {
                    approved: false,
                    agreement_digest: None,
                })
            },
            &mut |_| {},
        )
        .unwrap();
        assert!(e.used());
        assert_eq!(p.calls, 0);
    }
    #[test]
    fn unknown_is_persistent_and_state_alone_does_not_unblock() {
        let (d, mut e, mut p, plan) = setup();
        p.fail = true;
        e.execute(
            &plan,
            &mut p,
            &Cancellation::default(),
            &mut approve,
            &mut |_| {},
        )
        .unwrap();
        assert!(e.unresolved().unwrap());
        drop(e);
        let mut e = Engine::open(d.path()).unwrap();
        assert!(e.unresolved().unwrap());
        e.reconcile(&plan.operation_id, false, &p.package.installation)
            .unwrap();
        assert!(e.unresolved().unwrap());
        e.reconcile(&plan.operation_id, true, &p.package.installation)
            .unwrap();
        assert!(!e.unresolved().unwrap());
    }
    #[test]
    fn missing_agreement_digest_never_dispatches() {
        let (_d, mut e, mut p, _) = setup();
        p.package.agreement_digest = Some("license-hash".into());
        p.package.agreements = vec!["fixture terms".into()];
        let plan = Plan::prepare(Operation::Install, p.package.clone()).unwrap();
        assert!(
            e.execute(
                &plan,
                &mut p,
                &Cancellation::default(),
                &mut approve,
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(p.calls, 0);
        e.begin_turn();
        let mut plan = plan;
        plan.context =
            json!({"execution_id":"fixture-execution","configuration_revision":"fixture-revision"});
        let result = e
            .execute(
                &plan,
                &mut p,
                &Cancellation::default(),
                &mut |_| {
                    Ok(Approval {
                        approved: true,
                        agreement_digest: Some("license-hash".into()),
                    })
                },
                &mut |_| {},
            )
            .unwrap();
        assert_eq!(result["agreement_digest"], "license-hash");
        assert_eq!(result["context"], plan.context);
        assert_eq!(result["approval_origin"], "local_confirmation");
    }
}
