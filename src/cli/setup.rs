use super::terminal::{confirm, display_path};
use crate::{
    config::{Config, ModelConfig},
    core::{Cancellation, safe_text},
    files::{self, Scope},
};
use anyhow::{Context, Result, ensure};
use std::{
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

pub(super) fn run(config: &Config, path: &Path, cancel: &Cancellation) -> Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "setup 需要交互终端；脚本请使用 config add-read / add-write / model"
    );
    println!(
        "首次设置：指定目录和模型。输入 :q 或 Ctrl+C 后 Enter 取消，保存前不会修改配置。\n目录留空保留原范围；输入新目录会替换该类范围，可逐项添加；输入 - 清空。"
    );
    let next = collect(config, |prompt| {
        cancel.check()?;
        print!("{prompt} > ");
        io::stdout().flush()?;
        let mut input = String::new();
        ensure!(
            io::stdin().read_line(&mut input)? != 0,
            "设置已取消，原配置保留"
        );
        cancel.check()?;
        let input = input.trim().to_owned();
        ensure!(input != ":q", "设置已取消，原配置保留");
        Ok(input)
    })?;
    let scope = Scope::new(&next.read_roots, &next.write_roots)?;
    println!("\n即将保存到 {}", display_path(path));
    println!("可读取目录（含可写目录）：");
    show_roots(scope.read_roots());
    println!("可移动文件的目录（来源与目的地都需在范围内，每次仍需确认）：");
    show_roots(scope.write_roots());
    if let Some(model) = &next.model {
        println!(
            "模型：{}\n地址：{}\n密钥变量名：{}",
            safe_text(&model.model),
            safe_text(&model.endpoint),
            safe_text(&model.api_key_env)
        );
        println!(
            "开始对话后，输入与选中的文件元数据、资源样本会发往该服务；不发送文件正文。设置不会请求模型。"
        );
    } else {
        println!("模型尚未配置；自然语言入口需要模型，可之后再次运行 setup。");
    }
    cancel.check()?;
    let approved = confirm("保存以上设置？")?;
    cancel.check()?;
    if !approved {
        println!("已取消，原配置保留。");
        return Ok(());
    }
    next.save(path)?;
    println!(
        "设置已保存。运行 doctor 检查本地配置；doctor --check-model 验证连接（最多两次模型请求）。"
    );
    if let Some(model) = &next.model
        && !model.api_key_env.is_empty()
    {
        println!(
            "密钥只从 {} 读取；可在同一 PowerShell 终端安全输入：",
            model.api_key_env
        );
        println!(
            "$secret = Read-Host 'API Key' -AsSecureString\n$env:{} = [Net.NetworkCredential]::new('', $secret).Password",
            model.api_key_env
        );
    }
    Ok(())
}

fn show_roots(roots: &[PathBuf]) {
    if roots.is_empty() {
        println!("  未设置");
    }
    for root in roots {
        println!("  {}", display_path(root));
    }
}

fn collect(config: &Config, mut ask: impl FnMut(&str) -> Result<String>) -> Result<Config> {
    println!("当前读取目录：");
    show_roots(&config.read_roots);
    let read_roots = roots("读取", &config.read_roots, &mut ask)?;
    println!("当前可写目录：");
    show_roots(&config.write_roots);
    let write_roots = roots("可写", &config.write_roots, &mut ask)?;
    println!(
        "模型地址应包含完整的 /chat/completions 路径。只输入地址，不要输入密钥；留空保留，- 清除模型。"
    );
    // Do not echo an unvalidated existing endpoint, which may contain credentials.
    if let Some(model) = &config.model
        && model.validate().is_ok()
    {
        println!("当前模型地址：{}", safe_text(&model.endpoint));
    }
    let endpoint = ask("模型完整地址")?;
    let model = if endpoint == "-" {
        None
    } else if endpoint.is_empty() {
        config.model.clone()
    } else {
        let model = ask("模型 ID")?;
        let api_key_env =
            ask("密钥环境变量名（Enter 使用 DAO_SHELL_API_KEY；- 表示本地服务无需密钥）")?;
        Some(ModelConfig {
            endpoint,
            model,
            api_key_env: match api_key_env.as_str() {
                "" => "DAO_SHELL_API_KEY".into(),
                "-" => String::new(),
                _ => api_key_env,
            },
        })
    };
    Scope::new(&read_roots, &write_roots)?;
    if let Some(model) = &model {
        model.validate()?;
    }
    Ok(Config {
        read_roots,
        write_roots,
        model,
    })
}

fn roots(
    kind: &str,
    existing: &[PathBuf],
    ask: &mut impl FnMut(&str) -> Result<String>,
) -> Result<Vec<PathBuf>> {
    let first = ask(&format!("{kind}目录（Enter 保留，- 清空）"))?;
    if first.is_empty() {
        return Ok(existing.to_vec());
    }
    if first == "-" {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    let mut input = first;
    loop {
        let path = files::normalize_existing(Path::new(input.trim_matches('"')))
            .with_context(|| format!("{kind}目录不可用，原配置保留"))?;
        ensure!(path.is_dir(), "范围必须是目录，原配置保留");
        if !paths.contains(&path) {
            paths.push(path);
        }
        ensure!(paths.len() <= 32, "每类最多设置 32 个目录");
        input = ask(&format!("继续添加{kind}目录（Enter 结束）"))?;
        if input.is_empty() {
            break;
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_preserves_on_blank_and_can_replace_missing_roots() {
        let temp = tempfile::tempdir().unwrap();
        let old = Config {
            read_roots: vec![temp.path().join("missing")],
            ..Config::default()
        };
        let mut inputs = vec![
            temp.path().to_string_lossy().into_owned(),
            String::new(),
            String::new(),
            String::new(),
        ]
        .into_iter();
        let next = collect(&old, |_| Ok(inputs.next().unwrap())).unwrap();
        assert_eq!(
            next.read_roots,
            vec![files::normalize_existing(temp.path()).unwrap()]
        );
        assert!(next.write_roots.is_empty());
        assert_eq!(old.read_roots, vec![temp.path().join("missing")]);
        let preserved = collect(&next, |_| Ok(String::new())).unwrap();
        assert_eq!(preserved.read_roots, next.read_roots);
    }

    #[test]
    fn clearing_read_roots_does_not_grant_write_scope() {
        let temp = tempfile::tempdir().unwrap();
        let old = Config {
            read_roots: vec![temp.path().to_owned()],
            ..Config::default()
        };
        let mut inputs = ["-", "", ""].into_iter();
        let next = collect(&old, |_| Ok(inputs.next().unwrap().into())).unwrap();
        assert!(next.read_roots.is_empty() && next.write_roots.is_empty());
    }
}
