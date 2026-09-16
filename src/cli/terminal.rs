use crate::{
    capabilities::Interaction,
    core::{FileObject, Operation, safe_text},
};
use anyhow::Result;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};

pub struct Terminal;
impl Interaction for Terminal {
    fn confirm_move(&mut self, operation: &Operation) -> Result<bool> {
        println!(
            "\n准备移动 {} 个文件（操作 {}）：",
            operation.items.len(),
            operation.id
        );
        for item in &operation.items {
            println!(
                "  {}\n    → {}",
                display_path(&item.source),
                display_path(&item.destination)
            );
        }
        for directory in &operation.directories {
            println!("  将创建目录：{}", display_path(&directory.path));
        }
        confirm("执行这个方案？输入 y 执行，其他输入取消")
    }
    fn confirm_open(&mut self, object: &FileObject) -> Result<bool> {
        println!("\n打开：{}", display_path(&object.path));
        confirm("输入 y 打开，其他输入取消")
    }
    fn progress(&mut self, text: &str) {
        eprintln!("{}", safe_text(text));
    }
    fn result(&mut self, capability: &str, value: &Value) {
        render(capability, value);
    }
}
pub fn display_path(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    safe_text(path.strip_prefix(r"\\?\").unwrap_or(&path))
}
pub(super) fn confirm(prompt: &str) -> Result<bool> {
    // Piped model output or redirected stdin cannot become a confirmation click.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("当前不是交互终端，方案已取消；请在终端内执行确认。");
        return Ok(false);
    }
    print!("{prompt} [y/N] > ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}
pub(super) fn pretty(value: &Value) {
    let serialized = serde_json::to_string_pretty(value).unwrap_or_default();
    let escaped: String = serialized.chars().map(|c| {
        if matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            format!("\\u{:04x}", c as u32)
        } else { c.to_string() }
    }).collect();
    println!("{escaped}");
}
pub fn render(capability: &str, value: &Value) {
    match capability {
        "file_search" => {
            if let Some(items) = value["items"].as_array() {
                for (i, object) in items.iter().enumerate() {
                    println!(
                        "{}. {}  [{}]  {} bytes\n   {}",
                        i + 1,
                        safe_text(object["path"].as_str().unwrap_or("")),
                        if object["identity"]["directory"] == true {
                            "目录"
                        } else {
                            "文件"
                        },
                        object["identity"]["size"],
                        safe_text(object["modified_at"].as_str().unwrap_or("时间未知"))
                    );
                }
                if items.is_empty() {
                    println!("本次扫描未找到匹配项。");
                }
            }
            println!(
                "范围：{}；扫描命中 {}，本页 {} 项，{}ms。",
                value["roots"],
                value["matched_in_scan"],
                value["items"].as_array().map_or(0, Vec::len),
                value["elapsed_ms"]
            );
            if value["has_more"] == true {
                println!("还有其他结果，可继续翻页。");
            }
            if value["truncated"] == true || value["skipped"] != 0 {
                println!(
                    "扫描覆盖不完整（截断 {}，跳过 {}）；不能据此断言目录内没有其他结果。",
                    value["truncated"], value["skipped"]
                );
            }
        }
        "file_move_batch" => {
            println!("本地回执 {}：{}", value["id"], value["status"]);
            if let Some(items) = value["items"].as_array() {
                for item in items {
                    println!(
                        "  {} → {}：{}\n    {}",
                        safe_text(item["source"].as_str().unwrap_or("")),
                        safe_text(item["destination"].as_str().unwrap_or("")),
                        item["status"],
                        safe_text(item["evidence"].as_str().unwrap_or(""))
                    );
                }
            }
            if let Some(dirs) = value["directories"].as_array() {
                for d in dirs {
                    println!(
                        "  创建目录 {}：{}",
                        safe_text(d["path"].as_str().unwrap_or("")),
                        d["status"]
                    );
                }
            }
        }
        "resource_snapshot" | "process_list" => {
            if let Some(system) = value.get("system") {
                println!(
                    "CPU {}%，内存已用 {} / {} GiB；采样 {}ms。",
                    system["cpu_percent"],
                    gib(&system["memory_used_bytes"]),
                    gib(&system["memory_total_bytes"]),
                    value["sample_ms"]
                );
                if let Some(disks) = system["disks"].as_array() {
                    for d in disks {
                        println!(
                            "  磁盘 {} 可用 {} / {} GiB",
                            d["mount"],
                            gib(&d["available_bytes"]),
                            gib(&d["total_bytes"])
                        );
                    }
                }
            }
            if let Some(processes) = value["processes"].as_array() {
                for p in processes {
                    println!(
                        "  PID {}  {}  内存 {} GiB  CPU {}%（单核）",
                        p["pid"],
                        safe_text(p["name"].as_str().unwrap_or("")),
                        gib(&p["memory_bytes"]),
                        p["cpu_percent_one_core"]
                    );
                }
            }
            println!("{}", safe_text(value["limits"].as_str().unwrap_or("")));
        }
        _ => pretty(value),
    }
}
fn gib(v: &Value) -> String {
    v.as_u64()
        .map(|n| format!("{:.2}", n as f64 / 1_073_741_824.0))
        .unwrap_or_else(|| "未知".into())
}
