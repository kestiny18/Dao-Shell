use clap::Parser;

#[tokio::main]
async fn main() {
    if let Err(error) = dao_shell::cli::run(dao_shell::cli::Args::parse()).await {
        eprintln!(
            "{}",
            dao_shell::core::safe_text(&format!("错误：{error:#}"))
        );
        std::process::exit(1);
    }
}
