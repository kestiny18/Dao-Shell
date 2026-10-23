//! Independent deterministic checks; no native provider or real user files.
use dao_shell::{
    core::Cancellation,
    software::{
        contract::*,
        engine::{Approval, Engine},
    },
};
use serde_json::json;
fn package() -> Package {
    Package {
        identity: Identity {
            source_id: "Microsoft.Winget.Source_8wekyb3d8bbwe".into(),
            package_id: "Independent.Fixture".into(),
            version: "1".into(),
            scope: Scope::User,
            architecture: "x64".into(),
        },
        name: "Fixture".into(),
        publisher: "Fixture".into(),
        installation: Installation::AbsentVerified { scope: Scope::User },
        agreement_digest: None,
        agreements: vec![],
    }
}
struct Fake {
    p: Package,
    dispatches: usize,
}
impl Provider for Fake {
    fn search(&mut self, _: &str) -> anyhow::Result<Vec<Package>> {
        Ok(vec![self.p.clone()])
    }
    fn inspect(&mut self, _: &Identity) -> anyhow::Result<Package> {
        Ok(self.p.clone())
    }
    fn compare_versions(&self, _: &Identity, _: &str) -> anyhow::Result<std::cmp::Ordering> {
        Ok(std::cmp::Ordering::Equal)
    }
    fn dispatch(
        &mut self,
        _: &Plan,
        _: &Cancellation,
        _: &mut dyn FnMut(&str),
    ) -> anyhow::Result<DispatchResult> {
        self.dispatches += 1;
        Ok(DispatchResult {
            outcome: Outcome::Verified,
            provider_code: "fixture".into(),
            reboot_required: Some(false),
        })
    }
}
#[test]
fn claimed_success_without_poststate_is_persistent_unknown_and_never_replayed() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::open(dir.path()).unwrap();
    let p = package();
    let plan = Plan::prepare(Operation::Install, p.clone()).unwrap();
    let mut f = Fake { p, dispatches: 0 };
    let r = e
        .execute(
            &plan,
            &mut f,
            &Cancellation::default(),
            &mut |_| {
                Ok(Approval {
                    approved: true,
                    agreement_digest: None,
                })
            },
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(r["lifecycle"], "unknown");
    assert_eq!(f.dispatches, 1);
    drop(e);
    let mut e = Engine::open(dir.path()).unwrap();
    assert!(e.unresolved().unwrap());
    assert!(
        e.execute(
            &plan,
            &mut f,
            &Cancellation::default(),
            &mut |_| panic!("must not reconfirm"),
            &mut |_| {}
        )
        .is_err()
    );
    assert_eq!(f.dispatches, 1);
}
#[test]
fn exact_plan_rejects_every_identity_axis_and_expiration() {
    let p = package();
    let mut plan = Plan::prepare(Operation::Install, p.clone()).unwrap();
    for axis in 0..5 {
        let mut changed = p.clone();
        match axis {
            0 => changed.identity.package_id.push('x'),
            1 => changed.identity.source_id.push('x'),
            2 => changed.identity.version.push('x'),
            3 => changed.identity.scope = Scope::System,
            _ => changed.identity.architecture = "arm64".into(),
        };
        assert!(plan.revalidate(&changed, None).is_err());
    }
    plan.expires_at = 0;
    assert!(plan.revalidate(&p, None).is_err());
}
#[test]
fn reboot_receipt_remains_blocking_even_after_idle_check() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::open(dir.path()).unwrap();
    e.local_transition(
        json!({"operation_id":"reboot","operation":"fixture"}),
        &Cancellation::default(),
        &mut |_| Ok(true),
        &mut || Ok(json!({"lifecycle":"awaiting_reboot"})),
    )
    .unwrap();
    e.reconcile("reboot", true, &package().installation)
        .unwrap();
    assert!(e.unresolved().unwrap());
}
#[test]
fn ninety_second_local_confirmation_expires_without_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::open(dir.path()).unwrap();
    let mut calls = 0;
    let r = e
        .local_transition(
            json!({"operation_id":"expired","operation":"fixture"}),
            &Cancellation::default(),
            &mut |_| {
                std::thread::sleep(std::time::Duration::from_millis(90_100));
                Ok(true)
            },
            &mut || {
                calls += 1;
                Ok(json!({"lifecycle":"verified"}))
            },
        )
        .unwrap();
    assert_eq!(calls, 0);
    assert_eq!(r["status"], "cancelled_before_dispatch");
    assert!(e.history().unwrap().is_empty());
}
