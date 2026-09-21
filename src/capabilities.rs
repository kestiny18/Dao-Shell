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
