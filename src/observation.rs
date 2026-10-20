//! Bounded, session-owned read-only observations. Local data never goes straight to a model.
use crate::{
    config::ComputerQuerySharing,
    core::{Cancellation, now},
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

pub fn is_tool(name: &str) -> bool {
    matches!(
        name,
        "resource_snapshot" | "process_list" | "network_snapshot" | "app_list" | "computer_profile"
    )
}

/// Provider diagnostics stay local; only bounded public error codes enter dialogue.
pub fn error_code(error: &anyhow::Error, cancel: &Cancellation) -> &'static str {
    if cancel.is_cancelled() {
        return "cancelled";
    }
    match error.to_string().as_str() {
        "invalid_arguments" => "invalid_arguments",
        "observation_expired" => "observation_expired",
        "unsupported" => "unsupported",
        "timeout" => "timeout",
        "access_denied" => "access_denied",
        "observation_budget_exceeded" => "observation_budget_exceeded",
        _ => "internal_error",
    }
}

#[derive(Default)]
pub struct Store {
    pub desktop: bool,
    pub sharing: ComputerQuerySharing,
    entries: VecDeque<Entry>,
    cursors: HashMap<String, (String, String, usize)>,
    pub delivered: Vec<Value>,
}
struct Entry {
    id: String,
    tool: String,
    value: Value,
    created: Instant,
    bytes: usize,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Request {
    sections: Option<Vec<String>>,
    sample_ms: Option<u64>,
    limit: Option<usize>,
    filter: Option<String>,
    sort: Option<String>,
    name: Option<String>,
    publisher: Option<String>,
    view: Option<String>,
    cursor: Option<String>,
    refresh: bool,
    observation_id: Option<String>,
}

impl Store {
    pub fn catalog(&mut self) -> Value {
        self.prune();
        json!(self.entries.iter().map(|e| json!({"observation_id":e.id,"kind":e.tool,"observed_at":e.value["observed_at"]})).collect::<Vec<_>>())
    }
    pub fn reset(&mut self) {
        self.entries.clear();
        self.cursors.clear();
        self.delivered.clear();
    }
    fn prune(&mut self) {
        self.entries
            .retain(|e| e.created.elapsed() < Duration::from_secs(600));
        while self.entries.len() > 12
            || self.entries.iter().map(|e| e.bytes).sum::<usize>() > 8 * 1024 * 1024
        {
            self.entries.pop_front();
        }
        self.cursors
            .retain(|_, (id, _, _)| self.entries.iter().any(|e| &e.id == id));
    }
    pub fn query(&mut self, tool: &str, arguments: Value, cancel: &Cancellation) -> Result<Value> {
        ensure!(is_tool(tool), "unsupported");
        // Validate the selected schema as well as the union's strict decoder.
        let schema = crate::capabilities::desktop_definitions()
            .into_iter()
            .find(|s| s["function"]["name"] == tool)
            .unwrap();
        ensure!(arguments.is_object(), "invalid_arguments");
        ensure!(
            arguments.as_object().unwrap().keys().all(|k| {
                schema["function"]["parameters"]["properties"]
                    .get(k)
                    .is_some()
            }),
            "invalid_arguments"
        );
        let local_arguments = arguments.clone();
        let r: Request =
            serde_json::from_value(arguments).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        let limit = r.limit.unwrap_or(10);
        let sample = r.sample_ms.unwrap_or(700);
        ensure!(
            (1..=50).contains(&limit) && (500..=5000).contains(&sample),
            "invalid_arguments"
        );
        for text in [&r.filter, &r.name, &r.publisher].into_iter().flatten() {
            ensure!(text.chars().count() <= 128, "invalid_arguments");
        }
        ensure!(
            r.sort
                .as_deref()
                .is_none_or(|s| matches!(s, "cpu" | "memory" | "name")),
            "invalid_arguments"
        );
        ensure!(
            r.view
                .as_deref()
                .is_none_or(|s| matches!(s, "installed" | "running")),
            "invalid_arguments"
        );
        let sections = r
            .sections
            .clone()
            .unwrap_or_else(|| vec!["cpu".into(), "memory".into(), "disks".into()]);
        ensure!(
            !sections.is_empty()
                && sections.len() <= 3
                && sections
                    .iter()
                    .all(|s| matches!(s.as_str(), "cpu" | "memory" | "disks")),
            "invalid_arguments"
        );
        ensure!(
            (r.observation_id.is_none() && r.cursor.is_none())
                || (!r.refresh && r.sample_ms.is_none()),
            "invalid_arguments"
        );
        self.prune();
        cancel.check()?;
        let signature = json!({"tool":tool,"filter":r.filter,"sort":r.sort,"name":r.name,"publisher":r.publisher,"view":r.view,"limit":limit}).to_string();
        let (requested_id, offset) = if let Some(cursor) = &r.cursor {
            let (id, sig, offset) = self
                .cursors
                .get(cursor)
                .ok_or_else(|| anyhow::anyhow!("observation_expired"))?;
            ensure!(
                sig == &signature && r.observation_id.as_ref().is_none_or(|other| other == id),
                "invalid_arguments"
            );
            (Some(id.clone()), *offset)
        } else {
            (r.observation_id.clone(), 0)
        };
        let reused = requested_id.is_some();
        let mut value = if let Some(id) = requested_id {
            self.entries
                .iter()
                .find(|e| e.id == id && e.tool == tool)
                .ok_or_else(|| anyhow::anyhow!("observation_expired"))?
                .value
                .clone()
        } else {
            let began = Instant::now();
            let mut collected = began;
            let data = match tool {
                "resource_snapshot" | "process_list" => crate::resources::query_snapshot(
                    sample,
                    &sections,
                    cancel,
                    tool == "process_list",
                )?,
                "network_snapshot" => network(sample, cancel)?,
                "app_list" => {
                    // Static directory cache never supplies running evidence.
                    let cached = if !r.refresh && r.view.as_deref() != Some("running") {
                        self.entries
                            .iter()
                            .rev()
                            .find(|e| {
                                e.tool == tool
                                    && e.value["data"]["view"] == "installed"
                                    && e.created.elapsed() < Duration::from_secs(60)
                            })
                            .map(|e| (e.value.clone(), e.created))
                    } else {
                        None
                    };
                    let mut data = if let Some((v, at)) = cached {
                        collected = at;
                        let mut d = v["data"].clone();
                        d["cached"] = json!(true);
                        d
                    } else {
                        crate::computer::applications(
                            cancel,
                            false,
                            r.view.as_deref() == Some("running"),
                        )?
                    };
                    data["view"] = json!(r.view.as_deref().unwrap_or("installed"));
                    data
                }
                "computer_profile" => {
                    let old = if !r.refresh {
                        self.entries
                            .iter()
                            .rev()
                            .find(|e| {
                                e.tool == tool && e.created.elapsed() < Duration::from_secs(300)
                            })
                            .map(|e| (e.value["data"].clone(), e.created))
                    } else {
                        None
                    };
                    let mut d = if let Some((mut d, at)) = old {
                        collected = at;
                        d["cached"] = json!(true);
                        d
                    } else {
                        serde_json::to_value(crate::profile::collect(cancel)?)?
                    };
                    d["architecture"] = json!(sysinfo::System::cpu_arch());
                    d["uptime_seconds"] = json!(sysinfo::System::uptime());
                    d["uptime_observed_at"] = json!(now());
                    d
                }
                _ => unreachable!(),
            };
            ensure!(data["available"] != false, "unsupported");
            cancel.check()?;
            ensure!(began.elapsed() < Duration::from_secs(10), "timeout");
            let partial = data["partial"] == true
                || data["matched_processes"].as_u64().is_some_and(|n| n > 2048);
            let mut value = json!({"schema_version":1,"observation_id":uuid::Uuid::new_v4().to_string(),"kind":tool,"status":if partial {"partial"} else {"ok"},"observed_at":data["observed_at"],"sample_window":if data.get("started_at").is_some() {json!({"started_at":data["started_at"],"ended_at":data["observed_at"],"elapsed_ms":data["sample_ms"]})} else {Value::Null},"source":"local_windows","coverage":{"state":"partial","reasons":["provider_scoped_observation"]},"cached":data["cached"] == true,"limits":data["limits"],"data":data,"page":null});
            let id = value["observation_id"].as_str().unwrap().to_owned();
            for key in ["processes", "items"] {
                if let Some(rows) = value["data"][key].as_array_mut() {
                    for (i, row) in rows.iter_mut().enumerate() {
                        row["row_ref"] = json!(format!("{id}:{}", i + 1));
                    }
                }
            }
            let references: HashMap<String, Value> = value["data"]["processes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| (p["pid"].to_string(), p["row_ref"].clone()))
                .collect();
            for sort in ["memory", "cpu"] {
                if let Some(rows) = value["data"]["process_rankings"][sort].as_array_mut() {
                    for row in rows {
                        if let Some(reference) = references.get(&row["pid"].to_string()) {
                            row["row_ref"] = reference.clone();
                        }
                    }
                }
            }
            let bytes = serde_json::to_vec(&value)?.len();
            ensure!(bytes <= 8 * 1024 * 1024, "observation_budget_exceeded");
            self.entries.push_back(Entry {
                id: value["observation_id"].as_str().unwrap().into(),
                tool: tool.into(),
                value: value.clone(),
                created: collected,
                bytes,
            });
            self.prune();
            value
        };
        value["cached"] = json!(reused || value["cached"] == true);
        let key = if tool == "process_list" {
            Some("processes")
        } else if tool == "app_list" {
            Some("items")
        } else {
            None
        };
        if let Some(key) = key {
            let mut rows = value["data"][key].as_array().cloned().unwrap_or_default();
            ensure!(
                tool != "app_list"
                    || r.view
                        .as_deref()
                        .is_none_or(|v| v != "running" || value["data"]["view"] == "running"),
                "invalid_arguments"
            );
            rows.retain(|p| {
                let contains = |key: &str, needle: &Option<String>| {
                    needle.as_deref().is_none_or(|n| {
                        p[key]
                            .as_str()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&n.to_lowercase())
                    })
                };
                contains(
                    "name",
                    if tool == "app_list" {
                        &r.name
                    } else {
                        &r.filter
                    },
                ) && contains("publisher", &r.publisher)
                    && (r.view.as_deref() != Some("running") || p["running"] == true)
            });
            if tool == "process_list" {
                rows.sort_by(|a, b| match r.sort.as_deref().unwrap_or("memory") {
                    "name" => a["name"].as_str().cmp(&b["name"].as_str()),
                    "cpu" => b["cpu_percent_total"]
                        .as_f64()
                        .unwrap_or(-1.)
                        .total_cmp(&a["cpu_percent_total"].as_f64().unwrap_or(-1.)),
                    _ => b["memory_bytes"].as_u64().cmp(&a["memory_bytes"].as_u64()),
                });
            }
            let matched = rows.len();
            let page: Vec<_> = rows.into_iter().skip(offset).take(limit).collect();
            let next = if offset + page.len() < matched {
                let token = uuid::Uuid::new_v4().to_string();
                ensure!(self.cursors.len() < 1024, "observation_budget_exceeded");
                self.cursors.insert(
                    token.clone(),
                    (
                        value["observation_id"].as_str().unwrap().into(),
                        signature,
                        offset + page.len(),
                    ),
                );
                Some(token)
            } else {
                None
            };
            value["page"] =
                json!({"returned":page.len(),"matched":matched,"offset":offset,"next_cursor":next});
            value["data"][key] = json!(page);
        }
        if tool == "resource_snapshot" {
            if let Some(system) = value["data"]["system"].as_object_mut() {
                if !sections.iter().any(|s| s == "cpu") {
                    system.remove("cpu_percent");
                }
                if !sections.iter().any(|s| s == "memory") {
                    for key in [
                        "memory_total_bytes",
                        "memory_used_bytes",
                        "memory_available_bytes",
                    ] {
                        system.remove(key);
                    }
                }
                if !sections.iter().any(|s| s == "disks") {
                    system.remove("disks");
                }
            }
            // Rankings are independent pages over the same sampled process set.
            if let Some(rankings) = value["data"]["process_rankings"].as_object_mut() {
                for rows in rankings.values_mut() {
                    if let Some(rows) = rows.as_array_mut() {
                        rows.truncate(limit);
                    }
                }
            }
            value["data"].as_object_mut().unwrap().remove("processes");
        }
        if matches!(tool, "resource_snapshot" | "process_list") {
            value["data"]["facts_for_explanation"] =
                crate::resources::explanation_facts(&value["data"]);
        }
        value["query"] = json!({"tool":tool,"arguments":local_arguments});
        self.delivered.push(value.clone());
        Ok(value)
    }
}

fn rate(before: Option<u64>, after: u64, elapsed: f64) -> Option<f64> {
    before
        .and_then(|b| after.checked_sub(b))
        .filter(|_| elapsed > 0.)
        .map(|n| n as f64 / elapsed)
}
fn network(sample: u64, cancel: &Cancellation) -> Result<Value> {
    let mut networks = sysinfo::Networks::new_with_refreshed_list();
    let before: HashMap<_, _> = networks
        .iter()
        .map(|(name, d)| (name.clone(), (d.total_received(), d.total_transmitted())))
        .collect();
    let began_at = now();
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(sample) {
        cancel.check()?;
        std::thread::sleep(Duration::from_millis(50));
    }
    networks.refresh(true);
    cancel.check()?;
    let elapsed = began.elapsed().as_secs_f64();
    let adapters: Vec<_> = networks.iter().enumerate().map(|(i,(name,d))| json!({"label":format!("Interface {}",i+1),"name":name,"has_ip":!d.ip_networks().is_empty(),"received_bytes_per_second":rate(before.get(name).map(|n| n.0),d.total_received(),elapsed),"transmitted_bytes_per_second":rate(before.get(name).map(|n| n.1),d.total_transmitted(),elapsed)})).collect();
    let changed = before.len() != adapters.len()
        || adapters.iter().any(|a| {
            a["received_bytes_per_second"].is_null() || a["transmitted_bytes_per_second"].is_null()
        });
    Ok(
        json!({"adapters":adapters,"observed_at":now(),"started_at":began_at,"sample_ms":began.elapsed().as_millis(),"partial":changed,"limits":"Short interface-counter sample; IP configured does not prove internet connectivity. No active probes."}),
    )
}

/// Reconstruct, never blacklist: neither paths, PIDs, actions nor arbitrary provider fields escape.
pub fn model_projection(local: &Value) -> Value {
    fn fields(v: &Value, keys: &[&str]) -> Value {
        let mut result = json!({});
        for k in keys {
            if let Some(value) = v.get(*k) {
                result[*k] = clean(value);
            }
        }
        result
    }
    fn clean(value: &Value) -> Value {
        match value {
            Value::String(s) => json!(
                s.chars()
                    .filter(|c| !c.is_control())
                    .take(256)
                    .collect::<String>()
            ),
            Value::Array(a) => json!(a.iter().take(50).map(clean).collect::<Vec<_>>()),
            Value::Object(o) => {
                Value::Object(o.iter().map(|(k, v)| (k.clone(), clean(v))).collect())
            }
            _ => value.clone(),
        }
    }
    let mut out = fields(
        local,
        &[
            "schema_version",
            "observation_id",
            "kind",
            "status",
            "error",
            "observed_at",
            "sample_window",
            "source",
            "coverage",
            "cached",
            "limits",
            "page",
        ],
    );
    let d = &local["data"];
    let row_keys = [
        "row_ref",
        "name",
        "memory_bytes",
        "cpu_percent_total",
        "cpu_percent_one_core",
        "version",
        "publisher",
        "estimated_size_bytes",
        "running",
        "running_instances",
    ];
    let mut data = json!({});
    match local["kind"].as_str().unwrap_or("") {
        "resource_snapshot" | "process_list" => {
            data["system"] = fields(
                &d["system"],
                &[
                    "cpu_percent",
                    "memory_total_bytes",
                    "memory_used_bytes",
                    "memory_available_bytes",
                ],
            );
            data["system"]["disks"] = json!(
                d["system"]["disks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(50)
                    .map(|disk| {
                        let mut v = fields(disk, &["total_bytes", "available_bytes"]);
                        let mount = disk["mount"].as_str().unwrap_or("");
                        v["volume"] = json!(if mount.as_bytes().get(1) == Some(&b':') {
                            mount.chars().take(2).collect::<String>()
                        } else {
                            "Volume".into()
                        });
                        v
                    })
                    .collect::<Vec<_>>()
            );
            data["processes"] = json!(
                d["processes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(50)
                    .map(|r| fields(r, &row_keys))
                    .collect::<Vec<_>>()
            );
            for sort in ["cpu", "memory"] {
                data["process_rankings"][sort] = json!(
                    d["process_rankings"][sort]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .take(50)
                        .map(|r| fields(r, &row_keys))
                        .collect::<Vec<_>>()
                );
            }
            data["observed_at"] = local["observed_at"].clone();
            data["facts_for_explanation"] = crate::resources::explanation_facts(&data);
            // explanation formatter includes optional PID/mount slots, with null values only.
        }
        "network_snapshot" => {
            data["adapters"] = json!(
                d["adapters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(50)
                    .map(|r| fields(
                        r,
                        &[
                            "label",
                            "has_ip",
                            "received_bytes_per_second",
                            "transmitted_bytes_per_second"
                        ]
                    ))
                    .collect::<Vec<_>>()
            )
        }
        "app_list" => {
            data["items"] = json!(
                d["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(50)
                    .map(|r| fields(r, &row_keys))
                    .collect::<Vec<_>>()
            )
        }
        "computer_profile" => {
            data = fields(
                d,
                &[
                    "os",
                    "cpus",
                    "graphics",
                    "memory_bytes",
                    "architecture",
                    "uptime_seconds",
                    "uptime_observed_at",
                    "observed_at",
                ],
            )
        }
        _ => (),
    }
    out["data"] = data;
    // Truncate complete rows, never JSON bytes. Fixed metric fields are small.
    while serde_json::to_vec(&out).is_ok_and(|b| b.len() > 32 * 1024) {
        let mut removed = false;
        for key in ["items", "processes", "adapters"] {
            if let Some(a) = out["data"][key].as_array_mut() {
                removed |= a.pop().is_some();
            }
        }
        out["projection_truncated"] = json!(true);
        if !removed {
            out["data"] = json!({"projection_truncated":true});
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(s: &mut Store, id: &str, tool: &str, data: Value, age: u64) {
        let value = json!({"observation_id":id,"kind":tool,"observed_at":"2026-10-20T00:00:00Z","data":data});
        let bytes = serde_json::to_vec(&value).unwrap().len();
        s.entries.push_back(Entry {
            id: id.into(),
            tool: tool.into(),
            value,
            created: Instant::now() - Duration::from_secs(age),
            bytes,
        });
    }
    #[test]
    fn pagination_is_session_filter_bound_and_row_references_do_not_change() {
        let mut s = Store::default();
        let cancel = Cancellation::default();
        fixture(
            &mut s,
            "sample",
            "process_list",
            json!({"processes":[{"name":"a","memory_bytes":1,"row_ref":"sample:1"},{"name":"b","memory_bytes":2,"row_ref":"sample:2"},{"name":"c","memory_bytes":3,"row_ref":"sample:3"}]}),
            0,
        );
        let first = s
            .query(
                "process_list",
                json!({"observation_id":"sample","sort":"name","limit":1}),
                &cancel,
            )
            .unwrap();
        let cursor = first["page"]["next_cursor"].as_str().unwrap();
        let next = s
            .query(
                "process_list",
                json!({"cursor":cursor,"sort":"name","limit":1}),
                &cancel,
            )
            .unwrap();
        assert_eq!(next["data"]["processes"][0]["row_ref"], "sample:2");
        assert!(
            s.query(
                "process_list",
                json!({"cursor":cursor,"sort":"memory","limit":1}),
                &cancel
            )
            .is_err()
        );
        assert!(
            Store::default()
                .query(
                    "process_list",
                    json!({"cursor":cursor,"sort":"name","limit":1}),
                    &cancel
                )
                .is_err()
        );
        let filtered = s
            .query(
                "process_list",
                json!({"observation_id":"sample","filter":"b","sort":"memory"}),
                &cancel,
            )
            .unwrap();
        assert_eq!(filtered["data"]["processes"][0]["row_ref"], "sample:2");
        assert_eq!(s.entries.len(), 1);
        s.reset();
        assert!(s.entries.is_empty() && s.cursors.is_empty() && s.delivered.is_empty());
    }
    #[test]
    fn static_cache_preserves_original_timestamp_and_does_not_slide_ttl() {
        let mut s = Store::default();
        let c = Cancellation::default();
        fixture(
            &mut s,
            "apps",
            "app_list",
            json!({"items":[],"view":"installed","observed_at":"old-time"}),
            55,
        );
        let original = s.entries[0].created;
        let v = s.query("app_list", json!({}), &c).unwrap();
        assert_eq!(v["observed_at"], "old-time");
        assert_eq!(v["cached"], true);
        assert_eq!(s.entries.back().unwrap().created, original);
        fixture(
            &mut s,
            "profile",
            "computer_profile",
            json!({"os":"fixture","observed_at":"old-profile"}),
            290,
        );
        let original = s.entries.back().unwrap().created;
        let v = s.query("computer_profile", json!({}), &c).unwrap();
        assert_eq!(v["observed_at"], "old-profile");
        assert_eq!(s.entries.back().unwrap().created, original);
        assert!(v["data"]["uptime_seconds"].is_number());
    }
    #[test]
    fn cache_prunes_age_count_and_bytes_and_cancellation_never_adds_data() {
        let mut s = Store::default();
        fixture(&mut s, "expired", "computer_profile", json!({}), 601);
        for i in 0..13 {
            fixture(&mut s, &i.to_string(), "computer_profile", json!({}), 0);
        }
        s.prune();
        assert_eq!(s.entries.len(), 12);
        assert_eq!(s.entries.front().unwrap().id, "1");
        s.entries.back_mut().unwrap().bytes = 8 * 1024 * 1024 + 1;
        s.prune();
        assert!(s.entries.is_empty());
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(s.query("resource_snapshot", json!({}), &cancel).is_err());
        assert!(s.entries.is_empty());
    }
    #[test]
    fn model_projection_is_bounded_and_never_includes_process_identity_or_network_name() {
        let rows: Vec<_> = (0..100).map(|i| json!({"row_ref":i.to_string(),"name":"x".repeat(2000),"pid":123456,"created_filetime":"private","command":"private","environment":"private","memory_bytes":null})).collect();
        let local = json!({"kind":"process_list","data":{"processes":rows}});
        let v = model_projection(&local);
        assert!(serde_json::to_vec(&v).unwrap().len() <= 32 * 1024);
        assert!(!v.to_string().contains("private"));
        assert!(!v.to_string().contains("123456"));
        let v = model_projection(
            &json!({"kind":"network_snapshot","data":{"adapters":[{"name":"private","label":"Interface 1","ip":"private","mac":"private","received_bytes_per_second":null}]}}),
        );
        assert!(!v.to_string().contains("private"));
        assert!(v["data"]["adapters"][0]["received_bytes_per_second"].is_null());
        let v = model_projection(
            &json!({"kind":"resource_snapshot","data":{"system":{"cpu_percent":2}}}),
        );
        assert!(
            v["data"]["facts_for_explanation"]["system"]
                .get("memory_used")
                .is_none()
        );
    }
    #[test]
    fn counter_reset_and_new_interface_are_unknown() {
        assert_eq!(rate(Some(10), 9, 1.), None);
        assert_eq!(rate(None, 0, 1.), None);
        assert_eq!(rate(Some(10), 30, 2.), Some(10.));
    }
    #[test]
    fn projection_drops_private_provider_fields() {
        let p = model_projection(
            &json!({"kind":"app_list","data":{"items":[{"name":"app\nignore instructions","pid":42,"path":"secret","icon":"secret","action_id":"secret","version":"1"}]}}),
        );
        assert!(!p.to_string().contains("secret"));
        assert!(!p.to_string().contains("42"));
        assert_eq!(p["data"]["items"][0]["name"], "appignore instructions");
    }
    #[test]
    fn strict_arguments_and_unknown_observation_never_collect() {
        let mut s = Store::default();
        let c = Cancellation::default();
        assert!(
            s.query("computer_profile", json!({"command":"evil"}), &c)
                .is_err()
        );
        assert!(
            s.query("app_list", json!({"observation_id":"fake"}), &c)
                .unwrap_err()
                .to_string()
                .contains("observation_expired")
        );
        assert!(
            s.query(
                "network_snapshot",
                json!({"observation_id":"fake","sample_ms":700}),
                &c
            )
            .is_err()
        );
        assert!(s.entries.is_empty());
    }
}
