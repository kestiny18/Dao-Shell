use super::SelectedProcess;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System};
pub struct Sampler {
    domains: Vec<String>,
    system: System,
    networks: Networks,
    selected: Vec<SelectedProcess>,
    previous: HashMap<String, (u64, u64)>,
    last: f64,
    network_names: Vec<String>,
    pub definitions: BTreeMap<String, Value>,
    pub reasons: Vec<String>,
    gpu: super::coverage::Gpu,
}
pub fn rate(before: Option<u64>, after: u64, seconds: f64) -> Option<f64> {
    before
        .and_then(|b| after.checked_sub(b))
        .filter(|_| seconds > 0. && seconds.is_finite())
        .map(|n| n as f64 / seconds)
}
impl Sampler {
    pub fn new(domains: &[String], selected: Vec<SelectedProcess>) -> Self {
        let mut result = Self {
            domains: domains.into(),
            system: System::new(),
            networks: Networks::new(),
            selected,
            previous: HashMap::new(),
            last: 0.,
            network_names: Vec::new(),
            definitions: BTreeMap::new(),
            reasons: Vec::new(),
            gpu: super::coverage::Gpu::new(domains.iter().any(|d| d == "gpu")),
        };
        if domains.iter().any(|d| d == "network") {
            result.networks.refresh(true);
            result.network_names = result.networks.keys().cloned().collect();
            result.network_names.sort();
            if result.network_names.len() > 8 {
                result.reasons.push("interface_budget".into());
                result.network_names.truncate(8);
            }
        }
        result
    }
    fn has(&self, s: &str) -> bool {
        self.domains.iter().any(|d| d == s)
    }
    fn metric(
        &mut self,
        out: &mut BTreeMap<String, Value>,
        key: String,
        value: Option<f64>,
        unit: &str,
        semantics: &str,
        identity: Option<String>,
    ) {
        self.definitions.entry(key.clone()).or_insert_with(||json!({"label_key":if key.starts_with("cpu."){"diagnostics.metric_cpu"}else if key.starts_with("memory."){"computer.memory"}else if key.starts_with("network."){"diagnostics.interface"}else if key.starts_with("process."){"diagnostics.process"}else if key.starts_with("power.ac"){"diagnostics.ac"}else if key.starts_with("power."){"diagnostics.battery"}else if unit=="percent"{"diagnostics.engine"}else if unit=="dedicated_bytes"{"diagnostics.dedicated"}else{"diagnostics.shared"},"unit":unit,"semantics":semantics,"local_identity":identity,"high_threshold":if unit=="percent"{Some(80.)}else{None}}));
        out.insert(key, json!(value));
    }
    pub fn sample(&mut self, seconds: f64, reset: bool) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        let dt = seconds - self.last;
        if self.has("cpu") {
            self.system.refresh_cpu_usage();
            self.metric(
                &mut out,
                "cpu.percent".into(),
                (!reset).then_some(self.system.global_cpu_usage() as f64),
                "percent",
                "total-machine interval average",
                None,
            );
        }
        if self.has("memory") {
            self.system.refresh_memory();
            self.metric(
                &mut out,
                "memory.used_bytes".into(),
                Some(self.system.used_memory() as f64),
                "bytes",
                "point reading",
                None,
            );
            self.metric(
                &mut out,
                "memory.used_percent".into(),
                (self.system.total_memory() > 0).then(|| {
                    100. * self.system.used_memory() as f64 / self.system.total_memory() as f64
                }),
                "percent",
                "point reading",
                None,
            );
        }
        if self.has("network") {
            self.networks.refresh(true);
            let names = self.network_names.clone();
            for (i, name) in names.iter().enumerate() {
                let current = self
                    .networks
                    .get(name)
                    .map(|n| (n.total_received(), n.total_transmitted()));
                let old = self.previous.get(name).copied();
                for (direction, index) in [("rx", 0), ("tx", 1)] {
                    let v = if reset {
                        None
                    } else {
                        current.and_then(|c| {
                            rate(
                                old.map(|o| if index == 0 { o.0 } else { o.1 }),
                                if index == 0 { c.0 } else { c.1 },
                                dt,
                            )
                        })
                    };
                    self.metric(
                        &mut out,
                        format!("network.{}.{}_bytes_per_second", i + 1, direction),
                        v,
                        "bytes/second",
                        "interface counters, adjacent actual monotonic interval",
                        Some(name.clone()),
                    );
                }
                if let Some(c) = current {
                    self.previous.insert(name.clone(), c);
                } else if !self.reasons.contains(&"interface_changed".into()) {
                    self.reasons.push("interface_changed".into());
                }
            }
            if self
                .networks
                .iter()
                .any(|(n, _)| !self.network_names.contains(n))
                && !self.reasons.contains(&"interface_changed_or_budget".into())
            {
                self.reasons.push("interface_changed_or_budget".into());
            }
        }
        if !self.selected.is_empty() && (self.has("cpu") || self.has("memory") || self.has("io")) {
            let ids: Vec<_> = self
                .selected
                .iter()
                .map(|p| sysinfo::Pid::from_u32(p.pid))
                .collect();
            let mut refresh = ProcessRefreshKind::nothing();
            if self.has("cpu") {
                refresh = refresh.with_cpu();
            }
            if self.has("memory") {
                refresh = refresh.with_memory();
            } // I/O is read directly so failed native reads never become zero.
            self.system
                .refresh_processes_specifics(ProcessesToUpdate::Some(&ids), true, refresh);
            for selected in self.selected.clone() {
                let identity = format!("{}:{}", selected.pid, selected.created);
                let p = self
                    .system
                    .process(sysinfo::Pid::from_u32(selected.pid))
                    .filter(|p| {
                        p.start_time() == selected.started
                            && crate::process_control::creation(selected.pid).as_deref()
                                == Some(selected.created.as_str())
                    });
                let cpu_key = format!("cpu:{identity}");
                let cumulative = p.map(|p| p.accumulated_cpu_time());
                let cpu = cumulative
                    .and_then(|value| {
                        rate(self.previous.get(&cpu_key).map(|v| v.0), value, dt * 1000.)
                    })
                    .map(|n| n * 100. / self.system.cpus().len().max(1) as f64);
                if let Some(c) = cumulative {
                    self.previous.insert(cpu_key, (c, 0));
                }
                let memory = p.filter(|p| p.memory() > 0).map(|p| p.memory() as f64);
                let present = p.is_some();
                if !present
                    && !self
                        .reasons
                        .iter()
                        .any(|r| r == "process_instance_changed_or_unavailable")
                {
                    self.reasons
                        .push("process_instance_changed_or_unavailable".into());
                }
                if self.has("cpu") {
                    self.metric(
                        &mut out,
                        format!("{}.cpu_percent", selected.label),
                        if reset { None } else { cpu },
                        "percent",
                        "selected instance total-machine interval average",
                        Some(identity.clone()),
                    );
                }
                if self.has("memory") {
                    self.metric(
                        &mut out,
                        format!("{}.memory_bytes", selected.label),
                        memory,
                        "bytes",
                        "selected instance point reading",
                        Some(identity.clone()),
                    );
                }
                if self.has("io") {
                    let current = if present {
                        super::coverage::io_exact(selected.pid, &selected.created)
                    } else {
                        None
                    };
                    let key = format!("io:{identity}");
                    let old = self.previous.get(&key).copied();
                    for (dir, index) in [("read", 0), ("write", 1)] {
                        let v = if reset {
                            None
                        } else {
                            current.and_then(|c| {
                                rate(
                                    old.map(|o| if index == 0 { o.0 } else { o.1 }),
                                    if index == 0 { c.0 } else { c.1 },
                                    dt,
                                )
                            })
                        };
                        self.metric(
                            &mut out,
                            format!("{}.io_{}_bytes_per_second", selected.label, dir),
                            v,
                            "bytes/second",
                            "GetProcessIoCounters all process I/O, not physical disk",
                            Some(identity.clone()),
                        );
                    }
                    if let Some(c) = current {
                        self.previous.insert(key, c);
                    }
                }
            }
        }
        if self.has("io")
            && self.selected.is_empty()
            && !self
                .reasons
                .contains(&"io_requires_selected_process".into())
        {
            self.reasons.push("io_requires_selected_process".into());
        }
        if self.has("power") {
            let p = super::coverage::power();
            self.metric(
                &mut out,
                "power.battery_percent".into(),
                p["percent"].as_f64(),
                "percent",
                "system battery point reading, not a fault threshold",
                None,
            );
            self.definitions.get_mut("power.battery_percent").unwrap()["high_threshold"] =
                Value::Null;
            self.metric(
                &mut out,
                "power.ac_online".into(),
                p["ac_online"].as_bool().map(|b| if b { 1. } else { 0. }),
                "boolean",
                "system AC status point reading",
                None,
            );
            if let Some(reason) = p["reason"].as_str()
                && !self.reasons.iter().any(|r| r == reason)
            {
                self.reasons.push(reason.into());
            }
        }
        if self.has("gpu") {
            let gpu = self.gpu.sample();
            if gpu.is_empty() && !self.reasons.contains(&"gpu_counter_unavailable".into()) {
                self.reasons.push("gpu_counter_unavailable".into());
            }
            for (name, unit, value) in gpu {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                name.hash(&mut h);
                self.metric(&mut out,format!("gpu.{:x}",h.finish()),if reset&&unit=="percent"{None}else{Some(value)},unit,"individual adapter/engine counter; never summed across engines; shared memory separate",Some(name));
            }
        }
        self.last = seconds;
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_counter_is_missing_not_negative_or_zero() {
        assert_eq!(rate(Some(10), 5, 1.), None);
        assert_eq!(rate(Some(10), 20, 2.), Some(5.));
        assert_eq!(rate(None, 0, 1.), None);
        assert_eq!(rate(Some(0), 20, 0.), None);
    }
}
