//! Notifications are hints; only current filesystem observations enter a snapshot.
use super::*;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::sync::{
    Weak,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

const PERIOD: Duration = Duration::from_secs(300);
const TICK: Duration = Duration::from_millis(200);
#[derive(Default)]
struct Batch {
    paths: Vec<PathBuf>,
    rescan: bool,
    broken: bool,
}
fn enqueue(
    sender: &SyncSender<notify::Result<Event>>,
    lost: &AtomicBool,
    event: notify::Result<Event>,
) {
    if let Err(TrySendError::Full(_)) = sender.try_send(event) {
        lost.store(true, Ordering::SeqCst);
    }
}
impl Batch {
    fn merge(&mut self, other: Self) {
        self.rescan |= other.rescan;
        self.broken |= other.broken;
        if self.paths.len() + other.paths.len() > 1024 {
            self.rescan = true;
        }
        if !self.rescan {
            self.paths.extend(other.paths);
        } else {
            self.paths.clear();
        }
    }
    fn pending(&self) -> bool {
        !self.paths.is_empty() || self.rescan || self.broken
    }
    fn add(&mut self, event: notify::Result<Event>) {
        match event {
            Err(_) => {
                self.rescan = true;
                self.broken = true;
            }
            Ok(e) if e.need_rescan() => self.rescan = true,
            Ok(e) if matches!(e.kind, EventKind::Access(_)) => {}
            Ok(e) => {
                if matches!(e.kind, EventKind::Any | EventKind::Other)
                    || e.paths.is_empty()
                    || self.paths.len() + e.paths.len() > 1024
                {
                    self.rescan = true;
                }
                if !self.rescan {
                    self.paths.extend(e.paths);
                }
            }
        }
    }
}
fn drain(receiver: &Receiver<notify::Result<Event>>, lost: &AtomicBool) -> Batch {
    let mut batch = Batch {
        rescan: lost.swap(false, Ordering::SeqCst),
        ..Default::default()
    };
    for _ in 0..1024 {
        match receiver.try_recv() {
            Ok(e) => batch.add(e),
            Err(_) => break,
        }
    }
    batch
}
fn subscribe(
    root: &Path,
    sender: SyncSender<notify::Result<Event>>,
    lost: Arc<AtomicBool>,
) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event| enqueue(&sender, &lost, event))?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    Ok(watcher)
}
fn backoff(failures: u32) -> Duration {
    Duration::from_secs((30u64.saturating_mul(1u64 << failures.min(4))).min(300))
}

fn subscription(
    manager: &Weak<Inventory>,
    root: &Path,
    sender: SyncSender<notify::Result<Event>>,
    lost: Arc<AtomicBool>,
) -> Option<notify::RecommendedWatcher> {
    #[cfg(test)]
    if let Some(owner) = manager.upgrade()
        && owner.hooks.fail_watch.load(Ordering::SeqCst)
    {
        owner.hooks.armed.store(true, Ordering::SeqCst);
        return None;
    }
    #[cfg(not(test))]
    let _ = manager;
    let result = subscribe(root, sender, lost).ok();
    #[cfg(test)]
    if let Some(owner) = manager.upgrade() {
        owner.hooks.armed.store(true, Ordering::SeqCst);
    }
    result
}
fn period(manager: &Inventory) -> Duration {
    #[cfg(test)]
    {
        let ms = manager.hooks.period_ms.load(Ordering::SeqCst);
        if ms > 0 {
            return Duration::from_millis(ms);
        }
    }
    let _ = manager;
    PERIOD
}
fn retry_delay(manager: &Inventory, failures: u32) -> Duration {
    #[cfg(test)]
    {
        let ms = manager.hooks.retry_ms.load(Ordering::SeqCst);
        if ms > 0 {
            return Duration::from_millis(ms);
        }
    }
    let _ = manager;
    backoff(failures)
}

