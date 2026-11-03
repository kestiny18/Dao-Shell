use dao_shell::{
    config::{ComputerQuerySharing, Config, ModelConfig},
    core::Cancellation,
    runtime::Interaction,
    session::{Action, FileSession},
};
use std::{
    io::{Read, Write},
    net::TcpListener,
};
#[derive(Default)]
struct Ui;
impl Interaction for Ui {
    fn confirm_move(&mut self, _: &dao_shell::core::Operation) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn progress(&mut self, _: &str) {}
    fn result(&mut self, _: &str, _: &serde_json::Value) {}
    fn confirm_open(&mut self, _: &dao_shell::core::FileObject) -> anyhow::Result<bool> {
        Ok(false)
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn budgets_cap_requests_keep_unknown_usage_and_survive_context_recreation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = Cancellation::default();
    let server_cancel = cancellation.clone();
    let worker = std::thread::spawn(move || {
        for round in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut part = [0; 4096];
            loop {
                let n = stream.read(&mut part).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&part[..n]);
                if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&bytes[..end]);
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let end = bytes.windows(4).position(|s| s == b"\r\n\r\n").unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes[end + 4..]).unwrap();
            let parameter = if round < 2 {
                "max_tokens"
            } else {
                "max_completion_tokens"
            };
            assert_eq!(body[parameter], 32);
            assert!(
                body.get(if round < 2 {
                    "max_completion_tokens"
                } else {
                    "max_tokens"
                })
                .is_none()
            );
            if round == 3 {
                server_cancel.cancel();
                std::thread::sleep(std::time::Duration::from_millis(150));
                return;
            }
            let body = if round == 0 {
                r#"{"choices":[{"message":{"role":"assistant","content":"fixture"}}],"usage":{"total_tokens":100}}"#
            } else {
                r#"{"choices":[{"message":{"role":"assistant","content":"fixture"}}]}"#
            };
            let status = if round == 2 {
                "500 Internal Server Error"
            } else {
                "200 OK"
            };
            write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let mut config = Config {
        computer_query_sharing: ComputerQuerySharing::Enabled,
        model: Some(ModelConfig {
            endpoint: format!("http://{address}/v1/chat/completions"),
            model: "fixture".into(),
            api_key_env: "UNUSED_BUDGET_KEY".into(),
        }),
        ..Default::default()
    };
    config.model_budget.output_tokens = 32;
    let mut ui = Ui;
    let mut session = FileSession::new(&config, Cancellation::default()).unwrap();
    let first = session
        .execute(Action::ExplainComputer(serde_json::json!({})), &mut ui)
        .await;
    assert!(!first.error, "{}", first.message);
    assert_eq!(first.model_usage.actual_tokens, 100);
    assert_eq!(first.model_usage.uncertain_reserved_tokens, 0);
    let second = session
        .execute(Action::ExplainComputer(serde_json::json!({})), &mut ui)
        .await;
    assert!(!second.error);
    assert!(second.model_usage.uncertain_reserved_tokens > 32);
    config.model_budget.output_parameter =
        dao_shell::model::budget::OutputParameter::MaxCompletionTokens;
    let mut session = FileSession::new(&config, cancellation).unwrap();
    let third = session
        .execute(Action::ExplainComputer(serde_json::json!({})), &mut ui)
        .await;
    assert!(third.error);
    assert!(
        third.model_usage.uncertain_reserved_tokens > second.model_usage.uncertain_reserved_tokens
    );
    let cancelled = session
        .execute(Action::ExplainComputer(serde_json::json!({})), &mut ui)
        .await;
    assert!(cancelled.error);
    assert!(
        cancelled.model_usage.uncertain_reserved_tokens
            > third.model_usage.uncertain_reserved_tokens
    );
    worker.join().unwrap();
    config.model_budget.output_tokens = 1;
    config.model_budget.run_tokens = 1;
    let mut recreated = FileSession::new(&config, Cancellation::default()).unwrap();
    let blocked = recreated
        .execute(Action::ExplainComputer(serde_json::json!({})), &mut ui)
        .await;
    assert!(blocked.error);
    assert!(blocked.message.contains("model_budget_exhausted"));
    assert_eq!(blocked.model_usage.actual_tokens, 100);
    let dir = tempfile::tempdir().unwrap();
    config.model_budget.output_tokens = 0;
    assert!(config.save(&dir.path().join("config.json")).is_err());
}
