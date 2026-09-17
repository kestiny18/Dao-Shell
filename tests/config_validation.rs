use dao_shell::config::{Config, ModelConfig};
use std::{fs, net::TcpListener, process::Command};

#[test]
fn cmd_quotes_and_base_only_urls_have_actionable_errors() {
    for (endpoint, expected) in [
        ("'https://example.com/chat/completions'", "双引号"),
        ("https://example.com", "/chat/completions"),
    ] {
        let model = ModelConfig {
            endpoint: endpoint.into(),
            model: "fixture".into(),
            api_key_env: "DAO_SHELL_API_KEY".into(),
        };
        assert!(model.validate().unwrap_err().to_string().contains(expected));
    }
}

#[test]
fn bad_key_is_identified_before_network_and_never_printed() {
    let temp = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let path = temp.path().join("config.json");
    Config {
        model: Some(ModelConfig {
            endpoint: format!("http://{}/chat/completions", listener.local_addr().unwrap()),
            model: "fixture".into(),
            api_key_env: "DAO_TEST_BAD_KEY".into(),
        }),
        ..Config::default()
    }
    .save(&path)
    .unwrap();
    for key in ["PRIVATE\nSECRET", "PRIVATE SECRET", "PRIVATE\u{200b}SECRET"] {
        let result = Command::new(env!("CARGO_BIN_EXE_daosh"))
            .arg("--config")
            .arg(&path)
            .args(["doctor", "--check-model"])
            .env("DAO_TEST_BAD_KEY", key)
            .output()
            .unwrap();
        assert!(!result.status.success());
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(output.contains("API Key 包含非法字符"), "{output}");
        assert!(!output.contains("PRIVATE") && !output.contains("SECRET"));
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn accidental_cli_command_in_chat_is_not_forwarded_to_model() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let path = temp.path().join("config.json");
    Config {
        model: Some(ModelConfig {
            endpoint: format!("http://{}/chat/completions", listener.local_addr().unwrap()),
            model: "fixture".into(),
            api_key_env: String::new(),
        }),
        ..Config::default()
    }
    .save(&path)
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_daosh"))
        .arg("--config")
        .arg(&path)
        .arg("--data-dir")
        .arg(temp.path().join("state"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"config model --endpoint PRIVATE_INPUT\nquit\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let output = String::from_utf8_lossy(&result.stdout);
    assert!(output.contains("这是终端命令"));
    assert!(!output.contains("PRIVATE_INPUT"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!fs::read_to_string(&path).unwrap().contains("PRIVATE_INPUT"));
}
