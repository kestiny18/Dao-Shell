use super::terminal::display_path;
use crate::{
    config::Config,
    core::{Cancellation, safe_text},
    files::Scope,
    model::{ModelClient, check_connection},
};
use anyhow::{Result, bail};
use std::path::Path;

pub(super) async fn run(
    config: &Config,
    config_path: &Path,
    data_path: &Path,
    check_model: bool,
    cancel: &Cancellation,
) -> Result<()> {
    println!(
        "Dao-Shell {} / {}\n配置：{}\n记录目录：{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        display_path(config_path),
        display_path(data_path)
    );
    let mut problems = 0;
    if config.access_mode == crate::config::AccessMode::Full {
        println!("[完全访问] 受当前系统权限约束；配置目录是搜索起点，移动仍需确认和记录。");
    }
    for (kind, roots) in [("读取", &config.read_roots), ("可写", &config.write_roots)] {
        for root in roots {
            match Scope::new(std::slice::from_ref(root), &[]) {
                Ok(_) => println!("[可用] {kind}目录：{}", display_path(root)),
                Err(_) => {
                    problems += 1;
                    println!(
                        "[需处理] {kind}目录不可用：{}；请用 setup 重新指定",
                        display_path(root)
                    );
                }
            }
        }
    }
    if config.access_mode == crate::config::AccessMode::Restricted
        && config.read_roots.is_empty()
        && config.write_roots.is_empty()
    {
        println!("[待设置] 尚未授权目录，文件搜索没有范围；运行 setup 设置，资源观测仍可用。");
    }
    if config.access_mode == crate::config::AccessMode::Restricted && config.write_roots.is_empty()
    {
        println!("[只读] 未授权可写目录，不能移动文件。");
    }
    match &config.model {
        Some(model) => match ModelClient::new(model) {
            Ok(_) => {
                println!(
                    "[可用] 模型本地配置：{}（{}）",
                    safe_text(&model.model),
                    safe_text(&model.endpoint)
                );
                if check_model {
                    println!(
                        "正在向配置的模型发送固定诊断样例（最多两次请求，可能计费）；不读取本机文件或资源信息。"
                    );
                    let report = check_connection(model, cancel).await?;
                    println!(
                        "[通过] 模型连接、工具调用及结果回传；总耗时 {}ms；服务报告 token 总数：{}。",
                        report.elapsed_ms,
                        report
                            .total_tokens
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "未提供".into())
                    );
                    println!("这仅验证协议兼容性；真实文件场景的效果仍需单独试用。");
                } else {
                    println!("[未测试] 未发送网络请求；运行 doctor --check-model 验证模型连接。");
                }
            }
            Err(error) => {
                problems += 1;
                println!("[需处理] {}", safe_text(&format!("{error:#}")));
            }
        },
        None => {
            println!("[待设置] 模型未配置；运行 setup 接入模型，直接搜索和资源观测仍可用。");
            if check_model {
                problems += 1;
            }
        }
    }
    if problems > 0 {
        bail!("发现 {problems} 项需处理的配置；修正后重试 doctor");
    }
    Ok(())
}
