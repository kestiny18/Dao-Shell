//! Read-only discovery experiment. Not registered as an LLM tool or production backend.
#[path = "inventory/index.rs"]
mod index;
#[cfg(windows)]
#[path = "inventory/windows.rs"]
mod windows;

use anyhow::Result;
use clap::{Parser, Subcommand};
use dao_shell::core::Cancellation;
use std::{path::PathBuf, time::Duration};

#[derive(Parser)]
#[command(about = "实验性文件清单：仅记录元数据，不调用模型、不打开或修改源文件")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 扫描一个明确目录并原子替换快照；数据库必须位于该目录之外。
    Scan {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        db: PathBuf,
        #[arg(long, default_value_t = 100_000)]
        max_entries: usize,
        #[arg(long, default_value_t = 60)]
        max_seconds: u64,
    },
    /// 查询已保存的快照；不会刷新、扫描磁盘或授予操作权限。
    Query {
        #[arg(long)]
        db: PathBuf,
        #[arg(required = true)]
        terms: Vec<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 查看快照时间、覆盖范围与遗漏。
    Status {
        #[arg(long)]
        db: PathBuf,
    },
    /// Windows：只查询文件系统和 USN 日志状态，不提权、不创建或修改日志。
    Probe {
        #[arg(long)]
        root: PathBuf,
    },
}

fn main() -> Result<()> {
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let result = match Args::parse().command {
        Command::Scan {
            root,
            db,
            max_entries,
            max_seconds,
        } => serde_json::to_value(index::scan(
            &root,
            &db,
            max_entries,
            Duration::from_secs(max_seconds),
            &cancel,
        )?)?,
        Command::Query { db, terms, limit } => {
            serde_json::to_value(index::query(&db, &terms, limit)?)?
        }
        Command::Status { db } => serde_json::to_value(index::status(&db)?)?,
        Command::Probe { root } => {
            #[cfg(windows)]
            {
                windows::probe(&root)?
            }
            #[cfg(not(windows))]
            {
                let _ = root;
                anyhow::bail!("USN 探测仅支持 Windows");
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
