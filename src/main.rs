use clap::Parser;

fn main() {
    if let Some(code) = dao_shell::software::isolated::run_internal_mode() {
        std::process::exit(code);
    }
    if let Some(code) = dao_shell::shell_handoff::run_internal_mode() {
        std::process::exit(code);
    }
    run();
}

#[tokio::main]
async fn run() {
    if let Err(error) = dao_shell::cli::run(dao_shell::cli::Args::parse()).await {
        eprintln!(
            "{}",
            dao_shell::core::safe_text(&format!("错误：{error:#}"))
        );
        std::process::exit(1);
    }
}
