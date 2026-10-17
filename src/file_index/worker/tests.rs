use super::*;
use std::{fs, thread};
mod ordinary_token;
use ordinary_token::ordinary_child;

#[test]
fn restricted_child_runs_real_ipc_auth_cursor_incremental_and_restart() {
    ordinary_token::run("ordinary_ipc_auth_cursor_incremental_and_restart").unwrap();
}

#[test]
fn restricted_child_preserves_snapshot_after_nested_crash() {
    ordinary_token::run("abrupt_process_exit_preserves_published_snapshot").unwrap();
}

#[test]
fn fixture_rejects_elevated_token_before_opening_database() {
    if !security::current().unwrap().elevated {
        return; // The ordinary-token behavior is covered by the real IPC test above.
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    fs::create_dir(&root).unwrap();
    let db = temp.path().join("index.db");
    let result = run(
        Config {
            mode: Mode::Fixture(root),
            database: db.clone(),
            pipe: uuid::Uuid::new_v4().to_string(),
            max_resolutions: 1,
        },
        Cancellation::default(),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("without elevation")
    );
    assert!(!db.exists());
}

#[test]
fn exited_maintenance_preserves_publication_but_replaces_rebuildable_error() {
    for old in [
        None,
        Some("rebuild_required"),
        Some("storage_identity_changed"),
    ] {
        let mut state = initial();
        state.generation = Some(42);
        state.updated_unix_ms = Some(1700000000000);
        state.coverage_reasons = vec!["access_denied".into()];
        state.error = old.map(str::to_owned);
        maintenance_stopped(&mut state);
        assert_eq!(state.phase, "error");
        assert_eq!(state.generation, Some(42));
        assert_eq!(state.updated_unix_ms, Some(1700000000000));
        assert_eq!(state.coverage_reasons, ["access_denied"]);
        assert_eq!(
            state.error.as_deref(),
            Some(if old == Some("storage_identity_changed") {
                "storage_identity_changed"
            } else {
                "maintenance_stopped"
            })
        );
    }
}

#[test]
fn coverage_reasons_are_static_deduplicated_and_do_not_expose_private_text() {
    assert_eq!(
        public_reasons(&[
            "access_denied".into(),
            "E:\\fictional-private\\name.txt".into(),
            "SQL candidate 123".into(),
            "access_denied".into(),
        ]),
        ["access_denied", "metadata_unavailable"]
    );
}

#[test]
fn search_receipt_uses_actual_database_snapshot_not_earlier_shared_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("fixture-file.txt"), "fictional").unwrap();
    let db = temp.path().join("index.db");
    let mut source = fixture::FixtureSource::open(&root).unwrap();
    let mut store = IndexStore::open(&db).unwrap();
    let snapshot = store
        .update(
            &mut source,
            true,
            Budget::default(),
            &Cancellation::default(),
        )
        .unwrap();
    drop(store);
    let mut earlier = initial();
    earlier.generation = Some(0);
    earlier.updated_unix_ms = Some(0);
    earlier.phase = "catching_up".into();
    let shared = Shared {
        stop: Cancellation::default(),
        db,
        root,
        state: Mutex::new(earlier),
        active: Mutex::new(HashMap::new()),
        cursors: Mutex::new(HashMap::new()),
        rebuild: Mutex::new(Rebuild::default()),
    };
    let pipe = uuid::Uuid::new_v4().to_string();
    let mut handles = transport::listeners(&pipe, true).unwrap();
    let handle = handles.pop().unwrap();
    drop(handles);
    let owner = security::current().unwrap();
    let server = thread::spawn(move || {
        transport::accept(
            security::raw(&handle),
            &Cancellation::with_timeout(Duration::from_secs(3)),
        )
        .unwrap();
        serve_connection(security::raw(&handle), &shared, &owner).unwrap();
    });
    let reply = request(
        &pipe,
        &std::env::current_exe().unwrap(),
        true,
        &Request {
            version: VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            timeout_ms: 1000,
            operation: Operation::Search {
                query: "fixture".into(),
                page_size: 1,
                cursor: None,
            },
        },
        &Cancellation::with_timeout(Duration::from_secs(2)),
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(reply.hits.len(), 1);
    assert_eq!(reply.status.generation, Some(snapshot.generation));
    assert_eq!(reply.status.complete, snapshot.complete);
    assert_eq!(reply.status.updated_unix_ms, snapshot.published_unix_ms);
    assert_eq!(reply.status.phase, "catching_up");
}
#[test]
fn maintenance_retry_policy_is_bounded_and_does_not_retry_unknown_errors() {
    let error = anyhow::anyhow!("retry_required: incomplete link set");
    let mut attempts = 0;
    let delays = (0..7)
        .map(|_| retry_delay(&error, &mut attempts))
        .collect::<Vec<_>>();
    assert_eq!(
        delays,
        vec![
            Some(250),
            Some(500),
            Some(1000),
            Some(2000),
            Some(4000),
            Some(4000),
            None
        ]
    );
    assert_eq!(
        retry_delay(&anyhow::anyhow!("unsupported USN version"), &mut 0),
        None
    );
    assert_eq!(
        retry_delay(&anyhow::anyhow!("unsupported index schema"), &mut 0),
        None
    );
}

