mod args;
mod terminal;

pub use args::Args;
use args::{Command, ConfigAction, SearchSort};
use terminal::{Terminal, confirm, display_path, pretty, render};

use crate::{
    capabilities::{Interaction, Runtime},
    config::{Config, ModelConfig, data_dir},
    core::{Cancellation, safe_multiline, safe_text},
    dialogue::Dialogue,
    files::{self, Objects, Scope, Search, Sort},
    operations, resources,
    storage::Journal,
};
use anyhow::{Context, Result, bail};
use std::io::{self, Write};

pub async fn run(args: Args) -> Result<()> {
    let config_path = args.config.unwrap_or_else(Config::path);
    let data_path = args.data_dir.unwrap_or_else(data_dir);
    let mut config = Config::load(&config_path)?;
    if let Some(Command::Config { action }) = args.command {
        match action {
            ConfigAction::Show => {
                pretty(&serde_json::to_value(&config)?);
                return Ok(());
            }
            ConfigAction::AddRead { directory } => config
                .read_roots
                .push(files::normalize_existing(&directory)?),
            ConfigAction::AddWrite { directory } => config
                .write_roots
                .push(files::normalize_existing(&directory)?),
            ConfigAction::Model {
                endpoint,
                model,
                key_env,
            } => {
                config.model = Some(ModelConfig {
                    endpoint,
                    model,
                    api_key_env: key_env,
                })
            }
            ConfigAction::ClearModel => config.model = None,
        }
        Scope::new(&config.read_roots, &config.write_roots)?;
        config.save(&config_path)?;
        println!("配置已保存：{}", display_path(&config_path));
        return Ok(());
    }
    config.read_roots.extend(args.read_root);
    config.write_roots.extend(args.write_root);
    if let Some(Command::Search {
        directory: Some(path),
        ..
    }) = &args.command
    {
        config.read_roots.push(path.clone());
    }
    let scope = Scope::new(&config.read_roots, &config.write_roots)?;
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    match args.command {
        Some(Command::Doctor) => {
            println!(
                "Dao-Shell {} / {}\n配置：{}\n记录：{}\n读取范围：{}\n写入范围：{}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                display_path(&config_path),
                display_path(&data_path),
                serde_json::to_string(scope.read_roots())?,
                serde_json::to_string(scope.write_roots())?
            );
            if let Some(m) = config.model {
                println!(
                    "模型：{}；密钥环境变量 {} {}（未测试连接）",
                    safe_text(&m.model),
                    safe_text(&m.api_key_env),
                    if std::env::var(&m.api_key_env).is_ok_and(|s| !s.is_empty()) {
                        "已设置"
                    } else {
                        "未设置"
                    }
                );
            } else {
                println!("模型未配置；直接搜索与资源观测可用。");
            }
            return Ok(());
        }
        Some(Command::Search {
            query,
            directory,
            extension,
            modified_after,
            modified_before,
            min_bytes,
            max_bytes,
            sort,
            page,
            limit,
            json,
            open,
        }) => {
            let mut objects = Objects::default();
            let request = Search {
                query,
                directory,
                extension,
                modified_after,
                modified_before,
                min_bytes,
                max_bytes,
                sort: match sort {
                    SearchSort::Modified => Sort::ModifiedDesc,
                    SearchSort::Name => Sort::Name,
                    SearchSort::Size => Sort::SizeDesc,
                },
                page,
                limit: Some(limit),
            };
            let result = files::search(&request, &scope, &mut objects, &cancel)?;
            let value = serde_json::to_value(&result)?;
            if json {
                pretty(&value);
            } else {
                render("file_search", &value);
            }
            if let Some(index) = open {
                let object = result
                    .items
                    .get(index.checked_sub(1).context("序号从 1 开始")?)
                    .context("序号不在本页结果中")?;
                pretty(&files::open(&objects.checked(&object.id, &scope, false)?)?);
            }
            return Ok(());
        }
        Some(Command::Resources {
            sample_ms,
            limit,
            json,
        }) => {
            let result = resources::snapshot(
                &resources::Request {
                    sample_ms: Some(sample_ms),
                    limit: Some(limit),
                    ..Default::default()
                },
                &cancel,
                false,
            )?;
            if json {
                pretty(&result);
            } else {
                render("resource_snapshot", &result);
            }
            return Ok(());
        }
        _ => {}
    }
    let mut journal = Journal::open(&data_path)?;
    for receipt in operations::reconcile(&mut journal)? {
        render("file_move_batch", &serde_json::to_value(receipt)?);
    }
    if let Some(Command::History {
        reconcile: _,
        clear,
    }) = args.command
    {
        if clear && confirm("清除已完成的操作记录？未核对记录会保留")? {
            println!("已清除 {} 条记录。", journal.clear_completed()?);
        }
        for receipt in journal.list()?.into_iter().take(50) {
            render("file_move_batch", &serde_json::to_value(receipt)?);
        }
        return Ok(());
    }
    let mut runtime = Runtime {
        scope,
        objects: Objects::default(),
        journal,
        cancel,
        last_results: Vec::new(),
        side_effects_blocked: false,
    };
    chat(&config, &mut runtime).await
}

