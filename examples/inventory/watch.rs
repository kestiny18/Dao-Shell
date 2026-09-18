use super::index;
use anyhow::{Result, ensure};
use dao_shell::{core::Cancellation, files::normalize_existing, platform};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};

#[derive(Default, Serialize)]
pub struct Report {
    pub full_scans: usize,
    pub incremental_batches: usize,
    pub recovery_scans: usize,
    pub periodic_scans: usize,
    pub notifications: usize,
    pub reason: String,
}

// The callback must never block the native watcher. Losing details forces reconciliation.
fn enqueue(
    sender: &SyncSender<notify::Result<Event>>,
    lost: &AtomicBool,
    event: notify::Result<Event>,
) {
    if let Err(TrySendError::Full(_)) = sender.try_send(event) {
        lost.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct Batch {
    paths: Vec<PathBuf>,
    rescan: bool,
    notifications: usize,
}
impl Batch {
    fn add(&mut self, event: notify::Result<Event>) {
        self.notifications += 1;
        match event {
            Ok(event) if event.need_rescan() => self.rescan = true,
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => {}
            Ok(event) => {
                if matches!(event.kind, EventKind::Any | EventKind::Other) || event.paths.is_empty()
                {
                    self.rescan = true;
                }
                if self.paths.len() + event.paths.len() > 1024 {
                    self.rescan = true;
                }
                if !self.rescan {
                    self.paths.extend(event.paths);
                }
            }
            Err(_) => self.rescan = true,
        }
    }
}

fn drain(receiver: &Receiver<notify::Result<Event>>, lost: &AtomicBool) -> Batch {
    let mut batch = Batch {
        rescan: lost.swap(false, Ordering::SeqCst),
        ..Default::default()
    };
    // Bound draining under a continuous event stream so cancellation and deadlines remain usable.
    for _ in 0..1024 {
        match receiver.try_recv() {
            Ok(event) => batch.add(event),
            Err(_) => break,
        }
    }
    batch
}

fn emit(kind: &str, snapshot: &index::Snapshot) -> Result<()> {
    println!(
        "{}",
        serde_json::json!({"event": kind, "snapshot": snapshot})
    );
    std::io::stdout().flush()?;
    Ok(())
}

pub fn run(
    root: &Path,
    db: &Path,
    seconds: u64,
    reconcile_seconds: u64,
    max_entries: usize,
    cancel: &Cancellation,
) -> Result<Report> {
    ensure!((1..=3600).contains(&seconds), "seconds 必须为 1..3600");
    ensure!(
        (1..=300).contains(&reconcile_seconds),
        "reconcile-seconds 必须为 1..300"
    );
    let root = normalize_existing(root)?;
    let identity = platform::identity(&root)?;
    ensure!(identity.directory, "root 必须是目录");
    let (sender, receiver) = mpsc::sync_channel(1024);
    let lost = Arc::new(AtomicBool::new(false));
    let callback_lost = lost.clone();
    let mut watcher =
        notify::recommended_watcher(move |event| enqueue(&sender, &callback_lost, event))?;
    // Arm before scanning: notifications generated during bootstrap are processed afterwards.
    watcher.watch(&root, RecursiveMode::Recursive)?;
    let mut report = Report::default();
    let snapshot = index::scan(&root, db, max_entries, Duration::from_secs(60), cancel)?;
    report.full_scans += 1;
    emit("ready_after_startup_reconcile", &snapshot)?;
    let started = Instant::now();
    let mut reconciled = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) && !cancel.is_cancelled() {
        std::thread::sleep(Duration::from_millis(200));
        cancel.check()?;
        ensure!(
            platform::identity(&root)?.same_object(&identity),
            "监听目录已替换；停止监听，需要重新建立快照"
        );
        let batch = drain(&receiver, &lost);
        report.notifications += batch.notifications;
        let periodic = reconciled.elapsed() >= Duration::from_secs(reconcile_seconds);
        let snapshot = if batch.rescan || periodic {
            None
        } else if !batch.paths.is_empty() {
            index::refresh_paths(db, &batch.paths, max_entries, cancel)?
        } else {
            continue;
        };
        if let Some(snapshot) = snapshot {
            report.incremental_batches += 1;
            emit("incremental_snapshot", &snapshot)?;
        } else {
            let snapshot = index::scan(&root, db, max_entries, Duration::from_secs(60), cancel)?;
            report.full_scans += 1;
            if periodic {
                report.periodic_scans += 1;
            } else {
                report.recovery_scans += 1;
            }
            reconciled = Instant::now();
            emit("reconciled_snapshot", &snapshot)?;
        }
    }
    // No durable "live" flag: stopping/crashing never leaves the database claiming a running watcher.
    report.reason =
        "duration_elapsed_or_cancelled; persisted observations may already be stale".into();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{Flag, ModifyKind};

    #[test]
    fn overflow_error_and_native_rescan_request_reconciliation() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let lost = AtomicBool::new(false);
        enqueue(
            &sender,
            &lost,
            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path("one".into())),
        );
        enqueue(
            &sender,
            &lost,
            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path("two".into())),
        );
        assert!(drain(&receiver, &lost).rescan);
        let mut batch = Batch::default();
        batch.add(Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)));
        assert!(batch.rescan);
        let mut batch = Batch::default();
        batch.add(Err(notify::Error::generic("lost details")));
        assert!(batch.rescan);
    }
}
