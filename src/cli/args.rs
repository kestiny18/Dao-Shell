use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "以自然语言操作文件、查看资源的小型智能 Shell")]
pub struct Args {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    /// 为本次运行增加可读目录。
    #[arg(long, global = true)]
    pub read_root: Vec<PathBuf>,
    /// 为本次运行增加可写目录（同时可读）。
    #[arg(long, global = true)]
    pub write_root: Vec<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Subcommand)]
pub enum Command {
    /// 不调用模型，直接搜索名称与元数据。
    Search {
        #[arg(default_value = "")]
        query: String,
        #[arg(long = "in")]
        directory: Option<PathBuf>,
        #[arg(long)]
        extension: Option<String>,
        #[arg(long)]
        modified_after: Option<String>,
        #[arg(long)]
        modified_before: Option<String>,
        #[arg(long)]
        min_bytes: Option<u64>,
        #[arg(long)]
        max_bytes: Option<u64>,
        #[arg(long, value_enum, default_value = "modified")]
        sort: SearchSort,
        #[arg(long, default_value_t = 0)]
        page: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
        /// 打开这次结果中的第几项（从 1 开始）。
        #[arg(long)]
        open: Option<usize>,
    },
    /// 采样 CPU、内存、磁盘空间与进程，不请求模型。
    Resources {
        #[arg(long, default_value_t = 2000)]
        sample_ms: u64,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// 查看、核对已知操作，或清除已完成记录。
    History {
        #[arg(long)]
        reconcile: bool,
        #[arg(long)]
        clear: bool,
    },
    /// 本地配置；API Key 只从环境变量读取。
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 检查本地范围、模型配置和数据目录，不发送网络请求。
    Doctor,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum SearchSort {
    Modified,
    Name,
    Size,
}
#[derive(Subcommand)]
pub enum ConfigAction {
    Show,
    AddRead {
        directory: PathBuf,
    },
    AddWrite {
        directory: PathBuf,
    },
    Model {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value = "DAO_SHELL_API_KEY")]
        key_env: String,
    },
    ClearModel,
}