#[test]
#[ignore = "isolated crash helper, invoked only by the recovery test"]
fn crash_during_owned_fixture_transaction() {
    let path = std::env::var_os("DAO_INDEX_CRASH_FIXTURE").expect("owned fixture path required");
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE; UPDATE names SET name='crashed-uncommitted',folded='crashed-uncommitted'; UPDATE state SET json='invalid-uncommitted';").unwrap();
    // Intentionally bypass Rust/SQLite destructors to exercise actual crash recovery.
    std::process::exit(23);
}
#[test]
fn abrupt_process_exit_preserves_published_snapshot() {
    if ordinary_child("abrupt_process_exit_preserves_published_snapshot") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("fixture-a.txt"), "fictional").unwrap();
    let db = temp.path().join("index.db");
    let running = Running::start(&root, &db);
    let before = running.call(Operation::Status).unwrap().status;
    drop(running);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "file_index::worker::tests::crash_during_owned_fixture_transaction",
        ])
        .env("DAO_INDEX_CRASH_FIXTURE", &db)
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(23));
    let recovered = Running::start(&root, &db);
    let result = recovered.search(None);
    assert_eq!(result.status.generation, before.generation);
    assert_eq!(result.status.updated_unix_ms, before.updated_unix_ms);
    assert_eq!(result.hits.len(), 1);
    assert!(result.hits[0].path.ends_with("fixture-a.txt"));
}

#[test]
fn control_request_cancels_only_matching_principal_with_three_active_searches() {
    let pipe = uuid::Uuid::new_v4().to_string();
    let owner = security::current().unwrap();
    let first = Cancellation::default();
    let foreign = Cancellation::default();
    let first_id = uuid::Uuid::new_v4().to_string();
    let foreign_id = uuid::Uuid::new_v4().to_string();
    let mut different = owner.clone();
    different.pid = owner.pid.wrapping_add(1);
    let shared = Arc::new(Shared {
        stop: Cancellation::default(),
        db: PathBuf::new(),
        root: PathBuf::new(),
        state: Mutex::new(initial()),
        active: Mutex::new(HashMap::from([
            (first_id.clone(), (owner.clone(), first.clone())),
            (foreign_id.clone(), (different, foreign.clone())),
            (
                uuid::Uuid::new_v4().to_string(),
                (owner.clone(), Cancellation::default()),
            ),
        ])),
        cursors: Mutex::new(HashMap::new()),
        rebuild: Mutex::new(Rebuild::default()),
    });
    let mut handles = transport::listeners(&pipe, true).unwrap();
    let handle = handles.pop().unwrap();
    drop(handles);
    let server = thread::spawn(move || {
        for _ in 0..3 {
            transport::accept(
                security::raw(&handle),
                &Cancellation::with_timeout(Duration::from_secs(3)),
            )
            .unwrap();
            serve_connection(security::raw(&handle), &shared, &owner).unwrap();
            transport::disconnect(security::raw(&handle));
        }
    });
    for operation in [
        Operation::Cancel {
            request_id: first_id,
        },
        Operation::Cancel {
            request_id: foreign_id,
        },
        Operation::Status,
    ] {
        request(
            &pipe,
            &std::env::current_exe().unwrap(),
            true,
            &Request {
                version: VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                timeout_ms: 1000,
                operation,
            },
            &Cancellation::with_timeout(Duration::from_secs(2)),
        )
        .unwrap();
    }
    server.join().unwrap();
    assert!(first.cancelled());
    assert!(!foreign.cancelled());
}

