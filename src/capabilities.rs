//! Tool schemas; execution is owned by runtime.
pub use crate::runtime::{Interaction, Runtime};
use serde_json::{Value, json};

fn function(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function", "function":{"name":name,"description":description,
        "parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}
pub fn definitions() -> Vec<Value> {
    let reference =
        json!({"object_id":{"type":"string","description":"本次会话真实查询结果的 id，不能捏造"}});
    let resources = json!({"sample_ms":{"type":"integer","minimum":500,"maximum":5000},
        "limit":{"type":"integer","minimum":1,"maximum":50},"filter":{"type":"string"},"sort":{"type":"string","enum":["cpu","memory"]}});
    vec![
        function(
            "file_search",
            "在授权范围内查文件名、相对路径和元数据，不读正文。优先使用 terms + extension + kind 一次表达需求（如 terms=[客户端,初始化], extension=sql, kind=file）。默认按相关度排序，文件名命中优先于父目录及更远路径。返回当前候选编号；每次搜索替换候选，零结果清空。",
            json!({
            "query":{"type":"string","description":"文件名包含的关键词；空字符串匹配全部"},
            "terms":{"type":"array","maxItems":12,"items":{"type":"string","minLength":1,"maxLength":128},"description":"相对路径中的关键词；与 query 二选一。不要把已用扩展名表达的文件类型重复作为必需词"},
            "match_mode":{"type":"string","enum":["all","any"],"description":"terms 默认 all（都命中）；any 表示任一命中"},
            "kind":{"type":"string","enum":["any","file","directory"]},
            "directory":{"type":"string","description":"已授权范围内的绝对目录；省略则查询已配置根目录"},
            "extension":{"type":"string","description":"如 pdf；不带通配符"},
            "modified_after":{"type":"string","description":"RFC3339，含时区，闭区间下界"},
            "modified_before":{"type":"string","description":"RFC3339，含时区，开区间上界"},
            "min_bytes":{"type":"integer","minimum":0},"max_bytes":{"type":"integer","minimum":0},
            "sort":{"type":"string","enum":["relevance","modified_desc","name","size_desc"]},
            "page":{"type":"integer","minimum":0,"maximum":100},"limit":{"type":"integer","minimum":1,"maximum":50}}),
            &[],
        ),
        function(
            "file_inspect",
            "复核一个已观察对象的元数据；对象变化会报错，需要重新查找。",
            reference.clone(),
            &["object_id"],
        ),
        function(
            "file_open",
            "请求本地用户选择打开具体文件或目录。仅返回系统是否接受请求，不保证窗口可见。",
            reference,
            &["object_id"],
        ),
        function(
            "file_move_batch",
            "准备并展示不可变移动方案，由本地用户确认后执行并逐项核验；1–20 个普通文件，同卷，不覆盖。未知结果禁止重试。",
            json!({
            "object_ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":20,"uniqueItems":true},
            "destination":{"type":"string","description":"写入范围内的绝对目的目录，允许创建方案中明确列出的子目录"}}),
            &["object_ids", "destination"],
        ),
        function(
            "resource_snapshot",
            "采集短时间的整机 CPU、内存、磁盘可用空间和进程；只读，不是卡顿根因诊断。",
            resources.clone(),
            &[],
        ),
        function(
            "process_list",
            "短时采样并列出进程；只读，不结束进程，不查询命令行或环境变量。",
            resources,
            &[],
        ),
    ]
}

