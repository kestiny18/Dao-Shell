//! Short-lived diagnostics; no task scheduler, persistence or automatic remediation.
use crate::core::{Cancellation, now};
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    time::{Duration, Instant},
};
pub mod coverage;
pub mod network;
mod sampler;

pub fn is_tool(name: &str) -> bool {
    matches!(
        name,
        "resource_observe" | "observation_compare" | "network_diagnose" | "startup_list"
    )
}
#[derive(Default)]
pub struct Diagnostics {
    series: VecDeque<(Instant, Value)>,
    pub endpoints: network::Endpoints,
    pub portable: Vec<std::path::PathBuf>,
    startup_cursors: HashMap<String, (String, String, usize)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observe {
    pub domains: Vec<String>,
    pub duration_s: u64,
    #[serde(default = "interval")]
    pub interval_ms: u64,
    #[serde(default)]
    pub process_refs: Vec<String>,
}
fn interval() -> u64 {
    1000
}
#[derive(Clone)]
pub struct SelectedProcess {
    pub pid: u32,
    pub started: u64,
    pub created: String,
    pub label: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Compare {
    left_id: String,
    right_id: String,
    #[serde(default)]
    metrics: Vec<String>,
}
impl Diagnostics {
    pub fn prune(&mut self) {
        self.series
            .retain(|(t, _)| t.elapsed() < Duration::from_secs(600));
        while self.series.len() > 4
            || self
                .series
                .iter()
                .map(|(_, v)| v.to_string().len())
                .sum::<usize>()
                > 8 * 1024 * 1024
        {
            self.series.pop_front();
        }
    }
    pub fn explanation(&mut self, id: &str) -> Result<Value> {
        self.prune();
        self.series
            .iter()
            .find(|(_, v)| v["observation_id"] == id && v["kind"] == "resource_observe")
            .map(|(_, v)| projection(v))
            .ok_or_else(|| anyhow::anyhow!("observation_expired"))
    }
    pub fn startup(&mut self, args: Value, cancel: &Cancellation) -> Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            source: String,
            #[serde(default)]
            filter: String,
            #[serde(default = "limit")]
            limit: usize,
            #[serde(default)]
            cursor: Option<String>,
        }
        fn limit() -> usize {
            20
        }
        let r: Request =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        ensure!(
            matches!(r.source.as_str(), "user" | "machine")
                && r.filter.len() <= 128
                && (1..=50).contains(&r.limit),
            "invalid_arguments"
        );
        self.prune();
        self.startup_cursors
            .retain(|_, (id, _, _)| self.series.iter().any(|(_, v)| v["observation_id"] == *id));
        let signature = json!([r.source, r.filter, r.limit]).to_string();
        let (mut value, offset) = if let Some(cursor) = r.cursor {
            let (id, sig, offset) = self
                .startup_cursors
                .get(&cursor)
                .ok_or_else(|| anyhow::anyhow!("observation_expired"))?;
            ensure!(sig == &signature, "invalid_arguments");
            (
                self.series
                    .iter()
                    .find(|(_, v)| v["observation_id"] == *id)
                    .ok_or_else(|| anyhow::anyhow!("observation_expired"))?
                    .1
                    .clone(),
                *offset,
            )
        } else {
            let value = coverage::startup(
                json!({"source":r.source,"filter":r.filter,"limit":512}),
                cancel,
            )?;
            self.series.push_back((Instant::now(), value.clone()));
            self.prune();
            (value, 0)
        };
        let rows = value["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let items: Vec<_> = rows.iter().skip(offset).take(r.limit).cloned().collect();
        let next = if offset + items.len() < rows.len() {
            ensure!(
                self.startup_cursors.len() < 1024,
                "observation_budget_exceeded"
            );
            let cursor = uuid::Uuid::new_v4().to_string();
            self.startup_cursors.insert(
                cursor.clone(),
                (
                    value["observation_id"].as_str().unwrap().into(),
                    signature,
                    offset + items.len(),
                ),
            );
            Some(cursor)
        } else {
            None
        };
        value["data"]["items"] = json!(items);
        value["data"]["page"] = json!({"offset":offset,"next_cursor":next});
        value["query"] = json!({"source":r.source,"filter":r.filter,"limit":r.limit});
        Ok(value)
    }
    pub fn observe(
        &mut self,
        request: Observe,
        processes: Vec<SelectedProcess>,
        cancel: &Cancellation,
        event: &mut dyn FnMut(&Value),
    ) -> Result<Value> {
        ensure!(
            matches!(request.duration_s, 30 | 60)
                && matches!(request.interval_ms, 1000 | 2000)
                && !request.domains.is_empty()
                && request.domains.len() <= 6
                && request.process_refs.len() <= 5,
            "invalid_arguments"
        );
        ensure!(
            request.domains.iter().all(|s| matches!(
                s.as_str(),
                "cpu" | "memory" | "network" | "io" | "gpu" | "power"
            )),
            "invalid_arguments"
        );
        ensure!(
            request
                .domains
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == request.domains.len()
                && request
                    .process_refs
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == request.process_refs.len(),
            "invalid_arguments"
        );
        self.prune();
        let id = uuid::Uuid::new_v4().to_string();
        let mut collector = sampler::Sampler::new(&request.domains, processes);
        let start = Instant::now();
        let wall = std::time::SystemTime::now();
        let began_at = now();
        let mut points = Vec::new();
        let mut missed = 0;
        let mut late = false;
        let mut previous = 0.;
        let mut stopped = false;
        let mut reset_next = false;
        let interval = Duration::from_millis(request.interval_ms);
        let slots = request.duration_s * 1000 / request.interval_ms;
        for slot in 0..=slots {
            let due = interval * slot as u32;
            while start.elapsed() < due {
                if cancel.is_cancelled() {
                    stopped = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            if stopped || cancel.is_cancelled() {
                stopped = true;
                break;
            }
            let elapsed = start.elapsed();
            let (missed_slot, gap) = slot_state(elapsed, due, previous, interval, slot > 0);
            if missed_slot {
                missed += 1;
                points.push(json!({"elapsed_ms":due.as_millis(),"values":{},"unknown_reason":"missed_sampling_slot"}));
                continue;
            }
            let seconds = elapsed.as_secs_f64();
            let drift = wall
                .elapsed()
                .map(|d| (d.as_secs_f64() - seconds).abs() > 1.)
                .unwrap_or(true);

            let point_start = Instant::now();
            let mut values = if late {
                BTreeMap::new()
            } else {
                collector.sample(seconds, slot == 0 || gap || drift || reset_next)
            };
            if cancel.is_cancelled() {
                stopped = true;
                break;
            }
            let reason = if drift {
                Some("clock_drift")
            } else if gap {
                Some("sleep_or_sampling_gap")
            } else if late {
                Some("provider_soft_budget_exceeded")
            } else {
                None
            };
            if drift || gap {
                values.retain(|key, _| key.starts_with("memory") || key.starts_with("power"));
            }
            reset_next = late || gap || drift;
            late = point_start.elapsed() > Duration::from_millis(500);
            if late
                && !collector
                    .reasons
                    .iter()
                    .any(|r| r == "provider_soft_budget_exceeded")
            {
                collector
                    .reasons
                    .push("provider_soft_budget_exceeded".into());
            }
            points.push(json!({"elapsed_ms":elapsed.as_millis(),"observed_at":now(),"values":values,"unknown_reason":reason}));
            previous = seconds;
            event(
                &json!({"observation_id":id,"kind":"resource_observe","status":"observing","elapsed_ms":elapsed.as_millis(),"duration_s":request.duration_s,"point":points.last(),"metric_definitions":collector.definitions}),
            );
        }
        let definitions = json!(collector.definitions);
        let summary = summarize(&points, &definitions, request.interval_ms);
        let value = json!({"schema_version":2,"observation_id":id,"kind":"resource_observe","observed_at":now(),"status":if stopped {"stopped"}else{"complete"},"sample_window":{"started_at":began_at,"elapsed_ms":start.elapsed().as_millis(),"requested_duration_s":request.duration_s,"interval_ms":request.interval_ms},"data":{"series":points,"summary":summary,"metric_definitions":definitions},"coverage":{"state":"partial","reasons":collector.reasons,"missed_slots":missed},"limits":"At most 61 points, 5 selected process instances, 8 interfaces and bounded GPU engines. Correlation is not causation; missing values are not zero."});
        ensure!(
            value.to_string().len() <= 8 * 1024 * 1024,
            "observation_budget_exceeded"
        );
        self.series.push_back((Instant::now(), value.clone()));
        self.prune();
        Ok(value)
    }
    pub fn compare(&mut self, args: Value) -> Result<Value> {
        let r: Compare =
            serde_json::from_value(args).map_err(|_| anyhow::anyhow!("invalid_arguments"))?;
        self.prune();
        ensure!(r.metrics.len() <= 128, "invalid_arguments");
        let get = |id: &str| {
            self.series
                .iter()
                .find(|(_, v)| v["observation_id"] == id && v["kind"] == "resource_observe")
                .map(|(_, v)| v)
                .ok_or_else(|| anyhow::anyhow!("observation_expired"))
        };
        let left = get(&r.left_id)?;
        let right = get(&r.right_id)?;
        let mut comparisons = json!({});
        for (key, def) in left["data"]["metric_definitions"].as_object().unwrap() {
            if !r.metrics.is_empty() && !r.metrics.contains(key) {
                continue;
            }
            let compatible = right["data"]["metric_definitions"].get(key) == Some(def);
            let a = &left["data"]["summary"][key];
            let b = &right["data"]["summary"][key];
            let sufficient = a["valid_points"].as_u64().unwrap_or(0) >= 2
                && b["valid_points"].as_u64().unwrap_or(0) >= 2;
            comparisons[key] = json!({"status":if !compatible{"incompatible_definition_or_identity"}else if !sufficient{"insufficient_samples"}else{"comparable"},"left":a,"right":b,"mean_delta":if compatible&&sufficient {a["time_weighted_mean"].as_f64().zip(b["time_weighted_mean"].as_f64()).map(|(a,b)|b-a)}else{None}});
        }
        Ok(
            json!({"schema_version":2,"observation_id":uuid::Uuid::new_v4().to_string(),"kind":"observation_compare","status":"complete","observed_at":now(),"data":{"left_id":r.left_id,"right_id":r.right_id,"left_window":left["sample_window"],"right_window":right["sample_window"],"metric_definitions":left["data"]["metric_definitions"].as_object().map(|defs|defs.iter().map(|(key,def)|(key.clone(),json!({"unit":def["unit"],"label_key":def["label_key"]}))).collect::<serde_json::Map<_,_>>()),"comparisons":comparisons},"limits":"Only matching metric definitions and identities are compared; different durations remain explicit."}),
        )
    }
}
fn slot_state(
    elapsed: Duration,
    due: Duration,
    previous: f64,
    interval: Duration,
    has_previous: bool,
) -> (bool, bool) {
    (
        elapsed > due + interval,
        has_previous && elapsed.as_secs_f64() - previous > interval.as_secs_f64() * 1.75,
    )
}
/// Every interval belongs to its ending reading. Gaps contribute neither weight nor normal time.
pub fn summarize(points: &[Value], defs: &Value, interval_ms: u64) -> Value {
    let mut out = json!({});
    for (key, def) in defs.as_object().into_iter().flatten() {
        let mut values = Vec::new();
        let mut weighted = 0.;
        let mut weight = 0.;
        let mut high = 0.;
        let mut previous = None;
        for p in points {
            let t = p["elapsed_ms"].as_f64().unwrap_or(0.);
            let v = p["values"][key].as_f64().filter(|v| v.is_finite());
            if let Some(v) = v {
                values.push((t, v));
                if let Some(prev) = previous {
                    let dt = t - prev;
                    if dt > 0. && dt <= interval_ms as f64 * 1.75 && p["unknown_reason"].is_null() {
                        weighted += v * dt;
                        weight += dt;
                        if def["high_threshold"]
                            .as_f64()
                            .is_some_and(|limit| v >= limit)
                        {
                            high += dt;
                        }
                    }
                }
            }
            previous = if v.is_some() { Some(t) } else { None };
        }
        let peak = values.iter().max_by(|a, b| a.1.total_cmp(&b.1));
        out[key] = json!({"valid_points":values.len(),"missing_points":points.len()-values.len(),"min":values.iter().map(|v|v.1).reduce(f64::min),"max":peak.map(|p|p.1),"peak_elapsed_ms":peak.map(|p|p.0),"time_weighted_mean":if weight>0.{Some(weighted/weight)}else{None},"observed_weight_ms":weight,"high_threshold":def["high_threshold"],"high_duration_ms":high,"trend_status":if values.len()>=2&&weight>0.{"sufficient_samples"}else{"insufficient_samples"}});
    }
    out
}
/// Rebuild the model envelope, omit local names/addresses/paths and cap representative points.
pub fn projection(local: &Value) -> Value {
    let mut out = json!({});
    for key in [
        "schema_version",
        "observation_id",
        "kind",
        "status",
        "error",
        "observed_at",
        "sample_window",
        "coverage",
        "limits",
    ] {
        if let Some(v) = local.get(key) {
            out[key] = v.clone();
        }
    }
    let d = &local["data"];
    match local["kind"].as_str().unwrap_or("") {
        "resource_observe" => {
            out["data"]["summary"] = d["summary"].clone();
            let mut defs = d["metric_definitions"].clone();
            for (_, v) in defs.as_object_mut().into_iter().flatten() {
                v.as_object_mut().unwrap().remove("local_identity");
                v.as_object_mut().unwrap().remove("local_name");
            }
            out["data"]["metric_definitions"] = defs;
            let rows = d["series"].as_array().cloned().unwrap_or_default();
            let stride = rows.len().div_ceil(20).max(1);
            out["data"]["representative_points"] =
                json!(rows.iter().step_by(stride).take(20).collect::<Vec<_>>());
            out["data"]["statistics_are_local_deterministic"] = json!(true);
            // Summary rows take priority over representative points within the model budget.
            let mut omitted = 0usize;
            while out.to_string().len() > 31 * 1024 {
                if let Some(rows) = out["data"]["representative_points"].as_array_mut()
                    && !rows.is_empty()
                {
                    rows.pop();
                    continue;
                }
                let key = out["data"]["summary"]
                    .as_object()
                    .and_then(|o| {
                        o.keys()
                            .rev()
                            .find(|k| k.starts_with("gpu."))
                            .or_else(|| o.keys().next_back())
                    })
                    .cloned();
                let Some(key) = key else {
                    break;
                };
                out["data"]["summary"].as_object_mut().unwrap().remove(&key);
                out["data"]["metric_definitions"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
                omitted += 1;
            }
            if omitted > 0 {
                out["model_projection_metrics_omitted"] = json!(omitted);
            }
        }
        "observation_compare" => out["data"] = d.clone(),
        "network_diagnose" => out["data"] = network::projection(d),
        "startup_list" => {
            out["data"] = json!({"count":d["count"],"items":d["items"].as_array().into_iter().flatten().map(|r|json!({"name":r["name"].as_str().map(|s|s.chars().filter(|c|!c.is_control()).take(256).collect::<String>()),"source":r["source"],"enabled":"unknown"})).collect::<Vec<_>>(),"page":d["page"]})
        }
        _ => (),
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weighted_missing_and_counter_gap() {
        let defs = json!({"cpu":{"high_threshold":80}});
        let points = vec![
            json!({"elapsed_ms":0,"values":{"cpu":20}}),
            json!({"elapsed_ms":1000,"values":{"cpu":90}}),
            json!({"elapsed_ms":2000,"values":{}}),
            json!({"elapsed_ms":9000,"values":{"cpu":10}}),
        ];
        let s = summarize(&points, &defs, 1000);
        assert_eq!(s["cpu"]["time_weighted_mean"], 90.);
        assert_eq!(s["cpu"]["high_duration_ms"], 1000.);
        assert_eq!(s["cpu"]["missing_points"], 1);
    }
    #[test]
    fn series_budget_and_session_expiry() {
        let mut d = Diagnostics::default();
        for _ in 0..8 {
            d.series.push_back((Instant::now(), json!({})));
        }
        d.prune();
        assert_eq!(d.series.len(), 4);
        assert!(
            d.compare(json!({"left_id":"other-session","right_id":"none"}))
                .is_err()
        );
    }
    #[test]
    fn compare_requires_matching_identity_and_sufficient_points() {
        let mut d = Diagnostics::default();
        for (id, identity, points) in [
            ("a", "instance1", 2),
            ("b", "instance2", 2),
            ("c", "instance1", 1),
        ] {
            d.series.push_back((Instant::now(),json!({"observation_id":id,"kind":"resource_observe","data":{"metric_definitions":{"process.1.cpu":{"unit":"percent","local_identity":identity}},"summary":{"process.1.cpu":{"valid_points":points,"time_weighted_mean":10}}}})));
        }
        assert_eq!(
            d.compare(json!({"left_id":"a","right_id":"b"})).unwrap()["data"]["comparisons"]["process.1.cpu"]
                ["status"],
            "incompatible_definition_or_identity"
        );
        assert_eq!(
            d.compare(json!({"left_id":"a","right_id":"c"})).unwrap()["data"]["comparisons"]["process.1.cpu"]
                ["status"],
            "insufficient_samples"
        );
    }
    #[test]
    fn cancellation_keeps_only_completed_points_without_restarting() {
        let mut d = Diagnostics::default();
        let cancel = Cancellation::default();
        let token = cancel.clone();
        let mut events = 0;
        let value = d
            .observe(
                Observe {
                    domains: vec!["memory".into()],
                    duration_s: 30,
                    interval_ms: 1000,
                    process_refs: vec![],
                },
                vec![],
                &cancel,
                &mut |_| {
                    events += 1;
                    token.cancel();
                },
            )
            .unwrap();
        assert_eq!(events, 1);
        assert_eq!(value["status"], "stopped");
        assert_eq!(value["data"]["series"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["data"]["summary"]["memory.used_bytes"]["trend_status"],
            "insufficient_samples"
        );
        let invalid = Observe {
            domains: vec!["cpu".into()],
            duration_s: 86400,
            interval_ms: 1,
            process_refs: vec![],
        };
        assert!(
            d.observe(invalid, vec![], &Cancellation::default(), &mut |_| {})
                .is_err()
        );
    }
    #[test]
    fn virtual_schedule_marks_missed_and_sleep_gaps_and_enforces_bytes() {
        let interval = Duration::from_secs(1);
        assert_eq!(
            slot_state(Duration::from_millis(1020), interval, 0., interval, true),
            (false, false)
        );
        assert_eq!(
            slot_state(Duration::from_secs(20), interval, 0., interval, true),
            (true, true)
        );
        assert_eq!(
            slot_state(
                Duration::from_secs(20),
                Duration::from_secs(20),
                1.,
                interval,
                true
            ),
            (false, true)
        );
        let mut d = Diagnostics::default();
        d.series
            .push_back((Instant::now(), json!({"large":"x".repeat(8*1024*1024+1)})));
        d.prune();
        assert!(d.series.is_empty());
    }
    #[test]
    fn startup_projection_strips_injection_strings_and_limits_names() {
        let p = projection(
            &json!({"kind":"startup_list","data":{"items":[{"name":"a".repeat(16000),"source":"user_run","command":"PRIVATE.exe --key=secret","path":"C:/private/user"}]}}),
        );
        assert!(!p.to_string().contains("PRIVATE"));
        assert!(!p.to_string().contains("C:/private"));
        assert!(p.to_string().len() < 1024);
    }
    #[test]
    fn startup_cursor_binds_snapshot_filter_and_session() {
        let mut d = Diagnostics::default();
        d.series.push_back((Instant::now(),json!({"kind":"startup_list","observation_id":"stable","data":{"items":[{"name":"first"},{"name":"second"}]}})));
        d.startup_cursors.insert(
            "opaque".into(),
            ("stable".into(), json!(["user", "", 1]).to_string(), 1),
        );
        let args = json!({"source":"user","limit":1,"cursor":"opaque"});
        let value = d.startup(args.clone(), &Cancellation::default()).unwrap();
        assert_eq!(value["observation_id"], "stable");
        assert_eq!(value["data"]["items"][0]["name"], "second");
        assert!(
            Diagnostics::default()
                .startup(args, &Cancellation::default())
                .is_err()
        );
        assert!(
            d.startup(
                json!({"source":"user","filter":"changed","limit":1,"cursor":"opaque"}),
                &Cancellation::default()
            )
            .is_err()
        );
    }
    #[test]
    fn projection_never_shares_native_identity() {
        let local = json!({"kind":"resource_observe","data":{"series":(0..61).map(|i|json!({"elapsed_ms":i,"values":{"cpu":1}})).collect::<Vec<_>>(),"metric_definitions":{"cpu":{"local_identity":"SECRET","local_name":"private"}}}});
        let p = projection(&local);
        assert!(!p.to_string().contains("SECRET"));
        assert!(!p.to_string().contains("private"));
        assert!(p["data"]["representative_points"].as_array().unwrap().len() <= 20);
    }
}
