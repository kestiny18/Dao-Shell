use crate::core::{Cancellation, now};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Disks, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Request {
    pub sample_ms: Option<u64>,
    pub limit: Option<usize>,
    pub filter: Option<String>,
    pub sort: ProcessSort,
}
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSort {
    #[default]
    Memory,
    Cpu,
}
#[derive(Debug, Serialize)]
pub struct Process {
    pub pid: u32,
    pub started_at_unix: Option<u64>,
    pub name: String,
    pub memory_bytes: Option<u64>,
    pub cpu_percent_one_core: Option<f32>,
}

pub fn snapshot(
    request: &Request,
    cancel: &Cancellation,
    processes_only: bool,
) -> Result<serde_json::Value> {
    let sample_ms = request.sample_ms.unwrap_or(2000);
    let limit = request.limit.unwrap_or(10);
    ensure!(
        (500..=5000).contains(&sample_ms) && (1..=50).contains(&limit),
        "采样为 500–5000ms，进程上限 1–50"
    );
    let began_at = now();
    let began = Instant::now();
    let mut system = System::new();
    system.refresh_cpu_usage();
    let refresh = ProcessRefreshKind::nothing().with_memory().with_cpu();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
    let initial: std::collections::HashSet<_> = system
        .processes()
        .values()
        .map(|p| (p.pid(), p.start_time()))
        .collect();
    let interval = Instant::now();
    while interval.elapsed() < Duration::from_millis(sample_ms) {
        cancel.check()?;
        thread::sleep(Duration::from_millis(50));
    }
    system.refresh_cpu_usage();
    system.refresh_memory();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
    cancel.check()?;
    let filter = request.filter.as_deref().unwrap_or("").to_lowercase();
    let mut processes: Vec<_> = system
        .processes()
        .values()
        .filter(|p| p.name().to_string_lossy().to_lowercase().contains(&filter))
        .map(|p| {
            // sysinfo uses zero for inaccessible memory. Expose unknown instead of asserting 0 bytes.
            Process {
                pid: p.pid().as_u32(),
                started_at_unix: (p.start_time() > 0).then_some(p.start_time()),
                name: p.name().to_string_lossy().to_string(),
                memory_bytes: (p.memory() > 0).then_some(p.memory()),
                cpu_percent_one_core: (p.cpu_usage().is_finite()
                    && p.start_time() > 0
                    && initial.contains(&(p.pid(), p.start_time())))
                .then_some(p.cpu_usage()),
            }
        })
        .collect();
    match request.sort {
        ProcessSort::Memory => processes.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes)),
        ProcessSort::Cpu => processes.sort_by(|a, b| {
            b.cpu_percent_one_core
                .unwrap_or(-1.0)
                .total_cmp(&a.cpu_percent_one_core.unwrap_or(-1.0))
        }),
    }
    let matched = processes.len();
    processes.truncate(limit);
    let mut result = serde_json::json!({"verification":"scoped_observation", "started_at":began_at, "observed_at":now(),
        "sample_ms":began.elapsed().as_millis(), "logical_cpus":system.cpus().len(), "processes":processes, "matched_processes":matched,
        "limits":"短时采样；进程 CPU 以单核 100% 计，不能与整机百分比直接比较。缺失值为 null。不包括 GPU、磁盘 I/O、历史活动或未保存状态，不足以确定卡顿根因。"});
    if !processes_only {
        let disks = Disks::new_with_refreshed_list();
        let disks: Vec<_> = disks.iter().map(|d| serde_json::json!({"mount":d.mount_point(),"total_bytes":d.total_space(),"available_bytes":d.available_space()})).collect();
        result["system"] = serde_json::json!({"cpu_percent":(!system.cpus().is_empty()).then_some(system.global_cpu_usage()), "memory_total_bytes":(system.total_memory()>0).then_some(system.total_memory()),
            "memory_used_bytes":(system.total_memory()>0).then_some(system.used_memory()), "memory_available_bytes":(system.total_memory()>0).then_some(system.available_memory()), "disks":disks});
    }
    Ok(result)
}
