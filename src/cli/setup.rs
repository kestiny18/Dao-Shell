use super::terminal::{confirm, display_path};
use crate::{
    config::{Config, ModelConfig},
    core::{Cancellation, safe_text},
    credentials,
    files::{self, Scope},
    model::{ModelClient, check_client},
};
use anyhow::{Result, ensure};
use std::{
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

trait Input {
    fn ask(&mut self, prompt: &str) -> Result<String>;
    fn secret(&mut self) -> Result<String>;
}

struct TerminalInput<'a>(&'a Cancellation);
impl Input for TerminalInput<'_> {
    fn ask(&mut self, prompt: &str) -> Result<String> {
        self.0.check()?;
        print!("{prompt} > ");
        io::stdout().flush()?;
        let mut text = String::new();
        ensure!(
            io::stdin().read_line(&mut text)? != 0,
            "设置已取消，原配置保留"
        );
        self.0.check()?;
        answer(text)
    }
    fn secret(&mut self) -> Result<String> {
        self.0.check()?;
        let text =
            rpassword::prompt_password("粘贴 API Key（隐藏输入；Enter 使用已有密钥；:q 取消） > ")?;
        self.0.check()?;
        answer(text)
    }
}

fn answer(text: String) -> Result<String> {
    let text = text.trim().to_owned();
    ensure!(text != ":q", "设置已取消，原配置保留");
    Ok(text)
}

pub(super) async fn run(
    config: &Config,
    path: &Path,
    cancel: &Cancellation,
) -> Result<Option<Config>> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "setup 需要交互终端；脚本请使用 config add-read / add-write / model"
    );
    println!(
        "欢迎使用 Dao-Shell。先连接模型，再选择允许查找的目录。\n输入 :q 或 Ctrl+C 后 Enter 取消；保存前不修改原配置。"
    );
    let mut input = TerminalInput(cancel);
    let pending = prepare(config, &mut input, cancel).await?;
    println!("\n即将保存到 {}", display_path(path));
    println!("可读取目录：");
    show_roots(&pending.config.read_roots);
    println!("可写目录（本次向导不增加写入权限）：");
    show_roots(&pending.config.write_roots);
    if let Some(model) = &pending.config.model {
        if model.validate().is_ok() {
            println!(
                "模型：{}\n地址：{}",
                safe_text(&model.model),
                safe_text(&model.endpoint)
            );
        } else {
            println!("保留的模型配置尚不可用；可重新运行 setup 修正。");
        }
    }
    cancel.check()?;
    if !confirm("保存并开始使用？输入 y 保存，其他输入取消")? {
        println!("已取消，原配置保留。");
        return Ok(None);
    }
    cancel.check()?;
    pending.config.save(path)?;
    if let (Some(model), Some(key)) = (&pending.config.model, pending.key) {
        credentials::set_session(model, key.clone());
        if pending.persist {
            match credentials::save(model, &key) {
                Ok(()) => println!("密钥已保存到 Windows 凭据管理器；下次启动可直接使用。"),
                Err(error) => println!(
                    "{}。配置已保存，但密钥仅本次有效；可重新运行 daosh setup 保存。",
                    safe_text(&format!("{error:#}"))
                ),
            }
        } else {
            println!(
                "本次输入的密钥仅在当前 Dao-Shell 进程使用，退出后不保留；已有保存的密钥不受影响。"
            );
        }
    }
    println!("设置完成，开始吧。文件移动权限可之后使用 daosh config add-write 配置。");
    Ok(Some(pending.config))
}

struct Pending {
    config: Config,
    key: Option<String>,
    persist: bool,
}