pub(super) fn run(
    manager: Weak<Inventory>,
    r: Registration,
    cancel: Cancellation,
    generation: u64,
) {
    let (sender, receiver) = mpsc::sync_channel(1024);
    let lost = Arc::new(AtomicBool::new(false));
    // Subscribe before the initial scan so bootstrap changes enter the next batch.
    let mut watcher = subscription(&manager, &r.root, sender.clone(), lost.clone());
    let mut carried = Batch::default();
    let mut full = true;
    let mut failures = 0;
    let mut due = Instant::now();
    let mut retry_after = Instant::now();
    while !cancel.is_cancelled() {
        if manager.strong_count() == 0 {
            return;
        }
        #[cfg(test)]
        if let Some(owner) = manager.upgrade()
            && owner.hooks.overflow.swap(false, Ordering::SeqCst)
        {
            lost.store(true, Ordering::SeqCst);
        }
        let mut batch = std::mem::take(&mut carried);
        batch.merge(drain(&receiver, &lost));
        full |= batch.rescan;
        if batch.broken {
            watcher = None;
        }
        if Instant::now() < retry_after {
            // We intentionally discard details during backoff; reconciliation is mandatory.
            full |= !batch.paths.is_empty();
            std::thread::sleep(TICK);
            continue;
        }
        let periodic = Instant::now() >= due;
        if periodic {
            full = true;
        }
        if !full && batch.paths.is_empty() {
            std::thread::sleep(TICK);
            continue;
        }
        if watcher.is_none() {
            watcher = subscription(&manager, &r.root, sender.clone(), lost.clone());
        }
        let Some(manager) = manager.upgrade() else {
            return;
        };
        if !manager.working(&r, generation) {
            return;
        }
        let mut incremental = false;
        let result = (|| -> Result<_> {
            let _serial = manager.scanner.lock().unwrap();
            cancel.check()?;
            if !full
                && let Some(snapshot) = manager.refresh(&r, generation, &batch.paths, &cancel)?
            {
                incremental = true;
                return Ok(snapshot);
            }
            #[cfg(test)]
            {
                if manager.hooks.fail_budget.swap(false, Ordering::SeqCst) {
                    return Err(BudgetExceeded.into());
                }
                ensure!(
                    !manager.hooks.fail_scan.swap(false, Ordering::SeqCst),
                    "injected scan failure"
                );
            }
            manager.scan(&r, &cancel)
        })();
        // Do not advertise ready between bootstrap and already queued changes.
        carried = drain(&receiver, &lost);
        if carried.broken {
            watcher = None;
        }
        let settled = !carried.pending();
        match manager.publish(
            &r,
            generation,
            result,
            incremental,
            watcher.is_some(),
            settled,
        ) {
            Ok(false) => return,
            Ok(true) => {
                if watcher.is_some() {
                    failures = 0;
                }
                full = false;
                if !incremental {
                    due = Instant::now() + period(&manager);
                }
                if watcher.is_none() {
                    full = true;
                    retry_after = Instant::now() + retry_delay(&manager, failures);
                    failures = failures.saturating_add(1);
                }
            }
            Err(error) => {
                let budget = error.downcast_ref::<BudgetExceeded>().is_some();
                manager.failed(&r, generation, &error, budget);
                if budget {
                    return;
                }
                full = true;
                retry_after = Instant::now() + retry_delay(&manager, failures);
                failures = failures.saturating_add(1);
            }
        }
        drop(manager);
        std::thread::sleep(TICK);
    }
}