struct Running {
    stop: Cancellation,
    thread: Option<thread::JoinHandle<Result<PublicStatus>>>,
    pipe: String,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            // Preserve the first failure instead of aborting on a second panic in Drop.
            if !std::thread::panicking() {
                result.unwrap().unwrap();
            }
        }
    }
}
#[test]
fn worker_cleanup_joins_failed_thread_without_masking_original_panic() {
    let panic = std::panic::catch_unwind(|| {
        let _running = Running {
            stop: Cancellation::default(),
            thread: Some(thread::spawn(|| anyhow::bail!("synthetic worker failure"))),
            pipe: String::new(),
        };
        panic!("synthetic original failure");
    });
    assert_eq!(
        panic.unwrap_err().downcast_ref::<&str>(),
        Some(&"synthetic original failure")
    );
    assert!(
        std::panic::catch_unwind(|| {
            drop(Running {
                stop: Cancellation::default(),
                thread: Some(thread::spawn(|| anyhow::bail!("synthetic worker failure"))),
                pipe: String::new(),
            });
        })
        .is_err()
    );
}
impl Running {
    fn start(root: &Path, db: &Path) -> Self {
        let stop = Cancellation::default();
        let worker_stop = stop.clone();
        let pipe = uuid::Uuid::new_v4().to_string();
        let config = Config {
            mode: Mode::Fixture(root.into()),
            database: db.into(),
            pipe: pipe.clone(),
            max_resolutions: 10000,
        };
        let thread = thread::spawn(move || run(config, worker_stop));
        let running = Self {
            stop,
            thread: Some(thread),
            pipe,
        };
        running.wait_for(|r| r.status.complete);
        running
    }
    fn call(&self, operation: Operation) -> Result<Response> {
        request(
            &self.pipe,
            &std::env::current_exe()?,
            true,
            &Request {
                version: VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                timeout_ms: 1000,
                operation,
            },
            &Cancellation::with_timeout(Duration::from_secs(2)),
        )
    }
    fn wait_for(&self, predicate: impl Fn(&Response) -> bool) -> Response {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(reply) = self.call(Operation::Status)
                && predicate(&reply)
            {
                return reply;
            }
            assert!(
                Instant::now() < until,
                "worker did not reach expected state"
            );
            thread::sleep(Duration::from_millis(30));
        }
    }
    fn search(&self, cursor: Option<String>) -> Response {
        self.call(Operation::Search {
            query: "fixture".into(),
            page_size: 1,
            cursor,
        })
        .unwrap()
    }
}
#[test]
fn ordinary_ipc_auth_cursor_incremental_and_restart() {
    if ordinary_child("ordinary_ipc_auth_cursor_incremental_and_restart") {
        return;
    }
    assert!(!security::current().unwrap().elevated);
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("fixture-a.txt"), "a").unwrap();
    fs::write(root.join("fixture-b.txt"), "b").unwrap();
    let db = temp.path().join("index.db");
    let running = Running::start(&root, &db);
    let first = running.search(None);
    assert_eq!(first.hits.len(), 1);
    let cursor = first.cursor.unwrap();
    assert!(uuid::Uuid::parse_str(&cursor).is_ok());
    let second = running.search(Some(cursor.clone()));
    assert_eq!(second.hits.len(), 1);
    assert!(second.cursor.is_none());
    assert_ne!(first.hits[0].path, second.hits[0].path);
    assert_eq!(
        running.search(Some(cursor)).error.as_deref(),
        Some("stale_cursor")
    );
    let bad = request(
        &running.pipe,
        &root.join("wrong.exe"),
        true,
        &Request {
            version: VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            timeout_ms: 1000,
            operation: Operation::Status,
        },
        &Cancellation::with_timeout(Duration::from_secs(1)),
    );
    assert!(bad.is_err());
    assert!(running.call(Operation::Status).is_ok());
    let generation = first.status.generation.unwrap();
    fs::remove_file(root.join("fixture-a.txt")).unwrap();
    let after = running.wait_for(|r| r.status.generation.is_some_and(|g| g > generation));
    assert_eq!(running.search(None).hits.len(), 1);
    assert!(running.search(None).cursor.is_none());
    drop(running);
    let restarted = Running::start(&root, &db);
    let status = restarted.call(Operation::Status).unwrap().status;
    assert_eq!(status.generation, after.status.generation);
    assert_eq!(status.updated_unix_ms, after.status.updated_unix_ms);
    assert!(status.updated_unix_ms.is_some());
    // Unknown version closes without reflecting fields or candidates; subsequent clients work.
    let connection = transport::connect(
        &restarted.pipe,
        &std::env::current_exe().unwrap(),
        true,
        &Cancellation::with_timeout(Duration::from_secs(1)),
    )
    .unwrap();
    let bad = serde_json::json!({"version":999,"id":uuid::Uuid::new_v4().to_string(),"timeout_ms":1000,"operation":{"kind":"status"}});
    transport::send(
        security::raw(&connection),
        &serde_json::to_vec(&bad).unwrap(),
        &Cancellation::with_timeout(Duration::from_secs(1)),
    )
    .unwrap();
    assert!(
        transport::receive(
            security::raw(&connection),
            &Cancellation::with_timeout(Duration::from_secs(1))
        )
        .is_err()
    );
    assert!(restarted.call(Operation::Status).is_ok());
}
#[test]
fn ordinary_volume_mode_fails_before_creating_unprotected_database() {
    if ordinary_child("ordinary_volume_mode_fails_before_creating_unprotected_database") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("index.db");
    assert!(
        run(
            Config {
                mode: Mode::Volume("C:".into()),
                database: database.clone(),
                pipe: uuid::Uuid::new_v4().to_string(),
                max_resolutions: 1
            },
            Cancellation::default()
        )
        .is_err()
    );
    assert!(!database.exists());
}

