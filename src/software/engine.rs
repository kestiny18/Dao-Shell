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
        let approval = confirm(plan)?;
        if !approval.approved || cancel.is_cancelled() {
            return Ok(json!({"status":"cancelled_before_dispatch"}));
        }
        let current = provider.inspect(&plan.package.identity)?;
        plan.revalidate(&current, approval.agreement_digest.as_deref())?;
        let mut receipt = json!({"maintenance_version":1,"operation_id":plan.operation_id,"created_at":crate::core::now(),"operation":plan.operation,"package":plan.package.identity,"prestate":plan.package.installation,"lifecycle":"dispatching","evidence":{"code":"outcome_not_observed"}});
        self.journal.save(&receipt)?; // Must succeed before any native dispatch.
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
        self.journal.save(&receipt)?;
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
    struct Fake {
        package: Package,
        calls: usize,
        fail: bool,
    }
    impl Provider for Fake {
        fn search(&mut self, _: &str) -> Result<Vec<Package>> {
            Ok(vec![self.package.clone()])
        }
        fn inspect(&mut self, _: &Identity) -> Result<Package> {
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
    }
}
