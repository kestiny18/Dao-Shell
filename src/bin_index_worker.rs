use anyhow::Result;
use clap::{Parser, Subcommand};
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
    ServiceRun,
    ServiceStatus,
    ServiceHealth,
    ServiceVolumeStatus {
        #[arg(long)]
        volume_key: String,
    },
    ServiceSearch {
        #[arg(long)]
        volume_key: String,
        #[arg(long)]
        query: String,
        #[arg(long)]
        cursor: Option<String>,
    },
    ServiceStart,
    ServiceStop,
    DiscoverVolumes,
    Install {
        #[arg(long)]
        expected_sha256: String,
    },
    Upgrade {
        #[arg(long)]
        expected_sha256: String,
    },
    Repair {
        #[arg(long)]
        expected_sha256: String,
    },
    Rollback,
    Uninstall {
        #[arg(long)]
        delete_index: bool,
    },
    StorageLocation,
    Diagnostics {
        #[arg(long)]
        experiment: String,
    },
    Serve {
        #[arg(long)]
        volume: Option<String>,
        #[arg(long)]
        fixture_root: Option<PathBuf>,
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        pipe: String,
        #[arg(long, default_value_t = 1000000)]
        max_resolutions: usize,
        #[arg(long)]
        run_for_seconds: Option<u64>,
    },
    Status {
        #[arg(long)]
        pipe: String,
        #[arg(long)]
        server_exe: PathBuf,
        #[arg(long)]
        fixture: bool,
    },
    Search {
        #[arg(long)]
        pipe: String,
        #[arg(long)]
        server_exe: PathBuf,
        #[arg(long)]
        fixture: bool,
        #[arg(long)]
        query: String,
        #[arg(long)]
        cursor: Option<String>,
    },
}
#[cfg(windows)]
fn run(args: Args) -> Result<serde_json::Value> {
    use dao_shell::file_index::{
        Cancellation,
        worker::{self, protocol::*},
    };
    match args.command {
        Command::ServiceRun => {
            worker::service::run()?;
            Ok(serde_json::json!({"status":"stopped"}))
        }
        Command::ServiceStatus => worker::service::status(),
        Command::ServiceHealth => worker::service::health(),
        command @ (Command::ServiceVolumeStatus { .. } | Command::ServiceSearch { .. }) => {
            let (volume_key, operation) = match command {
                Command::ServiceVolumeStatus { volume_key } => (volume_key, Operation::Status),
                Command::ServiceSearch {
                    volume_key,
                    query,
                    cursor,
                } => (
                    volume_key,
                    Operation::Search {
                        query,
                        page_size: 50,
                        cursor,
                    },
                ),
                _ => unreachable!(),
            };
            let request = Request {
                version: VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                timeout_ms: 2000,
                operation,
            };
            Ok(serde_json::to_value(worker::service::request_volume(
                &volume_key,
                &request,
                &Cancellation::with_timeout(std::time::Duration::from_secs(4)),
            )?)?)
        }
        Command::DiscoverVolumes => Ok(serde_json::to_value(worker::service::local_discovery()?)?),
        Command::Install { expected_sha256 } => worker::service::manage(
            worker::service::Action::Install,
            Some(&expected_sha256),
            false,
        ),
        Command::Upgrade { expected_sha256 } => worker::service::manage(
            worker::service::Action::Upgrade,
            Some(&expected_sha256),
            false,
        ),
        Command::Repair { expected_sha256 } => worker::service::manage(
            worker::service::Action::Repair,
            Some(&expected_sha256),
            false,
        ),
        Command::Rollback => {
            worker::service::manage(worker::service::Action::Rollback, None, false)
        }
        Command::Uninstall { delete_index } => {
            worker::service::manage(worker::service::Action::Uninstall, None, delete_index)
        }
        Command::ServiceStart => {
            worker::service::manage(worker::service::Action::Start, None, false)
        }
        Command::ServiceStop => worker::service::manage(worker::service::Action::Stop, None, false),
        Command::StorageLocation => {
            Ok(serde_json::json!({"directory":worker::experiment_storage_location()?}))
        }
        Command::Diagnostics { experiment } => worker::experiment_diagnostics(&experiment),
        Command::Serve {
            volume,
            fixture_root,
            db,
            pipe,
            max_resolutions,
            run_for_seconds,
        } => {
            anyhow::ensure!(
                volume.is_some() != fixture_root.is_some(),
                "choose volume or fixture-root"
            );
            let mode = match volume {
                Some(v) => worker::Mode::Volume(v),
                None => worker::Mode::Fixture(fixture_root.unwrap()),
            };
            let stop = Cancellation::default();
            let signal = stop.clone();
            ctrlc::set_handler(move || signal.cancel())?;
            if let Some(seconds) = run_for_seconds {
                anyhow::ensure!((1..=3600).contains(&seconds), "invalid experiment duration");
                let timer = stop.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(seconds));
                    timer.cancel();
                });
            }
            Ok(serde_json::to_value(worker::run(
                worker::Config {
                    mode,
                    database: db,
                    pipe,
                    max_resolutions,
                },
                stop,
            )?)?)
        }
        command => {
            let (pipe, server_exe, fixture, operation) = match command {
                Command::Status {
                    pipe,
                    server_exe,
                    fixture,
                } => (pipe, server_exe, fixture, Operation::Status),
                Command::Search {
                    pipe,
                    server_exe,
                    fixture,
                    query,
                    cursor,
                } => (
                    pipe,
                    server_exe,
                    fixture,
                    Operation::Search {
                        query,
                        page_size: 50,
                        cursor,
                    },
                ),
                _ => unreachable!(),
            };
            let request = Request {
                version: VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                timeout_ms: 2000,
                operation,
            };
            Ok(serde_json::to_value(worker::request(
                &pipe,
                &server_exe,
                fixture,
                &request,
                &Cancellation::with_timeout(std::time::Duration::from_secs(4)),
            )?)?)
        }
    }
}
#[cfg(not(windows))]
fn run(_: Args) -> Result<serde_json::Value> {
    anyhow::bail!("Windows worker only")
}
fn create_report(path: &std::path::Path) -> Result<std::fs::File> {
    anyhow::ensure!(path.is_absolute(), "report must be absolute");
    for parent in path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing report parent"))?
        .ancestors()
    {
        let m = std::fs::symlink_metadata(parent)?;
        anyhow::ensure!(!m.file_type().is_symlink(), "linked report parent");
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            anyhow::ensure!(m.file_attributes() & 0x400 == 0, "reparse report parent");
        }
    }
    Ok(std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?)
}
fn validate_report(args: &Args) -> Result<()> {
    let experimental = matches!(
        args.command,
        Command::StorageLocation
            | Command::Diagnostics { .. }
            | Command::Serve { .. }
            | Command::Status { .. }
            | Command::Search { .. }
    );
    anyhow::ensure!(
        args.report.is_none() || experimental,
        "--report is unavailable for production service commands"
    );
    Ok(())
}
fn main() {
    use std::io::Write;
    let args = Args::parse();
    if let Err(error) = validate_report(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    // SCM has no interactive console and accepts no caller-controlled report path.
    if matches!(args.command, Command::ServiceRun) {
        std::process::exit(if run(args).is_ok() { 0 } else { 1 });
    }
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
            let code = i32::from(value.get("error").is_some_and(|error| !error.is_null()));
            (value, code)
        }
        Err(e) => (
            serde_json::json!({"status":"error","message":format!("{e:#}")}),
            1,
        ),
    };
    println!("{value}");
    if let Some(file) = &mut report
        && writeln!(file, "{value}")
            .and_then(|_| file.sync_all())
            .is_err()
    {
        std::process::exit(1);
    }
    std::process::exit(code);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn management_reports_are_rejected_before_any_file_creation() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("must-not-exist.json");
        let text = path.to_str().unwrap();
        for command in [
            "service-run",
            "service-status",
            "service-health",
            "service-start",
            "service-stop",
            "rollback",
            "uninstall",
            "discover-volumes",
        ] {
            let args = Args::try_parse_from(["worker", command, "--report", text]).unwrap();
            assert!(validate_report(&args).is_err());
            assert!(!path.exists());
        }
        for command in ["install", "upgrade", "repair"] {
            assert!(Args::try_parse_from(["worker", command]).is_err());
            let args = Args::try_parse_from([
                "worker",
                command,
                "--expected-sha256",
                &"A".repeat(64),
                "--report",
                text,
            ])
            .unwrap();
            assert!(validate_report(&args).is_err());
            assert!(!path.exists());
        }
        let args = Args::try_parse_from([
            "worker",
            "diagnostics",
            "--experiment",
            "fixture",
            "--report",
            text,
        ])
        .unwrap();
        assert!(validate_report(&args).is_ok());
        assert!(!path.exists());
    }
}