async fn prepare(
    config: &Config,
    input: &mut impl Input,
    cancel: &Cancellation,
) -> Result<Pending> {
    let mut next = config.clone();
    let mut key = None;
    let mut persist = false;
    println!(
        "\n1. DeepSeek（预填地址与模型）\n2. 其他兼容服务\n3. 本地模型\n4. 使用当前模型设置\n0. 暂时跳过模型（保留原设置）"
    );
    loop {
        let default_service = if config.model.as_ref().is_some_and(|m| m.validate().is_ok()) {
            "4"
        } else {
            "1"
        };
        let selection = choice(
            input,
            &format!("选择服务 [{default_service}]"),
            &["0", "1", "2", "3", "4"],
            default_service,
        )?;
        if selection == "0" {
            break;
        }
        let mut model = match selection.as_str() {
            "" | "1" => ModelConfig {
                endpoint: "https://api.deepseek.com/chat/completions".into(),
                model: "deepseek-flash".into(),
                api_key_env: "DAO_SHELL_API_KEY".into(),
            },
            "2" => ModelConfig {
                endpoint: String::new(),
                model: String::new(),
                api_key_env: "DAO_SHELL_API_KEY".into(),
            },
            "3" => ModelConfig {
                endpoint: "http://127.0.0.1:11434/v1/chat/completions".into(),
                model: String::new(),
                api_key_env: String::new(),
            },
            "4" => match config.model.clone() {
                Some(model) if model.validate().is_ok() => model,
                _ => {
                    println!("没有可用的当前设置，请选择服务。");
                    continue;
                }
            },
            _ => {
                println!("请输入 0–4。");
                continue;
            }
        };
        if selection == "2" || selection == "3" {
            edit_endpoint(input, &mut model)?;
        }
        model.model = field(input, "模型 ID", &model.model)?;
        // Validate before looking up credentials or displaying the endpoint.
        while let Err(error) = model.validate() {
            println!("{}", safe_text(&format!("{error:#}")));
            edit_endpoint(input, &mut model)?;
            model.model = field(input, "模型 ID", &model.model)?;
        }
        println!(
            "服务：{}\n模型：{}",
            safe_text(&model.endpoint),
            safe_text(&model.model)
        );
        let mut entered = false;
        let mut candidate = read_key(input, &model, &mut entered)?;
        loop {
            cancel.check()?;
            match ModelClient::with_key(&model, candidate.as_deref()) {
                Ok(client) => {
                    println!(
                        "连接验证使用固定虚构样例，最多两次请求，可能计费，不发送本机文件信息。"
                    );
                    let action = choice(input, "验证连接？[y] / s 跳过验证", &["y", "s"], "y")?;
                    if action == "s" {
                        println!("未验证连接；保存配置不代表模型已可用。");
                        break;
                    }
                    match check_client(&client, cancel).await {
                        Ok(report) => {
                            println!(
                                "✓ 连接、工具调用和结果回传通过（{}ms）。",
                                report.elapsed_ms
                            );
                            break;
                        }
                        Err(error) => {
                            println!("连接验证未通过：{}", safe_text(&format!("{error:#}")))
                        }
                    }
                }
                Err(error) => println!("{}", safe_text(&format!("{error:#}"))),
            }
            cancel.check()?;
            let action = choice(
                input,
                "1 重新输入密钥 / 2 修改地址 / 3 修改模型 / 4 重试",
                &["1", "2", "3", "4"],
                "1",
            )?;
            match action.as_str() {
                "1" => candidate = read_key(input, &model, &mut entered)?,
                "2" => {
                    edit_endpoint(input, &mut model)?;
                    // Never send the previous endpoint's key to a new endpoint implicitly.
                    entered = false;
                    candidate = read_key(input, &model, &mut entered)?;
                }
                "3" => model.model = field(input, "模型 ID", &model.model)?,
                _ => {}
            }
        }
        if entered {
            if cfg!(windows) {
                persist = choice(
                    input,
                    "密钥：1 仅本次使用 [默认] / 2 安全保存到 Windows 凭据管理器",
                    &["1", "2"],
                    "1",
                )? == "2";
            } else {
                println!("此平台暂不支持安全保存密钥；本次输入只在当前进程使用。");
            }
            key = candidate;
        }
        next.model = Some(model);
        break;
    }
    println!(
        "\n选择可查找的目录；可写权限保持原样。开始对话后，输入、候选文件元数据和资源样本会发往模型服务，不发送文件正文。"
    );
    next.read_roots = select_roots(input, &next.read_roots)?;
    if Scope::new(&[], &next.write_roots).is_err() {
        let action = choice(
            input,
            "原可写目录已不可用：1 清空可写范围 / 0 取消设置",
            &["0", "1"],
            "0",
        )?;
        ensure!(action == "1", "设置已取消，原配置保留");
        next.write_roots.clear();
    }
    Scope::new(&next.read_roots, &next.write_roots)?;
    Ok(Pending {
        config: next,
        key,
        persist,
    })
}

fn read_key(
    input: &mut impl Input,
    model: &ModelConfig,
    entered: &mut bool,
) -> Result<Option<String>> {
    if model.api_key_env.is_empty() {
        return Ok(None);
    }
    let text = input.secret()?;
    if text.is_empty() {
        *entered = false;
        match credentials::resolve(model) {
            Ok(key) => Ok(key),
            Err(error) => {
                println!("{}", safe_text(&format!("{error:#}")));
                Ok(None)
            }
        }
    } else {
        *entered = true;
        Ok(Some(text))
    }
}

