use dao_shell::{
    capabilities::Interaction,
    config::Config,
    core::{Cancellation, FileObject, Operation},
    session::{Action, FileSession},
};
use serde_json::Value;
use std::fs;

#[derive(Default)]
struct Ui {
    confirmations: usize,
}
impl Interaction for Ui {
    fn confirm_move(&mut self, _: &Operation) -> anyhow::Result<bool> {
        panic!("file entry must never prepare a move")
    }
    fn confirm_open(&mut self, _: &FileObject) -> anyhow::Result<bool> {
        self.confirmations += 1;
        Ok(false)
    }
    fn progress(&mut self, _: &str) {}
    fn result(&mut self, _: &str, _: &Value) {}
}

#[tokio::test(flavor = "multi_thread")]
async fn sessions_do_not_share_candidates_cancellation_or_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("合同.txt"), "fixture").unwrap();
    let config = Config {
        read_roots: vec![temp.path().into()],
        ..Default::default()
    };
    let cancel = Cancellation::default();
    let mut first = FileSession::new(&config, cancel.clone()).unwrap();
    let mut second = FileSession::new(&config, Cancellation::default()).unwrap();
    let mut ui = Ui::default();
    let found = first.execute(Action::Search("合同".into()), &mut ui).await;
    assert!(!found.error);
    assert_eq!(found.items.len(), 1);
    let id = found.items[0].id.clone();
    assert!(
        second
            .execute(Action::Open(id.clone()), &mut ui)
            .await
            .error
    );
    assert_eq!(ui.confirmations, 0);
    let declined = first.execute(Action::Open(id.clone()), &mut ui).await;
    assert!(!declined.error);
    assert!(declined.message.contains("未选择打开"));
    assert_eq!(ui.confirmations, 1);
    let empty = first
        .execute(Action::Search("absent".into()), &mut ui)
        .await;
    assert!(empty.items.is_empty());
    assert!(first.execute(Action::Open(id), &mut ui).await.error);
    cancel.cancel();
    assert!(
        first
            .execute(Action::Search("合同".into()), &mut ui)
            .await
            .error
    );
    assert!(
        !second
            .execute(Action::Search("合同".into()), &mut ui)
            .await
            .error
    );
    let old = second
        .execute(Action::Search("合同".into()), &mut ui)
        .await
        .items[0]
        .id
        .clone();
    second.execute(Action::Reset, &mut ui).await;
    assert!(second.execute(Action::Open(old), &mut ui).await.error);
}

#[tokio::test(flavor = "multi_thread")]
async fn changed_candidate_is_rejected_before_desktop_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("合同.txt");
    fs::write(&file, "before").unwrap();
    let mut session = FileSession::new(
        &Config {
            write_roots: vec![temp.path().into()],
            ..Default::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    let mut ui = Ui::default();
    let id = session
        .execute(Action::Search("合同".into()), &mut ui)
        .await
        .items[0]
        .id
        .clone();
    fs::write(file, "changed contents").unwrap();
    assert!(session.execute(Action::Open(id), &mut ui).await.error);
    assert_eq!(ui.confirmations, 0);
}