impl Inventory {
    fn refresh(
        &self,
        r: &Registration,
        generation: u64,
        paths: &[PathBuf],
        cancel: &Cancellation,
    ) -> Result<Option<(Vec<Record>, String, tempfile::NamedTempFile)>> {
        cancel.check()?;
        let root = normalize_existing(&r.root)?;
        ensure!(
            platform::identity(&root)?.same_object(&r.identity),
            "目录身份已变化，请重新登记"
        );
        let previous = {
            let inner = self.inner.lock().unwrap();
            let Some(e) = inner.entries.get(&r.id) else {
                return Ok(None);
            };
            if e.generation != generation {
                return Ok(None);
            }
            e.records.clone()
        };
        let Some(previous) = previous else {
            return Ok(None);
        };
        let mut records: BTreeMap<PathBuf, Record> = previous
            .iter()
            .map(|r| (r.path.clone(), r.clone()))
            .collect();
        let mut updates: BTreeMap<PathBuf, Option<Record>> = BTreeMap::new();
        for path in paths.iter().collect::<std::collections::BTreeSet<_>>() {
            cancel.check()?;
            if !path.starts_with(&root)
                || path == &root
                || crate::files::validate_components(path).is_err()
            {
                return Ok(None);
            }
            if records.get(path).is_some_and(|r| r.identity.directory) {
                return Ok(None);
            }
            let Some(parent) = path.parent() else {
                return Ok(None);
            };
            let Ok(parent) = normalize_existing(parent) else {
                return Ok(None);
            };
            if !parent.starts_with(&root)
                || !records.get(&parent).is_some_and(|r| r.identity.directory)
            {
                return Ok(None);
            }
            let parent_identity = platform::identity(&parent)?;
            if !records
                .get(&parent)
                .is_some_and(|old| old.identity.same_object(&parent_identity))
            {
                return Ok(None);
            }
            let record = match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_file() && !platform::is_link(&metadata) => {
                    let canonical = normalize_existing(path)?;
                    if canonical != *path {
                        return Ok(None);
                    }
                    Some(Record {
                        path: path.clone(),
                        identity: platform::identity(path)?,
                        modified: metadata
                            .modified()
                            .ok()
                            .map(chrono::DateTime::<chrono::Utc>::from),
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                _ => return Ok(None),
            };
            updates.insert(path.clone(), record);
            // Creating/removing a file changes its containing directory's metadata too.
            let metadata = fs::metadata(&parent)?;
            updates.insert(
                parent.clone(),
                Some(Record {
                    path: parent.clone(),
                    identity: parent_identity,
                    modified: metadata
                        .modified()
                        .ok()
                        .map(chrono::DateTime::<chrono::Utc>::from),
                }),
            );
        }
        for (path, record) in &updates {
            if let Some(record) = record {
                records.insert(path.clone(), record.clone());
            } else {
                records.remove(path);
            }
        }
        if records.len() > 100_000 {
            return Err(BudgetExceeded.into());
        }
        normalize_existing(&self.directory)?;
        let db = normalize_existing(&self.database(&r.id))?;
        let temp = tempfile::NamedTempFile::new_in(&self.directory)?;
        fs::copy(db, temp.path())?;
        let mut conn = Connection::open(temp.path())?;
        let tx = conn.transaction()?;
        for (path, record) in updates {
            cancel.check()?;
            let text = path.to_str().context("非 Unicode 路径需要完整校准")?;
            if let Some(record) = record {
                tx.execute(
                    "INSERT OR REPLACE INTO records VALUES (?1,?2,?3)",
                    params![
                        text,
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase(),
                        serde_json::to_string(&record)?
                    ],
                )?;
            } else {
                tx.execute("DELETE FROM records WHERE path=?1", [text])?;
            }
        }
        cancel.check()?;
        let updated = now();
        tx.execute(
            "UPDATE snapshot SET json=?1",
            [serde_json::to_string(&(r, &updated))?],
        )?;
        tx.commit()?;
        drop(conn);
        temp.as_file().sync_all()?;
        Ok(Some((records.into_values().collect(), updated, temp)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{Flag, ModifyKind};
    #[test]
    fn bounded_queue_and_ambiguous_events_force_reconcile_with_backoff() {
        let (tx, rx) = mpsc::sync_channel(1);
        let lost = AtomicBool::new(false);
        enqueue(
            &tx,
            &lost,
            Ok(Event::new(EventKind::Modify(ModifyKind::Data(
                notify::event::DataChange::Any,
            )))
            .add_path("one".into())),
        );
        enqueue(
            &tx,
            &lost,
            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path("two".into())),
        );
        assert!(drain(&rx, &lost).rescan);
        let mut batch = Batch::default();
        batch.add(Err(notify::Error::generic("fixture watcher failure")));
        assert!(batch.broken && batch.rescan);
        let mut batch = Batch::default();
        batch.add(Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)));
        assert!(batch.rescan);
        assert_eq!(backoff(0), Duration::from_secs(30));
        assert_eq!(backoff(1), Duration::from_secs(60));
        assert_eq!(backoff(9), PERIOD);
    }
}
