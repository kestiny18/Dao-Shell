#![cfg(windows)]

use dao_shell::config::{Config, ModelConfig};
use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn ordinary_open_keeps_windows_launch_semantics_without_parent_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("fixture.rs");
    let executable = temp.path().join("SyntheticLaunchProbe.exe");
    // Only this synthetic binary is executed. It records booleans, never environment values.
    fs::write(&source, r#"
#![windows_subsystem = "windows"]
fn main() {
    let keys = ["DAO_HANDOFF_FAKE_KEY", "TAURI_HANDOFF_FAKE_KEY", "WEBVIEW2_HANDOFF_FAKE_KEY", "CUSTOM_HANDOFF_FAKE_KEY"];
    let clean = keys.iter().all(|key| std::env::var_os(key).is_none());
    let ordinary = std::env::var_os("SystemRoot").is_some();
    let output = std::env::current_exe().unwrap().with_extension("observed");
    std::fs::write(output, format!("{clean},{ordinary}")).unwrap();
}
"#).unwrap();
    let mut compile = Command::new("rustc");
    compile.arg(&source).arg("-o").arg(&executable);
    if cfg!(target_env = "gnu") {
        compile.args(["-C", "linker=rust-lld", "-C", "link-self-contained=yes"]);
    }
    assert!(compile.status().unwrap().success());
    let config = Config {
        read_roots: vec![temp.path().into()],
        model: Some(ModelConfig {
            endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
            model: "unused".into(),
            api_key_env: "CUSTOM_HANDOFF_FAKE_KEY".into(),
        }),
        ..Default::default()
    };
    let config_path = temp.path().join("config.json");
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_daosh"))
        .arg("--config")
        .arg(&config_path)
        .args([
            "search",
            "SyntheticLaunchProbe.exe",
            "--open",
            "1",
            "--json",
        ])
        .env("DAO_HANDOFF_FAKE_KEY", "synthetic-not-a-secret")
        .env("TAURI_HANDOFF_FAKE_KEY", "synthetic-not-a-secret")
        .env("WEBVIEW2_HANDOFF_FAKE_KEY", "synthetic-not-a-secret")
        .env("CUSTOM_HANDOFF_FAKE_KEY", "synthetic-not-a-secret")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let observed = executable.with_extension("observed");
    let began = Instant::now();
    while !observed.exists() && began.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fs::read_to_string(observed).unwrap(), "true,true");
}

#[test]
fn internal_entry_rejects_invalid_or_oversized_input_before_loading_config() {
    use std::{io::Write, process::Stdio};
    for bytes in [
        br#"{"action":"command","path":"cmd.exe","identity":null}"#.to_vec(),
        vec![b'x'; 65537],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_daosh"))
            .arg("--dao-shell-handoff-v1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(23));
    }
}

#[test]
fn provider_entry_rejects_commands_before_com_activation_or_config_load() {
    use std::{io::Write, process::Stdio};
    for bytes in [
        br#"{"kind":"command","text":"arbitrary"}"#.to_vec(),
        vec![b'x'; 65537],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_daosh"))
            .arg("--dao-software-provider-v1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(23));
    }
}
