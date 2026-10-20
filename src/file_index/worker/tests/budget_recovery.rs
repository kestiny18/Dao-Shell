use super::*;
use crate::file_index::{Changes, Checkpoint, Enumeration, Name, Node, Resolved, VolumeIdentity};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Synthetic {
    nodes: usize,
    enumeration_pages: usize,
    catchup_pages: usize,
    label: &'static str,
    calls: Arc<AtomicUsize>,
    cancel_on_resolve: Option<Cancellation>,
    failure: bool,
}
impl Synthetic {
    fn new(nodes: usize) -> Self {
        Self {
            nodes,
            enumeration_pages: 1,
            catchup_pages: 1,
            label: "new",
            calls: Arc::default(),
            cancel_on_resolve: None,
            failure: false,
        }
    }
    fn ids(&self) -> Vec<String> {
        (0..self.nodes).map(|i| format!("node-{i}")).collect()
    }
}
impl WorkerSource for Synthetic {
    fn prepare(&mut self, _: &[PathBuf]) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl VolumeSource for Synthetic {
    fn identity(&self) -> VolumeIdentity {
        VolumeIdentity {
            backend: "synthetic".into(),
            stable_id: "owned-budget-fixture".into(),
        }
    }
    fn root(&self) -> String {
        "root".into()
    }
    fn checkpoint(&mut self) -> Result<Checkpoint> {
        Ok(Checkpoint("0".into()))
    }
    fn validate(&mut self, _: &Checkpoint) -> Result<()> {
        Ok(())
    }
    fn enumerate(&mut self, cursor: Option<&str>) -> Result<Enumeration> {
        let page: usize = cursor.unwrap_or("0").parse()?;
        Ok(Enumeration {
            ids: if page == 0 { self.ids() } else { vec![] },
            next: (page + 1 < self.enumeration_pages).then(|| (page + 1).to_string()),
        })
    }
    fn changes(&mut self, checkpoint: &Checkpoint) -> Result<Changes> {
        ensure!(
            !self.failure,
            "unsupported USN version; budget text is not a typed budget failure"
        );
        let next = checkpoint.0.parse::<usize>()? + 1;
        Ok(Changes {
            ids: self.ids(),
            next: Checkpoint(next.to_string()),
            caught_up: next >= self.catchup_pages,
        })
    }
    fn resolve(&mut self, id: &str) -> Result<Resolved> {
        if let Some(stop) = self.cancel_on_resolve.take() {
            stop.cancel();
        }
        Ok(Resolved::Present(Node {
            id: id.into(),
            directory: false,
            names: vec![Name {
                parent: "root".into(),
                name: format!("{}-{id}", self.label),
            }],
        }))
    }
}
struct Running {
    _temp: tempfile::TempDir,
    shared: Arc<Shared>,
    thread: Option<thread::JoinHandle<()>>,
    calls: Arc<AtomicUsize>,
}
impl Running {
    fn start(source: Synthetic, seed: bool, budget: Budget) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("index.db");
        let mut store = IndexStore::open(&db).unwrap();
        store.enable_concurrent_reads().unwrap();
        let mut state = initial();
        if seed {
            let mut old = Synthetic::new(1);
            old.label = "old";
            let published = store
                .update(&mut old, true, Budget::default(), &Cancellation::default())
                .unwrap();
            state.generation = Some(published.generation);
            state.updated_unix_ms = published.published_unix_ms;
            state.complete = published.complete;
        }
        let shared = Arc::new(Shared {
            stop: Cancellation::default(),
            db,
            root: temp.path().to_path_buf(),
            state: Mutex::new(state),
            active: Mutex::default(),
            cursors: Mutex::default(),
            rebuild: Mutex::default(),
        });
        let calls = source.calls.clone();
        let worker_shared = shared.clone();
        let thread = thread::spawn(move || {
            maintain_with_budget(
                store,
                Box::new(source),
                worker_shared.clone(),
                worker_shared.stop.clone(),
                budget,
                false,
            )
        });
        Self {
            _temp: temp,
            shared,
            thread: Some(thread),
            calls,
        }
    }
    fn wait(&self, expected: &str) -> PublicStatus {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            let status = self.shared.state.lock().unwrap().clone();
            if status.phase == expected {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "expected {expected}, got {} {:?}",
                status.phase,
                status.error
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn old_snapshot(&self) {
        let store = IndexStore::open_reader(&self.shared.db).unwrap();
        assert_eq!(store.status().unwrap().generation, 1);
        assert_eq!(store.status().unwrap().checkpoint.0, "1");
        assert_eq!(
            store
                .search("old", 10, None, &Cancellation::default())
                .unwrap()
                .items
                .len(),
            1
        );
        assert!(
            store
                .search("new", 10, None, &Cancellation::default())
                .unwrap()
                .items
                .is_empty()
        );
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.shared.stop.cancel();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn small() -> Budget {
    Budget {
        max_resolutions: 1,
        max_pages: 1,
    }
}

#[test]
fn metadata_budget_recovers_initial_and_existing_updates_without_publishing_partial_work() {
    for seed in [false, true] {
        let running = Running::start(Synthetic::new(3), seed, small());
        let retry = running.wait("budget_retrying");
        assert_eq!(retry.error.as_deref(), Some("budget_increasing_retrying"));
        assert_eq!(retry.diagnostic.unwrap().code, "budget_metadata_exhausted");
        if seed {
            running.old_snapshot();
        } else {
            assert!(
                IndexStore::open_reader(&running.shared.db)
                    .unwrap()
                    .status()
                    .is_err()
            );
        }
        let ready = running.wait("ready");
        assert_eq!(ready.generation, Some(if seed { 2 } else { 1 }));
        assert!(ready.diagnostic.is_none());
        // A first build resolves the same IDs during enumeration and catch-up.
        assert_eq!(
            running.calls.load(Ordering::SeqCst),
            if seed { 3 } else { 4 }
        );
        let store = IndexStore::open_reader(&running.shared.db).unwrap();
        assert_eq!(
            store
                .search("new", 10, None, &Cancellation::default())
                .unwrap()
                .items
                .len(),
            3
        );
    }
}
#[test]
fn enumeration_and_catchup_pages_recover_with_distinct_diagnostics() {
    for enumeration in [true, false] {
        let mut source = Synthetic::new(0);
        if enumeration {
            source.enumeration_pages = 3;
        } else {
            source.catchup_pages = 3;
        }
        let running = Running::start(source, false, small());
        let retry = running.wait("budget_retrying");
        assert_eq!(
            retry.diagnostic.unwrap().stage,
            if enumeration {
                "budget_enumeration"
            } else {
                "budget_catchup"
            }
        );
        assert_eq!(running.wait("ready").generation, Some(1));
        assert_eq!(running.calls.load(Ordering::SeqCst), 3);
    }
}
#[test]
fn final_limit_preserves_persistent_snapshot_and_stops_automatic_attempts() {
    let running = Running::start(Synthetic::new(9), true, small());
    let state = running.wait("error");
    assert_eq!(state.error.as_deref(), Some("budget_limit_exhausted"));
    assert_eq!(state.diagnostic.unwrap().code, "budget_metadata_exhausted");
    assert_eq!(state.generation, Some(1));
    assert!(state.updated_unix_ms.is_some());
    running.old_snapshot();
    assert_eq!(running.calls.load(Ordering::SeqCst), 4);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(running.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        running
            .shared
            .rebuild
            .lock()
            .unwrap()
            .request(Instant::now()),
        "accepted"
    );
    running.wait("budget_retrying");
    assert_eq!(running.calls.load(Ordering::SeqCst), 5);
    assert!(running.shared.rebuild.lock().unwrap().running);
    running.old_snapshot();
}
#[test]
fn cancellation_during_retry_wait_is_prompt_and_keeps_snapshot() {
    let mut running = Running::start(Synthetic::new(3), true, small());
    running.wait("budget_retrying");
    let started = Instant::now();
    running.shared.stop.cancel();
    running.thread.take().unwrap().join().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    running.old_snapshot();
    assert_eq!(running.calls.load(Ordering::SeqCst), 1);
    // Drop still owns the fixture but no longer has a running thread.
}
#[test]
fn cancellation_inside_resolution_and_non_budget_errors_never_retry() {
    let mut source = Synthetic::new(3);
    source.failure = true;
    let running = Running::start(source, true, small());
    assert_eq!(running.wait("error").phase, "error");
    assert_eq!(running.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        running.shared.state.lock().unwrap().error.as_deref(),
        Some("request_failed")
    );
    running.old_snapshot();
    let mut source = Synthetic::new(1);
    let cancel = Cancellation::default();
    source.cancel_on_resolve = Some(cancel.clone());
    // A source-side cancellation is observed by the same transaction token.
    let temp = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open(&temp.path().join("index.db")).unwrap();
    let mut seed = Synthetic::new(1);
    seed.label = "old";
    store
        .update(&mut seed, true, Budget::default(), &Cancellation::default())
        .unwrap();
    let error = store
        .update(&mut source, true, small(), &cancel)
        .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(!BudgetRecovery::new(small()).increase(&error));
    assert_eq!(store.status().unwrap().generation, 1);
    assert_eq!(store.status().unwrap().checkpoint.0, "1");
    drop(store);
    let reopened = IndexStore::open_reader(&temp.path().join("index.db")).unwrap();
    assert_eq!(
        reopened
            .search("old", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .len(),
        1
    );
}
#[test]
fn retry_bounds_saturate_and_explicit_rebuild_resets_the_episode() {
    let mut policy = BudgetRecovery::new(small());
    let error: anyhow::Error = BudgetExceeded::Metadata.into();
    for expected in [2, 4, 8] {
        assert!(policy.increase(&error));
        assert_eq!(policy.current.max_resolutions, expected);
    }
    assert!(!policy.increase(&error));
    policy.reset();
    assert_eq!(policy.current.max_resolutions, 1);
    assert_eq!(policy.attempts, 0);
    assert!(policy.increase(&error));
    assert!(!policy.increase(&anyhow::anyhow!("budget_exhausted: metadata")));
    let mut saturated = BudgetRecovery::new(Budget {
        max_resolutions: usize::MAX,
        max_pages: usize::MAX,
    });
    assert!(!saturated.increase(&error));
    assert_eq!(saturated.maximum.max_resolutions, usize::MAX);
    let mut zero = BudgetRecovery::new(Budget {
        max_resolutions: 0,
        max_pages: 0,
    });
    assert!(!zero.increase(&error));
}
