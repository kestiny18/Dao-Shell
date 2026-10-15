//! Bounded P11.1 metadata experiment. No service, IPC, source writes or journal changes.
use anyhow::Result;
use clap::{Parser, Subcommand};
#[cfg(windows)]
use dao_shell::file_index::{Budget, Cancellation, IndexStore, Resolved, VolumeSource};
use serde_json::json;
use std::path::PathBuf;
#[derive(Parser)]
struct Args {
    #[arg(long, global = true)]
    report: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Inspect {
        #[arg(long)]
        volume: String,
        #[arg(long)]
        id: Vec<String>,
        #[arg(long)]
        path: Vec<PathBuf>,
    },
    Build {
        #[arg(long)]
        volume: String,
        #[arg(long)]
        db: PathBuf,
        #[arg(long, default_value_t = 20000)]
        max_resolutions: usize,
    },
    Replay {
        #[arg(long)]
        volume: String,
        #[arg(long)]
        db: PathBuf,
        #[arg(long, default_value_t = 20000)]
        max_resolutions: usize,
    },
    Query {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        query: String,
    },
    Status {
        #[arg(long)]
        db: PathBuf,
    },
}
#[cfg(windows)]
fn run(args: Args) -> Result<serde_json::Value> {
    use dao_shell::file_index::windows::WindowsVolumeSource;
    let cancel = std::sync::Arc::new(Cancellation::default());
    let signal = cancel.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let rebuild = matches!(args.command, Command::Build { .. });
    match args.command {
        Command::Inspect { volume, id, path } => {
            let mut source = WindowsVolumeSource::open(&volume)?;
            let mut ids = id;
            let mut path_checks = Vec::new();
            anyhow::ensure!(ids.len() + path.len() <= 100, "too many inspection IDs");
            for path in path {
                let id = source.id_for_path(&path)?;
                let parent = source.id_for_path(
                    path.parent()
                        .ok_or_else(|| anyhow::anyhow!("fixture has no parent"))?,
                )?;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("invalid fixture name"))?;
                let matched = match source.resolve(&id)? {
                    Resolved::Present(node) => node
                        .names
                        .iter()
                        .any(|n| n.parent == parent && n.name == name),
                    _ => false,
                };
                anyhow::ensure!(matched, "fixture path not in reconciled name set");
                path_checks.push(json!({"id":id,"name_association_matches":matched}));
                ids.push(id);
            }
            anyhow::ensure!(ids.len() <= 100, "too many inspection IDs");
            let mut values = Vec::new();
            let mut failed = false;
            for id in ids {
                cancel.check()?;
                let value = match source.resolve(&id) {
                    Ok(Resolved::Present(n)) => {
                        json!({"id":id,"kind":"present","directory":n.directory,"names":n.names.len(),"parent_ids":n.names.iter().map(|n|n.parent.clone()).collect::<Vec<_>>()})
                    }
                    Ok(Resolved::Deleted) => json!({"id":id,"kind":"deleted"}),
                    Ok(Resolved::Unavailable(reason)) => {
                        json!({"id":id,"kind":"unavailable","reason":reason})
                    }
                    Err(error) => {
                        failed = true;
                        json!({"id":id,"kind":"error","error":format!("{error:#}")})
                    }
                };
                values.push(value);
            }
            Ok(
                json!({"status":if failed {"error"} else {"ok"},"inspection":values,"path_checks":path_checks,"metrics":source.metrics}),
            )
        }
        Command::Build {
            volume,
            db,
            max_resolutions,
        }
        | Command::Replay {
            volume,
            db,
            max_resolutions,
        } => {
            let mut source = WindowsVolumeSource::open(&volume)?;
            let mut store = IndexStore::open(&db)?;
            let time = std::time::Instant::now();
            let result = store.update(
                &mut source,
                rebuild,
                Budget {
                    max_resolutions,
                    max_pages: 10000,
                },
                &cancel,
            );
            Ok(
                json!({"result":match result {Ok(s)=>serde_json::to_value(s)?,Err(e)=>json!({"error":format!("{e:#}"),"published":false})},"metrics":source.metrics,"elapsed_ms":time.elapsed().as_millis()}),
            )
        }
        Command::Query { db, query } => {
            let store = IndexStore::open(&db)?;
            let time = std::time::Instant::now();
            let page = store.search(&query, 50, None, &cancel)?;
            Ok(
                json!({"returned":page.items.len(),"has_more":page.next.is_some(),"complete":page.status.complete,"generation":page.status.generation,"elapsed_ms":time.elapsed().as_millis()}),
            )
        }
        Command::Status { db } => Ok(serde_json::to_value(IndexStore::open(&db)?.status()?)?),
    }
}
#[cfg(not(windows))]
fn run(_: Args) -> Result<serde_json::Value> {
    anyhow::bail!("Windows probe only")
}
fn outcome_exit_code(value: &serde_json::Value) -> i32 {
    if value.get("status").and_then(|v| v.as_str()) == Some("error")
        || value
            .get("result")
            .and_then(|r| r.get("published"))
            .and_then(|v| v.as_bool())
            == Some(false)
    {
        1
    } else {
        0
    }
}
fn main() {
    use std::io::Write;
    let args = Args::parse();
    let report = args.report.as_deref().map(create_report).transpose();
    let mut report = match report {
        Ok(file) => file,
        Err(e) => {
            eprintln!("report: {e:#}");
            std::process::exit(1)
        }
    };
    let (value, code) = match run(args) {
        Ok(value) => {
            let code = outcome_exit_code(&value);
            (value, code)
        }
        Err(error) => (json!({"status":"error","message":format!("{error:#}")}), 1),
    };
    println!("{value}");
    if let Some(file) = report.as_mut()
        && let Err(error) = writeln!(file, "{value}").and_then(|_| file.sync_all())
    {
        eprintln!("report write failed: {error}");
        std::process::exit(1);
    }
    std::process::exit(code);
}
fn create_report(path: &std::path::Path) -> Result<std::fs::File> {
    anyhow::ensure!(path.is_absolute(), "report must be absolute");
    let mut ancestor = path.parent();
    while let Some(p) = ancestor {
        let metadata = std::fs::symlink_metadata(p)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "report parent must not be a link"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            anyhow::ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "report parent must not be a reparse point"
            );
        }
        ancestor = p.parent();
    }
    Ok(std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn unpublished_update_has_failure_exit_code() {
        let report = serde_json::json!({"result":{"error":"budget_exhausted","published":false},"metrics":{"metadata_lookups":1}});
        assert_ne!(super::outcome_exit_code(&report), 0);
        assert_eq!(
            super::outcome_exit_code(&serde_json::json!({"result":{"complete":true}})),
            0
        );
    }
    #[test]
    fn report_never_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        std::fs::write(&path, "keep").unwrap();
        assert!(super::create_report(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep");
    }
}
