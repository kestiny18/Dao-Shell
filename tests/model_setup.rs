use dao_shell::{
    config::{Config, ModelConfig},
    core::Cancellation,
    model::check_connection,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

fn listener() -> (TcpListener, ModelConfig) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = ModelConfig {
        endpoint: format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        ),
        model: "fixture".into(),
        api_key_env: String::new(),
    };
    (listener, config)
}

// Server accept is bounded so a failed client cannot hang the test suite.
fn accept(listener: &TcpListener) -> TcpStream {
    let start = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    start.elapsed() < Duration::from_secs(15),
                    "client did not connect"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    }
}

fn read_request(stream: &mut TcpStream) -> (String, Value) {
    let mut headers = Vec::new();
    let mut byte = [0];
    while !headers.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
        assert!(headers.len() < 65536);
    }
    let headers = String::from_utf8(headers).unwrap();
    let len: usize = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|s| s.trim().parse().unwrap())
        })
        .unwrap();
    let mut body = vec![0; len];
    stream.read_exact(&mut body).unwrap();
    (headers, serde_json::from_slice(&body).unwrap())
}

fn response(stream: &mut TcpStream, status: u16, body: &str, extra_headers: &str) {
    write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\n{extra_headers}Connection: close\r\n\r\n{body}", body.len()).unwrap();
}
fn completion(message: Value, usage: Option<u64>) -> String {
    json!({"choices":[{"message":message}],"usage":usage.map(|n| json!({"total_tokens":n}))})
        .to_string()
}
fn probe_call(name: &str) -> Value {
    json!({"content":null,"reasoning_content":"fixture-replay-only","tool_calls":[{
        "id":"probe-1","type":"function","function":{"name":name,"arguments":"{}"}
    }]})
}