/// One desktop allowlist used by schema selection and the execution boundary.
pub fn desktop_allowed(name: &str) -> bool {
    matches!(name, "file_search" | "file_inspect" | "file_open")
        || crate::observation::is_tool(name)
        || crate::application_actions::is_tool(name)
}
pub fn desktop_definitions() -> Vec<Value> {
    let mut tools: Vec<_> = definitions()
        .into_iter()
        .filter(|t| {
            matches!(
                t["function"]["name"].as_str(),
                Some("file_search" | "file_inspect" | "file_open")
            )
        })
        .collect();
    for (name, description, mut properties) in [
        (
            "resource_snapshot",
            "Read CPU/memory/volume space with both rankings from one sample. Reuse observation_id for follow-up; omit it for current state.",
            json!({"sections":{"type":"array","items":{"type":"string","enum":["cpu","memory","disks"]}},"sample_ms":{"type":"integer","minimum":500,"maximum":5000},"limit":{"type":"integer","minimum":1,"maximum":50}}),
        ),
        (
            "process_list",
            "Read process page sorted by cpu/memory/name. Keep filters on cursor pages. Row references are separate from file candidates.",
            json!({"filter":{"type":"string","maxLength":128},"sort":{"type":"string","enum":["cpu","memory","name"]},"sample_ms":{"type":"integer","minimum":500,"maximum":5000},"limit":{"type":"integer","minimum":1,"maximum":50},"cursor":{"type":"string"}}),
        ),
        (
            "network_snapshot",
            "Read interface rates and IP configuration presence. Does not test internet connectivity.",
            json!({"sample_ms":{"type":"integer","minimum":500,"maximum":5000}}),
        ),
        (
            "app_list",
            "Filter registered applications by name/publisher substring. running includes only conservative path matches; no match means unknown, not stopped or uninstalled.",
            json!({"name":{"type":"string","maxLength":128},"publisher":{"type":"string","maxLength":128},"view":{"type":"string","enum":["installed","running"]},"limit":{"type":"integer","minimum":1,"maximum":50},"cursor":{"type":"string"},"refresh":{"type":"boolean"}}),
        ),
        (
            "computer_profile",
            "Read hardware configuration, architecture and current uptime.",
            json!({"refresh":{"type":"boolean"}}),
        ),
    ] {
        properties["observation_id"] = json!({"type":"string"});
        tools.push(function(name, description, properties, &[]));
    }
    tools.push(function("resource_observe","Observe 30/60 seconds, no automatic action. Only selected domains are collected. Local progress, one deterministic summary; missing values are not zero; correlations are not causes.",json!({"domains":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"string","enum":["cpu","memory","network","io","gpu","power"]}},"duration_s":{"type":"integer","enum":[30,60]},"interval_ms":{"type":"integer","enum":[1000,2000]},"process_refs":{"type":"array","maxItems":5,"items":{"type":"string"}}}),&["domains","duration_s"]));
    tools.push(function("observation_compare","Compare two explicit current-session series; incompatible identities/definitions or insufficient points cannot support a trend.",json!({"left_id":{"type":"string"},"right_id":{"type":"string"},"metrics":{"type":"array","maxItems":128,"items":{"type":"string"}}}),&["left_id","right_id"]));
    tools.push(function("network_diagnose","Read local configuration or probe one-use endpoints previously confirmed LOCALLY. Never create a URL or target ref; sharing does not authorize networking. DNS/TCP/HTTPS evidence is separate; direct path is not system-proxy health.",json!({"mode":{"type":"string","enum":["local","probe"]},"endpoint_refs":{"type":"array","maxItems":3,"items":{"type":"string"}},"checks":{"type":"array","maxItems":3,"items":{"type":"string","enum":["dns","tcp","https"]}}}),&["mode"]));
    tools.push(function("startup_list","Read Run/RunOnce and Startup folder registrations only, never execute command strings; enabled state unknown.",json!({"source":{"type":"string","enum":["user","machine"]},"filter":{"type":"string","maxLength":128},"limit":{"type":"integer","minimum":1,"maximum":50},"cursor":{"type":"string"}}),&["source"]));
    tools.push(function("app_targets","Find supported application launch entries or exact current-user process instances. kind close is currently unavailable (close_identity_not_atomic); never substitute termination. Old observation refs require explicit reselection. Multiple matches require the user's choice. query is a name substring, never a command.",json!({"kind":{"type":"string","enum":["launch","close","terminate"]},"query":{"type":"string","maxLength":128},"observation_id":{"type":"string"},"row_ref":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":50},"cursor":{"type":"string"}}),&["kind"]));
    for (tool, description) in [
        (
            "app_launch",
            "Prepare ordinary launch from a supported target and wait for LOCAL user confirmation. One state-changing action per turn. Created process does not prove a visible/ready application.",
        ),
        (
            "app_close",
            "Normal close currently unavailable: no verified atomic window-identity dispatch provider. Do not fall back to force termination.",
        ),
        (
            "process_terminate",
            "Prepare force termination of ONE exact instance only when user explicitly requests force termination; unsaved data can be lost. Wait for LOCAL confirmation; never kill by name or process tree.",
        ),
    ] {
        tools.push(function(
            tool,
            description,
            json!({"target_ref":{"type":"string"}}),
            &["target_ref"],
        ));
    }
    tools
}
