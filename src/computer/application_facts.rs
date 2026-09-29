//! Conservative, read-only association. Never executes or resolves registry strings.
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn executable_path(value: &str) -> Option<String> {
    let value = value.trim();
    let value = match value.rsplit_once(',') {
        Some((path, index)) if index.trim().parse::<i32>().is_ok() => path.trim(),
        _ => value,
    };
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let path = value.replace('/', "\\").to_lowercase();
    let bytes = path.as_bytes();
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
        || path[3..].contains([':', '"', '\0'])
        || !path.ends_with(".exe")
        || path[3..]
            .split('\\')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(path)
}

pub(super) fn annotate_running(
    items: &mut [Value],
    sources: &HashMap<usize, String>,
    processes: &[String],
) {
    let registered: HashMap<_, _> = sources
        .iter()
        .filter_map(|(i, p)| executable_path(p).map(|p| (*i, p)))
        .collect();
    let mut counts = HashMap::new();
    for path in registered.values() {
        *counts.entry(path).or_insert(0) += 1;
    }
    let running: HashSet<_> = processes
        .iter()
        .filter_map(|p| executable_path(p))
        .collect();
    for (index, item) in items.iter_mut().enumerate() {
        let confirmed = registered
            .get(&index)
            .is_some_and(|path| counts.get(path) == Some(&1) && running.contains(path));
        item["running"] = if confirmed { json!(true) } else { Value::Null };
    }
    items.sort_by_key(|item| {
        (
            item["running"] != true,
            item["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_paths_only_and_unknown_is_not_false() {
        let mut items = vec![
            json!({"name":"A"}),
            json!({"name":"B"}),
            json!({"name":"C"}),
        ];
        let sources = HashMap::from([
            (0, r#""C:\Apps\A\app.exe",0"#.into()),
            (1, r"C:\Apps\B\app.exe".into()),
        ]);
        annotate_running(
            &mut items,
            &sources,
            &[
                r"c:/apps/b/APP.exe".into(),
                r"C:\Apps\A\app-helper.exe".into(),
            ],
        );
        assert_eq!(items[0]["name"], "B");
        assert_eq!(items[0]["running"], true);
        assert!(items[1]["running"].is_null());
        assert!(items[2]["running"].is_null());
    }
    #[test]
    fn shared_executable_is_ambiguous_and_order_is_stable() {
        let mut items = vec![
            json!({"name":"B"}),
            json!({"name":"A","version":"1"}),
            json!({"name":"A","version":"2"}),
        ];
        let sources = HashMap::from([
            (0, r"C:\Shared\app.exe".into()),
            (1, r"C:\Shared\app.exe".into()),
        ]);
        annotate_running(&mut items, &sources, &[r"C:\Shared\app.exe".into()]);
        assert!(items.iter().all(|i| i["running"].is_null()));
        assert_eq!(items[0]["version"], "1");
        assert_eq!(items[1]["version"], "2");
    }
    #[test]
    fn rejects_relative_network_shell_and_non_executable_sources() {
        for path in [
            r"app.exe",
            r"\\server\app.exe",
            r"C:\Apps\..\app.exe",
            r"C:\Apps\icon.dll",
            r"C:\app.exe --arg",
            r"C:\app.exe:stream.exe",
            r"%LOCALAPPDATA%\app.exe",
        ] {
            assert!(executable_path(path).is_none(), "{path}");
        }
    }
}
