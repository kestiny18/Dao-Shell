use dao_shell::{
    capabilities::{Interaction, Runtime},
    config::ModelConfig,
    core::{Cancellation, FileObject, Operation},
    dialogue::Dialogue,
    files::{Objects, Scope},
    storage::Journal,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

#[derive(Default)]
struct Ui {
    approve: bool,
    confirmations: usize,
    results: Vec<(String, Value)>,
}
impl Interaction for Ui {
    fn confirm_move(&mut self, op: &Operation) -> anyhow::Result<bool> {
        assert!(!op.items.is_empty());
        self.confirmations += 1;
        Ok(self.approve)
    }
    fn confirm_open(&mut self, _: &FileObject) -> anyhow::Result<bool> {
        self.confirmations += 1;
        Ok(false)
    }
    fn progress(&mut self, _: &str) {}
    fn result(&mut self, name: &str, value: &Value) {
        self.results.push((name.into(), value.clone()));
    }
}
fn request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 65536);
    }
    let headers = String::from_utf8(bytes).unwrap();
    assert!(headers.starts_with("POST /v1/chat/completions "));
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|n| n.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}
fn respond(stream: &mut TcpStream, message: Value) {
    let body = json!({"choices":[{"message":message}]}).to_string();
    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
}
fn call(id: &str, name: &str, args: Value) -> Value {
    json!({"role":"assistant","content":null,"tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]})
}
fn config(address: std::net::SocketAddr) -> ModelConfig {
    ModelConfig {
        endpoint: format!("http://{address}/v1/chat/completions"),
        model: "protocol-fixture".into(),
        api_key_env: String::new(),
    }
}
fn setup() -> (tempfile::TempDir, Runtime) {
    let temp = tempfile::tempdir().unwrap();
    let files = temp.path().join("files");
    fs::create_dir(&files).unwrap();
    fs::write(
        files.join("合同.txt"),
        "Never sent to the model: PRIVATE BODY",
    )
    .unwrap();
    let runtime = Runtime {
        scope: Scope::new(std::slice::from_ref(&files), std::slice::from_ref(&files)).unwrap(),
        objects: Objects::default(),
        journal: Journal::open(&temp.path().join("state")).unwrap(),
        cancel: Cancellation::default(),
        last_results: Vec::new(),
        side_effects_blocked: false,
    };
    (temp, runtime)
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread")]
async fn two_turn_search_then_move_preserves_real_reference_and_tool_protocol() {
    let (_temp, mut runtime) = setup();
    let dest = runtime.scope.write_roots()[0].join("归档");
    let dest_for_server = dest.clone();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = config(listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        assert_eq!(body["tools"].as_array().unwrap().len(), 6);
        assert_eq!(body["messages"][1]["content"], "找合同文件");
        respond(
            &mut stream,
            call("search-1", "file_search", json!({"query":"合同"})),
        );
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        assert!(!body.to_string().contains("PRIVATE BODY"));
        let result = body["messages"].as_array().unwrap().last().unwrap();
        assert_eq!(result["tool_call_id"], "search-1");
        let result: Value = serde_json::from_str(result["content"].as_str().unwrap()).unwrap();
        let id = result["items"][0]["id"].as_str().unwrap().to_owned();
        respond(
            &mut stream,
            json!({"content":"找到 1 个文件。","tool_calls":null}),
        );
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        assert_eq!(
            body["messages"].as_array().unwrap().last().unwrap()["content"],
            "把第一个移到归档目录"
        );
        respond(
            &mut stream,
            call(
                "move-1",
                "file_move_batch",
                json!({"object_ids":[id],"destination":dest_for_server}),
            ),
        );
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        let tool = body["messages"].as_array().unwrap().last().unwrap();
        assert_eq!(tool["tool_call_id"], "move-1");
        let receipt: Value = serde_json::from_str(tool["content"].as_str().unwrap()).unwrap();
        assert_eq!(receipt["status"], "Succeeded", "{receipt}");
        respond(&mut stream, json!({"content":"已移动 1 项并核验。"}));
    });
    let mut dialogue = Dialogue::new(&endpoint, &runtime).unwrap();
    let mut ui = Ui {
        approve: true,
        ..Default::default()
    };
    assert!(
        dialogue
            .turn("找合同文件", &mut runtime, &mut ui)
            .await
            .unwrap()
            .contains("1")
    );
    assert!(
        dialogue
            .turn("把第一个移到归档目录", &mut runtime, &mut ui)
            .await
            .unwrap()
            .contains("核验")
    );
    server.join().unwrap();
    assert_eq!(ui.confirmations, 1);
    assert!(dest.join("合同.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn model_cannot_inject_approval_or_execute_unregistered_commands() {
    let (_temp, mut runtime) = setup();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = config(listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request(&mut stream);
        respond(
            &mut stream,
            call(
                "bad-1",
                "file_move_batch",
                json!({"object_ids":[],"destination":"C:\\","approved":true}),
            ),
        );
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        assert!(
            body["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("unknown field")
        );
        respond(
            &mut stream,
            call(
                "bad-2",
                "run_powershell",
                json!({"command":"should never execute"}),
            ),
        );
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        assert!(
            body["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("不支持的能力")
        );
        respond(&mut stream, json!({"content":"无法执行。"}));
    });
    let mut dialogue = Dialogue::new(&endpoint, &runtime).unwrap();
    let mut ui = Ui::default();
    dialogue.turn("test", &mut runtime, &mut ui).await.unwrap();
    server.join().unwrap();
    assert_eq!(ui.confirmations, 0);
    assert!(runtime.journal.list().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn bounded_loop_stops_after_twelve_calls() {
    let (_temp, mut runtime) = setup();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = config(listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for number in 0..12 {
            let (mut stream, _) = listener.accept().unwrap();
            request(&mut stream);
            respond(
                &mut stream,
                call(
                    &format!("call-{number}"),
                    "file_search",
                    json!({"query":"合同"}),
                ),
            );
        }
    });
    let mut dialogue = Dialogue::new(&endpoint, &runtime).unwrap();
    let mut ui = Ui::default();
    let answer = dialogue.turn("test", &mut runtime, &mut ui).await.unwrap();
    server.join().unwrap();
    assert!(answer.contains("12"));
    assert_eq!(ui.results.len(), 12);
}

#[tokio::test(flavor = "multi_thread")]
async fn denied_open_is_not_reconfirmed_within_the_same_turn() {
    let (_temp, mut runtime) = setup();
    let object = runtime
        .objects
        .insert(&runtime.scope.read_roots()[0].join("合同.txt"))
        .unwrap();
    let args = json!({"object_id":object.id});
    let mut ui = Ui::default();
    let first = runtime.call("file_open", args.clone(), &mut ui).unwrap();
    assert_eq!(first["status"], "Cancelled");
    assert!(runtime.call("file_open", args, &mut ui).is_err());
    assert_eq!(ui.confirmations, 1);
}

#[test]
fn remote_plaintext_and_url_credentials_are_rejected_before_network() {
    let (_temp, runtime) = setup();
    for endpoint in [
        "http://example.com/v1/chat/completions",
        "https://secret@example.com/api",
        "https://example.com/api?key=secret",
    ] {
        assert!(
            Dialogue::new(
                &ModelConfig {
                    endpoint: endpoint.into(),
                    model: "test".into(),
                    api_key_env: String::new()
                },
                &runtime
            )
            .is_err()
        );
    }
}

#[test]
fn empty_or_failed_search_clears_numbers_but_preserves_explicit_references() {
    let (_temp, mut runtime) = setup();
    let mut ui = Ui::default();
    let first = runtime
        .call("file_search", json!({"query":"合同"}), &mut ui)
        .unwrap();
    let selection = runtime.last_results.clone();
    assert_eq!(selection.len(), 1);
    let second = runtime
        .call("file_search", json!({"query":"SLA-no-result"}), &mut ui)
        .unwrap();
    assert!(second["items"].as_array().unwrap().is_empty());
    assert!(second["active_selection"].as_array().unwrap().is_empty());
    assert!(runtime.last_results.is_empty());
    assert!(
        runtime.current_selection().unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        runtime
            .objects
            .checked(&selection[0], &runtime.scope, false)
            .is_ok()
    );
    let third = runtime
        .call("file_search", json!({"query":"合同"}), &mut ui)
        .unwrap();
    assert_eq!(third["items"][0]["id"], first["items"][0]["id"]);
    assert_eq!(
        runtime.last_results[0],
        third["items"][0]["id"].as_str().unwrap()
    );
    assert!(
        runtime
            .call("file_search", json!({"terms":[""]}), &mut ui)
            .is_err()
    );
    assert!(runtime.last_results.is_empty());
}

#[test]
fn tool_contract_accepts_path_terms_and_returns_current_numbered_candidates() {
    let (_temp, mut runtime) = setup();
    let folder = runtime.scope.read_roots()[0].join("示例项目/客户端初始化");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("dump-client.sql"), "PRIVATE SQL BODY").unwrap();
    let result = runtime.call("file_search", json!({"terms":["客户端", "初始化"], "extension":"sql", "kind":"file", "match_mode":"all"}), &mut Ui::default()).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["active_selection"][0]["object_id"],
        result["items"][0]["id"]
    );
    assert!(!result.to_string().contains("PRIVATE SQL BODY"));
    runtime.cancel.cancel();
    assert!(
        runtime
            .call("file_search", json!({"query":"合同"}), &mut Ui::default())
            .is_err()
    );
    assert!(runtime.last_results.is_empty());
}
