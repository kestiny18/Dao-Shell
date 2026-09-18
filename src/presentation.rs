//! Shared formatting for terminal output and model-facing facts.
use serde_json::Value;

pub(crate) fn percent(value: &Value) -> String {
    match value.as_f64().filter(|n| n.is_finite() && *n >= 0.0) {
        Some(n) if n > 0.0 && n < 0.1 => "<0.1%".into(),
        Some(n) => format!("{n:.1}%"),
        None => "未知".into(),
    }
}
pub(crate) fn human_bytes(value: &Value) -> String {
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
pub(crate) fn gib(value: &Value) -> String {
    value
        .as_u64()
        .map(|n| format!("{:.2}", n as f64 / 1_073_741_824.0))
        .unwrap_or_else(|| "未知".into())
}