fn edit_endpoint(input: &mut impl Input, model: &mut ModelConfig) -> Result<()> {
    loop {
        // Only show an existing endpoint if it passed validation (no credentials).
        let default = if model.validate().is_ok() {
            model.endpoint.clone()
        } else if model.endpoint.starts_with("http://127.0.0.1:") {
            "http://127.0.0.1:11434/v1/chat/completions".into()
        } else {
            String::new()
        };
        let endpoint = field(input, "接口地址（完整地址或服务的 /v1 地址）", &default)?;
        let mut check = model.clone();
        check.endpoint = match normalize_endpoint(&endpoint) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                println!("{}", safe_text(&format!("{error:#}")));
                continue;
            }
        };
        check.model = "validation".into();
        match check.validate() {
            Ok(url) => {
                model.endpoint = check.endpoint;
                if !crate::config::is_loopback(&url) && model.api_key_env.is_empty() {
                    model.api_key_env = "DAO_SHELL_API_KEY".into();
                }
                return Ok(());
            }
            Err(error) => println!("{}", safe_text(&format!("{error:#}"))),
        }
    }
}

fn normalize_endpoint(raw: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(raw)
        .map_err(|_| anyhow::anyhow!("请输入 http:// 或 https:// 开头的服务地址"))?;
    if !url
        .path()
        .trim_end_matches('/')
        .ends_with("/chat/completions")
    {
        let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url.to_string())
}

fn field(input: &mut impl Input, name: &str, default: &str) -> Result<String> {
    loop {
        let value = input.ask(&format!("{name} [{}]", safe_text(default)))?;
        let value = if value.is_empty() {
            default.to_owned()
        } else {
            value
        };
        if !value.trim().is_empty() {
            return Ok(value);
        }
        println!("此项不能为空。");
    }
}

fn choice(input: &mut impl Input, prompt: &str, allowed: &[&str], default: &str) -> Result<String> {
    loop {
        let value = input.ask(prompt)?.to_lowercase();
        let value = if value.is_empty() {
            default.to_owned()
        } else {
            value
        };
        if allowed.contains(&value.as_str()) {
            return Ok(value);
        }
        println!("请选择列出的选项；:q 取消。");
    }
}

fn select_roots(input: &mut impl Input, existing: &[PathBuf]) -> Result<Vec<PathBuf>> {
    println!("当前读取目录：");
    show_roots(existing);
    let mut paths = existing.to_vec();
    loop {
        let action = choice(
            input,
            "添加查找目录：1 下载 / 2 文档 / 3 自定义 / 4 清空读取目录 / 0 完成 [默认]",
            &["0", "1", "2", "3", "4"],
            "0",
        )?;
        let path = match action.as_str() {
            "0" => {
                if Scope::new(&paths, &[]).is_err() {
                    println!("有读取目录已不可用，请清空后重新选择。");
                    continue;
                }
                if paths.is_empty() {
                    println!("未选择读取目录；暂时只能观测资源，之后可用 setup 添加目录。");
                }
                return Ok(paths);
            }
            "1" => dirs::download_dir(),
            "2" => dirs::document_dir(),
            "3" => Some(PathBuf::from(input.ask("目录路径")?.trim_matches('"'))),
            "4" => {
                paths.clear();
                println!("已清空本次待保存的读取目录；可写目录仍可读。");
                continue;
            }
            _ => unreachable!(),
        };
        let Some(path) = path else {
            println!("未找到此系统目录，请使用自定义目录。");
            continue;
        };
        match files::normalize_existing(&path) {
            Ok(path) if path.is_dir() => {
                if !paths.contains(&path) && paths.len() < 32 {
                    paths.push(path);
                }
                println!("待保存的读取目录：");
                show_roots(&paths);
            }
            _ => println!("目录不存在或不可用，请重新选择。"),
        }
    }
}

