use super::*;
use crate::file_index::{
    Changes, Checkpoint, CollectionPolicy, Enumeration, Name, Node, PolicyBoundaryChanged,
    Resolved, VolumeIdentity,
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct Source {
    include: bool,
    changed: bool,
    boundary: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    cancel: Option<Cancellation>,
}
impl Source {
    fn new(include: bool) -> Self {
        Self {
            include,
            changed: true,
            boundary: Arc::default(),
            calls: Arc::default(),
            cancel: None,
        }
    }
    fn descriptor(include: bool) -> CollectionPolicy {
        CollectionPolicy {
            id: super::super::super::windows::exclusions::id(include).into(),
            categories: if include {
                vec![]
            } else {
                vec!["system_temp".into()]
            },
            excluded_roots: usize::from(!include),
            unavailable_roots: 0,
            skipped_resolutions: 0,
        }
    }
}
impl WorkerSource for Source {
    fn configure_policy(&mut self, include: bool) -> Result<()> {
        self.include = include;
        self.changed = true;
        Ok(())
    }
    fn prepare(&mut self, _: &[PathBuf]) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl VolumeSource for Source {
    fn policy(&self) -> Option<CollectionPolicy> {
        Some(Self::descriptor(self.include))
    }
    fn identity(&self) -> VolumeIdentity {
        VolumeIdentity {
            backend: "policy-fixture".into(),
            stable_id: "owned".into(),
        }
    }
    fn root(&self) -> String {
        "root".into()
    }
    fn checkpoint(&mut self) -> Result<Checkpoint> {
        Ok(Checkpoint("1".into()))
    }
    fn validate(&mut self, _: &Checkpoint) -> Result<()> {
        Ok(())
    }
    fn enumerate(&mut self, _: Option<&str>) -> Result<Enumeration> {
        Ok(Enumeration {
            ids: vec!["user".into(), "cache".into()],
            next: None,
        })
    }
    fn changes(&mut self, _: &Checkpoint) -> Result<Changes> {
        if self.boundary.load(Ordering::SeqCst) > 0 {
            self.boundary.fetch_sub(1, Ordering::SeqCst);
            return Err(PolicyBoundaryChanged.into());
        }
        Ok(Changes {
            ids: if std::mem::take(&mut self.changed) {
                vec!["user".into(), "cache".into()]
            } else {
                vec!["cache".into()]
            },
            next: Checkpoint("2".into()),
            caught_up: true,
        })
    }
    fn resolve(&mut self, id: &str) -> Result<Resolved> {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        if id == "cache" && !self.include {
            return Ok(Resolved::Excluded);
        }
        Ok(Resolved::Present(Node {
            id: id.into(),
            directory: false,
            names: vec![Name {
                parent: "root".into(),
                name: format!("{id}.txt"),
            }],
        }))
    }
}
fn shared(db: PathBuf, policy: Option<CollectionPolicy>) -> Arc<Shared> {
    let mut state = initial();
    state.requested_policy = Some(super::super::super::windows::exclusions::DEFAULT.into());
    state.policy = policy;
    Arc::new(Shared {
        stop: Cancellation::default(),
        root: db.parent().unwrap().into(),
        db,
        state: Mutex::new(state),
        active: Mutex::default(),
        cursors: Mutex::default(),
        rebuild: Mutex::default(),
    })
}
fn wait(shared: &Shared, condition: impl Fn(&PublicStatus) -> bool) -> PublicStatus {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let state = shared.state.lock().unwrap().clone();
        if condition(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "unexpected state: {:?}", state);
        thread::sleep(Duration::from_millis(10));
    }
}
struct Running {
    shared: Arc<Shared>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.shared.stop.cancel();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn run(store: IndexStore, source: Source, shared: Arc<Shared>) -> Running {
    let state = shared.clone();
    let thread = thread::spawn(move || {
        maintain_with_budget(
            store,
            Box::new(source),
            state.clone(),
            state.stop.clone(),
            Budget::default(),
            false,
        )
    });
    Running {
        shared,
        thread: Some(thread),
    }
}
#[test]
fn mode_switch_is_queued_atomic_persistent_and_repeated_excluded_events_do_not_publish() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("index.db");
    let store = IndexStore::open(&db).unwrap();
    store.enable_concurrent_reads().unwrap();
    let state = shared(db.clone(), None);
    let running = run(store, Source::new(false), state.clone());
    wait(&state, |s| s.generation == Some(1) && s.phase == "ready");
    let reader = IndexStore::open_reader(&db).unwrap();
    assert!(
        reader
            .search("cache", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .is_empty()
    );
    thread::sleep(Duration::from_millis(550));
    assert_eq!(reader.status().unwrap().generation, 1);
    assert_eq!(
        queue_policy(&state, true, || policy::save_checked(&db, true, |_| Ok(()))).unwrap(),
        "accepted"
    );
    let pending = state.state.lock().unwrap().clone();
    assert_eq!(
        pending.requested_policy.as_deref(),
        Some(super::super::super::windows::exclusions::ALL)
    );
    assert_eq!(
        pending.policy.unwrap().id,
        super::super::super::windows::exclusions::DEFAULT
    );
    wait(&state, |s| s.generation == Some(2) && s.phase == "ready");
    assert_eq!(
        reader
            .search("cache", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        queue_policy(&state, false, || panic!("rate limited write must not run")).unwrap(),
        "rate_limited"
    );
    drop(running);
    drop(reader);
    let include = serde_json::from_slice::<serde_json::Value>(
        &fs::read(temp.path().join("policy.json")).unwrap(),
    )
    .unwrap()["include_system_files"]
        .as_bool()
        .unwrap();
    assert!(include);
    let store = IndexStore::open(&db).unwrap();
    let state = shared(db.clone(), Some(Source::descriptor(true)));
    state.state.lock().unwrap().requested_policy =
        Some(super::super::super::windows::exclusions::ALL.into());
    let running = run(store, Source::new(include), state.clone());
    wait(&state, |s| s.phase == "ready");
    assert_eq!(
        queue_policy(&state, false, || policy::save_checked(&db, false, |_| Ok(
            ()
        )))
        .unwrap(),
        "accepted"
    );
    wait(&state, |s| s.generation == Some(3) && s.phase == "ready");
    assert!(
        IndexStore::open_reader(&db)
            .unwrap()
            .search("cache", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .is_empty()
    );
    drop(running);
}
#[test]
fn persisted_explicit_pending_policy_resumes_rebuild_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("index.db");
    let mut store = IndexStore::open(&db).unwrap();
    store.enable_concurrent_reads().unwrap();
    let mut source = Source::new(false);
    store
        .update(
            &mut source,
            true,
            Budget::default(),
            &Cancellation::default(),
        )
        .unwrap();
    policy::save_checked(&db, true, |_| Ok(())).unwrap();
    let state = shared(db.clone(), Some(Source::descriptor(false)));
    state.state.lock().unwrap().generation = Some(1);
    state.state.lock().unwrap().requested_policy =
        Some(super::super::super::windows::exclusions::ALL.into());
    let running = run(store, Source::new(true), state.clone());
    let ready = wait(&state, |s| s.generation == Some(2) && s.phase == "ready");
    assert_eq!(
        ready.policy.unwrap().id,
        super::super::super::windows::exclusions::ALL
    );
    assert_eq!(
        IndexStore::open_reader(&db)
            .unwrap()
            .search("cache", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .len(),
        1
    );
    drop(running);
}
#[test]
fn failed_save_cancel_and_duplicate_keep_actual_policy_and_publication() {
    let temp = tempfile::tempdir().unwrap();
    let state = shared(
        temp.path().join("index.db"),
        Some(Source::descriptor(false)),
    );
    state.cursors.lock().unwrap().insert(
        "old".into(),
        Cursor {
            owner: security::current().unwrap(),
            query: "fixture".into(),
            cursor: SearchCursor {
                generation: 1,
                after: 0,
            },
            expires: Instant::now() + Duration::from_secs(5),
        },
    );
    assert!(queue_policy(&state, true, || anyhow::bail!("synthetic save failure")).is_err());
    assert_eq!(
        state.state.lock().unwrap().requested_policy.as_deref(),
        Some(super::super::super::windows::exclusions::DEFAULT)
    );
    assert!(!state.rebuild.lock().unwrap().pending);
    assert_eq!(state.cursors.lock().unwrap().len(), 1);
    assert_eq!(queue_policy(&state, true, || Ok(())).unwrap(), "accepted");
    assert!(state.cursors.lock().unwrap().is_empty());
    assert_eq!(
        queue_policy(&state, false, || panic!("duplicate must not write")).unwrap(),
        "in_progress"
    );
    let stopped = shared(
        temp.path().join("stopped.db"),
        Some(Source::descriptor(false)),
    );
    stopped.stop.cancel();
    assert_eq!(
        queue_policy(&stopped, true, || panic!("cancelled must not write")).unwrap(),
        "unavailable"
    );
}
#[test]
fn existing_legacy_or_other_policy_requires_rebuild_and_cancel_retains_previous_names() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("index.db");
    let mut store = IndexStore::open(&db).unwrap();
    let mut source = Source::new(true);
    store
        .update(
            &mut source,
            true,
            Budget::default(),
            &Cancellation::default(),
        )
        .unwrap();
    source.include = false;
    assert!(
        store
            .update(
                &mut source,
                false,
                Budget::default(),
                &Cancellation::default()
            )
            .unwrap_err()
            .to_string()
            .contains("policy_rebuild_required")
    );
    let stop = Cancellation::default();
    source.cancel = Some(stop.clone());
    assert!(
        store
            .update(&mut source, true, Budget::default(), &stop)
            .is_err()
    );
    drop(store);
    let store = IndexStore::open_reader(&db).unwrap();
    assert_eq!(
        store.status().unwrap().policy.unwrap().id,
        super::super::super::windows::exclusions::ALL
    );
    assert_eq!(
        store
            .search("cache", 10, None, &Cancellation::default())
            .unwrap()
            .items
            .len(),
        1
    );
    // Removing only the optional policy field models an untouched older snapshot.
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let text: String = connection
        .query_row("SELECT json FROM state", [], |row| row.get(0))
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value.as_object_mut().unwrap().remove("policy");
    connection
        .execute("UPDATE state SET json=?1", [value.to_string()])
        .unwrap();
    drop(connection);
    let mut store = IndexStore::open(&db).unwrap();
    assert!(store.status().unwrap().policy.is_none());
    assert!(
        store
            .update(
                &mut source,
                false,
                Budget::default(),
                &Cancellation::default()
            )
            .unwrap_err()
            .to_string()
            .contains("policy_rebuild_required")
    );
}
#[test]
fn boundary_rebuild_is_bounded_and_snapshot_race_returns_structured_empty_response() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("index.db");
    let mut store = IndexStore::open(&db).unwrap();
    let mut source = Source::new(false);
    let snapshot = store
        .update(
            &mut source,
            true,
            Budget::default(),
            &Cancellation::default(),
        )
        .unwrap();
    source.boundary.store(3, Ordering::SeqCst);
    let calls = source.calls.clone();
    let state = shared(db.clone(), Some(Source::descriptor(false)));
    state.state.lock().unwrap().generation = Some(1);
    let running = run(store, source, state.clone());
    let error = wait(&state, |s| {
        s.error.as_deref() == Some("policy_rebuild_required")
    });
    assert_eq!(error.generation, Some(1));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        IndexStore::open_reader(&db)
            .unwrap()
            .status()
            .unwrap()
            .generation,
        1
    );
    let mut current = error;
    current.requested_policy = Some(super::super::super::windows::exclusions::ALL.into());
    let mut reply = Response {
        control: None,
        version: VERSION,
        id: "owned".into(),
        status: initial(),
        hits: vec![],
        cursor: None,
        error: None,
    };
    apply_search_snapshot(
        &mut reply,
        &current,
        snapshot,
        vec![Hit {
            file_id: "owned".into(),
            path: "private".into(),
        }],
        Some("stale".into()),
    );
    assert_eq!(reply.error.as_deref(), Some("policy_rebuild_required"));
    assert!(reply.hits.is_empty());
    assert!(reply.cursor.is_none());
    assert!(!serde_json::to_string(&reply).unwrap().contains("private"));
    drop(running);
}
