//! Read-only NTFS volume experiment. Never creates or modifies the journal.
use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
#[path = "ntfs_inventory/index.rs"]
mod index;
#[cfg(windows)]
#[path = "ntfs_inventory/windows.rs"]
mod windows;

#[derive(Parser)]
struct Args {
    #[arg(long, global = true)]
    report: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Probe {
        #[arg(long)]
        volume: String,
    },
    Build {
        #[arg(long)]
        volume: String,
        #[arg(long)]
        db: std::path::PathBuf,
    },
    Replay {
        #[arg(long)]
        db: std::path::PathBuf,
    },
    Query {
        #[arg(long)]
        db: std::path::PathBuf,
        #[arg(long)]
        query: String,
    },
    Status {
        #[arg(long)]
        db: std::path::PathBuf,
    },
}
fn run(args: Args) -> Result<serde_json::Value> {
    #[cfg(windows)]
    match args.command {
        Command::Probe { volume } => windows::probe(&volume),
        Command::Build { volume, db } => index::build(&volume, &db),
        Command::Replay { db } => index::replay(&db),
        Command::Query { db, query } => index::query(&db, &query),
        Command::Status { db } => index::status(&db),
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        anyhow::bail!("Windows NTFS only")
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
        Ok(value) => (value, 0),
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
    fn report_never_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        std::fs::write(&path, "keep").unwrap();
        assert!(super::create_report(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep");
    }
}
