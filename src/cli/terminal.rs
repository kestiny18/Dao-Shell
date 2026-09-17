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
    display_path_text(&path.to_string_lossy())
}
fn display_path_text(path: &str) -> String {
    safe_text(path.strip_prefix(r"\\?\").unwrap_or(path))
}
pub(super) fn display_roots(roots: &[std::path::PathBuf]) -> String {
    if roots.is_empty() {
        return "未设置".into();
    }
    roots
        .iter()
        .map(|p| display_path(p))
        .collect::<Vec<_>>()
        .join("；")
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
            render_items(value);
            if value["items"].as_array().is_some_and(Vec::is_empty) {
                println!("本次扫描未找到匹配项。");
            }
            let roots = value["roots"]
                .as_array()
                .map(|roots| {
                    roots
                        .iter()
                        .filter_map(Value::as_str)
                        .map(display_path_text)
                        .collect::<Vec<_>>()
                        .join("；")
                })
                .unwrap_or_default();
            println!(
                "范围：{}；扫描命中 {}，本页 {} 项，{}ms。",
                roots,
                value["matched_in_scan"],
                value["items"].as_array().map_or(0, Vec::len),
                value["elapsed_ms"]
            );
            if value["selection_retained"] == true {
                println!(
                    "候选编号沿用上一组 {} 项，/open 与 /move 仍指向它们；输入 /results 可查看。",
                    value["active_selection"].as_array().map_or(0, Vec::len)
                );
            }
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
        "file_selection" => {
            println!("当前候选编号（上次查询快照，执行前会重新核对）：");
            render_items(value);
            if value["items"].as_array().is_some_and(Vec::is_empty) {
                println!("暂无候选，请先查找。");
            }
        }
        "file_inspect" => render_items(&serde_json::json!({"items":[value]})),
        "file_open" => {
            if value["status"] == "accepted" {
                println!(
                    "已向系统提交打开请求：{}",
                    display_path_text(value["path"].as_str().unwrap_or(""))
                );
                println!("系统已接受；应用是否显示文件需以实际窗口为准。");
            } else if value["status"] == "Cancelled" {
                println!("打开已取消，没有提交请求。");
            } else {
                pretty(value);
            }
        }
        "file_move_batch" => {
            println!("本地回执 {}：{}", value["id"], value["status"]);
            if let Some(items) = value["items"].as_array() {
                for item in items {
                    println!(
                        "  {} → {}：{}\n    {}",
                        display_path_text(item["source"].as_str().unwrap_or("")),
                        display_path_text(item["destination"].as_str().unwrap_or("")),
                        item["status"],
                        safe_text(item["evidence"].as_str().unwrap_or(""))
                    );
                }
            }
            if let Some(dirs) = value["directories"].as_array() {
                for d in dirs {
                    println!(
                        "  创建目录 {}：{}",
                        display_path_text(d["path"].as_str().unwrap_or("")),
                        d["status"]
                    );
                }
            }
        }
        "resource_snapshot" | "process_list" => {
            if let Some(system) = value.get("system") {
                println!(
                    "CPU {}，内存已用 {} / {} GiB（可用 {} GiB）；采样 {:.1} 秒。",
                    percent(&system["cpu_percent"]),
                    gib(&system["memory_used_bytes"]),
                    gib(&system["memory_total_bytes"]),
                    gib(&system["memory_available_bytes"]),
                    value["sample_ms"].as_f64().unwrap_or_default() / 1000.0
                );
                if let Some(disks) = system["disks"].as_array() {
                    for d in disks {
                        println!(
                            "  磁盘 {} 可用 {} / {} GiB",
                            display_path_text(d["mount"].as_str().unwrap_or("未知")),
                            gib(&d["available_bytes"]),
                            gib(&d["total_bytes"])
                        );
                    }
                }
            }
            println!(
                "进程按{}排序；CPU 百分比按整机 100% 计。",
                if value["sort"] == "cpu" {
                    "CPU"
                } else {
                    "内存"
                }
            );
            if let Some(processes) = value["processes"].as_array() {
                for p in processes {
                    println!(
                        "  {}（PID {}）  内存 {}  CPU {}",
                        safe_text(p["name"].as_str().unwrap_or("")),
                        p["pid"],
                        human_bytes(&p["memory_bytes"]),
                        percent(&p["cpu_percent_total"])
                    );
                }
            }
            println!("短时观测；进程列表不能解释全部已用内存，未覆盖 GPU、磁盘 I/O 与历史活动。");
        }
        _ => pretty(value),
    }
}
fn render_items(value: &Value) {
    if let Some(items) = value["items"].as_array() {
        for (i, object) in items.iter().enumerate() {
            let path = object["path"].as_str().unwrap_or("");
            let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
            println!(
                "{}. {}  [{}]  {}\n   {}\n   修改：{}",
                i + 1,
                safe_text(name),
                if object["identity"]["directory"] == true {
                    "目录"
                } else {
                    "文件"
                },
                human_bytes(&object["identity"]["size"]),
                display_path_text(path),
                local_time(&object["modified_at"])
            );
        }
    }
}
fn local_time(value: &Value) -> String {
    value
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| "未知".into())
}
fn percent(value: &Value) -> String {
    match value.as_f64().filter(|n| n.is_finite() && *n >= 0.0) {
        Some(n) if n > 0.0 && n < 0.1 => "<0.1%".into(),
        Some(n) => format!("{n:.1}%"),
        None => "未知".into(),
    }
}
fn human_bytes(value: &Value) -> String {
    let Some(bytes) = value.as_u64() else {
        return "未知".into();
    };
    let mut size = bytes as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut index = 0;
    while size >= 1024.0 && index + 1 < units.len() {
        size /= 1024.0;
        index += 1;
    }
    format!("{size:.1} {}", units[index])
}
fn gib(v: &Value) -> String {
    v.as_u64()
        .map(|n| format!("{:.2}", n as f64 / 1_073_741_824.0))
        .unwrap_or_else(|| "未知".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn user_facing_values_keep_unknowns_and_remove_internal_path_prefixes() {
        assert_eq!(
            display_path_text(r"\\?\E:\试用\文件.pdf"),
            r"E:\试用\文件.pdf"
        );
        assert_eq!(percent(&json!(25.201576232910156)), "25.2%");
        assert_eq!(percent(&json!(0.0047)), "<0.1%");
        assert_eq!(percent(&Value::Null), "未知");
        assert_eq!(human_bytes(&json!(6684555)), "6.4 MiB");
        assert_eq!(human_bytes(&Value::Null), "未知");
        assert!(!display_path_text("file\u{1b}[2J").contains('\u{1b}'));
    }
}