fn show_roots(roots: &[PathBuf]) {
    if roots.is_empty() {
        println!("  未设置");
    }
    for root in roots {
        println!("  {}", display_path(root));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Answers(std::collections::VecDeque<String>);
    impl Input for Answers {
        fn ask(&mut self, _: &str) -> Result<String> {
            answer(self.0.pop_front().expect("missing test answer"))
        }
        fn secret(&mut self) -> Result<String> {
            self.ask("")
        }
    }
    #[tokio::test]
    async fn skipped_model_and_directory_selection_never_grant_write_access() {
        let temp = tempfile::tempdir().unwrap();
        let mut input = Answers(
            [
                "0".into(),
                "3".into(),
                temp.path().to_string_lossy().into_owned(),
                "0".into(),
            ]
            .into(),
        );
        let result = prepare(&Config::default(), &mut input, &Cancellation::default())
            .await
            .unwrap();
        assert_eq!(
            result.config.read_roots,
            vec![files::normalize_existing(temp.path()).unwrap()]
        );
        assert!(result.config.write_roots.is_empty());
        assert!(result.key.is_none() && result.config.model.is_none());
    }
    #[tokio::test]
    async fn cancelled_setup_leaves_configuration_and_credentials_untouched() {
        let original = Config::default();
        let mut input = Answers(
            ["1", "", "fixture-key", "s", "1", ":q"]
                .map(String::from)
                .into(),
        );
        assert!(
            prepare(&original, &mut input, &Cancellation::default())
                .await
                .is_err()
        );
        assert!(original.model.is_none());
    }
    #[test]
    fn endpoint_normalization_preserves_custom_paths_and_validation() {
        assert_eq!(
            normalize_endpoint("https://example.com/v1/").unwrap(),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("https://example.com/chat/completions").unwrap(),
            "https://example.com/chat/completions"
        );
        let model = ModelConfig {
            endpoint: normalize_endpoint("https://user:secret@example.com/v1").unwrap(),
            model: "fixture".into(),
            api_key_env: String::new(),
        };
        assert!(model.validate().is_err());
    }

    #[tokio::test]
    async fn connection_failure_can_repair_key_without_reentering_other_fields() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            time::{Duration, Instant},
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for index in 0..3 {
                let start = Instant::now();
                let mut stream = loop {
                    if let Ok((stream, _)) = listener.accept() {
                        break stream;
                    }
                    assert!(
                        start.elapsed() < Duration::from_secs(10),
                        "fixture timed out"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut headers = Vec::new();
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let headers = String::from_utf8(headers).unwrap().to_lowercase();
                let expected = if index == 0 {
                    "bearer wrong-fixture-key"
                } else {
                    "bearer fixed-fixture-key"
                };
                assert!(
                    headers.contains(expected),
                    "wrong credential used in diagnostic"
                );
                let len: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap();
                let mut body = vec![0; len];
                stream.read_exact(&mut body).unwrap();
                let (status, body) = match index {
                    0 => (401, "{}"),
                    1 => (
                        200,
                        r#"{"choices":[{"message":{"content":null,"tool_calls":[{"id":"probe","type":"function","function":{"name":"dao_shell_connection_check","arguments":"{}"}}]}}]}"#,
                    ),
                    _ => (
                        200,
                        r#"{"choices":[{"message":{"content":"DAO_SHELL_OK"}}]}"#,
                    ),
                };
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let mut answers = vec![
            "2".into(),
            "invalid-url".into(),
            endpoint,
            "fixture-model".into(),
            "wrong-fixture-key".into(),
            "y".into(),
            "1".into(),
            "fixed-fixture-key".into(),
            "y".into(),
        ];
        if cfg!(windows) {
            answers.push("1".into());
        }
        answers.push("0".into());
        let mut input = Answers(answers.into());
        let result = prepare(&Config::default(), &mut input, &Cancellation::default())
            .await
            .unwrap();
        server.join().unwrap();
        assert!(input.0.is_empty());
        assert_eq!(result.key.as_deref(), Some("fixed-fixture-key"));
        assert_eq!(result.config.model.as_ref().unwrap().model, "fixture-model");
        assert!(!result.persist);
        assert!(
            !serde_json::to_string(&result.config)
                .unwrap()
                .contains("fixture-key")
        );
        assert!(result.config.write_roots.is_empty());
    }

    #[tokio::test]
    async fn invalid_existing_directories_can_be_repaired_without_granting_write_scope() {
        let temp = tempfile::tempdir().unwrap();
        let config = Config {
            read_roots: vec![temp.path().join("missing-read")],
            write_roots: vec![temp.path().join("missing-write")],
            model: None,
        };
        let mut input = Answers(["0", "0", "4", "0", "1"].map(String::from).into());
        let result = prepare(&config, &mut input, &Cancellation::default())
            .await
            .unwrap();
        assert!(result.config.read_roots.is_empty() && result.config.write_roots.is_empty());
        assert_eq!(config.write_roots.len(), 1);
    }
}
