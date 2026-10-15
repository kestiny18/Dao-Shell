//! Bounded service scheduling state; workers remain owned by the SCM runtime.
use super::{Volume, volumes};
use std::collections::BTreeMap;
pub(super) const MAX_CATALOG: usize = 128;
pub(super) const MAX_WORKERS: usize = 16;
pub(super) struct Entry {
    pub volume: Volume,
    present: bool,
    pub online: bool,
    pub running: bool,
    pub stopping: bool,
    pub retry_at: Option<u64>,
    failures: u32,
}
#[derive(Default)]
pub(super) struct Catalog {
    pub entries: BTreeMap<String, Entry>,
    pub discovery_failed: bool,
    stopping: bool,
}
fn identity(v: &Volume) -> String {
    v.serial
        .and_then(|s| volumes::key(&v.guid, s).ok())
        .unwrap_or_else(|| v.guid.clone())
}
impl Catalog {
    pub fn shutdown(&mut self) {
        self.stopping = true;
        for e in self.entries.values_mut() {
            e.stopping |= e.running;
        }
    }
    pub fn discover(&mut self, result: anyhow::Result<Vec<Volume>>, now: u64) {
        let Ok(found) = result else {
            self.discovery_failed = true;
            return;
        };
        if found.len() > MAX_CATALOG {
            self.discovery_failed = true;
            return;
        }
        self.discovery_failed = false;
        let found: BTreeMap<_, _> = found.into_iter().map(|v| (identity(&v), v)).collect();
        for (key, entry) in &mut self.entries {
            entry.present = found.contains_key(key);
            if !entry.present {
                entry.online = false;
                entry.stopping |= entry.running;
                entry.volume.worker_started = false;
                entry.volume.pipe = None;
                entry.volume.worker_error = Some(
                    if entry.running {
                        "stopping_offline"
                    } else {
                        "offline"
                    }
                    .into(),
                );
            }
        }
        // Only evict historical, fully stopped entries; never forget a live writer.
        let needed = found
            .keys()
            .filter(|key| !self.entries.contains_key(*key))
            .count();
        let mut remove = self
            .entries
            .len()
            .saturating_add(needed)
            .saturating_sub(MAX_CATALOG);
        let victims: Vec<_> = self
            .entries
            .iter()
            .filter(|(k, e)| !e.running && !found.contains_key(*k))
            .map(|(k, _)| k.clone())
            .collect();
        for key in victims {
            if remove == 0 {
                break;
            }
            self.entries.remove(&key);
            remove -= 1;
        }
        for (key, mut volume) in found {
            if let Some(entry) = self.entries.get_mut(&key) {
                let returned = !entry.online && volume.supported;
                if volume.supported {
                    volume.worker_started = entry.volume.worker_started;
                    volume.worker_error = entry.volume.worker_error.clone();
                } else {
                    // Discovery eligibility is distinct from disappearance or a
                    // worker failure. Keep its actual reason while joining any
                    // old writer; never publish that writer's pipe again.
                    entry.stopping |= entry.running;
                    volume.worker_started = false;
                    volume.worker_error = None;
                    entry.retry_at = None;
                }
                if !entry.running || entry.stopping {
                    volume.pipe = None;
                }
                entry.volume = volume;
                entry.online = entry.volume.supported;
                if returned {
                    entry.retry_at = Some(now);
                    entry.failures = 0;
                }
            } else if self.entries.len() < MAX_CATALOG {
                let online = volume.supported;
                volume.pipe = None;
                self.entries.insert(
                    key,
                    Entry {
                        volume,
                        present: true,
                        online,
                        running: false,
                        stopping: false,
                        retry_at: online.then_some(now),
                        failures: 0,
                    },
                );
            } else {
                self.discovery_failed = true;
            }
        }
    }
    pub fn starts(&self, now: u64) -> Vec<String> {
        if self.stopping {
            return Vec::new();
        }
        let free = MAX_WORKERS.saturating_sub(self.entries.values().filter(|e| e.running).count());
        let mut selected = std::collections::HashSet::new();
        self.entries
            .iter()
            .filter(|(_, e)| {
                e.online && !e.running && !e.stopping && e.retry_at.is_some_and(|at| at <= now)
            })
            // Never open another identity on a GUID until its previous worker has joined.
            .filter(|(_, e)| {
                !self
                    .entries
                    .values()
                    .any(|old| old.running && old.volume.guid.eq_ignore_ascii_case(&e.volume.guid))
            })
            .filter(|(_, e)| selected.insert(e.volume.guid.to_ascii_lowercase()))
            .take(free)
            .map(|(key, _)| key.clone())
            .collect()
    }
    pub fn started(&mut self, key: &str) {
        let e = self.entries.get_mut(key).unwrap();
        e.running = true;
        e.retry_at = None;
        e.volume.worker_error = Some("initializing".into());
    }
    pub fn ready(&mut self, key: &str) {
        let e = self.entries.get_mut(key).unwrap();
        if !e.stopping && e.online {
            e.volume.worker_started = true;
            e.volume.worker_error = None;
            e.volume.pipe = Some(format!("volume-{key}"));
        }
    }
    pub fn ended(&mut self, key: &str, reason: &str, now: u64) {
        let e = self.entries.get_mut(key).unwrap();
        let expected = e.stopping;
        e.running = false;
        e.stopping = false;
        e.volume.worker_started = false;
        e.volume.pipe = None;
        e.volume.worker_error = if !e.present {
            Some("offline".into())
        } else if e.online {
            Some(reason.into())
        } else {
            None
        };
        e.retry_at = if expected && e.online {
            Some(now)
        } else if e.online && matches!(reason, "journal_unavailable" | "volume_unavailable") {
            e.failures = e.failures.saturating_add(1);
            Some(now + (5u64 << e.failures.min(6)).min(300))
        } else {
            None
        };
    }
    pub fn snapshot(&self) -> Vec<Volume> {
        if self.entries.is_empty() && self.discovery_failed {
            return vec![Volume {
                guid: String::new(),
                serial: None,
                mounts: vec![],
                supported: false,
                reason: Some("discovery_failed".into()),
                pipe: None,
                worker_started: false,
                worker_error: None,
            }];
        }
        self.entries
            .values()
            .map(|e| {
                let mut v = e.volume.clone();
                if self.discovery_failed {
                    v.reason = Some("discovery_failed".into());
                }
                v
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn volume(id: u128, serial: u32, mount: &str) -> Volume {
        Volume {
            guid: format!(r"\\?\Volume{{{}}}\", uuid::Uuid::from_u128(id)),
            serial: Some(serial),
            mounts: vec![mount.into()],
            supported: true,
            reason: None,
            pipe: None,
            worker_started: false,
            worker_error: None,
        }
    }
    #[test]
    fn changed_identity_waits_for_join_and_mount_reuse_never_reuses_key() {
        let mut c = Catalog::default();
        let a = volume(1, 1, "E:\\");
        let b = volume(1, 2, "E:\\");
        c.discover(Ok(vec![a.clone()]), 0);
        let old = c.starts(0).pop().unwrap();
        c.started(&old);
        c.ready(&old);
        c.discover(Ok(vec![b]), 5);
        assert!(c.entries[&old].stopping);
        assert!(c.starts(5).is_empty());
        c.discover(Ok(vec![volume(2, 1, "E:\\")]), 6);
        let replacement = c.starts(6);
        assert_eq!(replacement.len(), 1);
        assert_ne!(replacement[0], old);
        c.ended(&old, "worker_stopped", 7);
        assert!(!c.entries[&old].running);
        c.discover(Ok(vec![a]), 10);
        assert_eq!(c.starts(10), vec![old]);
    }
    #[test]
    fn discovery_error_retains_workers_and_offline_return_waits_for_join() {
        let mut c = Catalog::default();
        let a = volume(1, 1, "E:\\");
        c.discover(Ok(vec![a.clone()]), 0);
        let key = c.starts(0)[0].clone();
        c.started(&key);
        c.ready(&key);
        c.discover(Err(anyhow::anyhow!("synthetic")), 5);
        assert!(c.entries[&key].online);
        assert!(!c.entries[&key].stopping);
        assert_eq!(c.snapshot()[0].reason.as_deref(), Some("discovery_failed"));
        c.discover(Ok(vec![]), 10);
        assert!(c.entries[&key].stopping);
        c.discover(Ok(vec![a]), 15);
        assert!(c.starts(15).is_empty());
        c.ready(&key);
        assert!(!c.entries[&key].volume.worker_started);
        c.ended(&key, "worker_stopped", 16);
        assert_eq!(c.starts(16), vec![key]);
    }
    #[test]
    fn repeated_unsupported_discovery_preserves_eligibility_reason_not_offline() {
        for reason in [
            "unsupported_filesystem",
            "not_fixed_volume",
            "volume_worker_limit",
        ] {
            let mut c = Catalog::default();
            let mut v = volume(1, 1, "E:\\");
            v.supported = false;
            v.reason = Some(reason.into());
            for now in [0, 5, 10] {
                c.discover(Ok(vec![v.clone()]), now);
                let snapshot = c.snapshot();
                assert_eq!(snapshot[0].reason.as_deref(), Some(reason));
                assert!(snapshot[0].worker_error.is_none());
                assert!(!snapshot[0].worker_started);
                assert!(snapshot[0].pipe.is_none());
                assert!(c.starts(now).is_empty());
            }
            c.discover(Ok(vec![]), 15);
            assert_eq!(c.snapshot()[0].worker_error.as_deref(), Some("offline"));
            c.discover(Ok(vec![v]), 20);
            assert!(c.snapshot()[0].worker_error.is_none());
        }
    }
    #[test]
    fn eligibility_changes_cancel_and_join_writer_before_supported_restart() {
        let mut c = Catalog::default();
        let v = volume(1, 1, "E:\\");
        c.discover(Ok(vec![v.clone()]), 0);
        let key = c.starts(0)[0].clone();
        c.started(&key);
        c.ready(&key);
        let mut unsupported = v.clone();
        unsupported.supported = false;
        unsupported.reason = Some("unsupported_filesystem".into());
        c.discover(Ok(vec![unsupported.clone()]), 5);
        assert!(c.entries[&key].stopping);
        c.ready(&key); // A late initialization signal cannot restore its pipe.
        assert!(!c.snapshot()[0].worker_started);
        assert!(c.snapshot()[0].pipe.is_none());
        c.discover(Ok(vec![unsupported.clone()]), 10);
        c.ended(&key, "worker_stopped", 11);
        assert_eq!(
            c.snapshot()[0].reason.as_deref(),
            Some("unsupported_filesystem")
        );
        assert!(c.snapshot()[0].worker_error.is_none());
        assert!(c.starts(11).is_empty());
        c.discover(Ok(vec![v.clone()]), 15);
        assert_eq!(c.starts(15), vec![key.clone()]);
        c.started(&key);
        c.discover(Ok(vec![unsupported]), 20);
        c.discover(Ok(vec![v]), 25);
        assert!(c.starts(25).is_empty());
        c.ended(&key, "worker_stopped", 26);
        assert_eq!(c.starts(26), vec![key]);
        assert!(c.snapshot()[0].reason.is_none());
    }
    #[test]
    fn failed_worker_isolated_only_known_recovery_is_retried() {
        let mut c = Catalog::default();
        c.discover(Ok(vec![volume(1, 1, "E:\\"), volume(2, 1, "F:\\")]), 0);
        let keys = c.starts(0);
        for k in &keys {
            c.started(k);
            c.ready(k);
        }
        c.ended(&keys[0], "rebuild_required", 1);
        assert!(c.starts(1000).is_empty());
        assert!(c.entries[&keys[1]].running);
        c.ended(&keys[1], "journal_unavailable", 1);
        assert!(c.starts(5).is_empty());
        assert_eq!(c.starts(11), vec![keys[1].clone()]);
        c.started(&keys[1]);
        c.ended(&keys[1], "worker_panicked", 12);
        assert!(c.starts(10000).is_empty());
    }
    #[test]
    fn catalog_history_and_worker_count_remain_bounded() {
        let mut c = Catalog::default();
        c.discover(Ok((0..128).map(|i| volume(i, 1, "E:\\")).collect()), 0);
        let starts = c.starts(0);
        assert_eq!(starts.len(), 16);
        for k in starts {
            c.started(&k);
        }
        c.discover(Ok((128..256).map(|i| volume(i, 1, "F:\\")).collect()), 5);
        assert_eq!(c.entries.len(), 128);
        assert!(c.discovery_failed);
        assert!(c.starts(5).is_empty());
        assert_eq!(c.entries.values().filter(|e| e.running).count(), 16);
        c.shutdown();
        c.discover(Ok(vec![volume(1000, 1, "G:\\")]), 1000);
        assert!(c.starts(1000).is_empty());
    }
}