async fn chat(config: &Config, runtime: &mut Runtime) -> Result<()> {
    println!(
        "Dao-Shell — 自然语言使用电脑\n/help 查看快捷入口，/quit 退出。Ctrl+C 请求停止本轮；输入提示处按 Enter 返回。\n读取范围：{}\n写入范围：{}",
        serde_json::to_string(runtime.scope.read_roots())?,
        serde_json::to_string(runtime.scope.write_roots())?
    );
    let mut dialogue = match &config.model {
        Some(model) => match Dialogue::new(model, runtime) {
            Ok(d) => {
                println!(
                    "模型：{}（{}）。输入、选中的文件元数据和采样结果会发往该服务；不读取文件正文。",
                    safe_text(&model.model),
                    safe_text(&model.endpoint)
                );
                Some(d)
            }
            Err(e) => {
                eprintln!("{e:#}");
                None
            }
        },
        None => {
            println!("尚未配置模型。可先使用 /search；配置方法见 README。");
            None
        }
    };
    let mut ui = Terminal;
    loop {
        runtime.cancel.reset();
        runtime.side_effects_blocked = false;
        print!("\n你 > ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();
        if runtime.cancel.is_cancelled() || input.is_empty() {
            continue;
        }
        if input == "/quit" || input == "/exit" {
            break;
        }
        let result = if input.starts_with('/') {
            shortcut(input, runtime, &mut dialogue, &mut ui)
        } else if let Some(dialogue) = &mut dialogue {
            dialogue
                .turn(input, runtime, &mut ui)
                .await
                .map(|answer| println!("\nShell > {}", safe_multiline(&answer)))
        } else {
            Err(anyhow::anyhow!(
                "模型未配置或初始化失败；请配置后重启。直接搜索：/search 关键词"
            ))
        };
        if let Err(error) = result {
            eprintln!("{}", safe_text(&format!("{error:#}")));
        }
    }
    Ok(())
}
fn shortcut(
    input: &str,
    runtime: &mut Runtime,
    dialogue: &mut Option<Dialogue>,
    ui: &mut dyn Interaction,
) -> Result<()> {
    let (command, rest) = input.split_once(' ').unwrap_or((input, ""));
    match command {
        "/help" => println!(
            "自然语言：找 PDF、查看资源压力、把候选移到指定目录。\n/search 关键词：不调用模型\n/open 2：打开最近结果第 2 项\n/move 1,2 C:\\绝对目录：显示移动确认\n/resources：短时资源采样\n/history：本地回执\n/reset：清除当前上下文和候选\n/quit：退出"
        ),
        "/reset" => {
            runtime.objects = Objects::default();
            runtime.last_results.clear();
            if let Some(d) = dialogue {
                d.reset();
            }
            println!("会话与候选已清除；操作记录保留。");
        }
        "/search" => {
            let value = runtime.call("file_search", serde_json::json!({"query":rest}), ui)?;
            if let Some(d) = dialogue {
                d.observe_local("file_search", &value);
            }
        }
        "/open" => {
            let id = selection(rest, runtime)?;
            let object = runtime.objects.checked(&id, &runtime.scope, false)?;
            let value = files::open(&object)?;
            ui.result("file_open", &value);
            if let Some(d) = dialogue {
                d.observe_local("file_open", &value);
            }
        }
        "/move" => {
            let (indices, destination) = rest
                .split_once(' ')
                .context("用法：/move 1,2 C:\\绝对目录")?;
            let ids = indices
                .split(',')
                .map(|index| selection(index, runtime))
                .collect::<Result<Vec<_>>>()?;
            let value = runtime.call("file_move_batch", serde_json::json!({"object_ids":ids,"destination":destination.trim().trim_matches('"')}), ui)?;
            if let Some(d) = dialogue {
                d.observe_local("file_move_batch", &value);
            }
        }
        "/resources" => {
            let value = runtime.call("resource_snapshot", serde_json::json!({}), ui)?;
            if let Some(d) = dialogue {
                d.observe_local("resource_snapshot", &value);
            }
        }
        "/history" => {
            for op in runtime.journal.list()?.into_iter().take(50) {
                ui.result("file_move_batch", &serde_json::to_value(op)?);
            }
        }
        _ => bail!("未知快捷入口；输入 /help 查看"),
    }
    Ok(())
}
fn selection(value: &str, runtime: &Runtime) -> Result<String> {
    let index = value
        .trim()
        .parse::<usize>()
        .context("需要结果序号，如 2")?
        .checked_sub(1)
        .context("序号从 1 开始")?;
    runtime
        .last_results
        .get(index)
        .cloned()
        .context("序号不在最近一次结果中")
}