#[test]
fn silent_clients_expire_and_release_all_pipe_instances() {
    if ordinary_child("silent_clients_expire_and_release_all_pipe_instances") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fixture");
    fs::create_dir(&root).unwrap();
    let running = Running::start(&root, &dir.path().join("index.db"));
    let image = std::env::current_exe().unwrap();
    let mut idle = Vec::new();
    for _ in 0..4 {
        idle.push(
            transport::connect(
                &running.pipe,
                &image,
                true,
                &Cancellation::with_timeout(Duration::from_secs(1)),
            )
            .unwrap(),
        );
    }
    thread::sleep(Duration::from_millis(5200));
    assert!(running.call(Operation::Status).is_ok());
    drop(idle);
}

#[test]
fn ipc_hides_names_when_ancestor_listing_is_denied() {
    if ordinary_child("ipc_hides_names_when_ancestor_listing_is_denied") {
        return;
    }
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{Authorization::*, *},
    };
    struct Restore {
        path: Vec<u16>,
        descriptor: PSECURITY_DESCRIPTOR,
        acl: *mut ACL,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe {
                assert_eq!(
                    SetNamedSecurityInfoW(
                        self.path.as_ptr(),
                        SE_FILE_OBJECT,
                        DACL_SECURITY_INFORMATION,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        self.acl,
                        ptr::null()
                    ),
                    0
                );
                LocalFree(self.descriptor);
            }
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    let hidden = root.join("private");
    fs::create_dir_all(&hidden).unwrap();
    fs::write(hidden.join("fixture-secret.txt"), "fictional").unwrap();
    let running = Running::start(&root, &temp.path().join("index.db"));
    assert_eq!(running.search(None).hits.len(), 1);
    let path = security::wide(hidden.to_str().unwrap());
    let mut original = ptr::null_mut();
    let mut old_acl = ptr::null_mut();
    assert_eq!(
        unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut old_acl,
                ptr::null_mut(),
                &mut original,
            )
        },
        0
    );
    let restore = Restore {
        path,
        descriptor: original,
        acl: old_acl,
    };
    let sid = security::current().unwrap().sid;
    let sddl = format!("D:P(D;;0x1;;;{sid})(A;;FA;;;{sid})");
    let mut descriptor = ptr::null_mut();
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                security::wide(&sddl).as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        },
        0
    );
    let descriptor = security::Descriptor(descriptor);
    let mut present = 0;
    let mut defaulted = 0;
    let mut acl = ptr::null_mut();
    assert_ne!(
        unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted) },
        0
    );
    assert_eq!(
        unsafe {
            SetNamedSecurityInfoW(
                restore.path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                acl,
                ptr::null(),
            )
        },
        0
    );
    let hidden_reply = running.search(None);
    assert!(hidden_reply.error.is_none());
    assert!(hidden_reply.hits.is_empty());
    assert!(hidden_reply.cursor.is_none());
    drop(restore);
    assert_eq!(running.search(None).hits.len(), 1);
}

