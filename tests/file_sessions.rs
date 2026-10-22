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

#[tokio::test(flavor = "multi_thread")]
async fn local_computer_queries_need_no_sharing_and_preserve_files_and_sample_references() {
    use serde_json::json;
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("synthetic.txt"), "fixture").unwrap();
    let config = Config {
        read_roots: vec![temp.path().into()],
        computer_query_sharing: dao_shell::config::ComputerQuerySharing::Disabled,
        ..Default::default()
    };
    let mut s = FileSession::new(&config, Cancellation::default()).unwrap();
    let mut ui = Ui::default();
    let first = s.execute(Action::Search("synthetic".into()), &mut ui).await;
    let file_id = first.items[0].id.clone();
    let query = |tool: &str, arguments| Action::QueryComputer {
        tool: tool.into(),
        arguments,
    };
    let resources = s
        .execute(
            query(
                "resource_snapshot",
                json!({"sections":["cpu","memory"],"sample_ms":500,"limit":2}),
            ),
            &mut ui,
        )
        .await;
    assert!(!resources.error, "{}", resources.message);
    assert_eq!(resources.items[0].id, file_id);
    let obs = &resources.observations[0];
    let id = obs["observation_id"].clone();
    let at = obs["observed_at"].clone();
    assert_eq!(
        obs["data"]["process_rankings"]["cpu"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(obs["data"]["system"].get("disks").is_none());
    s.execute(Action::Search("synthetic".into()), &mut ui).await;
    let reused = s
        .execute(
            query(
                "resource_snapshot",
                json!({"observation_id":id,"sections":["cpu"],"limit":1}),
            ),
            &mut ui,
        )
        .await;
    assert!(!reused.error);
    assert_eq!(reused.observations[0]["observed_at"], at);
    assert!(
        reused.observations[0]["data"]["system"]
            .get("memory_used_bytes")
            .is_none()
    );
    assert!(
        reused.observations[0]["data"]["facts_for_explanation"]["system"]
            .get("memory_used")
            .is_none()
    );
    let mut other = FileSession::new(&config, Cancellation::default()).unwrap();
    assert!(
        other
            .execute(
                query("resource_snapshot", json!({"observation_id":id})),
                &mut ui
            )
            .await
            .error
    );
    let explain = s
        .execute(
            Action::ExplainComputer(json!({"host":"private","system":{"cpu_percent":4}})),
            &mut ui,
        )
        .await;
    assert!(explain.error);
    assert!(explain.message.contains("permission_denied"));
    s.execute(Action::Reset, &mut ui).await;
    assert!(
        s.execute(
            query("resource_snapshot", json!({"observation_id":id})),
            &mut ui
        )
        .await
        .error
    );
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn five_local_read_only_tools_have_timestamped_contracts_and_no_action_references() {
    use serde_json::json;
    let mut s = FileSession::new(&Config::default(), Cancellation::default()).unwrap();
    let mut ui = Ui::default();
    for (tool, args) in [
        ("resource_snapshot", json!({"sections":["disks"]})),
        ("process_list", json!({"sample_ms":500,"limit":2})),
        ("network_snapshot", json!({"sample_ms":500})),
        ("app_list", json!({"limit":2})),
        ("computer_profile", json!({})),
    ] {
        let reply = s
            .execute(
                Action::QueryComputer {
                    tool: tool.into(),
                    arguments: args,
                },
                &mut ui,
            )
            .await;
        assert!(!reply.error, "{tool}: {}", reply.message);
        assert_eq!(reply.observations.len(), 1);
        let v = &reply.observations[0];
        assert!(v["observed_at"].is_string());
        assert!(v["observation_id"].is_string());
        let text = v.to_string();
        assert!(!text.contains("termination_ref"));
        assert!(!text.contains("action_id"));
        assert!(!text.contains("created_filetime"));
    }
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
    let receipt = found.search_receipt.as_ref().unwrap();
    assert_eq!(receipt["source"], "scan");
    assert_eq!(receipt["items"][0]["id"], found.items[0].id);
    let elapsed = receipt["elapsed_ms"].as_u64().unwrap();
    assert!(receipt["discovery_ms"].as_u64().unwrap() <= elapsed);
    assert!(receipt["validation_ms"].as_u64().unwrap() <= elapsed);
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
    assert!(declined.search_receipt.is_none());
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

#[tokio::test(flavor = "multi_thread")]
async fn english_match_counts_are_clear_for_zero_one_and_multiple_without_changing_file_names() {
    use dao_shell::localization::Language;
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("one-中文.txt"), "fixture").unwrap();
    fs::write(temp.path().join("two-中文.txt"), "fixture").unwrap();
    let config = Config {
        read_roots: vec![temp.path().into()],
        ..Default::default()
    };
    let mut session =
        FileSession::new_with_language(&config, Cancellation::default(), Language::English)
            .unwrap();
    let mut ui = Ui::default();
    for (query, count) in [("absent", 0), ("one-", 1), ("中文", 2)] {
        let reply = session.execute(Action::Search(query.into()), &mut ui).await;
        assert!(!reply.error);
        assert_eq!(reply.message, format!("Matches: {count}."));
        assert_eq!(reply.items.len(), count);
        assert!(
            reply
                .items
                .iter()
                .all(|item| item.path.to_string_lossy().contains("中文"))
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn portable_registration_is_local_discovery_only_and_cannot_expand_scope() {
    use serde_json::json;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(root.path().join("Owned.exe"), b"synthetic inert fixture").unwrap();
    fs::write(outside.path().join("Outside.exe"), b"inert").unwrap();
    let mut session = FileSession::new(
        &Config {
            read_roots: vec![root.path().into()],
            ..Default::default()
        },
        Cancellation::default(),
    )
    .unwrap();
    let mut ui = Ui::default();
    let denied = session
        .execute(
            Action::DiagnosticLocal(
                json!({"kind":"portable_register","path":outside.path().join("Outside.exe")}),
            ),
            &mut ui,
        )
        .await;
    assert!(denied.error);
    let accepted = session
        .execute(
            Action::DiagnosticLocal(
                json!({"kind":"portable_register","path":root.path().join("Owned.exe")}),
            ),
            &mut ui,
        )
        .await;
    assert!(!accepted.error);
    assert_eq!(ui.confirmations, 0);
    assert!(accepted.action_targets.is_empty());
    assert!(accepted.action_receipts.is_empty());
}