fn cli(config: &std::path::Path, data: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_daosh"))
        .arg("--config")
        .arg(config)
        .arg("--data-dir")
        .arg(data)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn doctor_round_trip_is_synthetic_and_does_not_touch_files_or_journal() {
    let temp = tempfile::tempdir().unwrap();
    let private = temp.path().join("PRIVATE_DIRECTORY_MARKER");
    fs::create_dir(&private).unwrap();
    fs::write(private.join("private.txt"), "PRIVATE_CONTENT_MARKER").unwrap();
    let (listener, mut model) = listener();
    model.api_key_env = "DAO_TEST_KEY".into();
    let config_path = temp.path().join("config.json");
    Config {
        read_roots: vec![private.clone()],
        write_roots: vec![private.clone()],
        model: Some(model),
    }
    .save(&config_path)
    .unwrap();
    let original = fs::read(&config_path).unwrap();
    let server = thread::spawn(move || {
        for turn in 0..2 {
            let mut stream = accept(&listener);
            let (headers, body) = read_request(&mut stream);
            assert!(
                headers
                    .to_lowercase()
                    .contains("authorization: bearer test-only-secret")
            );
            let serialized = body.to_string();
            for private in [
                "PRIVATE_DIRECTORY_MARKER",
                "PRIVATE_CONTENT_MARKER",
                "test-only-secret",
                "file_search",
                "file_move_batch",
            ] {
                assert!(
                    !serialized.contains(private),
                    "diagnostic contains private or runtime data"
                );
            }
            assert_eq!(body["tools"].as_array().unwrap().len(), 1);
            assert_eq!(body["tool_choice"], "auto");
            assert_eq!(body["parallel_tool_calls"], false);
            if turn == 0 {
                response(
                    &mut stream,
                    200,
                    &completion(probe_call("dao_shell_connection_check"), Some(12)),
                    "",
                );
            } else {
                assert_eq!(
                    body["messages"][2]["reasoning_content"],
                    "fixture-replay-only"
                );
                assert_eq!(body["messages"][3]["tool_call_id"], "probe-1");
                let returned: Value =
                    serde_json::from_str(body["messages"][3]["content"].as_str().unwrap()).unwrap();
                assert_eq!(returned["reply"], "DAO_SHELL_OK");
                response(
                    &mut stream,
                    200,
                    &completion(json!({"content":"DAO_SHELL_OK","tool_calls":null}), Some(8)),
                    "",
                );
            }
        }
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    });
    let data = temp.path().join("state");
    let result = Command::new(env!("CARGO_BIN_EXE_daosh"))
        .arg("--config")
        .arg(&config_path)
        .arg("--data-dir")
        .arg(&data)
        .args(["doctor", "--check-model"])
        .env("DAO_TEST_KEY", "\r\n test-only-secret \n")
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8_lossy(&result.stdout);
    assert!(output.contains("[通过]") && output.contains("20"));
    assert!(!output.contains("test-only-secret") && !output.contains("fixture-replay-only"));
    assert_eq!(fs::read(config_path).unwrap(), original);
    assert!(!data.exists());
    assert_eq!(
        fs::read_to_string(private.join("private.txt")).unwrap(),
        "PRIVATE_CONTENT_MARKER"
    );
}

#[tokio::test]
async fn diagnostic_reports_http_errors_without_returning_remote_secrets_or_following_redirects() {
    for (status, hint) in [
        (401, "身份验证"),
        (403, "权限"),
        (404, "完整"),
        (429, "额度"),
        (503, "故障"),
        (302, "未跟随"),
    ] {
        let (listener, config) = listener();
        let redirect = format!("Location: {}\r\n", config.endpoint);
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            read_request(&mut stream);
            response(&mut stream, status, "PRIVATE_REMOTE_SECRET", &redirect);
            listener
        });
        let error = check_connection(&config, &Cancellation::default())
            .await
            .unwrap_err()
            .to_string();
        let listener = server.join().unwrap();
        assert!(error.contains(hint), "{error}");
        assert!(!error.contains("PRIVATE_REMOTE_SECRET"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[tokio::test]
async fn diagnostic_rejects_unexpected_tools_and_malformed_responses() {
    for body in [
        completion(probe_call("file_move_batch"), None),
        completion(json!({"content":"connected"}), None),
        "PRIVATE_NOT_JSON".into(),
        json!({"choices":[]}).to_string(),
    ] {
        let (listener, config) = listener();
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            read_request(&mut stream);
            response(&mut stream, 200, &body, "");
            listener
        });
        let error = check_connection(&config, &Cancellation::default())
            .await
            .unwrap_err()
            .to_string();
        let listener = server.join().unwrap();
        assert!(!error.contains("PRIVATE_NOT_JSON"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[tokio::test]
async fn cancellation_stops_a_waiting_model_request() {
    let (listener, config) = listener();
    let cancel = Cancellation::default();
    let server_cancel = cancel.clone();
    let (finished, wait_finished) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_request(&mut stream);
        server_cancel.cancel();
        // Do not let a server-side disconnect finish the request instead of cancellation.
        wait_finished.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let error = tokio::time::timeout(Duration::from_secs(5), check_connection(&config, &cancel))
        .await
        .unwrap()
        .unwrap_err()
        .to_string();
    finished.send(()).unwrap();
    server.join().unwrap();
    assert!(error.contains("取消"), "{error}");
}

#[tokio::test]
async fn diagnostic_requires_result_acknowledgement_and_keeps_missing_usage_unknown() {
    for (message, success) in [
        (json!({"content":"DAO_SHELL_OK"}), true),
        (json!({"content":"something else"}), false),
        (probe_call("dao_shell_connection_check"), false),
    ] {
        let (listener, config) = listener();
        let server = thread::spawn(move || {
            let mut first = accept(&listener);
            read_request(&mut first);
            response(
                &mut first,
                200,
                &completion(probe_call("dao_shell_connection_check"), Some(10)),
                "",
            );
            drop(first);
            let mut second = accept(&listener);
            read_request(&mut second);
            response(&mut second, 200, &completion(message, None), "");
            listener
        });
        let result = check_connection(&config, &Cancellation::default()).await;
        let listener = server.join().unwrap();
        assert_eq!(result.is_ok(), success);
        if let Ok(report) = result {
            assert_eq!(report.total_tokens, None);
        }
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[cfg(windows)]
#[test]
fn failed_config_replacement_preserves_original_and_cleans_temporary_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    Config::default().save(&path).unwrap();
    let original = fs::read(&path).unwrap();
    let _guard = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let changed = Config {
        read_roots: vec![temp.path().to_owned()],
        ..Config::default()
    };
    assert!(changed.save(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn invalid_model_config_is_rejected_without_overwriting_existing_config() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.json");
    Config::default().save(&path).unwrap();
    let original = fs::read(&path).unwrap();
    for endpoint in [
        "https://secret@example.com/api",
        "https://example.com/api?key=secret",
        "http://example.com/api",
    ] {
        let result = cli(
            &path,
            &temp.path().join("state"),
            &[
                "config",
                "model",
                "--endpoint",
                endpoint,
                "--model",
                "fixture",
            ],
        );
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("secret"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

#[test]
fn local_doctor_and_noninteractive_setup_never_contact_model_or_change_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let (listener, model) = listener();
    let path = temp.path().join("config.json");
    let data = temp.path().join("state");
    Config {
        read_roots: vec![temp.path().join("missing-a"), temp.path().join("missing-b")],
        model: Some(model),
        ..Config::default()
    }
    .save(&path)
    .unwrap();
    let original = fs::read(&path).unwrap();
    let result = cli(&path, &data, &["doctor"]);
    assert!(!result.status.success());
    let output = String::from_utf8_lossy(&result.stdout);
    assert!(
        output.contains("missing-a")
            && output.contains("missing-b")
            && output.contains("未发送网络请求")
    );
    let result = cli(&path, &data, &["setup"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("交互终端"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(path).unwrap(), original);
    assert!(!data.exists());
}