#[test]
fn rebuild_single_slot_rate_limit_and_stop_are_explicit() {
    let now = Instant::now();
    let mut r = Rebuild::default();
    assert_eq!(r.request(now), "accepted");
    assert_eq!(r.request(now), "in_progress");
    assert!(r.take());
    assert!(!r.take());
    assert_eq!(r.request(now + Duration::from_secs(60)), "in_progress");
    r.running = false;
    assert_eq!(r.request(now + Duration::from_secs(2)), "rate_limited");
    assert_eq!(r.request(now + Duration::from_secs(31)), "accepted");
    r.disabled = true;
    assert_eq!(r.request(now + Duration::from_secs(100)), "unavailable");
}
#[test]
fn ordinary_authenticated_rebuild_publishes_once_and_does_not_touch_source() {
    if ordinary_child("ordinary_authenticated_rebuild_publishes_once_and_does_not_touch_source") {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("fixture");
    fs::create_dir(&root).unwrap();
    let file = root.join("fixture.txt");
    fs::write(&file, "synthetic unchanged").unwrap();
    let running = Running::start(&root, &temp.path().join("index.db"));
    let before = running
        .call(Operation::Status)
        .unwrap()
        .status
        .generation
        .unwrap();
    let accepted = running.call(Operation::Rebuild {}).unwrap();
    assert_eq!(accepted.control.as_deref(), Some("accepted"));
    let duplicate = running.call(Operation::Rebuild {}).unwrap();
    assert!(matches!(
        duplicate.control.as_deref(),
        Some("in_progress" | "rate_limited")
    ));
    let complete = running.wait_for(|r| r.status.generation == Some(before + 1));
    assert!(complete.status.complete);
    assert_eq!(fs::read_to_string(file).unwrap(), "synthetic unchanged");
    assert!(serde_json::from_str::<Request>(r#"{"version":1,"id":"x","timeout_ms":1000,"operation":{"kind":"rebuild","path":"arbitrary"}}"#).is_err());
}
#[test]
fn foreign_authenticated_principal_cannot_queue_rebuild() {
    let name = uuid::Uuid::new_v4().to_string();
    let mut owner = security::current().unwrap();
    owner.sid = "S-1-5-21-0-0-0-1234".into();
    let shared = Arc::new(Shared {
        stop: Cancellation::default(),
        db: PathBuf::new(),
        root: PathBuf::new(),
        state: Mutex::new(initial()),
        active: Mutex::new(HashMap::new()),
        cursors: Mutex::new(HashMap::new()),
        rebuild: Mutex::new(Rebuild::default()),
    });
    let mut pipes = transport::listeners(&name, true).unwrap();
    let pipe = pipes.pop().unwrap();
    drop(pipes);
    let state = shared.clone();
    let server = thread::spawn(move || {
        transport::accept(
            security::raw(&pipe),
            &Cancellation::with_timeout(Duration::from_secs(3)),
        )
        .unwrap();
        assert!(serve_connection(security::raw(&pipe), &state, &owner).is_err());
        transport::disconnect(security::raw(&pipe));
    });
    let reply = request(
        &name,
        &std::env::current_exe().unwrap(),
        true,
        &Request {
            version: VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            timeout_ms: 1000,
            operation: Operation::Rebuild {},
        },
        &Cancellation::with_timeout(Duration::from_secs(2)),
    );
    assert!(reply.is_err());
    server.join().unwrap();
    assert!(!shared.rebuild.lock().unwrap().pending);
}
#[test]
fn source_identity_must_match_database_and_pipe_before_opening_writer() {
    let temp = tempfile::tempdir().unwrap();
    let guid = format!(r"\\?\Volume{{{}}}\", uuid::Uuid::new_v4());
    let key = service::source_key(&guid, 1).unwrap();
    let config = Config {
        mode: Mode::Volume(guid.clone()),
        database: temp.path().join(&key).join("index.db"),
        pipe: format!("volume-{key}"),
        max_resolutions: 1,
    };
    let mut identity = super::super::VolumeIdentity {
        backend: "windows_ntfs".into(),
        stable_id: format!("{guid}:00000001"),
    };
    assert!(validate_source_location(&identity, &config).is_ok());
    identity.stable_id = format!("{guid}:00000002");
    assert!(validate_source_location(&identity, &config).is_err());
    identity.stable_id = format!(r"\\?\Volume{{{}}}\:00000001", uuid::Uuid::new_v4());
    assert!(validate_source_location(&identity, &config).is_err());
    assert!(!config.database.exists());
}

#[test]
fn public_failure_diagnostic_is_bounded_and_old_v1_status_still_parses() {
    let error = anyhow::Error::new(std::io::Error::from_raw_os_error(87))
        .context("metadata: delete-pending state unavailable");
    let diagnostic = super::failure_diagnostic(&error);
    assert_eq!(diagnostic.stage, "metadata_standard");
    assert_eq!(diagnostic.native_code, Some(87));
    assert_eq!(diagnostic.code, "request_failed");
    let secret = super::failure_diagnostic(&anyhow::anyhow!("C:\\SECRET_PRIVATE\\file.txt"));
    let public = serde_json::to_string(&secret).unwrap();
    assert!(!public.contains("SECRET"));
    assert_eq!(secret.stage, "index_maintenance");
    let old = serde_json::json!({"coverage_reasons":[],"phase":"error","generation":null,"complete":false,"updated_unix_ms":null,"error":"request_failed"});
    let status: super::PublicStatus = serde_json::from_value(old).unwrap();
    assert!(status.diagnostic.is_none());
}
