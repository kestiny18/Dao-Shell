//! Only refusal/invalid-reference routes; never invokes a native maintenance mutation.
use dao_shell::{
    config::Config,
    core::{Cancellation, FileObject, Operation},
    runtime::Interaction,
    session::{Action, FileSession},
};
use serde_json::{Value, json};
#[derive(Default)]
struct Ui {
    confirmations: usize,
}
impl Interaction for Ui {
    fn confirm_maintenance(
        &mut self,
        _: &Value,
    ) -> anyhow::Result<dao_shell::software::engine::Approval> {
        self.confirmations += 1;
        Ok(dao_shell::software::engine::Approval {
            approved: false,
            agreement_digest: None,
        })
    }
    fn confirm_move(&mut self, _: &Operation) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn confirm_open(&mut self, _: &FileObject) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn progress(&mut self, _: &str) {}
    fn result(&mut self, _: &str, _: &Value) {}
}
#[tokio::test]
async fn local_maintenance_works_without_model_but_rejection_and_forged_approval_never_mutate() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        read_roots: vec![directory.path().to_path_buf()],
        ..Default::default()
    };
    let mut session = FileSession::new(&config, Cancellation::default()).unwrap();
    session.set_action_directory(directory.path().join("app-actions"), &config);
    let mut ui = Ui::default();
    let invalid = session
        .execute(
            Action::Maintenance {
                tool: "network_maintenance".into(),
                arguments: json!({"kind":"flush_dns_cache","confirmed":true}),
            },
            &mut ui,
        )
        .await;
    assert!(invalid.error);
    assert_eq!(ui.confirmations, 0);
    let refused = session
        .execute(
            Action::Maintenance {
                tool: "network_maintenance".into(),
                arguments: json!({"kind":"flush_dns_cache"}),
            },
            &mut ui,
        )
        .await;
    assert!(!refused.error);
    assert_eq!(ui.confirmations, 1);
    assert_eq!(
        refused.maintenance_results[0]["status"],
        "cancelled_before_dispatch"
    );
    let history = session
        .execute(Action::MaintenanceLocal(json!({"kind":"history"})), &mut ui)
        .await;
    assert!(!history.error);
    assert_eq!(history.maintenance_results[0]["receipts"], json!([]));
    let forged = session
        .execute(
            Action::Maintenance {
                tool: "software_install".into(),
                arguments: json!({"package_ref":"forged","version_ref":"forged","scope":"user"}),
            },
            &mut ui,
        )
        .await;
    assert!(forged.error);
    assert_eq!(ui.confirmations, 1);
}
