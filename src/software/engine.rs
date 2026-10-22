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
